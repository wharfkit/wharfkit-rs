pub mod codegen;
pub mod ident;

use thiserror::Error;

#[derive(Debug, Error)]
pub enum CodegenError {
    #[error("network fetch failed: {0}")]
    NetworkFetch(String),

    #[error("ABI hash error: {0}")]
    AbiHash(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("rustfmt failed: {0}")]
    Rustfmt(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("ident collision: {0}")]
    IdentCollision(String),

    #[error("missing builtin type mapping for ABI type '{0}' — wharfkit-cli is out of sync with antelope-rs is_builtin_type")]
    MissingBuiltin(String),
}

pub async fn codegen(
    chain: &str,
    account: &str,
    out: &std::path::Path,
) -> Result<(), CodegenError> {
    codegen::run(chain, account, out).await
}

#[cfg(test)]
mod codegen_error_tests {
    use super::*;

    #[test]
    fn codegen_error_implements_display() {
        let e = CodegenError::MissingBuiltin("int128".to_string());
        let msg = format!("{e}");
        assert!(msg.contains("int128"));
        assert!(msg.contains("missing builtin"));
    }

    #[test]
    fn codegen_error_from_serde_json() {
        let bad: Result<serde_json::Value, _> = serde_json::from_str("not json");
        let err: Result<serde_json::Value, CodegenError> = bad.map_err(CodegenError::from);
        assert!(err.is_err());
    }

    #[test]
    fn codegen_error_from_io() {
        let bad: Result<String, std::io::Error> =
            Err(std::io::Error::new(std::io::ErrorKind::NotFound, "x"));
        let err: Result<String, CodegenError> = bad.map_err(CodegenError::from);
        assert!(err.is_err());
    }

    #[test]
    fn codegen_error_ident_collision_variant() {
        let e = CodegenError::IdentCollision("foo/bar".to_string());
        let msg = format!("{e}");
        assert!(msg.contains("ident collision"));
        assert!(msg.contains("foo/bar"));
    }
}
