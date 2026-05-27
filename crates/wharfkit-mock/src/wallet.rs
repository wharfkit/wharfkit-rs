use antelope::chain::action::PermissionLevel;
use antelope::chain::name::Name;
use antelope::chain::private_key::PrivateKey;
use async_trait::async_trait;
use wharfkit_session::{
    LoginContext, SerializedWalletPlugin, TransactContext, WalletError, WalletPlugin,
    WalletPluginConfig, WalletPluginData, WalletPluginLoginResponse, WalletPluginMetadata,
    WalletPluginSignResponse,
};
use wharfkit_signing_request::ResolvedSigningRequest;

pub struct MockWalletPlugin {
    pub test_key: PrivateKey,
    pub permission: PermissionLevel,
}

impl MockWalletPlugin {
    pub fn new(test_key_wif: &str, actor: &str, permission: &str) -> Self {
        Self {
            test_key: PrivateKey::from_str(test_key_wif, false).expect("valid WIF"),
            permission: PermissionLevel::new(
                Name::new_from_str(actor),
                Name::new_from_str(permission),
            ),
        }
    }
}

#[async_trait]
impl WalletPlugin for MockWalletPlugin {
    fn id(&self) -> String {
        "mock".to_string()
    }
    fn config(&self) -> WalletPluginConfig {
        WalletPluginConfig::default()
    }
    fn metadata(&self) -> WalletPluginMetadata {
        WalletPluginMetadata {
            name: "Mock Wallet".into(),
            ..Default::default()
        }
    }

    async fn login(&self, ctx: &LoginContext) -> Result<WalletPluginLoginResponse, WalletError> {
        let chain = ctx
            .chain
            .as_ref()
            .ok_or_else(|| WalletError::Internal("no chain".into()))?;
        Ok(WalletPluginLoginResponse {
            chain: *chain.id(),
            permission_level: self.permission,
            identity_proof: None,
        })
    }

    async fn sign(
        &self,
        request: &ResolvedSigningRequest,
        _ctx: &TransactContext,
    ) -> Result<WalletPluginSignResponse, WalletError> {
        let data = request.signing_data();
        let signature = self.test_key.sign_message(&data);
        Ok(WalletPluginSignResponse {
            signatures: vec![signature],
            resolved: None,
        })
    }

    fn serialize(&self) -> SerializedWalletPlugin {
        SerializedWalletPlugin {
            id: self.id(),
            data: WalletPluginData::default(),
        }
    }

    fn restore(&self, _data: WalletPluginData) -> Result<(), WalletError> {
        Ok(())
    }
}
