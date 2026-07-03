mod implementations;

use std::convert::Infallible;

use thiserror::Error;

use crate::TaskFailure;

#[derive(Clone, Debug, Error)]
pub enum WrappedPromiseRejection<E = Infallible>
where
    E: Send + 'static,
{
    #[error("This Promise was consumed already.")]
    AlreadyConsumed,
    #[error("The underlying task failed: {0}")]
    TaskFailed(TaskFailure),
    #[error(transparent)]
    Rejected(#[from] E),
}

#[cfg(test)]
mod tests {
    use super::WrappedPromiseRejection;
    use crate::TaskFailure;

    #[test]
    fn clone_preserves_already_consumed() {
        let rejection = WrappedPromiseRejection::<i32>::AlreadyConsumed;
        let clone = rejection.clone();

        assert_eq!(clone, rejection);
    }

    #[test]
    fn clone_preserves_rejected() {
        let rejection = WrappedPromiseRejection::Rejected(42);
        let clone = rejection.clone();

        assert_eq!(clone, rejection);
    }

    #[test]
    fn clone_preserves_task_failed() {
        let rejection: WrappedPromiseRejection =
            WrappedPromiseRejection::TaskFailed(TaskFailure::Panic("boom".into()));
        let clone = rejection.clone();

        assert!(matches!(clone, WrappedPromiseRejection::TaskFailed(_)));
        assert_eq!(clone.to_string(), rejection.to_string());
    }
}
