mod implementations;

use std::error::Error;
use std::sync::Arc;

use thiserror::Error;

/// The cause of a task failure: the underlying task ended without producing
/// a rejection value, e.g. it panicked or was cancelled.
///
/// Passed to
/// [`PromiseRejection::task_failed`](crate::PromiseRejection::task_failed)
/// so the rejection type can represent the failure.
#[derive(Clone, Error)]
#[non_exhaustive]
pub enum TaskFailure {
    /// The task was aborted through an
    /// [`AbortHandle`](crate::AbortHandle).
    #[error("promise aborted")]
    Aborted,

    /// The task failed with an error, such as a timeout or dropped resolver
    /// handles.
    #[error(transparent)]
    Error(Arc<dyn Error + Send + Sync + 'static>),

    /// The task panicked. Carries the panic message.
    #[error("task panicked: {0}")]
    Panic(Arc<str>),
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::TaskFailure;

    #[test]
    fn clone_preserves_error() {
        let failure = TaskFailure::Error(Arc::new(std::io::Error::other("boom")));
        let clone = failure.clone();

        assert!(matches!(clone, TaskFailure::Error(_)));
        assert_eq!(clone.to_string(), failure.to_string());
    }

    #[test]
    fn clone_preserves_panic() {
        let failure = TaskFailure::Panic("boom".into());
        let clone = failure.clone();

        assert!(matches!(clone, TaskFailure::Panic(_)));
        assert_eq!(clone.to_string(), failure.to_string());
    }
}
