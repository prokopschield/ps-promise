use crate::{PromiseRejection, TaskFailure, WrappedPromiseRejection};

impl<E> PromiseRejection for WrappedPromiseRejection<E>
where
    E: Send + Unpin + 'static,
{
    fn already_consumed() -> Self {
        Self::AlreadyConsumed
    }

    fn task_failed(failure: TaskFailure) -> Self {
        Self::TaskFailed(failure)
    }
}
