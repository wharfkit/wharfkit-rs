use antelope::chain::action::PermissionLevel;
use antelope::chain::checksum::Checksum256;
use async_trait::async_trait;
use std::collections::HashMap;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum UiError {
    #[error("cancelled")]
    Cancelled,
    #[error("internal: {0}")]
    Internal(String),
}

#[derive(Debug, Clone)]
pub struct PromptArgs {
    pub title: String,
    pub body: Option<String>,
    pub optional: bool,
    pub elements: Vec<PromptElement>,
}

#[derive(Debug, Clone)]
pub enum LinkVariant {
    Primary,
    Secondary,
}

#[derive(Debug, Clone)]
pub enum PromptElement {
    Qr {
        data: String,
    },
    Link {
        id: String,
        href: String,
        label: String,
        variant: LinkVariant,
    },
    Button {
        id: String,
        label: String,
    },
    Countdown {
        id: String,
        label: String,
        end_unix_ms: i64,
    },
    Accept {
        label: String,
    },
    Close,
}

#[derive(Debug, Clone)]
pub enum PromptResponse {
    ButtonPressed { id: String },
    LinkOpened { id: String },
    Accepted,
    Closed,
    Expired,
}

#[derive(Debug, Clone, Default)]
pub struct LocaleDefinitions {
    pub locales: HashMap<String, HashMap<String, String>>,
}

#[derive(Debug, Clone, Default)]
pub struct TranslateOptions {
    pub default: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UserInterfaceLoginResponse {
    pub chain_id: Option<Checksum256>,
    pub permission_level: Option<PermissionLevel>,
    pub wallet_plugin_index: usize,
}

#[async_trait]
pub trait UserInterface: Send + Sync {
    async fn login(
        &self,
        ctx: &crate::login::LoginContext,
    ) -> Result<UserInterfaceLoginResponse, UiError>;

    async fn on_error(&self, err: &crate::error::SessionError) -> Result<(), UiError>;

    async fn on_login(&self) -> Result<(), UiError> {
        Ok(())
    }
    async fn on_login_complete(&self) -> Result<(), UiError> {
        Ok(())
    }
    async fn on_transact(&self) -> Result<(), UiError> {
        Ok(())
    }
    async fn on_transact_complete(&self) -> Result<(), UiError> {
        Ok(())
    }
    async fn on_sign(&self) -> Result<(), UiError> {
        Ok(())
    }
    async fn on_sign_complete(&self) -> Result<(), UiError> {
        Ok(())
    }
    async fn on_broadcast(&self) -> Result<(), UiError> {
        Ok(())
    }
    async fn on_broadcast_complete(&self) -> Result<(), UiError> {
        Ok(())
    }

    fn status(&self, _message: &str) {}

    async fn prompt(&self, args: PromptArgs) -> Result<PromptResponse, UiError>;

    fn translate(&self, key: &str, _opts: &TranslateOptions, _ns: Option<&str>) -> String {
        key.to_string()
    }

    fn add_translations(&self, _t: LocaleDefinitions) {}
}
