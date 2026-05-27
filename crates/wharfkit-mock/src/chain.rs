use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct MockChain {
    responses: Mutex<HashMap<String, String>>,
    calls: Mutex<Vec<String>>,
}

impl MockChain {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_response(&self, method: &str, path: &str, body: &str) {
        self.responses
            .lock()
            .unwrap()
            .insert(format!("{method} {path}"), body.to_string());
    }

    pub fn calls(&self) -> Vec<String> {
        self.calls.lock().unwrap().clone()
    }
}

#[async_trait]
impl antelope::api::client::Provider for MockChain {
    async fn get(&self, path: String) -> Result<String, String> {
        let key = format!("GET {path}");
        self.calls.lock().unwrap().push(key.clone());
        self.responses
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("MockChain: no response for {key}"))
    }
    async fn post(&self, path: String, _body: Option<String>) -> Result<String, String> {
        let key = format!("POST {path}");
        self.calls.lock().unwrap().push(key.clone());
        self.responses
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .ok_or_else(|| format!("MockChain: no response for {key}"))
    }
}
