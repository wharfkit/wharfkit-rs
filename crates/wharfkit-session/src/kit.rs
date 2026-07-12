use antelope::chain::action::PermissionLevel;
use antelope::chain::checksum::Checksum256;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use wharfkit_abicache::ABICache;
use wharfkit_common::ChainDefinition;
use wharfkit_signing_request::EsrOptions;

use crate::error::SessionError;
use crate::login::{LoginContext, LoginHooks, UiRequirements, UserInterfaceWalletPlugin};
use crate::platform::Platform;
use crate::plugins::{LoginPlugin, TransactPlugin};
use crate::session::Session;
use crate::storage::SessionStorage;
use crate::storage::{permission_string, session_key, DEFAULT_SESSION_KEY};
use crate::transact::ChainClient;
use crate::ui::UserInterface;
use crate::wallet::{LogoutContext, WalletPlugin, WalletPluginData};

pub struct SessionKitArgs {
    pub app_name: String,
    pub chains: Vec<ChainDefinition>,
    pub ui: Arc<dyn UserInterface>,
    pub platform: Arc<dyn Platform>,
    pub wallet_plugins: Vec<Arc<dyn WalletPlugin>>,
    pub storage: Arc<dyn SessionStorage>,
    pub client: ChainClient,
    pub abi_cache: Arc<ABICache>,
    pub login_plugins: Vec<Arc<dyn LoginPlugin>>,
    pub transact_plugins: Vec<Arc<dyn TransactPlugin>>,
    /// Deep-link URI wallets should return to after login/sign, e.g. for iOS same-device Anchor.
    pub return_path: Option<String>,
}

#[derive(Default)]
pub struct LoginOptions {
    pub chain: Option<Checksum256>,
    pub permission_level: Option<PermissionLevel>,
    pub wallet_plugin: Option<String>,
    pub arbitrary: HashMap<String, serde_json::Value>,
    pub set_as_default: bool,
}

pub struct RestoreArgs {
    pub chain: Checksum256,
    pub permission_level: PermissionLevel,
    pub wallet_plugin: String,
}

pub struct SessionKit {
    pub(crate) args: SessionKitArgs,
}

impl SessionKit {
    pub fn new(args: SessionKitArgs) -> Self {
        Self { args }
    }

    pub fn chains(&self) -> &[ChainDefinition] {
        &self.args.chains
    }

    pub fn app_name(&self) -> &str {
        &self.args.app_name
    }

    fn find_chain(&self, chain_id: &Checksum256) -> Option<ChainDefinition> {
        self.args
            .chains
            .iter()
            .find(|c| *c.id() == *chain_id)
            .cloned()
    }

    fn build_session(
        &self,
        chain: ChainDefinition,
        permission_level: PermissionLevel,
        wallet: Arc<dyn WalletPlugin>,
    ) -> Session {
        Session::new(
            chain,
            permission_level,
            wallet,
            self.args.ui.clone(),
            self.args.platform.clone(),
            self.args.client.clone(),
            self.args.abi_cache.clone(),
            self.args.return_path.clone(),
        )
    }

