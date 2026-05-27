use antelope::api::client::{APIClient, DefaultProvider};
use antelope::api::v1::structs::{ClientError, ErrorResponse};
use antelope::chain::abi::ABI;
use antelope::chain::name::Name;
use async_trait::async_trait;
use dashmap::DashMap;
use std::sync::Arc;
use tokio::sync::OnceCell;

#[derive(thiserror::Error, Debug, Clone)]
pub enum ABICacheError {
    #[error("no APIClient configured (offline cache)")]
    NoClient,
    #[error("upstream get_abi failed: {0}")]
    Upstream(String),
}

impl From<ClientError<ErrorResponse>> for ABICacheError {
    fn from(e: ClientError<ErrorResponse>) -> Self {
        ABICacheError::Upstream(format!("{e:?}"))
    }
}

#[async_trait]
pub trait AbiFetcher: Send + Sync {
    async fn fetch(&self, account: &Name) -> Result<Arc<ABI>, ABICacheError>;
}

pub struct ApiClientFetcher {
    client: Arc<APIClient<DefaultProvider>>,
}

impl ApiClientFetcher {
    pub fn new(client: Arc<APIClient<DefaultProvider>>) -> Self {
        Self { client }
    }
}

#[async_trait]
impl AbiFetcher for ApiClientFetcher {
    async fn fetch(&self, account: &Name) -> Result<Arc<ABI>, ABICacheError> {
        let resp = self
            .client
            .v1_chain
            .get_abi(account.to_string())
            .await
            .map_err(ABICacheError::from)?;
        Ok(Arc::new(resp.abi))
    }
}

type PendingCell = Arc<OnceCell<Result<Arc<ABI>, ABICacheError>>>;

pub struct ABICache {
    cache: DashMap<u64, Arc<ABI>>,
    pending: DashMap<u64, PendingCell>,
    fetcher: Option<Arc<dyn AbiFetcher>>,
}

impl ABICache {
    pub fn new(client: Arc<APIClient<DefaultProvider>>) -> Self {
        Self {
            cache: DashMap::new(),
            pending: DashMap::new(),
            fetcher: Some(Arc::new(ApiClientFetcher::new(client))),
        }
    }

    pub fn with_fetcher(fetcher: Arc<dyn AbiFetcher>) -> Self {
        Self {
            cache: DashMap::new(),
            pending: DashMap::new(),
            fetcher: Some(fetcher),
        }
    }

    pub fn new_offline() -> Self {
        Self {
            cache: DashMap::new(),
            pending: DashMap::new(),
            fetcher: None,
        }
    }

    pub fn set_abi(&self, name: Name, abi: ABI) {
        self.cache.insert(name.value(), Arc::new(abi));
    }

    pub fn get_cached(&self, name: &Name) -> Option<Arc<ABI>> {
        self.cache.get(&name.value()).map(|v| v.value().clone())
    }

    pub async fn get_abi(&self, name: &Name) -> Result<Arc<ABI>, ABICacheError> {
        if let Some(abi) = self.get_cached(name) {
            return Ok(abi);
        }
        let fetcher = self.fetcher.clone().ok_or(ABICacheError::NoClient)?;

        let cell = self
            .pending
            .entry(name.value())
            .or_insert_with(|| Arc::new(OnceCell::new()))
            .clone();

        if let Some(abi) = self.get_cached(name) {
            return Ok(abi);
        }

        let name_for_fetch = *name;
        let result = cell
            .get_or_init(|| async move { fetcher.fetch(&name_for_fetch).await })
            .await;

        self.pending.remove(&name.value());

        match result {
            Ok(abi) => {
                self.cache.insert(name.value(), abi.clone());
                Ok(abi.clone())
            }
            Err(e) => Err(e.clone()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[test]
    fn cache_set_then_get_returns_stored_abi() {
        let cache = ABICache::new_offline();
        cache.set_abi(Name::new_from_str("eosio.token"), ABI::default());
        assert!(cache
            .get_cached(&Name::new_from_str("eosio.token"))
            .is_some());
    }

    #[test]
    fn cache_get_uncached_returns_none() {
        let cache = ABICache::new_offline();
        assert!(cache
            .get_cached(&Name::new_from_str("nonexistent"))
            .is_none());
    }

    struct CountingFetcher {
        calls: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl AbiFetcher for CountingFetcher {
        async fn fetch(&self, _account: &Name) -> Result<Arc<ABI>, ABICacheError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            Ok(Arc::new(ABI::default()))
        }
    }

    #[tokio::test]
    async fn coalesces_concurrent_requests() {
        let calls = Arc::new(AtomicUsize::new(0));
        let fetcher = Arc::new(CountingFetcher {
            calls: calls.clone(),
        });
        let cache = Arc::new(ABICache::with_fetcher(fetcher));
        let name = Name::new_from_str("eosio.token");

        let handles: Vec<_> = (0..10)
            .map(|_| {
                let cache = cache.clone();
                tokio::spawn(async move { cache.get_abi(&name).await })
            })
            .collect();

        for h in handles {
            h.await.unwrap().unwrap();
        }

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "should have made exactly one underlying fetch"
        );
    }

    struct FlakyFetcher {
        calls: Arc<AtomicUsize>,
        fail_first: Arc<AtomicUsize>,
    }

    #[async_trait]
    impl AbiFetcher for FlakyFetcher {
        async fn fetch(&self, _account: &Name) -> Result<Arc<ABI>, ABICacheError> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst);
            if n < self.fail_first.load(Ordering::SeqCst) {
                Err(ABICacheError::Upstream("transient".into()))
            } else {
                Ok(Arc::new(ABI::default()))
            }
        }
    }

    #[tokio::test]
    async fn failed_lookup_is_retryable() {
        let calls = Arc::new(AtomicUsize::new(0));
        let fetcher = Arc::new(FlakyFetcher {
            calls: calls.clone(),
            fail_first: Arc::new(AtomicUsize::new(1)),
        });
        let cache = ABICache::with_fetcher(fetcher);
        let name = Name::new_from_str("eosio.token");

        assert!(cache.get_abi(&name).await.is_err());
        assert!(cache.get_abi(&name).await.is_ok());
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn caches_completed_results() {
        let calls = Arc::new(AtomicUsize::new(0));
        let fetcher = Arc::new(CountingFetcher {
            calls: calls.clone(),
        });
        let cache = ABICache::with_fetcher(fetcher);
        let name = Name::new_from_str("eosio.token");

        cache.get_abi(&name).await.unwrap();
        cache.get_abi(&name).await.unwrap();
        cache.get_abi(&name).await.unwrap();

        assert_eq!(
            calls.load(Ordering::SeqCst),
            1,
            "second/third call should hit cache"
        );
    }
}
