use std::{
    fmt::Debug,
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex, PoisonError},
    task::{Context, Poll, Waker},
};

use crate::{Promise, PromiseRejection, TaskFailure};

/// Rejection payload produced when every handle returned by
/// `Promise::with_resolvers` is dropped without settling the promise.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
#[error("all resolver handles were dropped without settling the promise")]
pub struct ResolversDropped;

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Creates a pending [`Promise`] along with handles that settle it.
    ///
    /// Mirrors ECMAScript's `Promise.withResolvers`: the promise stays
    /// pending until [`Resolve::resolve`] or [`Reject::reject`] is called,
    /// and the first settlement wins. Both handles are clonable, and any
    /// clone may settle the promise. If every handle, including clones, is
    /// dropped without settling, the promise rejects with
    /// [`ResolversDropped`] wrapped in [`TaskFailure::Error`], mapped
    /// through [`PromiseRejection::task_failed`].
    #[must_use = "Dropping the handles rejects the Promise!"]
    pub fn with_resolvers() -> (Self, Resolve<T, E>, Reject<T, E>) {
        let slot = Arc::new(Mutex::new(Slot {
            settled: false,
            outcome: None,
            waker: None,
        }));

        let guard = Arc::new(HandleGuard { slot: slot.clone() });

        (
            Self::lazy(SlotFuture { slot }),
            Resolve {
                guard: guard.clone(),
            },
            Reject { guard },
        )
    }
}

/// Resolves the [`Promise`] created by `Promise::with_resolvers`.
///
/// Cloning yields another handle to the same promise; any clone may settle
/// it, and the first settlement wins.
pub struct Resolve<T, E>
where
    E: PromiseRejection,
{
    guard: Arc<HandleGuard<T, E>>,
}

impl<T, E> Clone for Resolve<T, E>
where
    E: PromiseRejection,
{
    fn clone(&self) -> Self {
        Self {
            guard: self.guard.clone(),
        }
    }
}

impl<T, E> Debug for Resolve<T, E>
where
    E: PromiseRejection,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resolve").finish_non_exhaustive()
    }
}

impl<T, E> Resolve<T, E>
where
    E: PromiseRejection,
{
    /// Settles the associated [`Promise`] with `value`.
    ///
    /// Ignored if the promise is already settled.
    pub fn resolve(self, value: T) {
        settle(&self.guard.slot, Ok(value));
    }
}

/// Rejects the [`Promise`] created by `Promise::with_resolvers`.
///
/// Cloning yields another handle to the same promise; any clone may settle
/// it, and the first settlement wins.
pub struct Reject<T, E>
where
    E: PromiseRejection,
{
    guard: Arc<HandleGuard<T, E>>,
}

impl<T, E> Clone for Reject<T, E>
where
    E: PromiseRejection,
{
    fn clone(&self) -> Self {
        Self {
            guard: self.guard.clone(),
        }
    }
}

impl<T, E> Debug for Reject<T, E>
where
    E: PromiseRejection,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reject").finish_non_exhaustive()
    }
}

impl<T, E> Reject<T, E>
where
    E: PromiseRejection,
{
    /// Settles the associated [`Promise`] with `err`.
    ///
    /// Ignored if the promise is already settled.
    pub fn reject(self, err: E) {
        settle(&self.guard.slot, Err(err));
    }
}

struct Slot<T, E> {
    settled: bool,
    outcome: Option<Result<T, E>>,
    waker: Option<Waker>,
}

type SharedSlot<T, E> = Arc<Mutex<Slot<T, E>>>;

/// Stores `outcome` into `slot` and wakes the consumer, unless a prior
/// settlement already won.
fn settle<T, E>(slot: &SharedSlot<T, E>, outcome: Result<T, E>) {
    let mut guard = slot.lock().unwrap_or_else(PoisonError::into_inner);

    if !guard.settled {
        guard.settled = true;
        guard.outcome = Some(outcome);

        if let Some(waker) = guard.waker.take() {
            drop(guard);

            waker.wake();
        }
    }
}

/// Shared by every settlement handle; when the last one is dropped, rejects
/// the promise with [`ResolversDropped`] unless it has already been settled.
struct HandleGuard<T, E>
where
    E: PromiseRejection,
{
    slot: SharedSlot<T, E>,
}

impl<T, E> Drop for HandleGuard<T, E>
where
    E: PromiseRejection,
{
    fn drop(&mut self) {
        settle(
            &self.slot,
            Err(E::task_failed(TaskFailure::Error(Arc::new(
                ResolversDropped,
            )))),
        );
    }
}

struct SlotFuture<T, E> {
    slot: SharedSlot<T, E>,
}

