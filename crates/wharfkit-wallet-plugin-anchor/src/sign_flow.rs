use antelope::chain::action::Action;
use antelope::chain::checksum::Checksum256;
use antelope::chain::private_key::PrivateKey;
use antelope::chain::public_key::PublicKey;
use antelope::chain::signature::Signature;
use antelope::chain::time::TimePointSec;
use antelope::chain::{Encoder, Packer};
use futures_util::StreamExt;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;
use wharfkit_buoy_client::BuoyClient;
use wharfkit_session::{
    PromptArgs, PromptElement, PromptResponse, TransactContext, WalletError,
    WalletPluginSignResponse,
};
use wharfkit_signing_request::{
    CallbackPayload, CallbackSpec, EsrOptions, LinkInfo, ResolvedSigningRequest, SigningRequest,
    SigningRequestCreateArgs,
};

use crate::plugin::AnchorWalletPlugin;
use crate::same_device::apply_ios_same_device_info;
use crate::sealed::{pack_sealed_message, seal, SealedMessage};

pub const DEFAULT_SIGN_TIMEOUT_SECS: u64 = 120;

/// Expiration window, in seconds, for the `link` info key that wakes iOS Anchor's `?v=2` listener.
pub const LINK_INFO_EXPIRATION_SECS: u32 = 120;

pub async fn run_sign(
    plugin: &AnchorWalletPlugin,
    resolved: &ResolvedSigningRequest,
    ctx: &TransactContext,
) -> Result<WalletPluginSignResponse, WalletError> {
    let (channel_url, signer_key_str, private_key_wif, same_device, launch_url, channel_name) = {
        let data = plugin.data.lock().unwrap();
        (
            data.channel_url.clone(),
            data.signer_key.clone(),
            data.private_key.clone(),
            data.same_device,
            data.launch_url.clone(),
            data.channel_name.clone().unwrap_or_else(|| "Anchor".into()),
        )
    };

    if same_device {
        if let Some(url) = launch_url.as_ref() {
            ctx.platform.shell_open(url);
        }
    }

    let sign_uuid = Uuid::new_v4();
    let sign_callback_url = format!("{}/{}", plugin.buoy_url.trim_end_matches('/'), sign_uuid);

    let request_with_cb = build_sign_request(
        resolved.chain_id,
        resolved.transaction.actions.clone(),
        &sign_callback_url,
        &ctx.esr_options,
        ctx.platform.is_apple_handheld(),
        ctx.return_path.as_deref(),
    )?;
    let sign_uri = request_with_cb
        .encode(true, false, "esr:")
        .map_err(|e| WalletError::Internal(format!("encode sign request: {e}")))?;

    if let (Some(ch_url), Some(signer_key), Some(priv_wif)) = (
        channel_url.as_ref(),
        signer_key_str.as_ref(),
        private_key_wif.as_ref(),
    ) {
        send_sealed_to_anchor_channel(plugin, &sign_uri, ch_url, signer_key, priv_wif).await?;
    } else {
        ctx.platform.shell_open(&sign_uri);
    }

    let expiration_unix_ms = ctx_expiration_unix_ms(resolved);
    let prompt_args = build_sign_prompt_args(&channel_name, expiration_unix_ms, &sign_uri);

    let buoy = BuoyClient::new(plugin.buoy_url.clone());
    let channel = buoy.channel(sign_uuid);

    let listen_future = async move {
        let stream = channel.listen().await?;
        let mut stream = Box::pin(stream);
        match stream.next().await {
            Some(Ok(bytes)) => Ok(bytes),
            Some(Err(e)) => Err(WalletError::Buoy(e)),
            None => Err(WalletError::Internal("Buoy stream closed".into())),
        }
    };
    let prompt_future = ctx.ui.prompt(prompt_args);

    let payload_bytes = tokio::select! {
        biased;
        _ = ctx.cancel.cancelled() => return Err(WalletError::Cancelled),
        _ = sleep(Duration::from_secs(DEFAULT_SIGN_TIMEOUT_SECS)) => {
            return Err(WalletError::Expired);
        }
        result = listen_future => result?,
        prompt_response = prompt_future => {
            return Err(map_sign_prompt_response(prompt_response));
        }
    };

    parse_sign_callback(&payload_bytes)
}

pub(crate) fn build_sign_request(
    chain_id: Checksum256,
    actions: Vec<Action>,
    sign_callback_url: &str,
    esr_options: &EsrOptions,
    is_apple_handheld: bool,
    return_path: Option<&str>,
) -> Result<SigningRequest, WalletError> {
    let mut request = SigningRequest::create(
        SigningRequestCreateArgs {
            chain_id,
            actions,
            callback: Some(CallbackSpec {
                url: sign_callback_url.to_string(),
                background: true,
            }),
            expiration: None,
        },
        esr_options,
    )?;
    // Otherwise Anchor also broadcasts, and our own broadcast fails with tx_duplicate.
    request.set_broadcast(false);
    if is_apple_handheld {
        apply_ios_same_device_info(&mut request, return_path);
    }
    attach_link_info(&mut request);
    Ok(request)
}

