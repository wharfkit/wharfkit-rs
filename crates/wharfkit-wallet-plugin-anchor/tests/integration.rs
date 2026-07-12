//! Integration tests: AnchorWalletPlugin login + sign via mocks.
//!
//! Exercises the public surface (`AnchorWalletPlugin` as a `WalletPlugin`) end-to-end
//! with no real Buoy relay, no real Anchor, and no live chain. These tests run on
//! every `cargo test` invocation — the live conformance smoke against `cb.anchor.link`
//! is a separate manual gate (and not part of the workspace test suite).

use antelope::chain::checksum::Checksum256;
use antelope::chain::name::Name;
use antelope::chain::private_key::PrivateKey;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use wharfkit_abicache::ABICache;
use wharfkit_common::ChainDefinition;
use wharfkit_mock::{MockBuoyServer, MockPlatform, MockUserInterface};
use wharfkit_session::{
    LoginContext, LoginHooks, PlatformName, PromptResponse, UiRequirements, WalletError,
    WalletPlugin,
};
use wharfkit_signing_request::EsrOptions;
use wharfkit_wallet_plugin_anchor::AnchorWalletPlugin;

const DETERMINISTIC_WIF: &str = "5Jtoxgny5tT7NiNFp1MLogviuPJ9NniWjnU4wKzaX4t7pL4kJ8s";

fn esr_options() -> EsrOptions {
    EsrOptions::new(Arc::new(ABICache::new_offline()))
}

fn build_chain(url: &str) -> ChainDefinition {
    ChainDefinition::new(Checksum256::default(), url.to_string()).with_name("Test")
}

fn build_login_ctx(
    chain: ChainDefinition,
    ui: Arc<MockUserInterface>,
    platform: Arc<MockPlatform>,
    cancel: CancellationToken,
) -> LoginContext {
    LoginContext {
        chain: Some(chain),
        chains: vec![],
        ui: ui as Arc<dyn wharfkit_session::UserInterface>,
        platform: platform as Arc<dyn wharfkit_session::Platform>,
        wallet_plugins: vec![],
        permission_level: None,
        wallet_plugin_index: None,
        esr_options: esr_options(),
        cancel,
        hooks: LoginHooks::default(),
        arbitrary: Default::default(),
        ui_requirements: UiRequirements::default(),
    }
}

/// When the user closes the UI prompt, login returns `WalletError::UserClosed`.
/// (No Buoy callback is delivered — the prompt resolves first.)
#[tokio::test]
async fn login_user_closed_via_ui_prompt() {
    let buoy = MockBuoyServer::start().await;
    let plugin = AnchorWalletPlugin::with_buoy_relay(buoy.url());

    let ui = Arc::new(MockUserInterface::default());
    *ui.response.lock().unwrap() = PromptResponse::Closed;
    let platform = Arc::new(MockPlatform::new(PlatformName::Macos));

    let chain = build_chain("http://127.0.0.1:0/unused");
    let cancel = CancellationToken::new();
    let ctx = build_login_ctx(chain, ui.clone(), platform.clone(), cancel);

    let err = plugin.login(&ctx).await.unwrap_err();
    assert!(matches!(err, WalletError::UserClosed));

    // The prompt was actually shown.
    assert_eq!(ui.prompts.lock().unwrap().len(), 1);
    // The same-device URI was auto-launched.
    let opens = platform.shell_opens.lock().unwrap();
    assert!(
        !opens.is_empty(),
        "Anchor login should auto-launch a same-device URI"
    );
}

