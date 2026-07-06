use std::sync::Arc;

use tokio::task::JoinError;

use crate::TaskFailure;

impl From<JoinError> for TaskFailure {
    fn from(err: JoinError) -> Self {
        if err.is_cancelled() {
            return Self::Aborted;
        }

        err.try_into_panic()
            .map_or_else(|err| Self::Error(Arc::new(err)), Self::from)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use tokio::task::JoinError;

    use crate::TaskFailure;

    /// A cancelled task's [`JoinError`] converts to [`TaskFailure::Aborted`].
    #[test]
    fn cancelled_join_error_converts_to_aborted() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("build current-thread tokio runtime");

        let join_err: JoinError = rt.block_on(async {
            let handle = tokio::spawn(std::future::pending::<()>());

            handle.abort();

            handle
                .await
                .expect_err("expected JoinError from aborted task")
        });

        assert!(join_err.is_cancelled());
        assert!(matches!(TaskFailure::from(join_err), TaskFailure::Aborted));
    }
}
