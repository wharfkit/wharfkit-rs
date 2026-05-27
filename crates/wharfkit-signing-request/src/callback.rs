use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CallbackPayload {
    #[serde(default)]
    pub sa: Option<String>,
    #[serde(default)]
    pub sp: Option<String>,
    #[serde(default)]
    pub cid: Option<String>,
    #[serde(default)]
    pub sig: Option<String>,
    #[serde(default)]
    pub link_ch: Option<String>,
    #[serde(default)]
    pub link_key: Option<String>,
    #[serde(default)]
    pub link_name: Option<String>,
    #[serde(default)]
    pub rejected: Option<String>,
    #[serde(flatten)]
    pub extra: std::collections::HashMap<String, serde_json::Value>,
}

impl CallbackPayload {
    pub fn from_json(bytes: &[u8]) -> Result<Self, crate::error::EsrError> {
        Ok(serde_json::from_slice(bytes)?)
    }

    pub fn is_rejected(&self) -> bool {
        self.rejected.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_success_payload() {
        let json = br#"{
            "sa": "alice",
            "sp": "active",
            "cid": "73e4385a2708e6d7048834fbc1079f2fabb17b3c125b146af438971e90716c4d",
            "sig": "SIG_K1_KbSF8BCNVA95KzR1qLrFFKxw6oWhTLFqfwurY4UqU1zL7s2hgSAm"
        }"#;
        let payload = CallbackPayload::from_json(json).unwrap();
        assert_eq!(payload.sa.as_deref(), Some("alice"));
        assert_eq!(payload.sp.as_deref(), Some("active"));
        assert!(!payload.is_rejected());
    }

    #[test]
    fn parse_rejection_payload() {
        let json = br#"{"rejected": "User cancelled"}"#;
        let payload = CallbackPayload::from_json(json).unwrap();
        assert!(payload.is_rejected());
        assert_eq!(payload.rejected.as_deref(), Some("User cancelled"));
    }

    #[test]
    fn parse_anchor_link_handshake() {
        let json = br#"{"sa":"alice","sp":"active","cid":"x","sig":"y","link_ch":"https://cb.anchor.link/ch-uuid","link_key":"PUB_K1_...","link_name":"my-anchor"}"#;
        let payload = CallbackPayload::from_json(json).unwrap();
        assert_eq!(
            payload.link_ch.as_deref(),
            Some("https://cb.anchor.link/ch-uuid")
        );
        assert_eq!(payload.link_name.as_deref(), Some("my-anchor"));
    }

    #[test]
    fn extra_fields_preserved() {
        let json = br#"{"sa":"alice","custom":"value","number":42}"#;
        let payload = CallbackPayload::from_json(json).unwrap();
        assert_eq!(payload.sa.as_deref(), Some("alice"));
        assert_eq!(
            payload.extra.get("custom").and_then(|v| v.as_str()),
            Some("value")
        );
        assert_eq!(
            payload.extra.get("number").and_then(|v| v.as_u64()),
            Some(42)
        );
    }
}
