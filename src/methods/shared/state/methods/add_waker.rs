use std::{sync::PoisonError, task::Waker};

use crate::PromiseRejection;

use super::super::SharedState;

impl<T, E> SharedState<T, E>
where
    E: PromiseRejection,
{
    /// Registers `waker` as the wakeup handle for the consumer identified by
    /// `waiter_id`.
    ///
    /// At most one waker is kept per consumer: a re-poll with a different waker
    /// replaces the prior one, since the `Future` contract only requires the
    /// most recent waker to be woken. The clone is skipped when the stored waker
    /// already wakes the same task, the common case when a consumer is re-polled
    /// by an unchanged executor task.
    pub(in crate::methods::shared) fn add_waker(&self, waiter_id: usize, waker: &Waker) {
        let mut wakers = self.wakers.lock().unwrap_or_else(PoisonError::into_inner);

        if !wakers
            .get(&waiter_id)
            .is_some_and(|existing| existing.will_wake(waker))
        {
            wakers.insert(waiter_id, waker.clone());
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

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug, Clone, PartialEq)]
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

    fn poll<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
        Pin::new(future).poll(&mut Context::from_waker(Waker::noop()))
    }

    fn poll_with<F: Future + Unpin>(future: &mut F, waker: &Waker) -> Poll<F::Output> {
        Pin::new(future).poll(&mut Context::from_waker(waker))
    }

    struct CountingWaker {
        count: AtomicUsize,
    }

    impl std::task::Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    /// Polling one pending shared clone twice with the same task waker must register
    /// the waker only once, so settling wakes it exactly once.
    #[test]
    fn second_poll_with_same_waker_is_deduplicated() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();
        let mut shared = promise.shared();

        let cw = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let w = Waker::from(cw.clone());

        let first = poll_with(&mut shared, &w);
        let second = poll_with(&mut shared, &w);

        assert_eq!(first, Poll::Pending);
        assert_eq!(second, Poll::Pending);

        resolve.resolve(7);

        assert_eq!(cw.count.load(Ordering::SeqCst), 1);
    }

    /// A waker cloned from the same `Arc<CountingWaker>` wakes the same task, so it
    /// `will_wake` the original and the re-poll is likewise deduplicated.
    #[test]
    fn second_poll_with_cloned_same_arc_waker_is_deduplicated() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();
        let mut shared = promise.shared();

        let cw = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let w1 = Waker::from(cw.clone());
        let w2 = Waker::from(cw.clone());

        let first = poll_with(&mut shared, &w1);
        let second = poll_with(&mut shared, &w2);

        assert_eq!(first, Poll::Pending);
        assert_eq!(second, Poll::Pending);

        resolve.resolve(11);

        assert_eq!(cw.count.load(Ordering::SeqCst), 1);
    }

    /// Distinct consumers register distinct wakers and are not deduplicated against
    /// each other, so settling wakes each of them at least once.
    #[test]
    fn distinct_consumers_are_not_deduplicated() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();
        let shared = promise.shared();
        let mut clone_a = shared.clone();
        let mut clone_b = shared;

        let cw_a = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let cw_b = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let wa = Waker::from(cw_a.clone());
        let wb = Waker::from(cw_b.clone());

        assert_eq!(poll_with(&mut clone_a, &wa), Poll::Pending);
        assert_eq!(poll_with(&mut clone_b, &wb), Poll::Pending);

        resolve.resolve(13);

        assert!(cw_a.count.load(Ordering::SeqCst) >= 1);
        assert!(cw_b.count.load(Ordering::SeqCst) >= 1);
    }

    /// After deduplication, the consumer still settles correctly: a subsequent poll
    /// of the same clone returns the resolved value.
    #[test]
    fn consumer_still_resolves_after_deduplication() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();
        let mut shared = promise.shared();

        let cw = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let w = Waker::from(cw.clone());

        assert_eq!(poll_with(&mut shared, &w), Poll::Pending);
        assert_eq!(poll_with(&mut shared, &w), Poll::Pending);

        resolve.resolve(42);

        assert_eq!(cw.count.load(Ordering::SeqCst), 1);
        assert_eq!(poll(&mut shared), Poll::Ready(Ok(42)));
    }

    /// Re-polling one consumer with a different waker replaces its prior
    /// registration: the queue holds at most one waker per consumer, so only
    /// the most recent waker is woken on settlement. This is all the `Future`
    /// contract requires, since the executor only relies on the waker from the
    /// most recent poll.
    #[test]
    fn repolling_same_consumer_replaces_its_waker() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();
        let mut shared = promise.shared();

        let cw_a = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let cw_b = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let wa = Waker::from(cw_a.clone());
        let wb = Waker::from(cw_b.clone());

        assert_eq!(poll_with(&mut shared, &wa), Poll::Pending);
        assert_eq!(poll_with(&mut shared, &wb), Poll::Pending);

        resolve.resolve(42);

        assert_eq!(cw_a.count.load(Ordering::SeqCst), 0);
        assert_eq!(cw_b.count.load(Ordering::SeqCst), 1);
        assert_eq!(poll(&mut shared), Poll::Ready(Ok(42)));
    }
}
