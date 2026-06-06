mod implementations;

use thiserror::Error;

use crate::TaskFailure;

#[derive(Debug, Error)]
pub enum WrappedPromiseRejection<E>
where
    E: Send + Unpin + 'static,
{
    #[error("This Promise was consumed already.")]
    AlreadyConsumed,
    #[error("The underlying task failed: {0}")]
    TaskFailed(TaskFailure),
    #[error(transparent)]
    Rejected(#[from] E),
}
