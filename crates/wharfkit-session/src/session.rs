use antelope::chain::action::PermissionLevel;
use antelope::chain::checksum::Checksum256;
use antelope::chain::transaction::{SignedTransaction, Transaction};
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use wharfkit_abicache::ABICache;
use wharfkit_common::ChainDefinition;
use wharfkit_signing_request::{
    request::ResolveContext, EsrOptions, ResolvedSigningRequest, SigningRequest,
    SigningRequestCreateArgs,
};

use crate::platform::Platform;
use crate::storage::{permission_string, SerializedSession};
use crate::transact::{
    ChainClient, TransactArgs, TransactContext, TransactError, TransactHooks, TransactOptions,
    TransactResult,
};
use crate::ui::UserInterface;
use crate::wallet::WalletPlugin;

pub struct Session {
    chain: ChainDefinition,
    permission_level: PermissionLevel,
    pub(crate) wallet_plugin: Arc<dyn WalletPlugin>,
    ui: Arc<dyn UserInterface>,
    platform: Arc<dyn Platform>,
    client: ChainClient,
    abi_cache: Arc<ABICache>,
    return_path: Option<String>,
}

impl Session {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        chain: ChainDefinition,
        permission_level: PermissionLevel,
        wallet_plugin: Arc<dyn WalletPlugin>,
        ui: Arc<dyn UserInterface>,
        platform: Arc<dyn Platform>,
        client: ChainClient,
        abi_cache: Arc<ABICache>,
        return_path: Option<String>,
    ) -> Self {
        Self {
            chain,
            permission_level,
            wallet_plugin,
            ui,
            platform,
            client,
            abi_cache,
            return_path,
        }
    }

    pub fn permission_level(&self) -> &PermissionLevel {
        &self.permission_level
    }
    pub fn chain(&self) -> &ChainDefinition {
        &self.chain
    }

    pub fn serialize(&self) -> SerializedSession {
        SerializedSession {
            chain_id: self.chain.id().as_string(),
            permission_level: permission_string(&self.permission_level),
            wallet_plugin_id: self.wallet_plugin.id(),
            wallet_plugin_data: self.wallet_plugin.serialize().data.0,
        }
    }

    fn esr_options(&self) -> EsrOptions {
        EsrOptions::new(self.abi_cache.clone())
    }

    fn build_transact_context(
        &self,
        cancel: CancellationToken,
        hooks: TransactHooks,
    ) -> TransactContext {
        TransactContext {
            chain: self.chain.clone(),
            ui: self.ui.clone(),
            platform: self.platform.clone(),
            abi_cache: self.abi_cache.clone(),
            esr_options: self.esr_options(),
            cancel,
            permission_level: self.permission_level,
            client: self.client.clone(),
            hooks,
            return_path: self.return_path.clone(),
        }
    }

    pub async fn transact(
        &self,
        args: TransactArgs,
        opts: TransactOptions,
    ) -> Result<TransactResult, TransactError> {
        let cancel = CancellationToken::new();
        let hooks = TransactHooks::default();
        let ctx = self.build_transact_context(cancel.clone(), hooks.clone());
        let TransactArgs { actions } = args;

        let inner = async {
            self.ui.on_transact().await?;

            let (signer_override, head) = tokio::join!(
                self.wallet_plugin.resolve_permission(&actions, &ctx),
                self.fetch_chain_head(),
            );
            let signer = signer_override?.unwrap_or(self.permission_level);
            let (head_block_id, head_block_time_unix) = head?;

            let request = SigningRequest::create(
                SigningRequestCreateArgs {
                    chain_id: *ctx.chain.id(),
                    actions,
                    callback: None,
                    expiration: opts.expire_seconds,
                },
                &ctx.esr_options,
            )?;

            let mut result = TransactResult {
                chain: ctx.chain.clone(),
                request: request.clone(),
                resolved: None,
                signer,
                signatures: vec![],
                transaction: None,
                response: None,
            };

            for hook in &ctx.hooks.before_sign {
                hook.call(&ctx, &mut result).await?;
            }

            let resolve_ctx = ResolveContext {
                chain_id: *ctx.chain.id(),
                expire_seconds: opts.expire_seconds.unwrap_or(120),
                head_block_id,
                head_block_time_unix,
            };
            let resolved = request.resolve(&resolve_ctx, Some(signer)).await?;
            result.transaction = Some(resolved.transaction.clone());

            self.ui.on_sign().await?;

            let wallet_resp = self.wallet_plugin.sign(&resolved, &ctx).await?;
            result.signatures = wallet_resp.signatures;

            for hook in &ctx.hooks.after_sign {
                hook.call(&ctx, &mut result).await?;
            }

            self.ui.on_sign_complete().await?;

            if opts.broadcast {
                self.ui.on_broadcast().await?;
                let response = self.broadcast(&resolved, &result.signatures).await?;
                result.response = Some(response);

                for hook in &ctx.hooks.after_broadcast {
                    hook.call(&ctx, &mut result).await?;
                }

                self.ui.on_broadcast_complete().await?;
            }

            result.resolved = Some(resolved);
            self.ui.on_transact_complete().await?;
            Ok(result)
        };

        let result = tokio::select! {
            _ = cancel.cancelled() => Err(TransactError::Cancelled),
            result = inner => result,
        };

        if let Err(ref e) = result {
            let session_err = crate::SessionError::Internal(format!("{e}"));
            let _ = self.ui.on_error(&session_err).await;
        }

        result
    }

    pub async fn sign_transaction(
        &self,
        transaction: Transaction,
    ) -> Result<Vec<antelope::chain::signature::Signature>, TransactError> {
        let actions = transaction.actions.clone();
        let chain_id = *self.chain.id();
        let request = SigningRequest::create(
            SigningRequestCreateArgs {
                chain_id,
                actions,
                callback: None,
                expiration: None,
            },
            &self.esr_options(),
        )?;
        let resolved = ResolvedSigningRequest {
            request,
            transaction,
            signer: self.permission_level,
            chain_id,
        };
        let ctx = self.build_transact_context(CancellationToken::new(), TransactHooks::default());
        let response = self.wallet_plugin.sign(&resolved, &ctx).await?;
        Ok(response.signatures)
    }

    async fn fetch_chain_head(&self) -> Result<(Checksum256, u64), TransactError> {
        let info = self
            .client
            .v1_chain
            .get_info()
            .await
            .map_err(|e| TransactError::Internal(format!("get_info: {e:?}")))?;
        // BlockId always carries 32 bytes (validated upstream).
        let head_id = Checksum256::from_bytes(&info.head_block_id.bytes)
            .expect("BlockId guarantees 32 bytes");
        let head_unix_s = (info.head_block_time.elapsed / 1_000_000) as u64;
        Ok((head_id, head_unix_s))
    }

    async fn broadcast(
        &self,
        resolved: &ResolvedSigningRequest,
        signatures: &[antelope::chain::signature::Signature],
    ) -> Result<serde_json::Value, TransactError> {
        let signed = SignedTransaction {
            transaction: resolved.transaction.clone(),
            signatures: signatures.to_vec(),
            context_free_data: vec![],
        };
        // 404 string-match: antelope-rs's ClientError collapses non-2xx status into
        // `NETWORK("Failed to send transaction")` and loses the HTTP code — see the
        // deferred-cleanup entry for `send_transaction2_with_status`. Until that lands,
        // we match against the debug repr (`format!("{e:?}").contains("404")`) so older
        // chains that lack `/v1/chain/send_transaction2` fall back to `send_transaction`.
        match self
            .client
            .v1_chain
            .send_transaction2(signed.clone(), None)
            .await
        {
            Ok(resp) => Ok(serde_json::to_value(resp).unwrap_or(serde_json::Value::Null)),
            Err(e) if format!("{e:?}").contains("404") => {
                match self.client.v1_chain.send_transaction(signed.clone()).await {
                    Ok(resp) => Ok(serde_json::to_value(resp).unwrap_or(serde_json::Value::Null)),
                    Err(e2) => Err(broadcast_error(signed, signatures, e2)),
                }
            }
            Err(e) => Err(broadcast_error(signed, signatures, e)),
        }
    }
}

fn broadcast_error<E: std::fmt::Debug>(
    signed: SignedTransaction,
    signatures: &[antelope::chain::signature::Signature],
    e: E,
) -> TransactError {
    TransactError::Broadcast {
        signed: Box::new(signed),
        signatures: signatures.to_vec(),
        chain_error: format!("{e:?}"),
    }
}
