use crate::{ChainDefinition, TokenIdentifier};
use antelope::chain::asset::Symbol;
use antelope::chain::checksum::Checksum256;
use antelope::chain::name::Name;

pub struct Chains;

struct ChainEntry {
    chain_id: &'static str,
    url: &'static str,
    display_name: &'static str,
    token_contract: &'static str,
    token_symbol: &'static str,
    token_precision: u8,
}

const JUNGLE4: ChainEntry = ChainEntry {
    chain_id: "73e4385a2708e6d7048834fbc1079f2fabb17b3c125b146af438971e90716c4d",
    url: "https://jungle4.greymass.com",
    display_name: "Jungle 4",
    token_contract: "eosio.token",
    token_symbol: "EOS",
    token_precision: 4,
};
const EOS: ChainEntry = ChainEntry {
    chain_id: "aca376f206b8fc25a6ed44dbdc66547c36c6c33e3a119ffbeaef943642f0e906",
    url: "https://eos.greymass.com",
    display_name: "EOS",
    token_contract: "eosio.token",
    token_symbol: "EOS",
    token_precision: 4,
};
const WAX: ChainEntry = ChainEntry {
    chain_id: "1064487b3cd1a897ce03ae5b6a865651747e2e152090f99c1d19d44e01aea5a4",
    url: "https://wax.greymass.com",
    display_name: "WAX",
    token_contract: "eosio.token",
    token_symbol: "WAX",
    token_precision: 8,
};
const TELOS: ChainEntry = ChainEntry {
    chain_id: "4667b205c6838ef70ff7988f6e8257e8be0e1284a2f59699054a018f743b1d11",
    url: "https://telos.greymass.com",
    display_name: "Telos",
    token_contract: "eosio.token",
    token_symbol: "TLOS",
    token_precision: 4,
};
const VAULTA: ChainEntry = ChainEntry {
    chain_id: "aca376f206b8fc25a6ed44dbdc66547c36c6c33e3a119ffbeaef943642f0e906",
    url: "https://eos.greymass.com",
    display_name: "Vaulta",
    token_contract: "core.vaulta",
    token_symbol: "A",
    token_precision: 4,
};

fn build(entry: &ChainEntry) -> ChainDefinition {
    let id = Checksum256::from_hex(entry.chain_id).expect("chain id hex");
    let system_token = TokenIdentifier::new(
        id,
        Name::new_from_str(entry.token_contract),
        Symbol::new(entry.token_symbol, entry.token_precision),
    );
    ChainDefinition::new(id, entry.url.to_string())
        .with_name(entry.display_name)
        .with_system_token(system_token)
}

impl Chains {
    pub fn jungle4() -> ChainDefinition {
        build(&JUNGLE4)
    }
    pub fn eos() -> ChainDefinition {
        build(&EOS)
    }
    pub fn wax() -> ChainDefinition {
        build(&WAX)
    }
    pub fn telos() -> ChainDefinition {
        build(&TELOS)
    }
    pub fn vaulta() -> ChainDefinition {
        build(&VAULTA)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chains_jungle4_id_matches_canonical() {
        let chain = Chains::jungle4();
        assert_eq!(
            chain.id().as_string(),
            "73e4385a2708e6d7048834fbc1079f2fabb17b3c125b146af438971e90716c4d"
        );
        assert_eq!(chain.url(), "https://jungle4.greymass.com");
        assert_eq!(chain.name(), Some("Jungle 4"));
        assert!(chain.system_token().is_some());
    }

    #[test]
    fn chains_eos_definition() {
        let chain = Chains::eos();
        assert_eq!(
            chain.id().as_string(),
            "aca376f206b8fc25a6ed44dbdc66547c36c6c33e3a119ffbeaef943642f0e906"
        );
    }

    #[test]
    fn chains_wax_definition() {
        let chain = Chains::wax();
        assert_eq!(
            chain.id().as_string(),
            "1064487b3cd1a897ce03ae5b6a865651747e2e152090f99c1d19d44e01aea5a4"
        );
        let token = chain.system_token().unwrap();
        assert_eq!(token.symbol().code().to_string(), "WAX");
        assert_eq!(token.symbol().precision(), 8);
    }

    #[test]
    fn chains_telos_definition() {
        let chain = Chains::telos();
        assert_eq!(
            chain.id().as_string(),
            "4667b205c6838ef70ff7988f6e8257e8be0e1284a2f59699054a018f743b1d11"
        );
    }

    #[test]
    fn chains_vaulta_definition() {
        let chain = Chains::vaulta();
        assert_eq!(
            chain.id().as_string(),
            "aca376f206b8fc25a6ed44dbdc66547c36c6c33e3a119ffbeaef943642f0e906"
        );
        let token = chain.system_token().unwrap();
        assert_eq!(token.symbol().code().to_string(), "A");
        assert_eq!(token.contract().to_string(), "core.vaulta");
    }
}
