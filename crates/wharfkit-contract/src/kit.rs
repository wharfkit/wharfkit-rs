use antelope::chain::name::Name;
use std::sync::Arc;
use wharfkit_abicache::{ABICache, ABICacheError};

#[derive(thiserror::Error, Debug)]
pub enum ContractKitError {
    #[error("ABI fetch failed: {0}")]
    AbiFetch(#[from] ABICacheError),
}

pub struct ContractKit {
    abi_cache: Arc<ABICache>,
}

impl ContractKit {
    pub fn new(abi_cache: Arc<ABICache>) -> Self {
        Self { abi_cache }
    }

    pub async fn load(&self, account: Name) -> Result<crate::Contract, ContractKitError> {
        let abi = self.abi_cache.get_abi(&account).await?;
        Ok(crate::Contract::new(account, abi))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use antelope::api::client::{APIClient, DefaultProvider};

    #[tokio::test]
    #[ignore = "network; run manually"]
    async fn kit_load_eosio_token_from_jungle4() {
        let client = Arc::new(
            APIClient::<DefaultProvider>::default_provider(
                "https://jungle4.greymass.com".to_string(),
                None,
            )
            .unwrap(),
        );
        let cache = Arc::new(ABICache::new(client));
        let kit = ContractKit::new(cache);

        let contract = kit
            .load(Name::new_from_str("eosio.token"))
            .await
            .expect("eosio.token contract loads");

        assert_eq!(contract.account().to_string(), "eosio.token");
    }
}
