use async_trait::async_trait;
use std::sync::Mutex;
use wharfkit_session::{
    LoginContext, PromptArgs, PromptResponse, SessionError, UiError, UserInterface,
    UserInterfaceLoginResponse,
};

pub struct MockUserInterface {
    pub prompts: Mutex<Vec<PromptArgs>>,
    pub errors: Mutex<Vec<String>>,
    pub response: Mutex<PromptResponse>,
}

impl Default for MockUserInterface {
    fn default() -> Self {
        Self {
            prompts: Mutex::new(Vec::new()),
            errors: Mutex::new(Vec::new()),
            response: Mutex::new(PromptResponse::ButtonPressed {
                id: "default".into(),
            }),
        }
    }
}

#[async_trait]
impl UserInterface for MockUserInterface {
    async fn login(&self, _ctx: &LoginContext) -> Result<UserInterfaceLoginResponse, UiError> {
        Ok(UserInterfaceLoginResponse {
            chain_id: None,
            permission_level: None,
            wallet_plugin_index: 0,
        })
    }

    async fn on_error(&self, err: &SessionError) -> Result<(), UiError> {
        self.errors.lock().unwrap().push(format!("{err}"));
        Ok(())
    }

    async fn prompt(&self, args: PromptArgs) -> Result<PromptResponse, UiError> {
        self.prompts.lock().unwrap().push(args);
        Ok(self.response.lock().unwrap().clone())
    }
}
