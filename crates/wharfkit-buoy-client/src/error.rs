use thiserror::Error;

#[derive(Debug, Error)]
pub enum BuoyError {
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] tokio_tungstenite::tungstenite::Error),

    #[error("Channel closed")]
    Closed,

    #[error("Invalid relay response: {0}")]
    InvalidResponse(String),
}
