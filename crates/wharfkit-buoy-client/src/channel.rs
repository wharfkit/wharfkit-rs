use crate::error::BuoyError;
use crate::transport::BuoyTransport;
use futures_util::stream::Stream;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::future::ready;
use std::sync::Arc;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

#[derive(Default, Debug, Clone, Copy)]
pub struct PostOptions {
    pub wait_seconds: Option<u32>,
    pub soft_wait_seconds: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeliveryStatus {
    Delivered,
    Buffered,
    Timeout,
}

pub struct BuoyChannel {
    pub(crate) relay_url: String,
    pub(crate) uuid: Uuid,
    pub(crate) transport: Arc<dyn BuoyTransport>,
}

impl BuoyChannel {
    fn base(&self) -> &str {
        self.relay_url.trim_end_matches('/')
    }

    pub fn url(&self) -> String {
        format!("{}/{}", self.base(), self.uuid)
    }

    pub fn ws_url(&self) -> String {
        let base = self.base();
        let (scheme, body) = if let Some(rest) = base.strip_prefix("https://") {
            ("wss", rest)
        } else if let Some(rest) = base.strip_prefix("http://") {
            ("ws", rest)
        } else {
            return format!("{base}/{}", self.uuid);
        };
        format!("{scheme}://{body}/{}", self.uuid)
    }

    pub async fn listen(
        &self,
    ) -> Result<impl Stream<Item = Result<Vec<u8>, BuoyError>>, BuoyError> {
        let (ws_stream, _) = tokio_tungstenite::connect_async(self.ws_url()).await?;
        let (_write, read) = ws_stream.split();
        Ok(read.filter_map(|msg| {
            ready(match msg {
                Ok(Message::Binary(b)) => Some(Ok(b)),
                Ok(Message::Text(s)) => Some(Ok(s.into_bytes())),
                Ok(Message::Close(_)) => Some(Err(BuoyError::Closed)),
                Err(e) => Some(Err(BuoyError::WebSocket(e))),
                _ => None,
            })
        }))
    }

    pub async fn post(&self, data: &[u8], opts: PostOptions) -> Result<DeliveryStatus, BuoyError> {
        let mut headers: HashMap<&'static str, String> = HashMap::new();
        if let Some(secs) = opts.wait_seconds {
            headers.insert("X-Buoy-Wait", secs.to_string());
        }
        if let Some(secs) = opts.soft_wait_seconds {
            headers.insert("X-Buoy-Soft-Wait", secs.to_string());
        }
        let resp = self.transport.post(&self.url(), data, headers).await?;
        match resp.status {
            200 | 202 => Ok(DeliveryStatus::Delivered),
            204 => Ok(DeliveryStatus::Buffered),
            408 => Ok(DeliveryStatus::Timeout),
            other => Err(BuoyError::InvalidResponse(format!(
                "unexpected relay status {other}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn channel(relay: &str) -> BuoyChannel {
        BuoyChannel {
            relay_url: relay.into(),
            uuid: Uuid::nil(),
            transport: Arc::new(crate::transport::ReqwestBuoyTransport::new()),
        }
    }

    #[test]
    fn ws_url_from_https() {
        let ch = channel("https://cb.anchor.link");
        assert_eq!(ch.ws_url(), format!("wss://cb.anchor.link/{}", Uuid::nil()));
        assert_eq!(ch.url(), format!("https://cb.anchor.link/{}", Uuid::nil()));
    }

    #[test]
    fn ws_url_from_http() {
        let ch = channel("http://127.0.0.1:8080");
        assert_eq!(ch.ws_url(), format!("ws://127.0.0.1:8080/{}", Uuid::nil()));
    }

    #[test]
    fn ws_url_strips_trailing_slash() {
        let ch = channel("https://cb.anchor.link/");
        assert_eq!(ch.url(), format!("https://cb.anchor.link/{}", Uuid::nil()));
    }

    #[test]
    fn ws_url_passes_through_unknown_scheme() {
        let ch = channel("file://x");
        assert_eq!(ch.ws_url(), format!("file://x/{}", Uuid::nil()));
    }
}