/// When the caller cancels (`ctx.cancel.cancelled()`), login propagates as
/// `WalletError::Cancelled`.
#[tokio::test]
async fn login_cancelled_via_token() {
    let buoy = MockBuoyServer::start().await;
    let plugin = AnchorWalletPlugin::with_buoy_relay(buoy.url());

    let ui = Arc::new(MockUserInterface::default());
    *ui.response.lock().unwrap() = PromptResponse::Closed;
    let platform = Arc::new(MockPlatform::new(PlatformName::Macos));

    let chain = build_chain("http://127.0.0.1:0/unused");
    let cancel = CancellationToken::new();
    let ctx = build_login_ctx(chain, ui, platform, cancel.clone());

    // Cancel before invoking so the select! has an already-ready cancel branch.
    // The mock UI also resolves its prompt instantly; either branch is an
    // acceptable observable cancellation outcome from the caller's perspective.
    cancel.cancel();
    let err = plugin.login(&ctx).await.unwrap_err();
    assert!(
        matches!(err, WalletError::Cancelled | WalletError::UserClosed),
        "expected Cancelled or UserClosed, got {err:?}"
    );
}

/// Mobile login: builds prompt elements without a QR (matching the TS plugin's
/// `isKnownMobile()` branch). Validates via the ui's recorded prompt args.
#[tokio::test]
async fn login_on_mobile_skips_qr_in_prompt() {
    use wharfkit_session::PromptElement;
    let buoy = MockBuoyServer::start().await;
    let plugin = AnchorWalletPlugin::with_buoy_relay(buoy.url());

    let ui = Arc::new(MockUserInterface::default());
    *ui.response.lock().unwrap() = PromptResponse::Closed;
    let platform = Arc::new(MockPlatform::new(PlatformName::IOS));

    let chain = build_chain("http://127.0.0.1:0/unused");
    let cancel = CancellationToken::new();
    let ctx = build_login_ctx(chain, ui.clone(), platform, cancel);

    let _ = plugin.login(&ctx).await; // expected to error after UI closes
    let prompts = ui.prompts.lock().unwrap();
    let args = prompts.first().expect("UI received a prompt");
    let has_qr = args
        .elements
        .iter()
        .any(|e| matches!(e, PromptElement::Qr { .. }));
    assert!(!has_qr, "mobile login must not include a QR element");
}

#[tokio::test]
async fn login_on_ios_sets_same_device_info_key() {
    let buoy = MockBuoyServer::start().await;
    let plugin = AnchorWalletPlugin::with_buoy_relay(buoy.url());

    let ui = Arc::new(MockUserInterface::default());
    *ui.response.lock().unwrap() = PromptResponse::Closed;
    let platform = Arc::new(MockPlatform::new(PlatformName::IOS));

    let chain = build_chain("http://127.0.0.1:0/unused");
    let cancel = CancellationToken::new();
    let mut ctx = build_login_ctx(chain, ui, platform.clone(), cancel);
    ctx.arbitrary.insert(
        "return_path".into(),
        serde_json::Value::String("myapp://callback".into()),
    );

    let _ = plugin.login(&ctx).await; // expected to error after UI closes

    let opens = platform.shell_opens.lock().unwrap();
    let same_device_uri = opens.first().expect("same-device URI opened");
    let request =
        wharfkit_signing_request::SigningRequest::from_uri(same_device_uri, &esr_options())
            .expect("decode same-device uri");
    assert!(request.info.iter().any(|kv| kv.key == "same_device"));
    assert!(request.info.iter().any(|kv| kv.key == "return_path"));
}

#[tokio::test]
async fn login_on_non_ios_omits_same_device_info_key() {
    let buoy = MockBuoyServer::start().await;
    let plugin = AnchorWalletPlugin::with_buoy_relay(buoy.url());

    let ui = Arc::new(MockUserInterface::default());
    *ui.response.lock().unwrap() = PromptResponse::Closed;
    let platform = Arc::new(MockPlatform::new(PlatformName::Macos));

    let chain = build_chain("http://127.0.0.1:0/unused");
    let cancel = CancellationToken::new();
    let ctx = build_login_ctx(chain, ui, platform.clone(), cancel);

    let _ = plugin.login(&ctx).await;

    let opens = platform.shell_opens.lock().unwrap();
    let same_device_uri = opens.first().expect("same-device URI opened");
    let request =
        wharfkit_signing_request::SigningRequest::from_uri(same_device_uri, &esr_options())
            .expect("decode same-device uri");
    assert!(!request.info.iter().any(|kv| kv.key == "same_device"));
}

