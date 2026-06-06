mod implementations;

use thiserror::Error;

#[derive(Clone, Debug, Error, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub enum WrappedPromiseRejection<E>
where
    E: Send + Unpin + 'static,
{
    #[error("This Promise was consumed already.")]
    AlreadyConsumed,
    #[error("The underlying task failed.")]
    TaskFailed,
    #[error(transparent)]
    Rejected(#[from] E),
}
