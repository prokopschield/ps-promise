use std::{
    future::Future,
    panic::{catch_unwind, AssertUnwindSafe},
};

use crate::{Promise, PromiseRejection, TaskFailure};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Calls a closure returning a [`Future`] and captures the outcome in a
    /// [`Promise`].
    ///
    /// Extends [`Promise::attempt`] to async callbacks, completing the analog
    /// of ECMAScript's `Promise.try`: the closure is invoked synchronously
    /// during this call; a panic in the closure is caught and rejects the
    /// promise with [`TaskFailure::Panic`] mapped through
    /// [`PromiseRejection::task_failed`]. The returned future is handed to
    /// [`Promise::eager_or_lazy`]: it is eagerly scheduled when a runtime
    /// feature is enabled and lazy otherwise, and a panic inside it likewise
    /// surfaces as a rejection.
    pub fn attempt_async<F, Fut>(f: F) -> Self
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<T, E>> + Send + 'static,
    {
        match catch_unwind(AssertUnwindSafe(f)) {
            Ok(future) => Self::eager_or_lazy(future),
            Err(panic) => Self::Rejected(E::task_failed(TaskFailure::from(panic))),
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::future::{ready, Ready};

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug)]
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

    #[test]
    fn panic_in_closure_produces_task_failed_rejection() {
        let promise: Promise<u32, E> =
            Promise::attempt_async(|| -> Ready<Result<u32, E>> { panic!("boom") });

        match promise {
            Promise::Rejected(E::TaskFailed(failure @ TaskFailure::Panic(_))) => {
                assert_eq!(failure.to_string(), "task panicked: boom");
            }
            other => panic!("expected Rejected(TaskFailed(Panic(_))), got {other:?}"),
        }
    }

    #[test]
    fn closure_runs_synchronously() {
        let mut ran = false;

        let promise: Promise<(), E> = Promise::attempt_async(|| {
            ran = true;

            ready(Ok(()))
        });

        assert!(ran);

        drop(promise);
    }

    #[cfg(not(any(feature = "tokio", feature = "smol")))]
    #[test]
    fn delivers_value_lazily_without_runtime() {
        use std::task::{Context, Waker};

        let mut promise: Promise<i32, E> = Promise::attempt_async(|| async { Ok(42) });

        assert!(promise.is_pending());

        promise.poll(&mut Context::from_waker(Waker::noop()));

        match promise {
            Promise::Resolved(value) => assert_eq!(value, 42),
            other => panic!("expected Resolved(42), got {other:?}"),
        }
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn delivers_value_via_tokio() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("build current-thread tokio runtime");

        let result =
            rt.block_on(async { Promise::<i32, E>::attempt_async(|| async { Ok(42) }).await });

        assert!(matches!(result, Ok(42)));
    }

    #[cfg(all(feature = "smol", not(feature = "tokio")))]
    #[test]
    fn delivers_value_via_smol() {
        let result =
            smol::block_on(async { Promise::<i32, E>::attempt_async(|| async { Ok(42) }).await });

        assert!(matches!(result, Ok(42)));
    }
}
