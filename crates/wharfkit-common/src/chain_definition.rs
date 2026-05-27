use antelope::chain::checksum::Checksum256;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChainDefinition {
    id: Checksum256,
    url: String,
    name: Option<String>,
    system_token: Option<crate::TokenIdentifier>,
}

impl ChainDefinition {
    pub fn new(id: Checksum256, url: String) -> Self {
        Self {
            id,
            url,
            name: None,
            system_token: None,
        }
    }

    pub fn with_name(mut self, name: impl Into<String>) -> Self {
        self.name = Some(name.into());
        self
    }

    pub fn with_system_token(mut self, token: crate::TokenIdentifier) -> Self {
        self.system_token = Some(token);
        self
    }

    pub fn id(&self) -> &Checksum256 {
        &self.id
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn system_token(&self) -> Option<&crate::TokenIdentifier> {
        self.system_token.as_ref()
    }
}

impl PartialEq for ChainDefinition {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id && self.url == other.url
    }
}

impl Eq for ChainDefinition {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chain_definition_construct_from_id_and_url() {
        let id = Checksum256::from_hex(
            "73e4385a2708e6d7048834fbc1079f2fabb17b3c125b146af438971e90716c4d",
        )
        .unwrap();
        let def = ChainDefinition::new(id, "https://jungle4.greymass.com".to_string());

        assert_eq!(def.url(), "https://jungle4.greymass.com");
        assert_eq!(
            def.id().as_string(),
            "73e4385a2708e6d7048834fbc1079f2fabb17b3c125b146af438971e90716c4d"
        );
        assert_eq!(def.name(), None);
    }

    #[test]
    fn chain_definition_with_name_sets_name() {
        let id = Checksum256::from_hex(
            "73e4385a2708e6d7048834fbc1079f2fabb17b3c125b146af438971e90716c4d",
        )
        .unwrap();
        let def = ChainDefinition::new(id, "https://jungle4.greymass.com".to_string())
            .with_name("Jungle 4");
        assert_eq!(def.name(), Some("Jungle 4"));
    }

    #[test]
    fn chain_definition_eq_compares_id_and_url() {
        let id = Checksum256::from_hex(
            "73e4385a2708e6d7048834fbc1079f2fabb17b3c125b146af438971e90716c4d",
        )
        .unwrap();
        let a = ChainDefinition::new(id, "https://a.example".to_string());
        let b = ChainDefinition::new(id, "https://b.example".to_string());
        let c = ChainDefinition::new(id, "https://a.example".to_string());
        assert_ne!(
            a, b,
            "same id, different url should NOT be equal (matches TS)"
        );
        assert_eq!(a, c, "same id and url should be equal");
    }
}
