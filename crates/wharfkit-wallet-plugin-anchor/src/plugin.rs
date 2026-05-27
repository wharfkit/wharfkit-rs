use antelope::chain::key_type::KeyType;
use antelope::chain::private_key::PrivateKey;
use async_trait::async_trait;
use std::sync::{Arc, Mutex};
use wharfkit_buoy_client::{BuoyTransport, ReqwestBuoyTransport};
use wharfkit_session::LoginContext;
use wharfkit_session::{
    LogoutContext, SerializedWalletPlugin, TransactContext, WalletError, WalletPlugin,
    WalletPluginConfig, WalletPluginData, WalletPluginLoginResponse, WalletPluginMetadata,
    WalletPluginSignResponse,
};
use wharfkit_signing_request::ResolvedSigningRequest;

use crate::data::AnchorWalletData;

pub const DEFAULT_BUOY_URL: &str = "https://cb.anchor.link";

pub type ChannelKeypairFn = dyn Fn() -> Result<PrivateKey, WalletError> + Send + Sync;

pub struct AnchorWalletPlugin {
    pub(crate) data: Mutex<AnchorWalletData>,
    pub(crate) buoy_url: String,
    pub(crate) keypair_fn: Box<ChannelKeypairFn>,
    pub(crate) transport: Arc<dyn BuoyTransport>,
}

impl AnchorWalletPlugin {
    pub fn new() -> Self {
        Self::with_buoy_relay(DEFAULT_BUOY_URL)
    }

    pub fn with_buoy_relay(buoy_url: impl Into<String>) -> Self {
        Self {
            data: Mutex::new(AnchorWalletData::default()),
            buoy_url: buoy_url.into(),
            keypair_fn: Box::new(generate_k1_keypair),
            transport: Arc::new(ReqwestBuoyTransport::new()),
        }
    }

    pub fn with_keypair_fn<F>(mut self, f: F) -> Self
    where
        F: Fn() -> Result<PrivateKey, WalletError> + Send + Sync + 'static,
    {
        self.keypair_fn = Box::new(f);
        self
    }

    pub fn buoy_url(&self) -> String {
        self.buoy_url.clone()
    }

    pub fn generate_channel_keypair(&self) -> Result<PrivateKey, WalletError> {
        (self.keypair_fn)()
    }

    pub fn data_snapshot(&self) -> AnchorWalletData {
        self.data.lock().unwrap().clone()
    }

    pub fn set_data(&self, data: AnchorWalletData) {
        *self.data.lock().unwrap() = data;
    }
}

impl Default for AnchorWalletPlugin {
    fn default() -> Self {
        Self::new()
    }
}

// Workaround for upstream PrivateKey::random() returning pubkey bytes;
// see Rust/Docs/antelope-rs-deferred-cleanup.md.
pub fn generate_k1_keypair() -> Result<PrivateKey, WalletError> {
    use rand::RngCore;
    let mut rng = rand::thread_rng();
    for _ in 0..16 {
        let mut bytes = [0u8; 32];
        rng.fill_bytes(&mut bytes);
        if let Ok(sk) = k256::SecretKey::from_slice(&bytes) {
            return Ok(PrivateKey::from_bytes(sk.to_bytes().to_vec(), KeyType::K1));
        }
    }
    Err(WalletError::Internal(
        "failed to sample a valid secp256k1 secret scalar after 16 attempts".into(),
    ))
}

#[async_trait]
impl WalletPlugin for AnchorWalletPlugin {
    fn id(&self) -> String {
        "anchor".to_string()
    }

    fn config(&self) -> WalletPluginConfig {
        WalletPluginConfig {
            requires_chain_select: false,
            requires_permission_select: false,
            requires_permission_entry: false,
            supported_chains: None,
        }
    }

    fn metadata(&self) -> WalletPluginMetadata {
        WalletPluginMetadata {
            name: "Anchor".to_string(),
            description: Some("Anchor wallet via ESR + Buoy".to_string()),
            homepage: Some("https://greymass.com/anchor".to_string()),
            download: Some("https://greymass.com/anchor/download".to_string()),
        }
    }

    async fn login(&self, ctx: &LoginContext) -> Result<WalletPluginLoginResponse, WalletError> {
        crate::login_flow::run_login(self, ctx).await
    }

    async fn sign(
        &self,
        request: &ResolvedSigningRequest,
        ctx: &TransactContext,
    ) -> Result<WalletPluginSignResponse, WalletError> {
        crate::sign_flow::run_sign(self, request, ctx).await
    }

    async fn logout(&self, _ctx: &LogoutContext) -> Result<(), WalletError> {
        *self.data.lock().unwrap() = AnchorWalletData::default();
        Ok(())
    }

    fn serialize(&self) -> SerializedWalletPlugin {
        let data = self.data.lock().unwrap();
        SerializedWalletPlugin {
            id: self.id(),
            data: WalletPluginData(serde_json::to_value(&*data).unwrap_or(serde_json::Value::Null)),
        }
    }

    fn restore(&self, data: WalletPluginData) -> Result<(), WalletError> {
        let parsed: AnchorWalletData = serde_json::from_value(data.0)
            .map_err(|e| WalletError::Internal(format!("restore: {e}")))?;
        *self.data.lock().unwrap() = parsed;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_k1_keypair_round_trips() {
        let pk = generate_k1_keypair().expect("generation succeeds");
        let pub_k = pk.to_public();
        let secret = pk.shared_secret(&pub_k);
        assert_eq!(
            secret.data.len(),
            64,
            "shared_secret returns 64 bytes (sha512)"
        );
    }

    #[test]
    fn default_buoy_url_is_anchor() {
        let plugin = AnchorWalletPlugin::new();
        assert_eq!(plugin.buoy_url(), DEFAULT_BUOY_URL);
    }

    #[test]
    fn with_buoy_relay_overrides_default() {
        let plugin = AnchorWalletPlugin::with_buoy_relay("http://localhost:8080");
        assert_eq!(plugin.buoy_url(), "http://localhost:8080");
    }

    #[test]
    fn id_is_anchor() {
        assert_eq!(AnchorWalletPlugin::new().id(), "anchor");
    }

    #[test]
    fn config_does_not_require_chain_select() {
        let cfg = AnchorWalletPlugin::new().config();
        assert!(!cfg.requires_chain_select);
        assert!(!cfg.requires_permission_select);
    }

    #[test]
    fn metadata_includes_anchor_homepage() {
        let m = AnchorWalletPlugin::new().metadata();
        assert_eq!(m.name, "Anchor");
        assert!(m.homepage.as_deref().unwrap_or("").contains("greymass.com"));
    }
}
