use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Wraps a [`Future`] in a [`Promise`] eagerly scheduled via [`smol::spawn`].
    ///
    /// Scheduling happens synchronously during this call: by the time this
    /// function returns, the inner future is already registered with smol's
    /// global executor. The outer [`Promise`] must still be polled (or
    /// awaited) to receive the outcome.
    ///
    /// The spawned future runs to completion even if the returned [`Promise`]
    /// is dropped or never polled; the outcome is then discarded.
    pub fn eager_with_smol(future: impl Future<Output = Result<T, E>> + Send + 'static) -> Self {
        Self::eager_with(future, |promise| {
            let (relay, resolve, reject) = Self::with_resolvers();

            smol::spawn(async move {
                match promise.await {
                    Ok(value) => resolve.resolve(value),
                    Err(rejection) => reject.reject(rejection),
                }
            })
            .detach();

            relay
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
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

    #[test]
    fn resolves_value() {
        let result =
            smol::block_on(async { Promise::<i32, E>::eager_with_smol(async { Ok(42) }).await });

        assert!(matches!(result, Ok(42)));
    }

    #[test]
    fn rejects_app_error() {
        let result = smol::block_on(async {
            Promise::<i32, E>::eager_with_smol(async { Err(E::Fail) }).await
        });

        assert!(matches!(result, Err(E::Fail)));
    }

    #[test]
    fn catches_panic_in_inner_future() {
        let result = smol::block_on(async {
            Promise::<i32, E>::eager_with_smol(async { panic!("boom") }).await
        });

        match result {
            Err(E::TaskFailed(failure @ TaskFailure::Panic(_))) => {
                assert_eq!(failure.to_string(), "task panicked: boom");
            }
            other => panic!("expected Err(TaskFailed(Panic(_))), got {other:?}"),
        }
    }

    /// Dropping the outer [`Promise`] abandons the outcome but must not
    /// cancel the spawned future, mirroring ECMAScript promise semantics.
    #[test]
    #[allow(clippy::expect_used)]
    fn dropped_promise_leaves_the_task_running() {
        let (start_tx, start_rx) = async_channel::bounded::<()>(1);
        let (done_tx, done_rx) = async_channel::bounded::<()>(1);

        let promise = Promise::<i32, E>::eager_with_smol(async move {
            start_rx.recv().await.ok();
            done_tx.send(()).await.ok();

            Ok(0)
        });

        drop(promise);

        smol::block_on(async move {
            start_tx
                .send(())
                .await
                .expect("the spawned task must still be listening");

            done_rx
                .recv()
                .await
                .expect("the spawned task must run to completion");
        });
    }

    #[test]
    fn schedules_at_construction() {
        let started = Arc::new(AtomicBool::new(false));
        let inner_flag = started.clone();
        let wait_flag = started.clone();

        smol::block_on(async move {
            let _promise = Promise::<i32, E>::eager_with_smol(async move {
                inner_flag.store(true, Ordering::Relaxed);
                Ok(7)
            });

            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

            while !wait_flag.load(Ordering::Relaxed) && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });

        assert!(
            started.load(Ordering::Relaxed),
            "inner future must run without outer polling"
        );
    }
}