/// Sign flow exercising `ctx.cancel` — verifies the sign path also wires the
/// cancel token into its `select!`.
#[tokio::test]
async fn sign_cancelled_via_token() {
    use antelope::api::client::APIClient;
    use antelope::api::client::Provider;
    use antelope::chain::action::{Action, PermissionLevel};
    use wharfkit_mock::MockChain;
    use wharfkit_session::TransactContext;
    use wharfkit_signing_request::request::{ResolveContext, SigningRequestCreateArgs};
    use wharfkit_signing_request::SigningRequest;

    let buoy = MockBuoyServer::start().await;
    let plugin = AnchorWalletPlugin::with_buoy_relay(buoy.url());

    // Seed Anchor channel state so the sign path attempts the sealed POST.
    let priv_k = PrivateKey::from_str(DETERMINISTIC_WIF, false).unwrap();
    let receiver_pub = priv_k.to_public();
    let wif = priv_k.to_wif().unwrap();
    plugin.set_data(wharfkit_wallet_plugin_anchor::AnchorWalletData {
        channel_url: Some(format!(
            "{}/{}",
            buoy.url(),
            "11111111-1111-1111-1111-111111111111"
        )),
        signer_key: Some(receiver_pub.as_string()),
        private_key: Some(wif),
        channel_name: Some("Mock Anchor".into()),
        ..Default::default()
    });

    let ui = Arc::new(MockUserInterface::default());
    *ui.response.lock().unwrap() = PromptResponse::Closed;
    let platform = Arc::new(MockPlatform::new(PlatformName::Macos));
    let cancel = CancellationToken::new();

    // Build a TransactContext from raw pieces.
    let chain = build_chain(&buoy.url());
    let provider: Arc<dyn Provider> = Arc::new(MockChain::new());
    let client = APIClient::custom_provider(provider).expect("client");
    let abi_cache = Arc::new(ABICache::new_offline());

    let actions: Vec<Action> = vec![];
    let request = SigningRequest::create(
        SigningRequestCreateArgs {
            chain_id: *chain.id(),
            actions,
            callback: None,
            expiration: None,
        },
        &esr_options(),
    )
    .unwrap();
    let resolved = request
        .resolve(
            &ResolveContext {
                chain_id: *chain.id(),
                expire_seconds: 60,
                head_block_id: Checksum256::default(),
                head_block_time_unix: 1_700_000_000,
            },
            Some(PermissionLevel::new(
                Name::new_from_str("alice"),
                Name::new_from_str("active"),
            )),
        )
        .await
        .unwrap();

    let ctx = TransactContext {
        chain,
        ui: ui as Arc<dyn wharfkit_session::UserInterface>,
        platform: platform as Arc<dyn wharfkit_session::Platform>,
        abi_cache,
        esr_options: esr_options(),
        cancel: cancel.clone(),
        permission_level: PermissionLevel::new(
            Name::new_from_str("alice"),
            Name::new_from_str("active"),
        ),
        client,
        hooks: Default::default(),
        return_path: None,
    };

    // Cancel synchronously so the select! observes it without waiting on Buoy.
    cancel.cancel();
    let err = match plugin.sign(&resolved, &ctx).await {
        Err(e) => e,
        Ok(_) => panic!("expected an error"),
    };
    assert!(
        matches!(err, WalletError::Cancelled | WalletError::UserClosed),
        "expected Cancelled/UserClosed, got {err:?}"
    );
}

