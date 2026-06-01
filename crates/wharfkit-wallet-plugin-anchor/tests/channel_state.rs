use antelope::chain::private_key::PrivateKey;
use wharfkit_session::WalletError;
use wharfkit_wallet_plugin_anchor::{AnchorChannelState, AnchorWalletData, AnchorWalletPlugin};

const DETERMINISTIC_WIF: &str = "5Jtoxgny5tT7NiNFp1MLogviuPJ9NniWjnU4wKzaX4t7pL4kJ8s";

#[test]
fn set_channel_keys_persists_request_key_and_private_wif() {
    let plugin = AnchorWalletPlugin::new();
    let private = PrivateKey::from_str(DETERMINISTIC_WIF, false).expect("private key");
    let public = private.to_public();
    let wif = private.to_wif().expect("wif");

    plugin.set_channel_keys(public.clone(), wif.clone());

    let snap = plugin.data_snapshot();
    assert_eq!(
        snap.request_key.as_deref(),
        Some(public.as_string().as_str())
    );
    assert_eq!(snap.private_key.as_deref(), Some(wif.as_str()));
}

#[test]
fn channel_state_returns_none_when_unset() {
    let plugin = AnchorWalletPlugin::new();
    assert!(plugin.channel_state().is_none());
}

#[test]
fn channel_state_returns_typed_struct_when_set() {
    let plugin = AnchorWalletPlugin::new();
    let private = PrivateKey::from_str(DETERMINISTIC_WIF, false).expect("private key");
    let signer_key = private.to_public().as_string();
    let wif = private.to_wif().expect("wif");
    let data = AnchorWalletData {
        channel_url: Some("https://cb.anchor.link/abc".into()),
        signer_key: Some(signer_key.clone()),
        private_key: Some(wif.clone()),
        channel_name: Some("My iPhone".into()),
        same_device: false,
        launch_url: Some("anchor://link".into()),
        ..Default::default()
    };
    plugin.set_data(data);

    let state: AnchorChannelState = plugin.channel_state().expect("channel_state Some");
    assert_eq!(state.channel_url, "https://cb.anchor.link/abc");
    assert_eq!(state.signer_key.as_string(), signer_key);
    assert_eq!(state.private_key.to_wif().unwrap(), wif);
    assert_eq!(state.channel_name.as_deref(), Some("My iPhone"));
    assert_eq!(state.launch_url.as_deref(), Some("anchor://link"));
    assert!(!state.same_device);
}

#[test]
fn channel_state_debug_redacts_private_key() {
    let plugin = AnchorWalletPlugin::new();
    let private = PrivateKey::from_str(DETERMINISTIC_WIF, false).expect("private key");
    plugin.set_data(AnchorWalletData {
        channel_url: Some("https://cb.anchor.link/abc".into()),
        signer_key: Some(private.to_public().as_string()),
        private_key: Some(private.to_wif().expect("wif")),
        ..Default::default()
    });

    let state = plugin.channel_state().expect("channel_state Some");
    let debug = format!("{state:?}");
    assert!(!debug.contains(DETERMINISTIC_WIF));
    assert!(debug.contains("<redacted>"));
}

#[test]
fn try_channel_state_errors_on_invalid_signer_key() {
    let plugin = AnchorWalletPlugin::new();
    let private = PrivateKey::from_str(DETERMINISTIC_WIF, false).expect("private key");
    plugin.set_data(AnchorWalletData {
        channel_url: Some("https://cb.anchor.link/abc".into()),
        signer_key: Some("not-a-public-key".into()),
        private_key: Some(private.to_wif().expect("wif")),
        ..Default::default()
    });

    let err = plugin
        .try_channel_state()
        .expect_err("malformed signer key");
    assert!(matches!(err, WalletError::Internal(msg) if msg.contains("signer_key")));
    assert!(plugin.channel_state().is_none());
}

#[test]
fn try_channel_state_errors_on_invalid_private_key() {
    let plugin = AnchorWalletPlugin::new();
    let private = PrivateKey::from_str(DETERMINISTIC_WIF, false).expect("private key");
    plugin.set_data(AnchorWalletData {
        channel_url: Some("https://cb.anchor.link/abc".into()),
        signer_key: Some(private.to_public().as_string()),
        private_key: Some("not-a-private-key".into()),
        ..Default::default()
    });

    let err = plugin
        .try_channel_state()
        .expect_err("malformed private key");
    assert!(matches!(err, WalletError::Internal(msg) if msg.contains("private_key")));
    assert!(plugin.channel_state().is_none());
}
