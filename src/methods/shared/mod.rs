mod constants;
mod promise;
mod state;
mod waker;

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Arc, Mutex,
    },
};

pub use promise::SharedPromise;
use state::SharedState;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Clone + Send + 'static,
    E: PromiseRejection + Clone,
{
    /// Converts this [`Promise`] into a clonable, multi-consumer handle.
    ///
    /// The inner promise runs once; every clone observes the same settled
    /// result, cloned per consumer. Unlike a [`Promise`], a
    /// [`SharedPromise`] is never consumed: the result stays available for
    /// repeated polling as long as any handle is alive. This mirrors
    /// ECMAScript promises, which can be awaited any number of times.
    pub fn shared(self) -> SharedPromise<T, E> {
        SharedPromise {
            state: Arc::new(SharedState {
                inner: Mutex::new(Some(self)),
                wakers: Mutex::new(HashMap::default()),
                next_waiter_id: AtomicUsize::new(1),
                woke: AtomicBool::new(false),
            }),
            waiter_id: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        task::{Context, Poll, Waker},
    };

    use crate::{Promise, PromiseRejection, SharedPromise, TaskFailure};

    #[derive(Debug, Clone, PartialEq)]
    enum E {
        AlreadyConsumed,
        Fail,
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

    fn cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    fn poll<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
        Pin::new(future).poll(&mut cx())
    }

    #[test]
    fn shared_resolved_promise_polls_to_value() {
        let shared: SharedPromise<i32, E> = Promise::<i32, E>::resolve(42).shared();

        let mut handle = shared;

        assert_eq!(poll(&mut handle), Poll::Ready(Ok(42)));
    }

    #[test]
    fn shared_rejected_promise_polls_to_error() {
        let shared: SharedPromise<i32, E> =
            Promise::<i32, E>::lazy(async { Err(E::Fail) }).shared();

        let mut handle = shared;

        assert_eq!(poll(&mut handle), Poll::Ready(Err(E::Fail)));
    }

    #[test]
    fn shared_does_not_run_body_eagerly() {
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_in_body = Arc::clone(&counter);
        let shared: SharedPromise<i32, E> = Promise::<i32, E>::lazy(async move {
            counter_in_body.fetch_add(1, Ordering::SeqCst);
            Ok(7)
        })
        .shared();

        assert_eq!(counter.load(Ordering::SeqCst), 0);

        drop(shared);
    }

    #[test]
    fn shared_inner_body_runs_at_most_once_across_clones() {
        let counter = Arc::new(AtomicUsize::new(0));

        let counter_in_body = Arc::clone(&counter);
        let shared: SharedPromise<i32, E> = Promise::<i32, E>::lazy(async move {
            counter_in_body.fetch_add(1, Ordering::SeqCst);
            Ok(99)
        })
        .shared();

        let mut a = shared.clone();
        let mut b = shared.clone();
        let mut c = shared;

        assert_eq!(poll(&mut a), Poll::Ready(Ok(99)));
        assert_eq!(poll(&mut b), Poll::Ready(Ok(99)));
        assert_eq!(poll(&mut c), Poll::Ready(Ok(99)));

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn shared_clones_agree_on_resolved_result() {
        let shared: SharedPromise<i32, E> = Promise::<i32, E>::resolve(123).shared();

        let mut first = shared.clone();
        let mut second = shared;

        let first_result = poll(&mut first);
        let second_result = poll(&mut second);

        assert_eq!(first_result, Poll::Ready(Ok(123)));
        assert_eq!(second_result, Poll::Ready(Ok(123)));
        assert_eq!(first_result, second_result);
    }

    #[test]
    fn shared_clones_agree_on_rejected_result() {
        let shared: SharedPromise<i32, E> =
            Promise::<i32, E>::lazy(async { Err(E::Fail) }).shared();

        let mut first = shared.clone();
        let mut second = shared;

        let first_result = poll(&mut first);
        let second_result = poll(&mut second);

        assert_eq!(first_result, Poll::Ready(Err(E::Fail)));
        assert_eq!(second_result, Poll::Ready(Err(E::Fail)));
        assert_eq!(first_result, second_result);
    }

    #[test]
    fn shared_clones_agree_on_task_failure() {
        let shared: SharedPromise<i32, E> =
            Promise::<i32, E>::lazy(async { panic!("boom") }).shared();

        let mut first = shared.clone();
        let mut second = shared;

        let first_result = poll(&mut first);
        let second_result = poll(&mut second);

        assert_eq!(first_result, Poll::Ready(Err(E::TaskFailed)));
        assert_eq!(second_result, Poll::Ready(Err(E::TaskFailed)));
        assert_eq!(first_result, second_result);
    }
}
