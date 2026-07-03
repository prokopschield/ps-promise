use std::{
    pin::Pin,
    sync::{atomic::Ordering, PoisonError},
    task::Context,
};

use crate::PromiseRejection;

use super::super::super::waker::SharedWaker;
use super::super::SharedPromise;

pub(in crate::methods::shared) enum PollStep<T, E>
where
    T: Clone + Send + 'static,
    E: PromiseRejection + Clone,
{
    /// Equvalent to `Poll::Pending`
    Pending,
    /// Equivalent to `Poll::Ready(Ok(T))`
    Resolved(T),
    /// Equivalent to `Poll::Ready(Err(E))`
    Rejected(E),
    /// Equivalent to `Poll::Ready(Err(E::already_consumed()))`
    Consumed,
    /// Re-enter this Promise now, ready to be resolved.
    ReEnter,
    /// The underlying Promise woke itself up.
    Woke,
}

impl<T, E> SharedPromise<T, E>
where
    T: Clone + Send + 'static,
    E: PromiseRejection + Clone,
{
    pub(in crate::methods::shared) fn poll_step(
        self: &Pin<&mut Self>,
        cx: &Context<'_>,
    ) -> PollStep<T, E> {
        // critical section: check for inner promise presence
        // lock ordering: always acquire inner promise lock first
        let mut guard = self
            .state
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        if let Some(ref mut inner) = &mut *guard {
            if let Some(result) = inner.peek() {
                // Promise is resolved or rejected -> clone value or error
                return result.map_or_else(
                    |err| PollStep::Rejected(err.clone()),
                    |value| PollStep::Resolved(value.clone()),
                );
            } else if inner.is_consumed() {
                // Promise is already consumed (invalid state) -> reject
                return PollStep::Consumed;

                // Path is reachable:
                // let mut p = Promise::resolve(123);
                // p.consume();
                // p.share().poll(cx);
            } else if inner.is_failed() {
                // Task failures are repeat-consumable: consuming converts the
                // stored failure through E::task_failed while leaving it in
                // place, so every waiter receives its own rejection.
                if let Some(Err(err)) = inner.consume() {
                    return PollStep::Rejected(err);
                }
            }
        } else {
            // another executor thread is in the process of executing the underlying Promise
            // or recursion has occured
            // either way, keep the waker and bail!
            self.state.add_waker(self.waiter_id, cx.waker());

            return PollStep::Pending;
        }

        // Register this consumer; whichever consumer completes the inner promise wakes all of the others.
        // Lock ordering: inner promise lock is held across this section.
        self.state.add_waker(self.waiter_id, cx.waker());

        // Now we know we have to actually poll the underlying Promise to continue.
        let Some(mut inner) = guard.take() else {
            // We just checked this, so this code path is genuinely unreachable.
            // This handling is only replicated here for completeness.
            // Waker is already registered in the prior statement.
            return PollStep::Pending;
        };

        // Exit critical section – other pollers bail and return Pending.
        drop(guard);

        // Poll the inner promise with a waker that fans out to every registered consumer,
        // so that progress is observed even if the current poller goes away.
        let shared_waker = SharedWaker::new_waker(&self.state);
        let mut shared_cx = Context::from_waker(&shared_waker);

        // About to call inner promise, clear the woke flag.
        self.state.woke.store(false, Ordering::Relaxed);

        // Call inner Promise (no lock is held)
        let is_ready = inner.ready(&mut shared_cx);

        // Critical section: re-acquire inner promise lock
        let mut guard = self
            .state
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner);

        // place promise back into its container
        guard.replace(inner);

        if !is_ready {
            return if self.state.woke.load(Ordering::Relaxed) {
                // Waker fired during or after poll of underlying Promise.
                PollStep::Woke
            } else {
                // There is genuinely no value ready.
                PollStep::Pending
            };
        }

        // exit critical section
        drop(guard);

        // wake every remaining waker
        // lock ordering: we're only acquiring one lock, and the promise is already in place
        shared_waker.wake();

        PollStep::ReEnter
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

    /// Inner future that, on its first poll, re-enters [`SharedPromise::poll`]
    /// by polling each registered clone with its own independent waker, then
    /// resolves. Because the driver polls the inner promise *outside* the
    /// `inner` lock, these re-entrant polls observe the taken slot and park
    /// instead of deadlocking on the non-reentrant mutex.
    type SharedClones = Arc<Mutex<Vec<(SharedPromise<i32, E>, Waker)>>>;
    type SharedOutcomes = Arc<Mutex<Vec<Poll<Result<i32, E>>>>>;

    struct ReentrantProbe {
        clones: SharedClones,
        outcomes: SharedOutcomes,
        polled: bool,
    }

    impl Future for ReentrantProbe {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();

            if !this.polled {
                this.polled = true;

                let mut clones = this.clones.lock().expect("probe clones");
                let mut outcomes = this.outcomes.lock().expect("probe outcomes");

                for (clone, waker) in clones.iter_mut() {
                    let mut probe_cx = Context::from_waker(waker);

                    outcomes.push(Pin::new(clone).poll(&mut probe_cx));
                }
            }

            Poll::Ready(Ok(7))
        }
    }

    /// Inner future that yields once: on its first poll it wakes its own
    /// waker (the `shared_waker` the driver passes in) and returns `Pending`,
    /// then resolves on the second poll. This is the `tokio::task::yield_now`
    /// pattern.
    struct YieldOnce {
        yielded: bool,
    }

    impl Future for YieldOnce {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();

            if this.yielded {
                return Poll::Ready(Ok(42));
            }

            this.yielded = true;
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    #[test]
    fn shared_waker_fires_during_a_drive_that_returns_pending() {
        let mut shared = Promise::<i32, E>::lazy(YieldOnce { yielded: false }).shared();
        let waker = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let task_waker = Waker::from(waker.clone());

        // First poll: the driver registers `task_waker`, takes the inner promise
        // out, and polls it. `YieldOnce` wakes `shared_waker` *during* that poll
        // (fanning out to `task_waker`) and returns `Pending`. So when the driver
        // re-checks, `is_ready == false` even though `shared_waker` already fired
        // within this single drive: exactly the `!is_ready && woken-during-drive`
        // state, reached deterministically on one thread.
        let first = poll_with(&mut shared, &task_waker);

        assert!(first.is_ready());
        assert_eq!(
            waker.count.load(Ordering::Relaxed),
            2,
            "shared_waker must have fired during the pending drive"
        );

        // The inner promise is ready now; the next poll observes it.
        assert_eq!(poll(&mut shared), Poll::Ready(Ok(42)));
    }

    #[test]
    fn reentrant_consumers_park_without_deadlock_and_are_all_woken() {
        let clones: SharedClones = Arc::new(Mutex::new(Vec::new()));
        let outcomes = Arc::new(Mutex::new(Vec::new()));

        let shared = Promise::<i32, E>::lazy(ReentrantProbe {
            clones: clones.clone(),
            outcomes: outcomes.clone(),
            polled: false,
        })
        .shared();

        let wakers: Vec<Arc<CountingWaker>> = (0..2)
            .map(|_| {
                Arc::new(CountingWaker {
                    count: AtomicUsize::new(0),
                })
            })
            .collect();

        {
            let mut registered = clones.lock().expect("register clones");

            for waker in &wakers {
                registered.push((shared.clone(), Waker::from(waker.clone())));
            }
        }

        // Driving polls the inner future outside the `inner` lock; that future
        // re-enters by polling both clones while the slot is taken. If the poll
        // happened under the lock, these re-entrant `inner.lock()` calls would
        // deadlock the thread and this test would hang.
        let mut driver = shared.clone();

        assert_eq!(poll(&mut driver), Poll::Ready(Ok(7)));

        // Both re-entrant polls saw the in-flight drive and parked.
        let (parked_count, all_parked) = {
            let outcomes = outcomes.lock().expect("read outcomes");

            (outcomes.len(), outcomes.iter().all(Poll::is_pending))
        };

        assert_eq!(parked_count, 2);
        assert!(all_parked);

        // Completion fanned out a wake to *every* parked consumer. This
        // regresses to zero wakes if waker registration ever drops all but the
        // first registration.
        for waker in &wakers {
            assert_eq!(waker.count.load(Ordering::Relaxed), 1);
        }

        // A fresh poll observes the settled result.
        let mut latecomer = shared;

        assert_eq!(poll(&mut latecomer), Poll::Ready(Ok(7)));
    }

    #[test]
    fn shared_resolved_polls_to_ready_ok() {
        let mut shared = Promise::<i32, E>::resolve(7).shared();

        assert_eq!(poll(&mut shared), Poll::Ready(Ok(7)));
    }

    #[test]
    fn shared_rejected_polls_to_ready_err() {
        let mut shared = Promise::<i32, E>::lazy(async { Err(E::Fail) }).shared();

        assert_eq!(poll(&mut shared), Poll::Ready(Err(E::Fail)));
    }

    #[test]
    fn shared_consumed_polls_to_already_consumed() {
        let mut promise = Promise::<i32, E>::resolve(7);

        let taken = promise.consume();

        assert_eq!(taken, Some(Ok(7)));

        let mut shared = promise.shared();

        assert_eq!(poll(&mut shared), Poll::Ready(Err(E::AlreadyConsumed)));
    }

    #[test]
    fn shared_pending_then_resolves() {
        let (promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let mut shared = promise.shared();

        assert_eq!(poll(&mut shared), Poll::Pending);

        resolve.resolve(42);

        assert_eq!(poll(&mut shared), Poll::Ready(Ok(42)));
    }

    /// A future that, on its first poll, wakes its own waker and returns `Pending`,
    /// then resolves on the next poll. Exercises the bounded self-wake re-entry path.
    struct SelfWaker {
        polled: bool,
        value: i32,
    }

    impl Future for SelfWaker {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let this = self.get_mut();

            if this.polled {
                return Poll::Ready(Ok(this.value));
            }

            this.polled = true;
            cx.waker().wake_by_ref();

            Poll::Pending
        }
    }

    #[test]
    fn shared_self_waking_inner_drives_to_ready() {
        let inner = SelfWaker {
            polled: false,
            value: 99,
        };

        let mut shared = Promise::<i32, E>::lazy(inner).shared();

        assert_eq!(poll(&mut shared), Poll::Ready(Ok(99)));
    }
}
