use crate::error::BuoyError;
use async_trait::async_trait;
use std::collections::HashMap;

pub struct BuoyHttpResponse {
    pub status: u16,
    pub body: Vec<u8>,
}

#[async_trait]
pub trait BuoyTransport: Send + Sync {
    async fn post(
        &self,
        url: &str,
        body: &[u8],
        headers: HashMap<&'static str, String>,
    ) -> Result<BuoyHttpResponse, BuoyError>;
}

pub struct ReqwestBuoyTransport {
    client: reqwest::Client,
}

impl ReqwestBuoyTransport {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for ReqwestBuoyTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl BuoyTransport for ReqwestBuoyTransport {
    async fn post(
        &self,
        url: &str,
        body: &[u8],
        headers: HashMap<&'static str, String>,
    ) -> Result<BuoyHttpResponse, BuoyError> {
        let mut req = self.client.post(url).body(body.to_vec());
        for (k, v) in headers {
            req = req.header(k, v);
        }
        let resp = req.send().await?;
        let status = resp.status().as_u16();
        let body = resp.bytes().await?.to_vec();
        Ok(BuoyHttpResponse { status, body })
    }
}
