use antelope::api::client::Provider;
use antelope::chain::action::PermissionLevel;
use antelope::chain::checksum::Checksum256;
use antelope::chain::name::Name;
use antelope::chain::public_key::PublicKey;
use antelope::chain::signature::Signature;
use antelope::chain::time::TimePointSec;
use futures_util::StreamExt;
use std::sync::Arc;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;
use wharfkit_buoy_client::BuoyClient;
use wharfkit_session::{
    LinkVariant, LoginContext, PromptArgs, PromptElement, PromptResponse, WalletError,
    WalletPluginLoginResponse,
};
use wharfkit_signing_request::{
    CallbackPayload, IdentityProof, IdentityRequest, IdentityRequestArgs, ResolvedSigningRequest,
};

use crate::data::AnchorWalletData;
use crate::plugin::AnchorWalletPlugin;

pub const DEFAULT_LOGIN_TIMEOUT_SECS: u64 = 120;

pub async fn run_login(
    plugin: &AnchorWalletPlugin,
    ctx: &LoginContext,
) -> Result<WalletPluginLoginResponse, WalletError> {
    let chain = ctx
        .chain
        .as_ref()
        .ok_or_else(|| WalletError::Internal("Anchor login requires a chain selection".into()))?;
    let chain_id = *chain.id();

    let uuid = Uuid::new_v4();
    let private_key = plugin.generate_channel_keypair()?;
    let request_key = private_key.to_public();

    let app_name = ctx
        .arbitrary
        .get("app_name")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let identity = IdentityRequest::create_with_keypair(
        IdentityRequestArgs {
            chain_id,
            buoy_url: plugin.buoy_url.clone(),
            uuid,
            app_name,
            user_agent: format!("@wharfkit-rs/anchor {}", env!("CARGO_PKG_VERSION")),
        },
        &ctx.esr_options,
        private_key.clone(),
        request_key.clone(),
    )?;

    {
        let mut data = plugin.data.lock().unwrap();
        data.request_key = Some(request_key.as_string());
        data.private_key = Some(
            private_key
                .to_wif()
                .map_err(|e| WalletError::Internal(format!("to_wif: {e}")))?,
        );
    }

    let same_device_uri = identity
        .same_device_request
        .encode(true, false, "esr:")
        .map_err(|e| WalletError::Internal(format!("encode same_device: {e}")))?;
    ctx.platform.shell_open(&same_device_uri);

    let multi_uri = identity
        .request
        .encode(true, false, "esr:")
        .map_err(|e| WalletError::Internal(format!("encode request: {e}")))?;
    let prompt_args =
        build_login_prompt_args(&multi_uri, &same_device_uri, ctx.platform.is_known_mobile());

    let buoy = BuoyClient::new(plugin.buoy_url.clone());
    let channel = buoy.channel(uuid);

    let prompt_future = ctx.ui.prompt(prompt_args);
    let listen_future = async move {
        let stream = channel.listen().await?;
        let mut stream = Box::pin(stream);
        match stream.next().await {
            Some(Ok(bytes)) => Ok(bytes),
            Some(Err(e)) => Err(WalletError::Buoy(e)),
            None => Err(WalletError::Internal("Buoy stream closed".into())),
        }
    };

    let payload_bytes = tokio::select! {
        biased;
        _ = ctx.cancel.cancelled() => return Err(WalletError::Cancelled),
        _ = sleep(Duration::from_secs(DEFAULT_LOGIN_TIMEOUT_SECS)) => {
            return Err(WalletError::Expired);
        }
        listen_result = listen_future => listen_result?,
        prompt_response = prompt_future => {
            return Err(map_prompt_response(prompt_response));
        }
    };

    parse_login_callback(
        plugin,
        ctx,
        &payload_bytes,
        chain_id,
        &request_key,
        &private_key,
        &identity.callback,
        &identity.request,
    )
    .await
}

fn map_prompt_response(r: Result<PromptResponse, wharfkit_session::UiError>) -> WalletError {
    match r {
        Ok(PromptResponse::Closed) => WalletError::UserClosed,
        Ok(PromptResponse::Expired) => WalletError::Expired,
        Ok(_) => WalletError::Internal("unexpected prompt response on login".into()),
        Err(e) => WalletError::Internal(format!("UI error: {e}")),
    }
}

