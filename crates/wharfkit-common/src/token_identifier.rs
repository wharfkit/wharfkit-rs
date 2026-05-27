use antelope::chain::asset::Symbol;
use antelope::chain::checksum::Checksum256;
use antelope::chain::name::Name;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TokenIdentifier {
    chain: Checksum256,
    contract: Name,
    symbol: Symbol,
}

impl TokenIdentifier {
    pub fn new(chain: Checksum256, contract: Name, symbol: Symbol) -> Self {
        Self {
            chain,
            contract,
            symbol,
        }
    }

    pub fn chain(&self) -> &Checksum256 {
        &self.chain
    }

    pub fn contract(&self) -> &Name {
        &self.contract
    }

    pub fn symbol(&self) -> &Symbol {
        &self.symbol
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use antelope::chain::asset::Symbol;
    use antelope::chain::checksum::Checksum256;
    use antelope::chain::name::Name;

    #[test]
    fn token_identifier_construct() {
        let chain_id = Checksum256::from_hex(
            "73e4385a2708e6d7048834fbc1079f2fabb17b3c125b146af438971e90716c4d",
        )
        .unwrap();
        let token = TokenIdentifier::new(
            chain_id,
            Name::new_from_str("eosio.token"),
            Symbol::new("EOS", 4),
        );

        assert_eq!(token.contract().to_string(), "eosio.token");
        assert_eq!(token.symbol().code().to_string(), "EOS");
    }
}
