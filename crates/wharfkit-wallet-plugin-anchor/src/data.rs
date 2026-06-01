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

use antelope::chain::private_key::PrivateKey;
use antelope::chain::public_key::PublicKey;
use std::fmt;

/// Typed projection of an Anchor channel established during login.
#[derive(Clone)]
pub struct AnchorChannelState {
    pub channel_url: String,
    pub signer_key: PublicKey,
    pub private_key: PrivateKey,
    pub same_device: bool,
    pub launch_url: Option<String>,
    pub channel_name: Option<String>,
}

impl fmt::Debug for AnchorChannelState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AnchorChannelState")
            .field("channel_url", &self.channel_url)
            .field("signer_key", &self.signer_key)
            .field("private_key", &"<redacted>")
            .field("same_device", &self.same_device)
            .field("launch_url", &self.launch_url)
            .field("channel_name", &self.channel_name)
            .finish()
    }
}
