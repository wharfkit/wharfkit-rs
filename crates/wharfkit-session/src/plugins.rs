use crate::login::LoginHooks;
use crate::transact::TransactHooks;

pub trait TransactPlugin: Send + Sync {
    fn register(&self, hooks: &mut TransactHooks);
}

pub trait LoginPlugin: Send + Sync {
    fn register(&self, hooks: &mut LoginHooks);
}