pub(crate) fn build_login_prompt_args(
    multi_uri: &str,
    same_device_uri: &str,
    is_mobile: bool,
) -> PromptArgs {
    let mut elements: Vec<PromptElement> = Vec::with_capacity(2);
    if !is_mobile {
        elements.push(PromptElement::Qr {
            data: multi_uri.to_string(),
        });
    }
    elements.push(PromptElement::Link {
        id: "launch_anchor".into(),
        href: same_device_uri.to_string(),
        label: "Launch Anchor".into(),
        variant: LinkVariant::Primary,
    });
    PromptArgs {
        title: "Connect with Anchor".into(),
        body: Some(
            "Scan with Anchor on your mobile device or click the button below to open on this device."
                .into(),
        ),
        optional: true,
        elements,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn parse_login_callback(
    plugin: &AnchorWalletPlugin,
    ctx: &LoginContext,
    bytes: &[u8],
    expected_chain: Checksum256,
    request_key: &PublicKey,
    private_key: &antelope::chain::private_key::PrivateKey,
    callback_url: &str,
    request: &wharfkit_signing_request::SigningRequest,
) -> Result<WalletPluginLoginResponse, WalletError> {
    let chain = ctx
        .chain
        .as_ref()
        .ok_or_else(|| WalletError::Internal("no chain in context".into()))?;
    parse_login_callback_with_chain_url(
        plugin,
        chain.url(),
        bytes,
        expected_chain,
        request_key,
        private_key,
        callback_url,
        request,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn parse_login_callback_with_chain_url(
    plugin: &AnchorWalletPlugin,
    chain_url: &str,
    bytes: &[u8],
    expected_chain: Checksum256,
    _request_key: &PublicKey,
    _private_key: &antelope::chain::private_key::PrivateKey,
    _callback_url: &str,
    request: &wharfkit_signing_request::SigningRequest,
) -> Result<WalletPluginLoginResponse, WalletError> {
    let payload = CallbackPayload::from_json(bytes)
        .map_err(|e| WalletError::Internal(format!("callback parse: {e}")))?;
    if let Some(reason) = payload.rejected.as_ref() {
        return Err(WalletError::UserRejected(reason.clone()));
    }
    let (sa, sp, cid, sig) = match (
        payload.sa.as_deref(),
        payload.sp.as_deref(),
        payload.cid.as_deref(),
        payload.sig.as_deref(),
    ) {
        (Some(sa), Some(sp), Some(cid), Some(sig)) => (sa, sp, cid, sig),
        _ => {
            return Err(WalletError::Internal(
                "callback payload missing sa/sp/cid/sig".into(),
            ));
        }
    };

    let expected_hex = hex::encode(expected_chain.data);
    if cid.to_lowercase() != expected_hex.to_lowercase() {
        return Err(WalletError::Internal(format!(
            "callback chain id {cid} does not match expected {expected_hex}"
        )));
    }

    let signer = PermissionLevel::new(Name::new_from_str(sa), Name::new_from_str(sp));
    let signature = parse_signature(sig)?;

    let resolved_for_proof =
        build_resolved_for_identity_proof(request.clone(), expected_chain, signer)?;
    let identity_proof: IdentityProof = resolved_for_proof
        .get_identity_proof(&signature)
        .map_err(|e| WalletError::Internal(format!("identity proof: {e}")))?;

    let client = build_chain_client_from_url(chain_url).await?;
    verify_recovered_key_on_chain(&client, &signer, &identity_proof.recovered_key).await?;

    {
        let mut data = plugin.data.lock().unwrap();
        update_data_from_link_fields(&mut data, &payload);
    }

    Ok(WalletPluginLoginResponse {
        chain: expected_chain,
        permission_level: signer,
        identity_proof: Some(identity_proof),
    })
}

pub(crate) async fn build_chain_client_from_url(
    chain_url: &str,
) -> Result<antelope::api::client::APIClient<Arc<dyn Provider>>, WalletError> {
    let provider: Arc<dyn Provider> = Arc::new(
        antelope::api::default_provider::DefaultProvider::new(chain_url.to_string(), None)
            .map_err(|e| WalletError::Internal(format!("provider: {e}")))?,
    );
    antelope::api::client::APIClient::custom_provider(provider)
        .map_err(|e| WalletError::Internal(format!("client: {e}")))
}

fn parse_signature(s: &str) -> Result<Signature, WalletError> {
    Signature::from_string(s).map_err(|e| WalletError::Internal(format!("signature parse: {e}")))
}

fn build_resolved_for_identity_proof(
    request: wharfkit_signing_request::SigningRequest,
    chain_id: Checksum256,
    signer: PermissionLevel,
) -> Result<ResolvedSigningRequest, WalletError> {
    request
        .resolve_explicit_tapos(chain_id, signer, TimePointSec::new(0), 0, 0)
        .map_err(WalletError::Esr)
}

pub(crate) async fn verify_recovered_key_on_chain(
    client: &antelope::api::client::APIClient<Arc<dyn Provider>>,
    signer: &PermissionLevel,
    recovered_key: &PublicKey,
) -> Result<(), WalletError> {
    let account = client
        .v1_chain
        .get_account(signer.actor.to_string())
        .await
        .map_err(|e| WalletError::Internal(format!("get_account: {e:?}")))?;

    let permission_name = signer.permission.to_string();
    let permission = account
        .permissions
        .iter()
        .find(|p| p.perm_name().to_string() == permission_name)
        .ok_or_else(|| {
            WalletError::Internal(format!(
                "account {} has no permission {permission_name}",
                signer.actor
            ))
        })?;

    let on_chain_match = permission
        .required_auth()
        .keys
        .iter()
        .any(|kw| &kw.key == recovered_key);
    if on_chain_match {
        Ok(())
    } else {
        Err(WalletError::UserRejected(format!(
            "recovered key {} is not in required_auth.keys for {}@{permission_name}",
            recovered_key.as_string(),
            signer.actor,
        )))
    }
}

pub(crate) fn update_data_from_link_fields(data: &mut AnchorWalletData, payload: &CallbackPayload) {
    if let Some(link_ch) = payload.link_ch.as_ref() {
        data.channel_url = Some(link_ch.clone());
    }
    if let Some(link_key) = payload.link_key.as_ref() {
        data.signer_key = Some(link_key.clone());
    }
    if let Some(link_name) = payload.link_name.as_ref() {
        data.channel_name = Some(link_name.clone());
    }
    if let Some(link_meta) = payload.extra.get("link_meta").and_then(|v| v.as_str()) {
        if let Ok(meta) = serde_json::from_str::<serde_json::Value>(link_meta) {
            data.same_device = meta
                .get("sameDevice")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            data.launch_url = meta
                .get("launchUrl")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            data.trigger_url = meta
                .get("triggerUrl")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wharfkit_session::PromptElement;

    #[test]
    fn prompt_args_include_qr_on_desktop() {
        let args = build_login_prompt_args("esr:multi", "esr:samedev", false);
        let has_qr = args
            .elements
            .iter()
            .any(|e| matches!(e, PromptElement::Qr { .. }));
        let has_link = args
            .elements
            .iter()
            .any(|e| matches!(e, PromptElement::Link { .. }));
        assert!(has_qr, "desktop must include QR");
        assert!(has_link, "desktop must include the launch link");
    }

    #[test]
    fn prompt_args_skip_qr_on_mobile() {
        let args = build_login_prompt_args("esr:multi", "esr:samedev", true);
        let has_qr = args
            .elements
            .iter()
            .any(|e| matches!(e, PromptElement::Qr { .. }));
        let has_link = args
            .elements
            .iter()
            .any(|e| matches!(e, PromptElement::Link { .. }));
        assert!(!has_qr, "mobile must not include QR");
        assert!(has_link, "mobile must still include the launch link");
    }

    #[test]
    fn update_data_from_link_fields_populates_anchor_channel() {
        let mut data = AnchorWalletData::default();
        let mut payload = CallbackPayload {
            link_ch: Some("https://cb.anchor.link/uuid".into()),
            link_key: Some("PUB_K1_someKey".into()),
            link_name: Some("My Anchor".into()),
            ..Default::default()
        };
        payload.extra.insert(
            "link_meta".into(),
            serde_json::Value::String(
                r#"{"sameDevice":true,"launchUrl":"anchor://link","triggerUrl":"anchor://trigger"}"#
                    .into(),
            ),
        );

        update_data_from_link_fields(&mut data, &payload);
        assert_eq!(
            data.channel_url.as_deref(),
            Some("https://cb.anchor.link/uuid")
        );
        assert_eq!(data.signer_key.as_deref(), Some("PUB_K1_someKey"));
        assert_eq!(data.channel_name.as_deref(), Some("My Anchor"));
        assert!(data.same_device);
        assert_eq!(data.launch_url.as_deref(), Some("anchor://link"));
        assert_eq!(data.trigger_url.as_deref(), Some("anchor://trigger"));
    }
}
