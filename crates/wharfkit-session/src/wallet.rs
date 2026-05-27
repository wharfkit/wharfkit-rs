use antelope::chain::action::{Action, PermissionLevel};
use antelope::chain::checksum::Checksum256;
use antelope::chain::public_key::PublicKey;
use antelope::chain::signature::Signature;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;
use wharfkit_buoy_client::BuoyError;
use wharfkit_signing_request::{EsrError, IdentityProof, ResolvedSigningRequest};

use crate::login::LoginContext;
use crate::transact::TransactContext;

#[derive(Debug, Error)]
pub enum WalletError {
    #[error("user closed")]
    UserClosed,
    #[error("user rejected: {0}")]
    UserRejected(String),
    #[error("cancelled")]
    Cancelled,
    #[error("expired")]
    Expired,
    #[error("buoy: {0}")]
    Buoy(#[from] BuoyError),
    #[error("ESR: {0}")]
    Esr(#[from] EsrError),
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WalletPluginConfig {
    pub requires_chain_select: bool,
    pub requires_permission_select: bool,
    pub requires_permission_entry: bool,
    pub supported_chains: Option<Vec<String>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WalletPluginMetadata {
    pub name: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub download: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WalletPluginData(pub serde_json::Value);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedWalletPlugin {
    pub id: String,
    pub data: WalletPluginData,
}

#[derive(Debug, Clone)]
pub struct WalletPluginLoginResponse {
    pub chain: Checksum256,
    pub permission_level: PermissionLevel,
    /// Set by ESR-driven wallets that complete an identity request during login.
    /// Plugins authenticating out-of-band leave this `None`.
    pub identity_proof: Option<IdentityProof>,
}

#[derive(Clone)]
pub struct WalletPluginSignResponse {
    pub signatures: Vec<Signature>,
    /// Required for ESR-callback wallets: the broadcaster signs the exact bytes
    /// the wallet signed; rebuilding TAPOS/expiration locally recovers a wrong key.
    pub resolved: Option<ResolvedSigningRequest>,
}

pub struct LogoutContext {
    pub chain: Checksum256,
    pub permission_level: PermissionLevel,
}

#[async_trait]
pub trait WalletPlugin: Send + Sync {
    fn id(&self) -> String;
    fn config(&self) -> WalletPluginConfig;
    fn metadata(&self) -> WalletPluginMetadata;

    async fn login(&self, ctx: &LoginContext) -> Result<WalletPluginLoginResponse, WalletError>;

    async fn sign(
        &self,
        request: &ResolvedSigningRequest,
        ctx: &TransactContext,
    ) -> Result<WalletPluginSignResponse, WalletError>;

    async fn logout(&self, _ctx: &LogoutContext) -> Result<(), WalletError> {
        Ok(())
    }

    async fn retrieve_public_key(
        &self,
        _chain_id: &Checksum256,
    ) -> Result<Option<PublicKey>, WalletError> {
        Ok(None)
    }

    async fn resolve_permission(
        &self,
        _actions: &[Action],
        _ctx: &TransactContext,
    ) -> Result<Option<PermissionLevel>, WalletError> {
        Ok(None)
    }

    fn serialize(&self) -> SerializedWalletPlugin;
    fn restore(&self, data: WalletPluginData) -> Result<(), WalletError>;
}

pub struct AbstractWalletPlugin {
    pub config: WalletPluginConfig,
    pub metadata: WalletPluginMetadata,
    pub data: std::sync::Mutex<WalletPluginData>,
}

impl AbstractWalletPlugin {
    pub fn new(metadata: WalletPluginMetadata) -> Self {
        Self {
            config: WalletPluginConfig::default(),
            metadata,
            data: std::sync::Mutex::new(WalletPluginData::default()),
        }
    }
}
