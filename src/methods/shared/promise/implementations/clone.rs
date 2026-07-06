use crate::{sync::atomic::Ordering, PromiseRejection};

use super::super::SharedPromise;

impl<T, E> Clone for SharedPromise<T, E>
where
    E: PromiseRejection,
{
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            waiter_id: self.state.next_waiter_id.fetch_add(1, Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        task::{Context, Poll, Waker},
    };

    use crate::{Promise, PromiseRejection, TaskFailure};

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

    fn poll<F: std::future::Future + Unpin>(future: &mut F) -> std::task::Poll<F::Output> {
        std::pin::Pin::new(future).poll(&mut cx())
    }

    #[test]
    fn clone_created_after_settlement_observes_the_result() {
        let mut shared = Promise::<i32, E>::lazy(async { Ok(11) }).shared();

        assert_eq!(poll(&mut shared), std::task::Poll::Ready(Ok(11)));

        let mut latecomer = shared.clone();

        assert_eq!(poll(&mut latecomer), std::task::Poll::Ready(Ok(11)));
    }

    #[test]
    fn many_clones_all_observe_the_result() {
        let shared = Promise::<i32, E>::lazy(async { Ok(3) }).shared();

        let mut clones: Vec<_> = (0..5).map(|_| shared.clone()).collect();

        for clone in &mut clones {
            assert_eq!(poll(clone), std::task::Poll::Ready(Ok(3)));
        }
    }

    #[test]
    fn surviving_clone_observes_result_after_others_dropped() {
        let shared = Promise::<i32, E>::lazy(async { Ok(99) }).shared();

        let extra = shared.clone();
        let another = shared.clone();

        drop(extra);
        drop(another);

        let mut survivor = shared;

        assert_eq!(poll(&mut survivor), std::task::Poll::Ready(Ok(99)));
    }

    #[test]
    fn two_clones_observe_same_resolved_value() {
        let shared = Promise::<i32, E>::resolve(42).shared();

        let mut a = shared.clone();
        let mut b = shared;

        assert_eq!(poll(&mut a), Poll::Ready(Ok(42)));
        assert_eq!(poll(&mut b), Poll::Ready(Ok(42)));
    }

    #[test]
    fn many_clones_observe_same_value() {
        let shared = Promise::<i32, E>::resolve(7).shared();

        let mut clones: Vec<_> = (0..5).map(|_| shared.clone()).collect();

        for c in &mut clones {
            assert_eq!(poll(c), Poll::Ready(Ok(7)));
        }
    }

    #[test]
    fn clone_before_and_after_settlement_observe_same_value() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();
        let shared = promise.shared();

        let mut early = shared.clone();

        resolve.resolve(99);

        let mut late = shared;

        assert_eq!(poll(&mut early), Poll::Ready(Ok(99)));
        assert_eq!(poll(&mut late), Poll::Ready(Ok(99)));
    }

    #[test]
    fn cloning_does_not_run_inner_computation_more_than_once() {
        let counter = Arc::new(AtomicUsize::new(0));

        let probe = counter.clone();
        let shared = Promise::<i32, E>::lazy(async move {
            probe.fetch_add(1, Ordering::SeqCst);
            Ok(123)
        })
        .shared();

        let mut clones: Vec<_> = (0..4).map(|_| shared.clone()).collect();

        for c in &mut clones {
            assert_eq!(poll(c), Poll::Ready(Ok(123)));
        }

        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn dropping_some_clones_does_not_prevent_survivor_observing_result() {
        let shared = Promise::<i32, E>::resolve(55).shared();

        let mut survivor = shared.clone();

        let doomed_a = shared.clone();
        let doomed_b = shared.clone();

        drop(doomed_a);
        drop(doomed_b);
        drop(shared);

        assert_eq!(poll(&mut survivor), Poll::Ready(Ok(55)));
    }

    #[test]
    fn clone_observes_rejection_identically() {
        let shared = Promise::<i32, E>::lazy(async { Err(E::Fail) }).shared();

        let mut a = shared.clone();
        let mut b = shared;

        assert_eq!(poll(&mut a), Poll::Ready(Err(E::Fail)));
        assert_eq!(poll(&mut b), Poll::Ready(Err(E::Fail)));
    }
}
