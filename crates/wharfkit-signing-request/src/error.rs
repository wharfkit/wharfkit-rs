use thiserror::Error;
use wharfkit_abicache::ABICacheError;

#[derive(Debug, Error)]
pub enum EsrError {
    #[error("invalid ESR URI: {0}")]
    InvalidUri(String),

    #[error("inflate failed: {0:?}")]
    Inflate(miniz_oxide::inflate::DecompressError),

    #[error("ABI cache error: {0}")]
    AbiCache(#[from] ABICacheError),

    #[error("missing required field: {0}")]
    MissingField(String),

    #[error("unsupported scheme: {0}")]
    UnsupportedScheme(String),

    #[error("serialization error: {0}")]
    Serialization(String),

    #[error("callback payload error: {0}")]
    Callback(#[from] serde_json::Error),

    #[error("identity proof verification failed: {0}")]
    IdentityProofInvalid(String),

    #[error("internal: {0}")]
    Internal(String),
}

// miniz_oxide 0.7's DecompressError does not impl std::error::Error, so a manual
// From is required instead of `#[from]`. Drops in upgrade to miniz_oxide 0.8+.
impl From<miniz_oxide::inflate::DecompressError> for EsrError {
    fn from(e: miniz_oxide::inflate::DecompressError) -> Self {
        EsrError::Inflate(e)
    }
}
