use antelope::chain::action::PermissionLevel;
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio_util::sync::CancellationToken;
use wharfkit_common::ChainDefinition;
use wharfkit_signing_request::EsrOptions;

use crate::error::SessionError;
use crate::platform::Platform;
use crate::ui::UserInterface;
use crate::wallet::{WalletPluginConfig, WalletPluginMetadata};

#[derive(Debug, Clone)]
pub struct UserInterfaceWalletPlugin {
    pub id: String,
    pub config: WalletPluginConfig,
    pub metadata: WalletPluginMetadata,
}

#[derive(Default, Clone)]
pub struct LoginHooks {
    pub before_login: Vec<Arc<dyn LoginHookFn>>,
    pub after_login: Vec<Arc<dyn LoginHookFn>>,
}

#[async_trait]
pub trait LoginHookFn: Send + Sync {
    async fn call(&self, ctx: &LoginContext) -> Result<(), SessionError>;
}

#[derive(Default, Debug, Clone)]
pub struct UiRequirements {
    pub requires_chain_select: bool,
    pub requires_permission_select: bool,
    pub requires_permission_entry: bool,
    pub requires_wallet_select: bool,
}

pub struct LoginContext {
    pub chain: Option<ChainDefinition>,
    pub chains: Vec<ChainDefinition>,
    pub ui: Arc<dyn UserInterface>,
    pub platform: Arc<dyn Platform>,
    pub wallet_plugins: Vec<UserInterfaceWalletPlugin>,
    pub permission_level: Option<PermissionLevel>,
    pub wallet_plugin_index: Option<usize>,
    pub esr_options: EsrOptions,
    pub cancel: CancellationToken,
    pub hooks: LoginHooks,
    pub arbitrary: HashMap<String, serde_json::Value>,
    pub ui_requirements: UiRequirements,
}
