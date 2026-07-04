use std::{
    future::Future,
    pin::Pin,
    sync::PoisonError,
    task::{
        Context,
        Poll::{self, Pending, Ready},
        Waker,
    },
};

use crate::PromiseRejection;

use super::super::super::constants::LIVELOCK_MAX_SELF_POLLS;
use super::super::methods::poll_step::PollStep;
use super::super::SharedPromise;

impl<T, E> Future for SharedPromise<T, E>
where
    T: Clone + Send + 'static,
    E: PromiseRejection + Clone,
{
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let mut counter = 0;

        loop {
            match self.poll_step(cx) {
                PollStep::Pending => return Pending,
                PollStep::Rejected(err) => return Ready(Err(err)),
                PollStep::Resolved(value) => return Ready(Ok(value)),
                PollStep::Consumed => return Ready(Err(E::already_consumed())),
                PollStep::ReEnter => continue,
                PollStep::Woke => counter += 1,
            }

            if counter >= LIVELOCK_MAX_SELF_POLLS {
                let wakers: Vec<Waker> = self
                    .state
                    .wakers
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .values()
                    .cloned()
                    .collect();

                for waker in wakers {
                    waker.wake();
                }

                return Pending;
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc, Mutex,
        },
        task::{Context, Poll, Waker},
    };

    use crate::{Promise, PromiseRejection, SharedPromise, TaskFailure};

    use super::LIVELOCK_MAX_SELF_POLLS;

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
    fn every_clone_observes_the_result() {
        let shared = Promise::<i32, E>::lazy(async { Ok(42) }).shared();
        let mut first = shared.clone();
        let mut second = shared;

        assert_eq!(poll(&mut first), std::task::Poll::Ready(Ok(42)));
        assert_eq!(poll(&mut second), std::task::Poll::Ready(Ok(42)));
    }

    #[test]
    fn inner_promise_runs_once() {
        let runs = Arc::new(AtomicUsize::new(0));
        let counter = runs.clone();

        let shared = Promise::<i32, E>::lazy(async move {
            counter.fetch_add(1, Ordering::Relaxed);

            Ok(7)
        })
        .shared();

        let mut first = shared.clone();
        let mut second = shared;

        assert_eq!(poll(&mut first), std::task::Poll::Ready(Ok(7)));
        assert_eq!(poll(&mut second), std::task::Poll::Ready(Ok(7)));
        assert_eq!(runs.load(Ordering::Relaxed), 1);
    }

    #[test]
    fn repeated_polls_keep_returning_the_result() {
        let mut shared = Promise::<i32, E>::lazy(async { Ok(9) }).shared();

        assert_eq!(poll(&mut shared), std::task::Poll::Ready(Ok(9)));
        assert_eq!(poll(&mut shared), std::task::Poll::Ready(Ok(9)));
    }

    #[test]
    fn every_clone_observes_the_rejection() {
        let shared = Promise::<i32, E>::lazy(async { Err(E::Fail) }).shared();
        let mut first = shared.clone();
        let mut second = shared;

        assert_eq!(poll(&mut first), std::task::Poll::Ready(Err(E::Fail)));
        assert_eq!(poll(&mut second), std::task::Poll::Ready(Err(E::Fail)));
    }

    #[test]
    fn pending_then_settled_for_late_consumers() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let shared = promise.shared();
        let mut early = shared.clone();
        let mut late = shared;

        assert!(poll(&mut early).is_pending());

        resolve.resolve(5);

        assert_eq!(poll(&mut late), std::task::Poll::Ready(Ok(5)));
        assert_eq!(poll(&mut early), std::task::Poll::Ready(Ok(5)));
    }

    /// Inner future that self-wakes and returns `Pending` on every poll until it
    /// has been polled `limit` times, then resolves. A correct driver must not
    /// poll it `limit` times within a single `SharedPromise::poll`: the inline
    /// re-entry loop is expected to yield to the executor instead of re-entering
    /// without bound. The terminal resolve is an escape hatch so that a spinning
    /// driver terminates and fails the assertion rather than hanging the test.
    struct SpinProbe {
        polls: Arc<AtomicUsize>,
        limit: usize,
    }

    impl Future for SpinProbe {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let polled = self.polls.fetch_add(1, Ordering::Relaxed) + 1;

            if polled >= self.limit {
                return Poll::Ready(Ok(0));
            }

            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    #[test]
    fn inner_future_is_not_spin_polled_unboundedly() {
        const LIMIT: usize = 10_000;

        let polls = Arc::new(AtomicUsize::new(0));

        let mut shared = Promise::<i32, E>::lazy(SpinProbe {
            polls: polls.clone(),
            limit: LIMIT,
        })
        .shared();

        // A single poll must not re-drive the inner future `LIMIT` times: the
        // driver is expected to yield to the executor rather than spin-poll a
        // self-waking-pending future inline without bound.
        let _ = poll(&mut shared);

        let polled = polls.load(Ordering::Relaxed);

        assert!(
            polled < LIMIT,
            "inner future was polled {polled} times in one SharedPromise::poll; \
             the inline re-entry loop must yield instead of spin-polling"
        );
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

    #[test]
    fn resolved_shared_polls_ready_ok() {
        let mut shared: SharedPromise<i32, E> = Promise::resolve(42).shared();

        assert_eq!(poll(&mut shared), Poll::Ready(Ok(42)));
    }

    #[test]
    fn rejected_shared_polls_ready_err() {
        let inner: Promise<i32, E> = Promise::lazy(async { Err(E::Fail) });

        let mut shared = inner.shared();

        assert_eq!(poll(&mut shared), Poll::Ready(Err(E::Fail)));
    }

    #[test]
    fn inner_runs_exactly_once_across_clones() {
        let runs = Arc::new(AtomicUsize::new(0));

        let runs_inner = Arc::clone(&runs);
        let inner: Promise<i32, E> = Promise::lazy(async move {
            runs_inner.fetch_add(1, Ordering::SeqCst);
            Ok(7)
        });

        let mut a = inner.shared();
        let mut b = a.clone();
        let mut c = a.clone();

        assert_eq!(poll(&mut a), Poll::Ready(Ok(7)));
        assert_eq!(poll(&mut b), Poll::Ready(Ok(7)));
        assert_eq!(poll(&mut c), Poll::Ready(Ok(7)));

        assert_eq!(runs.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn every_clone_observes_same_result() {
        let inner: Promise<i32, E> = Promise::lazy(async { Ok(123) });

        let mut a = inner.shared();
        let mut b = a.clone();
        let mut c = b.clone();

        assert_eq!(poll(&mut a), Poll::Ready(Ok(123)));
        assert_eq!(poll(&mut b), Poll::Ready(Ok(123)));
        assert_eq!(poll(&mut c), Poll::Ready(Ok(123)));
    }

    #[test]
    fn repeated_polls_are_idempotent() {
        let mut shared: SharedPromise<i32, E> = Promise::resolve(9).shared();

        assert_eq!(poll(&mut shared), Poll::Ready(Ok(9)));
        assert_eq!(poll(&mut shared), Poll::Ready(Ok(9)));
        assert_eq!(poll(&mut shared), Poll::Ready(Ok(9)));
    }

    #[test]
    fn pending_clone_then_resolves() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let mut shared = promise.shared();
        let mut clone = shared.clone();

        assert_eq!(poll(&mut shared), Poll::Pending);
        assert_eq!(poll(&mut clone), Poll::Pending);

        resolve.resolve(55);

        assert_eq!(poll(&mut shared), Poll::Ready(Ok(55)));
        assert_eq!(poll(&mut clone), Poll::Ready(Ok(55)));
    }

    #[test]
    fn pending_consumer_is_woken_on_resolve() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let mut shared = promise.shared();

        let counter = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let waker = Waker::from(Arc::clone(&counter));

        assert_eq!(poll_with(&mut shared, &waker), Poll::Pending);
        assert_eq!(counter.count.load(Ordering::SeqCst), 0);

        resolve.resolve(88);

        assert!(counter.count.load(Ordering::SeqCst) >= 1);

        assert_eq!(poll_with(&mut shared, &waker), Poll::Ready(Ok(88)));
    }

    #[test]
    fn late_consumer_observes_settled_result() {
        let inner: Promise<i32, E> = Promise::lazy(async { Ok(321) });

        let mut shared = inner.shared();

        let late = shared.clone();

        assert_eq!(poll(&mut shared), Poll::Ready(Ok(321)));

        let mut late = late;

        assert_eq!(poll(&mut late), Poll::Ready(Ok(321)));
    }

    type ParkedConsumer = Arc<Mutex<Option<(SharedPromise<i32, E>, Waker)>>>;

    /// A slow self-waking inner future: it returns `Pending` and re-arms its own
    /// waker on every poll until it has been polled `LIVELOCK_MAX_SELF_POLLS`
    /// times, which trips the driver's inline self-poll bound. On the poll that
    /// trips the bound it lets a late consumer park into the just-drained waker
    /// queue, then keeps returning `Pending` (it would resolve on a later poll
    /// that the dropped driver never issues).
    struct SlowStrandProbe {
        parked: ParkedConsumer,
        polls: usize,
    }

    impl Future for SlowStrandProbe {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();

            this.polls += 1;

            cx.waker().wake_by_ref();

            if this.polls == LIVELOCK_MAX_SELF_POLLS {
                let mut parked = this.parked.lock().expect("parked consumer");

                if let Some((clone, waker)) = parked.as_mut() {
                    let mut parked_cx = Context::from_waker(waker);

                    assert!(Pin::new(clone).poll(&mut parked_cx).is_pending());
                }

                drop(parked);
            }

            Poll::Pending
        }
    }

    #[test]
    fn slow_self_waker_does_not_strand_a_late_consumer_when_driver_is_dropped() {
        let parked: ParkedConsumer = Arc::new(Mutex::new(None));

        let shared = Promise::<i32, E>::lazy(SlowStrandProbe {
            parked: parked.clone(),
            polls: 0,
        })
        .shared();

        let late_waker = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });

        {
            let mut slot = parked.lock().expect("register late consumer");

            *slot = Some((shared.clone(), Waker::from(late_waker.clone())));
        }

        let driver_waker = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let mut driver = shared;

        assert!(poll_with(&mut driver, &Waker::from(driver_waker)).is_pending());

        drop(driver);

        assert!(
            late_waker.count.load(Ordering::Relaxed) >= 1,
            "late consumer was stranded: parked, owed a wake, never woken"
        );
    }
}
