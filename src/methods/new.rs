use std::panic::{catch_unwind, AssertUnwindSafe};

use crate::{Promise, PromiseRejection, Reject, Resolve, TaskFailure};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Creates a [`Promise`] settled by an executor.
    ///
    /// Mirrors ECMAScript's `new Promise(executor)` as a wrapper over
    /// [`Promise::with_resolvers`]: `executor` is invoked synchronously with
    /// the [`Resolve`] and [`Reject`] handles, and the first settlement wins.
    /// The handles may be cloned or stored for later; if every handle is
    /// dropped without settling, the promise rejects with
    /// [`ResolversDropped`](crate::ResolversDropped) wrapped in
    /// [`TaskFailure::Error`], mapped through
    /// [`PromiseRejection::task_failed`]. A panic in the executor
    /// is caught and rejects the promise with [`TaskFailure::Panic`] mapped
    /// through [`PromiseRejection::task_failed`], unless the promise was
    /// settled before the panic.
    pub fn new<F>(executor: F) -> Self
    where
        F: FnOnce(Resolve<T, E>, Reject<T, E>),
    {
        let (promise, resolve, reject) = Self::with_resolvers();

        let reject_on_panic = reject.clone();

        if let Err(panic) = catch_unwind(AssertUnwindSafe(move || executor(resolve, reject))) {
            reject_on_panic.reject(E::task_failed(TaskFailure::from(panic)));
        }

        promise
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        task::{Context, Waker},
    };

    use crate::{Promise, PromiseRejection, Resolve, ResolversDropped, TaskFailure};

    #[derive(Debug, PartialEq)]
    enum E {
        AlreadyConsumed,
        Fail,
        TaskFailed(String),
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(failure: TaskFailure) -> Self {
            Self::TaskFailed(failure.to_string())
        }
    }

    fn cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    #[test]
    fn executor_resolves_synchronously() {
        let mut promise = Promise::<i32, E>::new(|resolve, _reject| resolve.resolve(42));

        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(42)));
    }

    #[test]
    fn executor_rejects_synchronously() {
        let mut promise = Promise::<i32, E>::new(|_resolve, reject| reject.reject(E::Fail));

        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Err(E::Fail)));
    }

    #[test]
    fn first_settlement_wins() {
        let mut promise = Promise::<i32, E>::new(|resolve, reject| {
            resolve.resolve(1);
            reject.reject(E::Fail);
        });

        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(1)));
    }

    #[test]
    fn stored_handle_settles_later() {
        let stash: Arc<Mutex<Option<Resolve<i32, E>>>> = Arc::new(Mutex::new(None));

        let slot = stash.clone();

        let mut promise = Promise::<i32, E>::new(move |resolve, _reject| {
            *slot.lock().expect("stash the resolve handle") = Some(resolve);
        });

        assert!(promise.poll_pending(&mut cx()));

        stash
            .lock()
            .expect("take the resolve handle")
            .take()
            .expect("stored handle")
            .resolve(7);

        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(7)));
    }

    #[test]
    fn dropping_handles_without_settling_rejects() {
        let mut promise = Promise::<i32, E>::new(|_resolve, _reject| {});

        promise.poll_settled(&mut cx());

        assert_eq!(
            promise.consume(),
            Some(Err(E::TaskFailed(ResolversDropped.to_string())))
        );
    }

    #[test]
    fn panicking_executor_rejects() {
        let mut promise = Promise::<i32, E>::new(|_resolve, _reject| panic!("executor panicked"));

        promise.poll_settled(&mut cx());

        match promise.consume() {
            Some(Err(E::TaskFailed(msg))) => assert!(msg.contains("executor panicked")),
            other => panic!("expected TaskFailed rejection, got {other:?}"),
        }
    }

    #[test]
    fn settlement_before_panic_wins() {
        let mut promise = Promise::<i32, E>::new(|resolve, _reject| {
            resolve.resolve(9);

            panic!("too late to matter");
        });

        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(9)));
    }
}
