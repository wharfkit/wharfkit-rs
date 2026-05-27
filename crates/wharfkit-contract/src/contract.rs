use antelope::chain::abi::ABI;
use antelope::chain::name::Name;
use antelope::serializer::Packer;
use std::sync::Arc;

pub struct Contract {
    account: Name,
    abi: Arc<ABI>,
}

impl Contract {
    pub fn new(account: Name, abi: Arc<ABI>) -> Self {
        Self { account, abi }
    }

    pub fn account(&self) -> &Name {
        &self.account
    }

    pub fn abi(&self) -> &ABI {
        &self.abi
    }

    pub fn table<R: Packer + Default>(&self, table_name: Name, scope: Name) -> crate::Table<R> {
        crate::Table::<R>::new(self.account, table_name, scope)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn contract_construct_from_account_and_abi() {
        let contract = Contract::new(Name::new_from_str("eosio.token"), Arc::new(ABI::default()));
        assert_eq!(contract.account().to_string(), "eosio.token");
    }
}
