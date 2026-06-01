use std::sync::Arc;
use wharfkit_abicache::ABICache;

#[derive(Clone)]
pub struct EsrOptions {
    pub abi_cache: Arc<ABICache>,
}

impl EsrOptions {
    pub fn new(abi_cache: Arc<ABICache>) -> Self {
        Self { abi_cache }
    }

    /// Constructs options backed by a fresh offline ABI cache.
    pub fn offline() -> Self {
        Self::new(Arc::new(ABICache::new_offline()))
    }
}

#[cfg(test)]
mod tests {
    use super::EsrOptions;
    use std::sync::Arc;

    #[test]
    fn offline_constructor_returns_options_with_offline_cache() {
        let opts = EsrOptions::offline();
        let opts2 = EsrOptions::offline();
        assert_ne!(
            Arc::as_ptr(&opts.abi_cache),
            Arc::as_ptr(&opts2.abi_cache),
            "EsrOptions::offline must return a fresh ABICache each call"
        );
    }
}