impl<T, E> Future for SlotFuture<T, E>
where
    E: PromiseRejection,
{
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let Ok(mut guard) = self.slot.lock() else {
            return Poll::Ready(Err(E::task_failed(TaskFailure::Panic(
                "a panic corrupted the promise's settlement state".into(),
            ))));
        };

        if !guard.settled {
            guard.waker = Some(cx.waker().clone());

            return Poll::Pending;
        }

        Poll::Ready(
            guard
                .outcome
                .take()
                .unwrap_or_else(|| Err(E::already_consumed())),
        )
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
        task::{Context, Poll, Wake, Waker},
        thread,
    };

    use crate::{Promise, PromiseRejection, TaskFailure};

    use super::{settle, ResolversDropped, SharedSlot, Slot, SlotFuture};

    const POISONED_MSG: &str = "task panicked: a panic corrupted the promise's settlement state";

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

    /// The rejection produced when every settlement handle is dropped.
    fn resolvers_dropped() -> E {
        E::TaskFailed(ResolversDropped.to_string())
    }

    fn cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    fn poll_future<F: Future + Unpin>(future: &mut F) -> Poll<F::Output> {
        Pin::new(future).poll(&mut cx())
    }

    fn empty_slot() -> SharedSlot<i32, E> {
        Arc::new(Mutex::new(Slot {
            settled: false,
            outcome: None,
            waker: None,
        }))
    }

    /// Poisons `slot` by panicking on a helper thread while the lock is held.
    fn poison(slot: &SharedSlot<i32, E>) {
        let poisoner = slot.clone();

        thread::spawn(move || {
            let _guard = poisoner.lock().expect("first lock of a fresh slot");

            panic!("poison the slot");
        })
        .join()
        .expect_err("poisoning thread must panic");
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
        let counter = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });

        let waker = Waker::from(counter.clone());

        (counter, waker)
    }

    #[test]
    fn resolve_settles_promise() {
        let (mut promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        resolve.resolve(42);
        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(42)));
    }

    #[test]
    fn reject_settles_promise() {
        let (mut promise, _resolve, reject) = Promise::<i32, E>::with_resolvers();

        reject.reject(E::Fail);
        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Err(E::Fail)));
    }

    #[test]
    fn pending_until_settled() {
        let (mut promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        assert!(promise.poll_pending(&mut cx()));

        resolve.resolve(7);

        assert!(promise.poll_settled(&mut cx()));
        assert_eq!(promise.consume(), Some(Ok(7)));
    }

    #[test]
    fn first_settlement_wins() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        resolve.resolve(1);
        reject.reject(E::Fail);
        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(1)));
    }

    #[test]
    fn rejects_when_handles_dropped() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        drop(resolve);
        drop(reject);
        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Err(resolvers_dropped())));
    }

    #[test]
    fn settles_even_if_other_handle_dropped_later() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        resolve.resolve(9);
        drop(reject);
        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(9)));
    }

    #[test]
    fn clone_can_settle_after_originals_are_dropped() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        let clone = resolve.clone();

        drop(resolve);
        drop(reject);

        clone.resolve(3);
        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(3)));
    }

    #[test]
    fn first_settlement_wins_across_clones() {
        let (mut promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let clone = resolve.clone();

        clone.resolve(1);
        resolve.resolve(2);
        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(1)));
    }

    #[test]
    fn live_clone_keeps_the_promise_pending() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        let clone = reject.clone();

        drop(resolve);
        drop(reject);

        assert!(promise.poll_pending(&mut cx()));

        clone.reject(E::Fail);
        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Err(E::Fail)));
    }

    #[test]
    fn rejects_only_when_every_clone_is_dropped() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        let resolve_clone = resolve.clone();
        let reject_clone = reject.clone();

        drop(resolve);
        drop(reject);

        assert!(promise.poll_pending(&mut cx()));

        drop(resolve_clone);
        drop(reject_clone);
        promise.poll_settled(&mut cx());

        assert_eq!(promise.consume(), Some(Err(resolvers_dropped())));
    }

    #[test]
    fn debug_output_names_the_handles() {
        let (_promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        assert_eq!(format!("{resolve:?}"), "Resolve { .. }");
        assert_eq!(format!("{reject:?}"), "Reject { .. }");
    }

    #[test]
    fn resolve_wakes_pending_consumer() {
        let (mut promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let (counter, waker) = counting_waker();

        assert!(promise.poll_pending(&mut Context::from_waker(&waker)));
        assert_eq!(counter.count.load(Ordering::SeqCst), 0);

        resolve.resolve(8);

        assert_eq!(counter.count.load(Ordering::SeqCst), 1);

        assert!(promise.poll_settled(&mut cx()));
        assert_eq!(promise.consume(), Some(Ok(8)));
    }

    #[test]
    fn reject_wakes_pending_consumer() {
        let (mut promise, _resolve, reject) = Promise::<i32, E>::with_resolvers();

        let (counter, waker) = counting_waker();

        assert!(promise.poll_pending(&mut Context::from_waker(&waker)));
        assert_eq!(counter.count.load(Ordering::SeqCst), 0);

        reject.reject(E::Fail);

        assert_eq!(counter.count.load(Ordering::SeqCst), 1);

        assert!(promise.poll_settled(&mut cx()));
        assert_eq!(promise.consume(), Some(Err(E::Fail)));
    }

    #[test]
    fn dropping_last_handle_wakes_pending_consumer() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        let (counter, waker) = counting_waker();

        assert!(promise.poll_pending(&mut Context::from_waker(&waker)));

        drop(resolve);

        assert_eq!(counter.count.load(Ordering::SeqCst), 0);

        drop(reject);

        assert_eq!(counter.count.load(Ordering::SeqCst), 1);

        assert!(promise.poll_settled(&mut cx()));
        assert_eq!(promise.consume(), Some(Err(resolvers_dropped())));
    }

    /// Only the waker from the most recent poll is woken; a re-poll with a
    /// different waker replaces the prior registration, which is all the
    /// `Future` contract requires.
    #[test]
    fn repoll_replaces_the_stored_waker() {
        let (mut promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let (counter_a, waker_a) = counting_waker();
        let (counter_b, waker_b) = counting_waker();

        assert!(promise.poll_pending(&mut Context::from_waker(&waker_a)));
        assert!(promise.poll_pending(&mut Context::from_waker(&waker_b)));

        resolve.resolve(4);

        assert_eq!(counter_a.count.load(Ordering::SeqCst), 0);
        assert_eq!(counter_b.count.load(Ordering::SeqCst), 1);

        assert!(promise.poll_settled(&mut cx()));
        assert_eq!(promise.consume(), Some(Ok(4)));
    }

    #[test]
    fn settlement_from_another_thread_wakes_and_delivers() {
        let (mut promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let (counter, waker) = counting_waker();

        assert!(promise.poll_pending(&mut Context::from_waker(&waker)));

        thread::spawn(move || resolve.resolve(21))
            .join()
            .expect("settler thread");

        assert!(counter.count.load(Ordering::SeqCst) >= 1);

        assert!(promise.poll_settled(&mut cx()));
        assert_eq!(promise.consume(), Some(Ok(21)));
    }

    #[test]
    fn settling_after_promise_dropped_is_ignored() {
        let (promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        drop(promise);

        resolve.resolve(1);
        reject.reject(E::Fail);
    }

    #[test]
    fn dropping_handles_after_promise_dropped_is_ignored() {
        let (promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        drop(promise);
        drop(resolve);
        drop(reject);
    }

    #[test]
    fn settle_before_first_poll_delivers_immediately() {
        let slot = empty_slot();

        settle(&slot, Ok(11));

        let mut future = SlotFuture { slot };

        assert_eq!(poll_future(&mut future), Poll::Ready(Ok(11)));
    }

    /// Regression test: polling the settlement future again after it has
    /// delivered its outcome must report `already_consumed`, not hang as
    /// `Pending`.
    #[test]
    fn repoll_after_delivery_reports_already_consumed() {
        let slot = empty_slot();

        let mut future = SlotFuture { slot: slot.clone() };

        assert_eq!(poll_future(&mut future), Poll::Pending);

        settle(&slot, Ok(5));

        assert_eq!(poll_future(&mut future), Poll::Ready(Ok(5)));
        assert_eq!(
            poll_future(&mut future),
            Poll::Ready(Err(E::AlreadyConsumed))
        );
    }

    /// A settlement arriving after the outcome was delivered must not
    /// resurrect the future with a new outcome.
    #[test]
    fn settlement_after_delivery_is_ignored() {
        let slot = empty_slot();

        let mut future = SlotFuture { slot: slot.clone() };

        settle(&slot, Ok(1));

        assert_eq!(poll_future(&mut future), Poll::Ready(Ok(1)));

        settle(&slot, Ok(2));

        assert_eq!(
            poll_future(&mut future),
            Poll::Ready(Err(E::AlreadyConsumed))
        );
    }

    #[test]
    fn poisoned_slot_rejects_with_panic_failure() {
        let slot = empty_slot();

        poison(&slot);

        let mut future = SlotFuture { slot };

        assert_eq!(
            poll_future(&mut future),
            Poll::Ready(Err(E::TaskFailed(POISONED_MSG.into())))
        );
    }

    /// Pins the chosen conservative semantics: once the slot is poisoned, the
    /// consumer rejects even though a real outcome was stored before the
    /// poisoning panic.
    #[test]
    fn poisoned_slot_rejects_even_after_settlement() {
        let slot = empty_slot();

        settle(&slot, Ok(9));
        poison(&slot);

        let mut future = SlotFuture { slot };

        assert_eq!(
            poll_future(&mut future),
            Poll::Ready(Err(E::TaskFailed(POISONED_MSG.into())))
        );
    }

    /// The producer side ignores poisoning, so settling a poisoned slot is a
    /// quiet no-op rather than a second panic.
    #[test]
    fn settling_a_poisoned_slot_does_not_panic() {
        let slot = empty_slot();

        poison(&slot);

        settle(&slot, Ok(3));
    }

    #[test]
    fn resolvers_dropped_error_message() {
        assert_eq!(
            ResolversDropped.to_string(),
            "all resolver handles were dropped without settling the promise"
        );
    }
}
