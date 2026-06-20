use std::sync::atomic::Ordering;

use crate::PromiseRejection;

use super::super::SharedState;

impl<T, E> SharedState<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    pub(in crate::methods::shared) fn woke(&self) {
        self.woke.store(true, Ordering::Relaxed);
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

    /// Inner future that self-wakes and returns `Pending` on its first poll, then
    /// resolves to `Ok(value)` on its second poll. Tracks the total poll count.
    struct SelfWakeOnce {
        polls: Arc<AtomicUsize>,
        value: i32,
    }

    impl Future for SelfWakeOnce {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let n = self.polls.fetch_add(1, Ordering::SeqCst);

            if n == 0 {
                cx.waker().wake_by_ref();

                return Poll::Pending;
            }

            Poll::Ready(Ok(self.value))
        }
    }

    /// Inner future that returns `Pending` on its first poll WITHOUT waking, then
    /// resolves to `Ok(value)` on its second poll. Tracks the total poll count.
    struct PendingNoWakeOnce {
        polls: Arc<AtomicUsize>,
        value: i32,
    }

    impl Future for PendingNoWakeOnce {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            let n = self.polls.fetch_add(1, Ordering::SeqCst);

            if n == 0 {
                return Poll::Pending;
            }

            Poll::Ready(Ok(self.value))
        }
    }

    /// Inner future that self-wakes and returns `Pending` on every poll until an
    /// escape threshold is reached, at which point it resolves. The escape exists
    /// solely so the test cannot hang; the bounded re-entry must yield long before.
    struct AlwaysSelfWake {
        polls: Arc<AtomicUsize>,
        escape: usize,
        value: i32,
    }

    impl Future for AlwaysSelfWake {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let n = self.polls.fetch_add(1, Ordering::SeqCst);

            if n >= self.escape {
                return Poll::Ready(Ok(self.value));
            }

            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    #[test]
    fn self_wake_reenters_and_finishes_within_one_outer_poll() {
        let polls = Arc::new(AtomicUsize::new(0));

        let inner = SelfWakeOnce {
            polls: polls.clone(),
            value: 42,
        };

        let mut shared: SharedPromise<i32, E> = Promise::lazy(inner).shared();

        let result = poll(&mut shared);

        assert_eq!(result, Poll::Ready(Ok(42)));
        assert_eq!(polls.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn non_self_waking_pending_returns_pending_from_outer_poll() {
        let polls = Arc::new(AtomicUsize::new(0));

        let inner = PendingNoWakeOnce {
            polls: polls.clone(),
            value: 7,
        };

        let mut shared: SharedPromise<i32, E> = Promise::lazy(inner).shared();

        let result = poll(&mut shared);

        assert_eq!(result, Poll::Pending);
        assert_eq!(polls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn bounded_reentry_yields_pending_without_spinning() {
        let polls = Arc::new(AtomicUsize::new(0));

        let escape = 10_000;

        let inner = AlwaysSelfWake {
            polls: polls.clone(),
            escape,
            value: 99,
        };

        let mut shared: SharedPromise<i32, E> = Promise::lazy(inner).shared();

        let result = poll(&mut shared);

        let observed = polls.load(Ordering::SeqCst);

        assert_eq!(result, Poll::Pending);
        assert!(
            observed < 1_000,
            "expected bounded re-entry to yield well below the escape, but inner was polled {observed} times"
        );
        assert!(observed < escape);
    }
}