/// Without `info["link"]`, iOS Anchor's `?v=2` channel listener never wakes for the sealed message.
fn attach_link_info(request: &mut SigningRequest) {
    let now_unix = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as u32)
        .unwrap_or(0);
    let link_info = LinkInfo {
        expiration: TimePointSec::new(now_unix.saturating_add(LINK_INFO_EXPIRATION_SECS)),
    };
    let mut enc = Encoder::new(link_info.size());
    link_info.pack(&mut enc);
    request.set_info_bytes("link", enc.get_bytes().to_vec());
}

pub async fn send_sealed_to_anchor_channel(
    plugin: &AnchorWalletPlugin,
    sign_uri: &str,
    channel_url: &str,
    signer_key_str: &str,
    private_key_wif: &str,
) -> Result<(), WalletError> {
    let receiver_pub = PublicKey::new_from_str(signer_key_str)
        .map_err(|e| WalletError::Internal(format!("signer_key parse: {e}")))?;
    let sender_priv = PrivateKey::from_str(private_key_wif, false)
        .map_err(|e| WalletError::Internal(format!("private_key parse: {e}")))?;
    let nonce: u64 = rand::random();
    let sealed: SealedMessage = seal(sign_uri.as_bytes(), &sender_priv, &receiver_pub, nonce)
        .map_err(|e| WalletError::Internal(format!("seal: {e}")))?;
    let envelope = pack_sealed_message(&sealed);

    // Channel URL is case-sensitive on the buoy relay; iOS Anchor uses uppercase
    // UUIDs. Strip ?query/#frag only — do not round-trip via Uuid.
    let post_url = strip_query_fragment(channel_url);
    let resp = plugin
        .transport
        .post(&post_url, &envelope, HashMap::new())
        .await
        .map_err(WalletError::Buoy)?;
    match resp.status {
        200 | 202 | 204 => Ok(()),
        other => Err(WalletError::Internal(format!(
            "sealed POST to {post_url}: unexpected status {other}"
        ))),
    }
}

fn strip_query_fragment(url: &str) -> String {
    let no_frag = url.split('#').next().unwrap_or(url);
    no_frag.split('?').next().unwrap_or(no_frag).to_string()
}

pub(crate) fn build_sign_prompt_args(
    channel_name: &str,
    expiration_unix_ms: i64,
    same_device_uri: &str,
) -> PromptArgs {
    PromptArgs {
        title: "Complete using Anchor".into(),
        body: Some(format!(
            "Please open Anchor on \"{channel_name}\" to review and approve this transaction.",
        )),
        optional: true,
        elements: vec![
            PromptElement::Countdown {
                id: "expire".into(),
                label: "Waiting for response from Anchor".into(),
                end_unix_ms: expiration_unix_ms,
            },
            PromptElement::Link {
                id: "sign_manually".into(),
                href: same_device_uri.to_string(),
                label: "Sign manually or with another device".into(),
                variant: wharfkit_session::LinkVariant::Secondary,
            },
        ],
    }
}

fn ctx_expiration_unix_ms(resolved: &ResolvedSigningRequest) -> i64 {
    (resolved.transaction.header.expiration.seconds as i64).saturating_mul(1000)
}

fn map_sign_prompt_response(r: Result<PromptResponse, wharfkit_session::UiError>) -> WalletError {
    match r {
        Ok(PromptResponse::Closed) => WalletError::UserClosed,
        Ok(PromptResponse::Expired) => WalletError::Expired,
        Ok(_) => WalletError::Internal("unexpected prompt response on sign".into()),
        Err(e) => WalletError::Internal(format!("UI error: {e}")),
    }
}

pub(crate) fn parse_sign_callback(bytes: &[u8]) -> Result<WalletPluginSignResponse, WalletError> {
    let payload = CallbackPayload::from_json(bytes)
        .map_err(|e| WalletError::Internal(format!("callback parse: {e}")))?;
    if let Some(reason) = payload.rejected.as_ref() {
        return Err(WalletError::UserRejected(reason.clone()));
    }
    let signatures = extract_signatures(&payload)?;
    if signatures.is_empty() {
        return Err(WalletError::Internal(
            "callback payload contained no signatures".into(),
        ));
    }
    Ok(WalletPluginSignResponse {
        signatures,
        resolved: None,
    })
}

