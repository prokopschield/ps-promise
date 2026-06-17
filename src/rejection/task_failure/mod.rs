mod implementations;

use std::error::Error;
use std::sync::Arc;

use thiserror::Error;

#[derive(Clone, Error)]
pub enum TaskFailure {
    #[error(transparent)]
    Error(Arc<dyn Error + Send + Sync + 'static>),

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
