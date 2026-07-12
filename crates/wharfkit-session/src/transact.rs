use antelope::api::client::{APIClient, Provider};
use antelope::chain::action::{Action, PermissionLevel};
use antelope::chain::signature::Signature;
use antelope::chain::transaction::{SignedTransaction, Transaction};
use async_trait::async_trait;
use std::sync::Arc;
use thiserror::Error;
use tokio_util::sync::CancellationToken;
use wharfkit_abicache::{ABICache, ABICacheError};
use wharfkit_common::ChainDefinition;
use wharfkit_signing_request::{EsrError, EsrOptions, ResolvedSigningRequest, SigningRequest};

use crate::platform::Platform;
use crate::ui::{UiError, UserInterface};
use crate::wallet::WalletError;

pub type ChainClient = APIClient<Arc<dyn Provider>>;

pub struct TransactArgs {
    pub actions: Vec<Action>,
}

#[derive(Default)]
pub struct TransactOptions {
    pub broadcast: bool,
    pub expire_seconds: Option<u32>,
}

#[derive(Clone)]
pub struct TransactResult {
    pub chain: ChainDefinition,
    pub request: SigningRequest,
    pub resolved: Option<ResolvedSigningRequest>,
    pub signer: PermissionLevel,
    pub signatures: Vec<Signature>,
    pub transaction: Option<Transaction>,
    pub response: Option<serde_json::Value>,
}

#[derive(Error)]
pub enum TransactError {
    #[error("wallet: {0}")]
    Wallet(#[from] WalletError),
    #[error("UI: {0}")]
    Ui(#[from] UiError),
    #[error("ABI cache: {0}")]
    AbiCache(#[from] ABICacheError),
    #[error("ESR: {0}")]
    Esr(#[from] EsrError),
    #[error("broadcast failed: {chain_error}")]
    Broadcast {
        signed: Box<SignedTransaction>,
        signatures: Vec<Signature>,
        chain_error: String,
    },
    #[error("cancelled")]
    Cancelled,
    #[error("internal: {0}")]
    Internal(String),
}

// Manual Debug: SignedTransaction lacks Debug upstream.
impl std::fmt::Debug for TransactError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransactError::Wallet(e) => write!(f, "Wallet({e:?})"),
            TransactError::Ui(e) => write!(f, "Ui({e:?})"),
            TransactError::AbiCache(e) => write!(f, "AbiCache({e:?})"),
            TransactError::Esr(e) => write!(f, "Esr({e:?})"),
            TransactError::Broadcast {
                chain_error,
                signatures,
                ..
            } => write!(
                f,
                "Broadcast {{ signatures: {} sigs, chain_error: {chain_error:?} }}",
                signatures.len()
            ),
            TransactError::Cancelled => write!(f, "Cancelled"),
            TransactError::Internal(s) => write!(f, "Internal({s:?})"),
        }
    }
}

#[derive(Default, Clone)]
pub struct TransactHooks {
    pub before_sign: Vec<Arc<dyn TransactHookFn>>,
    pub after_sign: Vec<Arc<dyn TransactHookFn>>,
    pub after_broadcast: Vec<Arc<dyn TransactHookFn>>,
}

#[async_trait]
pub trait TransactHookFn: Send + Sync {
    async fn call(
        &self,
        ctx: &TransactContext,
        result: &mut TransactResult,
    ) -> Result<(), TransactError>;
}

pub struct TransactContext {
    pub chain: ChainDefinition,
    pub ui: Arc<dyn UserInterface>,
    pub platform: Arc<dyn Platform>,
    pub abi_cache: Arc<ABICache>,
    pub esr_options: EsrOptions,
    pub cancel: CancellationToken,
    pub permission_level: PermissionLevel,
    pub client: ChainClient,
    pub hooks: TransactHooks,
    pub return_path: Option<String>,
}
