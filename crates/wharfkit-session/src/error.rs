use thiserror::Error;
use wharfkit_abicache::ABICacheError;
use wharfkit_signing_request::EsrError;

use crate::storage::StorageError;
use crate::ui::UiError;
use crate::wallet::WalletError;

#[derive(Debug, Error)]
pub enum SessionError {
    #[error("wallet: {0}")]
    Wallet(#[from] WalletError),
    #[error("UI: {0}")]
    Ui(#[from] UiError),
    #[error("storage: {0}")]
    Storage(#[from] StorageError),
    #[error("ABI cache: {0}")]
    AbiCache(#[from] ABICacheError),
    #[error("ESR: {0}")]
    Esr(#[from] EsrError),
    #[error("serialization: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("missing chain")]
    MissingChain,
    #[error("missing UI")]
    MissingUi,
    #[error("missing wallet")]
    MissingWallet,
    #[error("session not found in storage")]
    SessionNotFound,
    #[error("operation cancelled")]
    Cancelled,
    #[error("internal: {0}")]
    Internal(String),
}
