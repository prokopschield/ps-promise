use std::sync::PoisonError;

use crate::PromiseRejection;

use super::super::SharedState;

impl<T, E> SharedState<T, E>
where
    E: PromiseRejection,
{
    /// Deregisters the consumer identified by `waiter_id`, dropping its
    /// registered waker if one is present.
    ///
    /// Called from the `SharedPromise` `Drop` implementation, so that a
    /// consumer dropped while pending does not leave a stale waker behind, and
    /// from the completion path, where the settling driver deregisters itself
    /// before the fan-out. Only the entry keyed by `waiter_id` is removed, so
    /// every other consumer's registration stays intact. Does nothing if the
    /// consumer has no registered waker, for example because a wake already
    /// drained the queue.
    pub(in crate::methods::shared) fn remove_waker(&self, waiter_id: usize) {
        let mut guard = self.wakers.lock().unwrap_or_else(PoisonError::into_inner);

        let waker = guard.remove(&waiter_id);

        drop(guard);
        drop(waker);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, PoisonError,
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

    /// Dropping a pending consumer must retract the waker it registered, so a
    /// surviving consumer is still woken on settlement while the dropped one is
    /// left untouched.
    #[test]
    fn dropped_pending_consumer_removes_registered_waker() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();
        let mut shared = promise.shared();
        let mut dropped = shared.clone();

        let dropped_waker = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let survivor_waker = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let dropped_task_waker = Waker::from(dropped_waker.clone());
        let survivor_task_waker = Waker::from(survivor_waker.clone());

        assert_eq!(poll_with(&mut dropped, &dropped_task_waker), Poll::Pending);
        drop(dropped);

        assert_eq!(poll_with(&mut shared, &survivor_task_waker), Poll::Pending);

        resolve.resolve(42);

        assert_eq!(dropped_waker.count.load(Ordering::SeqCst), 0);
        assert_eq!(survivor_waker.count.load(Ordering::SeqCst), 1);
        assert_eq!(poll(&mut shared), Poll::Ready(Ok(42)));
    }

    /// Polling distinct clones, each with its own task waker, while the inner
    /// promise is unresolved registers one waker per clone, since distinct
    /// wakers are not deduplicated against one another. Dropping every pending
    /// clone must retract each registration, so the queue returns to empty even
    /// though the inner promise never settles, and a settled promise is the only
    /// event that would otherwise drain the queue. Without per-consumer removal
    /// on drop, these stale wakers would persist for the whole pending window,
    /// growing without bound if the inner promise never resolves.
    #[test]
    fn dropped_pending_consumers_do_not_accumulate_wakers() {
        let (promise, _resolve, _reject) = Promise::<i32, E>::with_resolvers();
        let shared = promise.shared();

        let pending: Vec<_> = (0..8)
            .map(|_| {
                let mut consumer = shared.clone();

                let waker = Waker::from(Arc::new(CountingWaker {
                    count: AtomicUsize::new(0),
                }));

                assert_eq!(poll_with(&mut consumer, &waker), Poll::Pending);

                (consumer, waker)
            })
            .collect();

        assert_eq!(pending.len(), 8);

        let registered_while_live = shared
            .state
            .wakers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len();

        assert_eq!(registered_while_live, 8);

        drop(pending);

        let registered_after_drop = shared
            .state
            .wakers
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len();

        assert_eq!(registered_after_drop, 0);
    }
}
