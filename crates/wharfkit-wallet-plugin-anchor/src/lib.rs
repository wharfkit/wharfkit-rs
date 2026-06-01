// `WalletError::Buoy(BuoyError)` chains reqwest/tungstenite source errors; boxing
// them just to satisfy the lint costs more than the wider Result on cold paths.
#![allow(clippy::result_large_err)]

pub mod data;
pub mod login_flow;
pub mod plugin;
pub mod sealed;
pub mod sign_flow;

pub use data::{AnchorChannelState, AnchorWalletData};
pub use plugin::AnchorWalletPlugin;
pub use sealed::{SealError, SealedMessage};