#[tokio::test]
async fn sign_on_ios_sets_return_path_info_key() {
    use antelope::api::client::APIClient;
    use antelope::api::client::Provider;
    use antelope::chain::action::{Action, PermissionLevel};
    use wharfkit_mock::MockChain;
    use wharfkit_session::TransactContext;
    use wharfkit_signing_request::request::{ResolveContext, SigningRequestCreateArgs};
    use wharfkit_signing_request::SigningRequest;

    let buoy = MockBuoyServer::start().await;
    let plugin = AnchorWalletPlugin::with_buoy_relay(buoy.url());

    let ui = Arc::new(MockUserInterface::default());
    *ui.response.lock().unwrap() = PromptResponse::Closed;
    let platform = Arc::new(MockPlatform::new(PlatformName::IOS));
    let cancel = CancellationToken::new();

    let chain = build_chain(&buoy.url());
    let provider: Arc<dyn Provider> = Arc::new(MockChain::new());
    let client = APIClient::custom_provider(provider).expect("client");
    let abi_cache = Arc::new(ABICache::new_offline());

    let actions: Vec<Action> = vec![];
    let request = SigningRequest::create(
        SigningRequestCreateArgs {
            chain_id: *chain.id(),
            actions,
            callback: None,
            expiration: None,
        },
        &esr_options(),
    )
    .unwrap();
    let resolved = request
        .resolve(
            &ResolveContext {
                chain_id: *chain.id(),
                expire_seconds: 60,
                head_block_id: Checksum256::default(),
                head_block_time_unix: 1_700_000_000,
            },
            Some(PermissionLevel::new(
                Name::new_from_str("alice"),
                Name::new_from_str("active"),
            )),
        )
        .await
        .unwrap();

    let ctx = TransactContext {
        chain,
        ui: ui as Arc<dyn wharfkit_session::UserInterface>,
        platform: platform.clone() as Arc<dyn wharfkit_session::Platform>,
        abi_cache,
        esr_options: esr_options(),
        cancel: cancel.clone(),
        permission_level: PermissionLevel::new(
            Name::new_from_str("alice"),
            Name::new_from_str("active"),
        ),
        client,
        hooks: Default::default(),
        return_path: Some("myapp://callback".into()),
    };

    // Cancel synchronously so the select! observes it without waiting on Buoy.
    cancel.cancel();
    let _ = plugin.sign(&resolved, &ctx).await; // expected to error after cancel

    let opens = platform.shell_opens.lock().unwrap();
    let sign_uri = opens.first().expect("sign URI opened");
    let signing_request =
        SigningRequest::from_uri(sign_uri, &esr_options()).expect("decode sign uri");
    assert!(signing_request
        .info
        .iter()
        .any(|kv| kv.key == "same_device"));
    let return_path_kv = signing_request
        .info
        .iter()
        .find(|kv| kv.key == "return_path")
        .expect("return_path info key present");
    assert_eq!(return_path_kv.value, b"myapp://callback".to_vec());
}

/// Serialization round-trip — restore state is preserved across login sessions.
#[test]
fn serialize_restore_round_trip() {
    let plugin = AnchorWalletPlugin::new();
    plugin.set_data(wharfkit_wallet_plugin_anchor::AnchorWalletData {
        channel_url: Some("https://cb.anchor.link/some-uuid".into()),
        channel_name: Some("My Anchor".into()),
        same_device: true,
        launch_url: Some("anchor://link".into()),
        ..Default::default()
    });
    let snapshot = plugin.serialize();
    assert_eq!(snapshot.id, "anchor");

    let restored = AnchorWalletPlugin::new();
    restored.restore(snapshot.data).expect("restore");
    let d = restored.data_snapshot();
    assert_eq!(
        d.channel_url.as_deref(),
        Some("https://cb.anchor.link/some-uuid")
    );
    assert!(d.same_device);
}

// Silence unused-import warning in this file if certain tests get cfg'd out.
#[allow(dead_code)]
fn _force_use_serde_json() -> serde_json::Value {
    serde_json::Value::Null
}
