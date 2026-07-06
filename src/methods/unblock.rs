use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Runs a blocking closure on a thread pool and wraps the result in a
    /// [`Promise`].
    ///
    /// Dispatch is selected at compile time based on which runtime features
    /// are enabled, with a runtime check when `tokio` is on:
    ///
    /// - `tokio` enabled and called from within a tokio runtime context
    ///   (detected via `tokio::runtime::Handle::try_current`): dispatches to
    ///   `tokio::task::spawn_blocking`; a panic in the closure is mapped to
    ///   [`TaskFailure::Panic`](crate::TaskFailure::Panic), and a cancelled
    ///   task (runtime shutdown) to
    ///   [`TaskFailure::Aborted`](crate::TaskFailure::Aborted).
    /// - Otherwise, with `smol` enabled: dispatches to `smol::unblock`.
    /// - Otherwise: dispatches to `blocking::unblock`, which uses the
    ///   `blocking` crate's runtime-independent thread pool.
    ///
    /// On the smol and blocking paths, the pool task is detached, and a
    /// panic in the closure is caught on the worker thread and mapped to
    /// [`TaskFailure::Panic`](crate::TaskFailure::Panic); a closure panic
    /// rejects the promise on every path.
    ///
    /// The closure is scheduled synchronously during this call and runs to
    /// completion even if the [`Promise`] is dropped, in which case its
    /// outcome is discarded. The outer [`Promise`] must still be polled (or
    /// awaited) to receive the outcome.
    pub fn unblock<F>(f: F) -> Self
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
    {
        #[cfg(feature = "tokio")]
        if tokio::runtime::Handle::try_current().is_ok() {
            let handle = tokio::task::spawn_blocking(f);

            return Self::lazy(
                async move { super::eager_with_tokio::map_join_result(handle.await) },
            );
        }

        let (relay, resolve, reject) = Self::with_resolvers();

        let run = move || match catch_unwind(AssertUnwindSafe(f)) {
            Ok(Ok(value)) => resolve.resolve(value),
            Ok(Err(rejection)) => reject.reject(rejection),
            Err(panic) => reject.reject(E::task_failed(crate::TaskFailure::from(panic))),
        };

        #[cfg(feature = "smol")]
        smol::unblock(run).detach();

        #[cfg(not(feature = "smol"))]
        blocking::unblock(run).detach();

        relay
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        task::{Context, Waker},
        time::{Duration, Instant},
    };

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

    fn wait_settled<T: Send + 'static>(promise: &mut Promise<T, E>) {
        let deadline = Instant::now() + Duration::from_secs(5);

        while !promise.poll_settled(&mut Context::from_waker(Waker::noop())) {
            assert!(Instant::now() < deadline, "promise did not settle in time");

            std::thread::yield_now();
        }
    }

    #[test]
    fn resolves_value() {
        let mut promise: Promise<i32, E> = Promise::unblock(|| Ok(42));

        wait_settled(&mut promise);

        assert!(matches!(promise.consume(), Some(Ok(42))));
    }

    #[test]
    fn rejects_app_error() {
        let mut promise: Promise<i32, E> = Promise::unblock(|| Err(E::Fail));

        wait_settled(&mut promise);

        assert!(matches!(promise.consume(), Some(Err(E::Fail))));
    }

    #[test]
    fn closure_panic_rejects() {
        let mut promise: Promise<i32, E> = Promise::unblock(|| panic!("boom"));

        wait_settled(&mut promise);

        match promise.consume() {
            Some(Err(E::TaskFailed(failure @ TaskFailure::Panic(_)))) => {
                assert_eq!(failure.to_string(), "task panicked: boom");
            }
            other => panic!("expected Err(TaskFailed(Panic(_))), got {other:?}"),
        }
    }

    /// Dropping the outer [`Promise`] abandons the outcome but must not
    /// cancel the closure, even one the pool has not started yet, mirroring
    /// ECMAScript promise semantics.
    #[test]
    fn dropped_promise_leaves_the_closure_running() {
        let (start_tx, start_rx) = std::sync::mpsc::channel::<()>();
        let (done_tx, done_rx) = std::sync::mpsc::channel::<()>();

        let promise: Promise<i32, E> = Promise::unblock(move || {
            start_rx.recv().ok();
            done_tx.send(()).ok();

            Ok(0)
        });

        drop(promise);

        start_tx
            .send(())
            .expect("the closure must still be listening");

        done_rx
            .recv_timeout(Duration::from_secs(5))
            .expect("the closure must run to completion");
    }
}
