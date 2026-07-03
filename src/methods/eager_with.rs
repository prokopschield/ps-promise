use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    E: PromiseRejection,
{
    /// Wraps a [`Future`] in a [`Promise`] driven eagerly by the supplied spawner.
    ///
    /// The spawner schedules the inner [`Promise`] on its runtime and returns a handle
    /// future resolving to `Result<T, E>`. Panics inside `future` are caught at the
    /// [`Promise`] layer; the spawner only maps runtime-specific errors into `E`.
    pub fn eager_with<F, G, S>(future: F, spawner: S) -> Self
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
        G: Future<Output = Result<T, E>> + Send + 'static,
        S: FnOnce(Self) -> G,
    {
        Self::lazy(spawner(Self::lazy(future)))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicBool, Ordering},
            Arc,
        },
        task::{Context, Waker},
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

    fn cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    #[test]
    fn spawner_invoked_synchronously() {
        let called = Arc::new(AtomicBool::new(false));
        let flag = called.clone();

        let promise: Promise<i32, E> = Promise::eager_with(async { Ok(1) }, move |p| {
            flag.store(true, Ordering::Relaxed);
            p
        });

        drop(promise);

        assert!(called.load(Ordering::Relaxed));
    }

    #[test]
    fn resolves_value() {
        let mut p: Promise<i32, E> = Promise::eager_with(async { Ok(42) }, |p| p);

        p.ready(&mut cx());

        match p.consume() {
            Some(Ok(v)) => assert_eq!(v, 42),
            other => panic!("expected Resolved(42), got {other:?}"),
        }
    }

    #[test]
    fn rejects_error() {
        let mut p: Promise<i32, E> = Promise::eager_with(async { Err(E::Fail) }, |p| p);

        p.ready(&mut cx());

        match p.consume() {
            Some(Err(E::Fail)) => {}
            other => panic!("expected Rejected(Fail), got {other:?}"),
        }
    }

    #[test]
    fn catches_panic_in_inner_future() {
        let mut p: Promise<i32, E> = Promise::eager_with(async { panic!("boom") }, |p| p);

        p.ready(&mut cx());

        match p.consume() {
            Some(Err(E::TaskFailed(failure @ TaskFailure::Panic(_)))) => {
                assert_eq!(failure.to_string(), "task panicked: boom");
            }
            other => panic!("expected Rejected(TaskFailed(Panic(_))), got {other:?}"),
        }
    }

    #[test]
    fn spawner_can_map_to_task_failed() {
        let mut p: Promise<i32, E> = Promise::eager_with(async { Ok(99) }, |_p| async {
            Err(E::task_failed(TaskFailure::Error(Arc::new(
                std::io::Error::other("cancelled"),
            ))))
        });

        p.ready(&mut cx());

        match p.consume() {
            Some(Err(E::TaskFailed(TaskFailure::Error(_)))) => {}
            other => panic!("expected Rejected(TaskFailed(Error(_))), got {other:?}"),
        }
    }

    #[test]
    fn spawner_can_override_outcome() {
        let mut p: Promise<i32, E> =
            Promise::eager_with(async { Ok(99) }, |_p| async { Err(E::Fail) });

        p.ready(&mut cx());

        match p.consume() {
            Some(Err(E::Fail)) => {}
            other => panic!("expected Rejected(Fail), got {other:?}"),
        }
    }
}
