use std::{sync::Arc, time::Duration};

use crate::{Promise, PromiseRejection, TaskFailure};

/// Rejection payload produced when a [`Promise::timeout`] deadline elapses.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
#[error("promise timed out")]
pub struct TimeoutError;

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Rejects this [`Promise`] if it does not settle within `duration`.
    ///
    /// The deadline timer is provided by [`Promise::sleep`] and follows its
    /// runtime dispatch. On expiry the promise rejects with [`TimeoutError`]
    /// wrapped in [`TaskFailure::Error`], mapped through
    /// [`PromiseRejection::task_failed`].
    pub fn timeout(self, duration: Duration) -> Self {
        let deadline = Promise::<(), E>::sleep(duration);

        Self::race([
            self,
            Self::lazy(async move {
                deadline.await?;

                Err(E::task_failed(TaskFailure::Error(Arc::new(TimeoutError))))
            }),
        ])
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::time::Duration;

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug, PartialEq)]
    enum E {
        AlreadyConsumed,
        TaskFailed,
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(_: TaskFailure) -> Self {
            Self::TaskFailed
        }
    }

    const SHORT: Duration = Duration::from_millis(30);
    const LONG: Duration = Duration::from_secs(10);

    fn drive<T, F>(make: F) -> Result<T, E>
    where
        T: Send + 'static,
        F: FnOnce() -> Promise<T, E> + Send + 'static,
    {
        #[cfg(feature = "tokio")]
        return tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("build current-thread tokio runtime")
            .block_on(async move { make().await });

        #[cfg(all(feature = "smol", not(feature = "tokio")))]
        return smol::block_on(async move { make().await });

        #[cfg(not(any(feature = "tokio", feature = "smol")))]
        {
            use std::task::{Context, Waker};

            let mut promise = make();
            let mut cx = Context::from_waker(Waker::noop());

            while !promise.settle(&mut cx) {}

            promise.consume().expect("promise settled")
        }
    }

    #[test]
    fn resolves_in_time() {
        let result = drive(|| Promise::<i32, E>::resolve(42).timeout(LONG));

        assert_eq!(result, Ok(42));
    }

    #[test]
    fn rejects_after_deadline() {
        let result = drive(|| {
            Promise::<i32, E>::lazy(std::future::pending::<Result<i32, E>>()).timeout(SHORT)
        });

        assert_eq!(result, Err(E::TaskFailed));
    }
}
