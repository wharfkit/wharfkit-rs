use crate::channel::BuoyChannel;
use crate::transport::{BuoyTransport, ReqwestBuoyTransport};
use std::sync::Arc;
use uuid::Uuid;

pub struct BuoyClient {
    relay_url: String,
    transport: Arc<dyn BuoyTransport>,
}

impl BuoyClient {
    pub fn new(relay_url: impl Into<String>) -> Self {
        Self {
            relay_url: relay_url.into(),
            transport: Arc::new(ReqwestBuoyTransport::new()),
        }
    }

    pub fn with_transport(relay_url: impl Into<String>, transport: Arc<dyn BuoyTransport>) -> Self {
        Self {
            relay_url: relay_url.into(),
            transport,
        }
    }

    pub fn channel(&self, uuid: Uuid) -> BuoyChannel {
        BuoyChannel {
            relay_url: self.relay_url.clone(),
            uuid,
            transport: self.transport.clone(),
        }
    }
}
