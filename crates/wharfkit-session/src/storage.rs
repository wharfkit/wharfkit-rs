use antelope::chain::action::PermissionLevel;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Mutex;
use thiserror::Error;

pub const DEFAULT_SESSION_KEY: &str = "wharfkit:session:default";

pub fn permission_string(p: &PermissionLevel) -> String {
    format!("{}@{}", p.actor, p.permission)
}

pub fn session_key(chain_id: &str, permission_level: &str) -> String {
    format!("wharfkit:session:{chain_id}:{permission_level}")
}

#[derive(Debug, Error)]
pub enum StorageError {
    #[error("IO error: {0}")]
    Io(String),
    #[error("serialization: {0}")]
    Serialization(String),
}

#[async_trait]
pub trait SessionStorage: Send + Sync {
    async fn read(&self, key: &str) -> Option<Vec<u8>>;
    async fn write(&self, key: &str, value: &[u8]) -> Result<(), StorageError>;
    async fn remove(&self, key: &str) -> Result<(), StorageError>;
}

#[derive(Default)]
pub struct InMemorySessionStorage {
    map: Mutex<HashMap<String, Vec<u8>>>,
}

#[async_trait]
impl SessionStorage for InMemorySessionStorage {
    async fn read(&self, key: &str) -> Option<Vec<u8>> {
        self.map.lock().ok()?.get(key).cloned()
    }
    async fn write(&self, key: &str, value: &[u8]) -> Result<(), StorageError> {
        self.map
            .lock()
            .map_err(|e| StorageError::Io(e.to_string()))?
            .insert(key.to_string(), value.to_vec());
        Ok(())
    }
    async fn remove(&self, key: &str) -> Result<(), StorageError> {
        self.map
            .lock()
            .map_err(|e| StorageError::Io(e.to_string()))?
            .remove(key);
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SerializedSession {
    pub chain_id: String,
    pub permission_level: String,
    pub wallet_plugin_id: String,
    pub wallet_plugin_data: serde_json::Value,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn roundtrip() {
        let storage = InMemorySessionStorage::default();
        storage.write("k", b"v").await.unwrap();
        assert_eq!(storage.read("k").await, Some(b"v".to_vec()));
        storage.remove("k").await.unwrap();
        assert_eq!(storage.read("k").await, None);
    }

    #[tokio::test]
    async fn unknown_key_returns_none() {
        let storage = InMemorySessionStorage::default();
        assert_eq!(storage.read("missing").await, None);
    }
}
