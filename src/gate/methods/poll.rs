use std::task::{Context, Waker};

use super::super::GatedPromise;

impl<T, E> GatedPromise<T, E> {
    /// Polls the inner promise once if it requested a wakeup, and otherwise
    /// drops the poll as spurious. Does nothing once the inner promise has
    /// settled.
    ///
    /// While the inner promise is pending, stores `cx`'s waker as the outer
    /// waker to notify on the next wakeup request, replacing any previously
    /// stored waker. The inner promise is polled with the wrapper's own
    /// waker; the outer waker is never passed through.
    pub fn poll(&mut self, cx: &Context<'_>) {
        if !self.inner.is_pending() {
            return;
        }

        self.state.register(cx.waker());

        if !self.state.take_wake_request() {
            return;
        }

        let waker = Waker::from(self.state.clone());
        let mut inner_cx = Context::from_waker(&waker);

        self.inner.poll(&mut inner_cx);
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
        task::{Context, Poll, Wake, Waker},
    };

    use crate::{Promise, PromiseRejection, TaskFailure};

    use super::super::super::GatedPromise;

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

    struct CountingWaker {
        count: AtomicUsize,
    }

    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn counting_waker() -> (Arc<CountingWaker>, Waker) {
        let inner = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });

        let waker = Waker::from(inner.clone());

        (inner, waker)
    }

    /// Inner future that counts its polls and never settles.
    struct CountPolls {
        polls: Arc<AtomicUsize>,
    }

    impl Future for CountPolls {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.polls.fetch_add(1, Ordering::SeqCst);

            Poll::Pending
        }
    }

    /// Inner future that self-wakes and stays pending on its first poll, then
    /// resolves on its second poll.
    struct SelfWakeOnce {
        polls: Arc<AtomicUsize>,
    }

    impl Future for SelfWakeOnce {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let n = self.polls.fetch_add(1, Ordering::SeqCst);

            if n == 0 {
                cx.waker().wake_by_ref();

                return Poll::Pending;
            }

            Poll::Ready(Ok(42))
        }
    }

    #[test]
    fn first_poll_reaches_the_inner_promise() {
        let polls = Arc::new(AtomicUsize::new(0));

        let mut gated: GatedPromise<i32, E> = GatedPromise::new(Promise::lazy(CountPolls {
            polls: polls.clone(),
        }));

        let (_counter, waker) = counting_waker();

        gated.poll(&Context::from_waker(&waker));

        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert!(gated.inner.is_pending());
    }

    #[test]
    fn spurious_repolls_are_dropped() {
        let polls = Arc::new(AtomicUsize::new(0));

        let mut gated: GatedPromise<i32, E> = GatedPromise::new(Promise::lazy(CountPolls {
            polls: polls.clone(),
        }));

        let (_counter, waker) = counting_waker();

        gated.poll(&Context::from_waker(&waker));
        gated.poll(&Context::from_waker(&waker));
        gated.poll(&Context::from_waker(&waker));

        assert_eq!(polls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repoll_after_a_wakeup_request_reaches_the_inner_promise() {
        let polls = Arc::new(AtomicUsize::new(0));

        let mut gated: GatedPromise<i32, E> = GatedPromise::new(Promise::lazy(SelfWakeOnce {
            polls: polls.clone(),
        }));

        let (counter, waker) = counting_waker();

        gated.poll(&Context::from_waker(&waker));

        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert_eq!(counter.count.load(Ordering::SeqCst), 1);

        gated.poll(&Context::from_waker(&waker));

        assert_eq!(polls.load(Ordering::SeqCst), 2);
        assert_eq!(gated.inner.consume(), Some(Ok(42)));
    }

    #[test]
    fn wakeup_notifies_the_most_recent_outer_waker() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let mut gated = GatedPromise::new(promise);

        let (counter_a, waker_a) = counting_waker();
        let (counter_b, waker_b) = counting_waker();

        gated.poll(&Context::from_waker(&waker_a));
        gated.poll(&Context::from_waker(&waker_b));

        resolve.resolve(7);

        assert_eq!(counter_a.count.load(Ordering::SeqCst), 0);
        assert!(counter_b.count.load(Ordering::SeqCst) >= 1);

        let (_counter, waker) = counting_waker();

        gated.poll(&Context::from_waker(&waker));

        assert_eq!(gated.inner.consume(), Some(Ok(7)));
    }
}
