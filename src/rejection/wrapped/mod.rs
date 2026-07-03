mod implementations;

use std::convert::Infallible;

use thiserror::Error;

use crate::TaskFailure;

/// A ready-made [`PromiseRejection`](crate::PromiseRejection) wrapping an
/// arbitrary error type `E`.
///
/// [`Promise::wrap`](crate::Promise::wrap) uses this to adapt futures whose
/// error type does not implement the trait.
#[derive(Clone, Debug, Error)]
pub enum WrappedPromiseRejection<E = Infallible>
where
    E: Send + 'static,
{
    /// The promise was consumed more than once.
    #[error("This Promise was consumed already.")]
    AlreadyConsumed,
    /// The underlying task failed.
    #[error("The underlying task failed: {0}")]
    TaskFailed(TaskFailure),
    /// The wrapped rejection value.
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
