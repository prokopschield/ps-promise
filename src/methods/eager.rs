use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    /// Wraps a [`Future`] in a [`Promise`] eagerly scheduled via `tokio::spawn`
    /// or `smol::spawn`.
    ///
    /// Dispatch is selected at compile time based on which runtime features
    /// are enabled, with a runtime check when both are on:
    ///
    /// - Only `tokio` enabled: always dispatches to `Promise::eager_with_tokio`.
    /// - Only `smol` enabled: always dispatches to `Promise::eager_with_smol`.
    /// - Both enabled: dispatches to `Promise::eager_with_tokio` when called
    ///   from within a tokio runtime context (detected via
    ///   `tokio::runtime::Handle::try_current`), otherwise to
    ///   `Promise::eager_with_smol`.
    ///
    /// Requires at least one of the `tokio` or `smol` features; if neither is
    /// enabled this method does not exist and call sites fail to compile.
    pub fn eager(future: impl Future<Output = Result<T, E>> + Send + 'static) -> Self {
        #[cfg(all(feature = "tokio", feature = "smol"))]
        return if tokio::runtime::Handle::try_current().is_ok() {
            Self::eager_with_tokio(future)
        } else {
            Self::eager_with_smol(future)
        };

        #[cfg(all(feature = "tokio", not(feature = "smol")))]
        return Self::eager_with_tokio(future);

        #[cfg(all(feature = "smol", not(feature = "tokio")))]
        return Self::eager_with_smol(future);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug)]
    #[allow(dead_code)]
    enum E {
        AlreadyConsumed,
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

    #[cfg(feature = "tokio")]
    #[test]
    fn resolves_value_via_tokio() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("build current-thread tokio runtime");

        let result = rt.block_on(async { Promise::<i32, E>::eager(async { Ok(42) }).await });

        assert!(matches!(result, Ok(42)));
    }

    #[cfg(all(feature = "smol", not(feature = "tokio")))]
    #[test]
    fn resolves_value_via_smol() {
        let result = smol::block_on(async { Promise::<i32, E>::eager(async { Ok(42) }).await });

        assert!(matches!(result, Ok(42)));
    }
}