    pub async fn login(&self, mut opts: LoginOptions) -> Result<Session, SessionError> {
        let cancel = CancellationToken::new();

        let mut ctx = LoginContext {
            chain: None,
            chains: self.args.chains.clone(),
            ui: self.args.ui.clone(),
            platform: self.args.platform.clone(),
            wallet_plugins: self
                .args
                .wallet_plugins
                .iter()
                .map(|wp| UserInterfaceWalletPlugin {
                    id: wp.id(),
                    config: wp.config(),
                    metadata: wp.metadata(),
                })
                .collect(),
            permission_level: opts.permission_level,
            wallet_plugin_index: None,
            esr_options: EsrOptions::new(self.args.abi_cache.clone()),
            cancel: cancel.clone(),
            hooks: LoginHooks::default(),
            arbitrary: std::mem::take(&mut opts.arbitrary),
            ui_requirements: UiRequirements::default(),
        };

        for plugin in &self.args.login_plugins {
            plugin.register(&mut ctx.hooks);
        }

        let inner = async {
            self.args.ui.on_login().await?;

            let mut wallet: Option<Arc<dyn WalletPlugin>> = None;
            if self.args.wallet_plugins.len() == 1 {
                wallet = Some(self.args.wallet_plugins[0].clone());
                ctx.wallet_plugin_index = Some(0);
                ctx.ui_requirements.requires_wallet_select = false;
            } else if let Some(id) = &opts.wallet_plugin {
                wallet = self
                    .args
                    .wallet_plugins
                    .iter()
                    .find(|wp| wp.id() == *id)
                    .cloned();
                ctx.wallet_plugin_index = self
                    .args
                    .wallet_plugins
                    .iter()
                    .position(|wp| wp.id() == *id);
                ctx.ui_requirements.requires_wallet_select = false;
            } else {
                ctx.ui_requirements.requires_wallet_select = true;
            }

            if let Some(chain_id) = opts.chain {
                ctx.chain = self.find_chain(&chain_id);
                ctx.ui_requirements.requires_chain_select = false;
            } else if self.args.chains.len() == 1 {
                ctx.chain = Some(self.args.chains[0].clone());
                ctx.ui_requirements.requires_chain_select = false;
            } else {
                ctx.ui_requirements.requires_chain_select = true;
            }

            if ctx.ui_requirements.requires_chain_select
                || ctx.ui_requirements.requires_wallet_select
                || ctx.ui_requirements.requires_permission_select
                || ctx.ui_requirements.requires_permission_entry
            {
                let ui_response = self.args.ui.login(&ctx).await?;
                if wallet.is_none() {
                    wallet = self
                        .args
                        .wallet_plugins
                        .get(ui_response.wallet_plugin_index)
                        .cloned();
                }
                if let Some(chain_id) = ui_response.chain_id {
                    ctx.chain = self.find_chain(&chain_id);
                }
                if let Some(pl) = ui_response.permission_level {
                    ctx.permission_level = Some(pl);
                }
            }

            let wallet = wallet.ok_or(SessionError::MissingWallet)?;
            ctx.chain.as_ref().ok_or(SessionError::MissingChain)?;

            for hook in &ctx.hooks.before_login {
                hook.call(&ctx).await?;
            }

            let response = wallet.login(&ctx).await?;

            let chain = self
                .find_chain(&response.chain)
                .ok_or(SessionError::MissingChain)?;

            let session = self.build_session(chain, response.permission_level, wallet);

            for hook in &ctx.hooks.after_login {
                hook.call(&ctx).await?;
            }

            self.persist_session(&session, opts.set_as_default).await?;

            self.args.ui.on_login_complete().await?;

            Ok(session)
        };

        let result = tokio::select! {
            _ = cancel.cancelled() => Err(SessionError::Cancelled),
            result = inner => result,
        };

        if let Err(ref e) = result {
            let _ = self.args.ui.on_error(e).await;
        }

        result
    }

    async fn persist_session(
        &self,
        session: &Session,
        set_as_default: bool,
    ) -> Result<(), SessionError> {
        let serialized = session.serialize();
        let bytes = serde_json::to_vec(&serialized)?;
        let key = session_key(&serialized.chain_id, &serialized.permission_level);
        self.args.storage.write(&key, &bytes).await?;
        if set_as_default {
            self.args.storage.write(DEFAULT_SESSION_KEY, &bytes).await?;
        }
        Ok(())
    }

    pub async fn restore(&self, args: RestoreArgs) -> Result<Session, SessionError> {
        let key = session_key(
            &args.chain.as_string(),
            &permission_string(&args.permission_level),
        );
        let bytes = self
            .args
            .storage
            .read(&key)
            .await
            .ok_or(SessionError::SessionNotFound)?;
        let serialized: crate::storage::SerializedSession = serde_json::from_slice(&bytes)?;

        let wallet = self
            .args
            .wallet_plugins
            .iter()
            .find(|wp| wp.id() == serialized.wallet_plugin_id)
            .cloned()
            .ok_or(SessionError::MissingWallet)?;
        wallet.restore(WalletPluginData(serialized.wallet_plugin_data))?;

        let chain = self
            .find_chain(&args.chain)
            .ok_or(SessionError::MissingChain)?;

        Ok(self.build_session(chain, args.permission_level, wallet))
    }

    pub async fn logout(&self, session: &Session) -> Result<(), SessionError> {
        let serialized = session.serialize();
        let key = session_key(&serialized.chain_id, &serialized.permission_level);
        self.args.storage.remove(&key).await?;
        session
            .wallet_plugin
            .logout(&LogoutContext {
                chain: *session.chain().id(),
                permission_level: *session.permission_level(),
            })
            .await?;
        Ok(())
    }
}
