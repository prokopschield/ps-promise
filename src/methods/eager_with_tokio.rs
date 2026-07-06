use std::future::Future;

use tokio::task::JoinError;

use crate::{Promise, PromiseRejection, TaskFailure};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Wraps a [`Future`] in a [`Promise`] eagerly scheduled via [`tokio::spawn`].
    ///
    /// Scheduling happens synchronously during this call: by the time this
    /// function returns, the inner future is already registered with the
    /// ambient tokio runtime and runs to completion regardless of whether the
    /// outer [`Promise`] is ever polled. The outer [`Promise`] must still be
    /// polled (or awaited) to receive the outcome.
    ///
    /// # Panics
    ///
    /// Calling this function outside of a tokio runtime context panics,
    /// propagated from [`tokio::spawn`].
    pub fn eager_with_tokio(future: impl Future<Output = Result<T, E>> + Send + 'static) -> Self {
        Self::eager_with(future, |promise| {
            let task = tokio::spawn(promise);

            async move { map_join_result(task.await) }
        })
    }
}

/// Flattens the outcome of awaiting a [`tokio::task::JoinHandle`] returning
/// `Result<T, E>` into a single `Result<T, E>`.
///
/// `Ok(inner)` is returned as-is; a [`JoinError`] is surfaced as a rejection
/// through [`PromiseRejection::task_failed`].
fn map_join_result<T, E>(result: Result<Result<T, E>, JoinError>) -> Result<T, E>
where
    E: PromiseRejection,
{
    match result {
        Ok(result) => result,
        Err(join_err) => Err(E::task_failed(TaskFailure::from(join_err))),
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };

    use tokio::runtime::{Builder, Runtime};
    use tokio::task::JoinError;

    use super::map_join_result;
    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug)]
    enum E {
        AlreadyConsumed,
        Fail,
        TaskFailed(TaskFailure),
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(failure: TaskFailure) -> Self {
            Self::TaskFailed(failure)
        }
    }

    fn rt() -> Runtime {
        Builder::new_current_thread()
            .build()
            .expect("build current-thread tokio runtime")
    }

    #[test]
    fn resolves_value() {
        let result =
            rt().block_on(async { Promise::<i32, E>::eager_with_tokio(async { Ok(42) }).await });

        assert!(matches!(result, Ok(42)));
    }

    #[test]
    fn rejects_app_error() {
        let result = rt()
            .block_on(async { Promise::<i32, E>::eager_with_tokio(async { Err(E::Fail) }).await });

        assert!(matches!(result, Err(E::Fail)));
    }

    #[test]
    fn catches_panic_in_inner_future() {
        let result = rt().block_on(async {
            Promise::<i32, E>::eager_with_tokio(async { panic!("boom") }).await
        });

        match result {
            Err(E::TaskFailed(failure @ TaskFailure::Panic(_))) => {
                assert_eq!(failure.to_string(), "task panicked: boom");
            }
            other => panic!("expected Err(TaskFailed(Panic(_))), got {other:?}"),
        }
    }

    #[test]
    fn map_passes_through_ok() {
        let result: Result<i32, E> = map_join_result(Ok(Ok(42)));

        assert!(matches!(result, Ok(42)));
    }

    #[test]
    fn map_passes_through_app_error() {
        let result: Result<i32, E> = map_join_result(Ok(Err(E::Fail)));

        assert!(matches!(result, Err(E::Fail)));
    }

    #[test]
    fn map_routes_cancelled_join_error_to_aborted() {
        let join_err: JoinError = rt().block_on(async {
            let handle = tokio::spawn(std::future::pending::<Result<i32, E>>());

            handle.abort();

            handle
                .await
                .expect_err("expected JoinError from aborted task")
        });

        assert!(join_err.is_cancelled());

        let result: Result<i32, E> = map_join_result(Err(join_err));

        match result {
            Err(E::TaskFailed(TaskFailure::Aborted)) => {}
            other => panic!("expected Err(TaskFailed(Aborted)), got {other:?}"),
        }
    }

    #[test]
    fn schedules_at_construction() {
        let started = Arc::new(AtomicBool::new(false));
        let flag = started.clone();

        let rt = rt();

        rt.block_on(async move {
            let _promise = Promise::<i32, E>::eager_with_tokio(async move {
                flag.store(true, Ordering::Relaxed);
                Ok(7)
            });

            for _ in 0..5 {
                tokio::task::yield_now().await;
            }
        });

        assert!(
            started.load(Ordering::Relaxed),
            "inner future must run without outer polling"
        );
    }
}
