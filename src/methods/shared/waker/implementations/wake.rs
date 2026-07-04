use std::{
    collections::HashSet,
    sync::{Arc, PoisonError},
    task::Wake,
};

use crate::PromiseRejection;

use super::super::SharedWaker;

impl<T, E> Wake for SharedWaker<T, E>
where
    T: Send,
    E: PromiseRejection,
{
    fn wake(self: Arc<Self>) {
        if let Some(state) = self.state.upgrade() {
            state.woke();

            let mut called = HashSet::new();

            // Each entry is removed from the queue before its waker runs, so
            // a foreign waker that panics mid-drain aborts the drain and
            // leaves the remaining entries registered but unwoken until the
            // next wake. Tolerated as pathological: a panicking waker
            // violates the `Wake` contract.
            loop {
                let next = {
                    let mut guard = state.wakers.lock().unwrap_or_else(PoisonError::into_inner);

                    guard
                        .keys()
                        .copied()
                        .find(|key| !called.contains(key))
                        .and_then(|key| guard.remove(&key).map(|waker| (key, waker)))
                };

                let Some((key, waker)) = next else { break };

                called.insert(key);

                waker.wake();
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

    fn poll_with<F: std::future::Future + Unpin>(
        future: &mut F,
        waker: &Waker,
    ) -> std::task::Poll<F::Output> {
        std::pin::Pin::new(future).poll(&mut Context::from_waker(waker))
    }

    #[test]
    fn pending_consumer_is_woken_when_inner_settles() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let mut shared = promise.shared();
        let waker = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let task_waker = Waker::from(waker.clone());

        assert!(poll_with(&mut shared, &task_waker).is_pending());
        assert_eq!(waker.count.load(Ordering::Relaxed), 0);

        resolve.resolve(8);

        assert_eq!(waker.count.load(Ordering::Relaxed), 1);
        assert_eq!(poll(&mut shared), std::task::Poll::Ready(Ok(8)));
    }

    /// Inner future that drains the waker queue mid-drive, then parks a late
    /// consumer into the emptied queue:
    ///
    /// 1. On its first poll it wakes its own waker (`shared_waker`), draining
    ///    the queue while the only registered waker is the driver's. The driver
    ///    is woken, and the queue is emptied.
    /// 2. It then lets a *second* consumer poll. That consumer sees the slot
    ///    taken (the driver still holds the inner promise), parks, and registers
    ///    into the just-emptied queue.
    /// 3. It returns `Pending`, but resolves on its second poll.
    ///
    /// Because the inner future resolves within the driver's inline `Woke`
    /// re-entry, the driver re-drives it to completion in the same `poll` call,
    /// and the completion fan-out wakes the parked consumer before `poll`
    /// returns. This guards that the fast-path re-drive delivers the wake even
    /// when the late consumer registered after the queue was drained.
    type ParkedConsumer = Arc<Mutex<Option<(SharedPromise<i32, E>, Waker)>>>;

    struct StrandProbe {
        parked: ParkedConsumer,
        polled: bool,
    }

    impl Future for StrandProbe {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();

            if this.polled {
                return Poll::Ready(Ok(123));
            }

            this.polled = true;

            // (1) Drain the queue while it holds only the driver's waker.
            cx.waker().wake_by_ref();

            // (2) A late consumer parks into the now-empty queue.
            let mut parked = this.parked.lock().expect("parked consumer");

            if let Some((clone, waker)) = parked.as_mut() {
                let mut parked_cx = Context::from_waker(waker);

                assert!(Pin::new(clone).poll(&mut parked_cx).is_pending());
            }

            // satisfy lint
            drop(parked);

            Poll::Pending
        }
    }

    #[test]
    fn parked_consumer_is_woken_after_driver_is_dropped() {
        let parked: ParkedConsumer = Arc::new(Mutex::new(None));

        let shared = Promise::<i32, E>::lazy(StrandProbe {
            parked: parked.clone(),
            polled: false,
        })
        .shared();

        let late_waker = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });

        {
            let mut slot = parked.lock().expect("register late consumer");

            *slot = Some((shared.clone(), Waker::from(late_waker.clone())));
        }

        // Drive once: the inner promise resolves within this single poll. Drop
        // the driver afterwards to show the late consumer's wake does not depend
        // on the driver surviving.
        let driver_waker = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let mut driver = shared;

        assert!(poll_with(&mut driver, &Waker::from(driver_waker)).is_ready());

        drop(driver);

        // The late consumer registered after the queue was drained, yet the
        // inline re-drive resolved the inner promise, and the completion fan-out
        // woke it within the driver's single poll. The wake therefore stands
        // even though the driver is now gone.
        assert!(
            late_waker.count.load(Ordering::Relaxed) >= 1,
            "parked consumer must be woken once the inner promise becomes resolvable"
        );
    }

    /// Builds a real task `Waker` backed by a fresh `CountingWaker`, returning the
    /// shared counter alongside the waker so the test can observe wake calls.
    fn counting_waker() -> (Arc<CountingWaker>, Waker) {
        let inner = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });

        let waker = Waker::from(inner.clone());

        (inner, waker)
    }

    /// A single pending consumer parked on its own waker must be woken when the
    /// inner promise resolves, and then poll through to the resolved value.
    #[test]
    fn single_pending_consumer_woken_on_resolve() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let mut shared = promise.shared();

        let (counter, waker) = counting_waker();

        assert_eq!(poll_with(&mut shared, &waker), Poll::Pending);
        assert_eq!(counter.count.load(Ordering::SeqCst), 0);

        resolve.resolve(7);

        assert!(counter.count.load(Ordering::SeqCst) >= 1);
        assert_eq!(poll(&mut shared), Poll::Ready(Ok(7)));
    }

    /// Resolving the inner promise must fan out the wake to EVERY clone that
    /// previously polled while pending, not just the first one.
    #[test]
    fn resolve_fans_out_to_all_pending_consumers() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let base = promise.shared();

        let mut clone_a = base.clone();
        let mut clone_b = base.clone();
        let mut clone_c = base;

        let (counter_a, waker_a) = counting_waker();
        let (counter_b, waker_b) = counting_waker();
        let (counter_c, waker_c) = counting_waker();

        assert_eq!(poll_with(&mut clone_a, &waker_a), Poll::Pending);
        assert_eq!(poll_with(&mut clone_b, &waker_b), Poll::Pending);
        assert_eq!(poll_with(&mut clone_c, &waker_c), Poll::Pending);

        resolve.resolve(42);

        assert!(counter_a.count.load(Ordering::SeqCst) >= 1);
        assert!(counter_b.count.load(Ordering::SeqCst) >= 1);
        assert!(counter_c.count.load(Ordering::SeqCst) >= 1);

        assert_eq!(poll(&mut clone_a), Poll::Ready(Ok(42)));
        assert_eq!(poll(&mut clone_b), Poll::Ready(Ok(42)));
        assert_eq!(poll(&mut clone_c), Poll::Ready(Ok(42)));
    }

    /// Rejecting the inner promise must fan out the wake to every pending clone,
    /// and each clone must observe the rejection error.
    #[test]
    fn reject_fans_out_to_all_pending_consumers() {
        let (promise, _resolve, reject) = Promise::<i32, E>::with_resolvers();

        let base = promise.shared();

        let mut clone_a = base.clone();
        let mut clone_b = base.clone();
        let mut clone_c = base;

        let (counter_a, waker_a) = counting_waker();
        let (counter_b, waker_b) = counting_waker();
        let (counter_c, waker_c) = counting_waker();

        assert_eq!(poll_with(&mut clone_a, &waker_a), Poll::Pending);
        assert_eq!(poll_with(&mut clone_b, &waker_b), Poll::Pending);
        assert_eq!(poll_with(&mut clone_c, &waker_c), Poll::Pending);

        reject.reject(E::Fail);

        assert!(counter_a.count.load(Ordering::SeqCst) >= 1);
        assert!(counter_b.count.load(Ordering::SeqCst) >= 1);
        assert!(counter_c.count.load(Ordering::SeqCst) >= 1);

        assert_eq!(poll(&mut clone_a), Poll::Ready(Err(E::Fail)));
        assert_eq!(poll(&mut clone_b), Poll::Ready(Err(E::Fail)));
        assert_eq!(poll(&mut clone_c), Poll::Ready(Err(E::Fail)));
    }
}
