use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AnchorWalletData {
    pub request_key: Option<String>,
    pub private_key: Option<String>,
    pub signer_key: Option<String>,
    pub channel_url: Option<String>,
    pub channel_name: Option<String>,
    #[serde(default)]
    pub same_device: bool,
    pub launch_url: Option<String>,
    pub trigger_url: Option<String>,
}
