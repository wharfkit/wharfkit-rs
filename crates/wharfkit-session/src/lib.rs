// `#[from]` chains pull large source errors into our variants; boxing them just
// to satisfy the lint costs more than the wider Result on cold paths.
#![allow(clippy::result_large_err)]

pub mod error;
pub mod kit;
pub mod login;
pub mod platform;
pub mod plugins;
pub mod session;
pub mod storage;
pub mod transact;
pub mod ui;
pub mod wallet;

pub use error::SessionError;
pub use kit::{LoginOptions, RestoreArgs, SessionKit, SessionKitArgs};
pub use login::{LoginContext, LoginHookFn, LoginHooks, UiRequirements, UserInterfaceWalletPlugin};
pub use platform::{HeadlessPlatform, Platform, PlatformName};
pub use plugins::{LoginPlugin, TransactPlugin};
pub use session::Session;
pub use storage::{InMemorySessionStorage, SerializedSession, SessionStorage, StorageError};
pub use transact::{
    TransactArgs, TransactContext, TransactError, TransactHookFn, TransactHooks, TransactOptions,
    TransactResult,
};
pub use ui::{
    LinkVariant, LocaleDefinitions, PromptArgs, PromptElement, PromptResponse, TranslateOptions,
    UiError, UserInterface, UserInterfaceLoginResponse,
};
pub use wallet::{
    AbstractWalletPlugin, LogoutContext, SerializedWalletPlugin, WalletError, WalletPlugin,
    WalletPluginConfig, WalletPluginData, WalletPluginLoginResponse, WalletPluginMetadata,
    WalletPluginSignResponse,
};
