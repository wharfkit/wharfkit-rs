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
}