pub async fn parse_sign_callback_with_options(
    bytes: &[u8],
    opts: &wharfkit_signing_request::EsrOptions,
) -> Result<WalletPluginSignResponse, WalletError> {
    let payload = CallbackPayload::from_json(bytes)
        .map_err(|e| WalletError::Internal(format!("callback parse: {e}")))?;
    if let Some(reason) = payload.rejected.as_ref() {
        return Err(WalletError::UserRejected(reason.clone()));
    }
    let signatures = extract_signatures(&payload)?;
    if signatures.is_empty() {
        return Err(WalletError::Internal(
            "callback payload contained no signatures".into(),
        ));
    }
    let resolved = ResolvedSigningRequest::from_payload(&payload, opts)
        .map_err(|e| WalletError::Internal(format!("resolved from_payload: {e}")))?;
    Ok(WalletPluginSignResponse {
        signatures,
        resolved: Some(resolved),
    })
}

fn extract_signatures(payload: &CallbackPayload) -> Result<Vec<Signature>, WalletError> {
    let mut sigs = Vec::new();
    if let Some(sig) = payload.sig.as_ref() {
        sigs.push(
            Signature::from_string(sig)
                .map_err(|e| WalletError::Internal(format!("sig parse: {e}")))?,
        );
    }
    for idx in 0u32.. {
        let key = format!("sig{idx}");
        let Some(v) = payload.extra.get(&key) else {
            break;
        };
        let Some(s) = v.as_str() else { break };
        sigs.push(
            Signature::from_string(s)
                .map_err(|e| WalletError::Internal(format!("{key} parse: {e}")))?,
        );
    }
    Ok(sigs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sign_request(is_apple_handheld: bool, return_path: Option<&str>) -> SigningRequest {
        build_sign_request(
            Checksum256::default(),
            vec![],
            "https://cb.example/uuid",
            &EsrOptions::offline(),
            is_apple_handheld,
            return_path,
        )
        .expect("build_sign_request")
    }

    #[test]
    fn built_sign_request_disables_broadcast() {
        assert!(!sign_request(false, None).broadcast());
    }

    #[test]
    fn built_sign_request_carries_link_info() {
        let request = sign_request(false, None);
        assert!(request.info.iter().any(|kv| kv.key == "link"));
    }

    #[test]
    fn built_sign_request_on_ios_sets_same_device_and_return_path() {
        let request = sign_request(true, Some("myapp://callback"));
        assert!(request.info.iter().any(|kv| kv.key == "same_device"));
        assert!(request.info.iter().any(|kv| kv.key == "return_path"));
    }

    #[test]
    fn built_sign_request_on_ios_sets_same_device_without_return_path() {
        let request = sign_request(true, None);
        assert!(request.info.iter().any(|kv| kv.key == "same_device"));
        assert!(!request.info.iter().any(|kv| kv.key == "return_path"));
    }

    #[test]
    fn built_sign_request_on_non_ios_omits_same_device() {
        let request = sign_request(false, Some("myapp://callback"));
        assert!(!request.info.iter().any(|kv| kv.key == "same_device"));
    }

    #[test]
    fn sign_prompt_has_countdown_and_link() {
        let args = build_sign_prompt_args("My Wallet", 1_700_000_000_000, "esr:sd");
        let n_countdown = args
            .elements
            .iter()
            .filter(|e| matches!(e, PromptElement::Countdown { .. }))
            .count();
        let n_link = args
            .elements
            .iter()
            .filter(|e| matches!(e, PromptElement::Link { .. }))
            .count();
        assert_eq!(n_countdown, 1);
        assert_eq!(n_link, 1);
    }

    fn fixture_signature() -> String {
        let priv_key = antelope::chain::private_key::PrivateKey::from_str(
            "5Jtoxgny5tT7NiNFp1MLogviuPJ9NniWjnU4wKzaX4t7pL4kJ8s",
            false,
        )
        .unwrap();
        priv_key
            .sign_message(&b"fixture-payload".to_vec())
            .to_string()
    }

    #[test]
    fn parse_sign_callback_extracts_signature() {
        let sig = fixture_signature();
        let json = format!(r#"{{"sig":"{sig}"}}"#);
        let resp = parse_sign_callback(json.as_bytes()).expect("happy path");
        assert_eq!(resp.signatures.len(), 1);
    }

    #[test]
    fn parse_sign_callback_handles_multi_sig() {
        let sig = fixture_signature();
        let json = format!(r#"{{"sig":"{sig}","sig0":"{sig}"}}"#);
        let resp = parse_sign_callback(json.as_bytes()).expect("happy path");
        assert_eq!(resp.signatures.len(), 2);
    }

    #[test]
    fn parse_sign_callback_rejects_explicit_rejection() {
        let json = br#"{"rejected":"User cancelled"}"#;
        let err = match parse_sign_callback(json) {
            Err(e) => e,
            Ok(_) => panic!("expected an error"),
        };
        assert!(matches!(err, WalletError::UserRejected(_)));
    }

    #[test]
    fn parse_sign_callback_errors_on_empty() {
        let json = br#"{}"#;
        let err = match parse_sign_callback(json) {
            Err(e) => e,
            Ok(_) => panic!("expected an error"),
        };
        match err {
            WalletError::Internal(m) => assert!(m.contains("no signatures")),
            other => panic!("expected Internal, got {other:?}"),
        }
    }
}
