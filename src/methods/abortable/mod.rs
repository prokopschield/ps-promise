mod aborted;
mod handle;
mod outcome;
mod promise;

use promise::AbortablePromise;

use crate::{Promise, PromiseRejection};

pub use aborted::Aborted;
pub use handle::AbortHandle;
pub use outcome::{PromiseAborted, PromiseSettled};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Wraps this [`Promise`] with an external abort handle.
    ///
    /// Returns the wrapped promise alongside an [`AbortHandle`]. Aborting is a
    /// suggestion, not a command: it takes effect only while the promise is
    /// still pending and is observed on a subsequent poll. When an abort is
    /// visible on a poll it takes precedence, so the promise rejects even if
    /// the underlying promise is simultaneously ready; but an abort that
    /// arrives after the promise has already settled simply has no effect. On
    /// abort the promise rejects with [`Aborted`] wrapped in
    /// [`TaskFailure::Error`](crate::TaskFailure::Error), mapped through
    /// [`PromiseRejection::task_failed`]. The handle is clonable, so any clone
    /// may abort, and [`AbortHandle::abort`]'s return value is only a hint, as
    /// described on that method. Dropping every handle without aborting has no
    /// effect: the underlying promise simply runs to completion.
    ///
    /// # Cancellation
    ///
    /// Aborting drops the underlying future, which does not preempt running
    /// code. What an abort actually stops therefore depends on how
    /// the underlying promise is driven:
    ///
    /// - A poll-driven promise (the default, such as [`Promise::lazy`]) makes
    ///   progress only while polled, so dropping it halts the work at its last
    ///   `.await`. This is genuine cancellation.
    /// - An `eager_with_smol` promise holds a cancel-on-drop `smol::Task`, so
    ///   its spawned future is cancelled.
    /// - An `eager_with_tokio` promise holds a detached `tokio` task. Aborting
    ///   abandons the result, but the task runs to completion; the work is not
    ///   stopped.
    /// - [`Promise::unblock`] work runs a blocking closure on a thread pool. A
    ///   closure that has already started cannot be interrupted, so aborting
    ///   abandons the result while the closure runs to completion.
    pub fn abortable(self) -> (Self, AbortHandle) {
        let (sender, receiver) = async_channel::bounded(1);

        let promise = AbortablePromise::new(self, receiver).into();
        let handle = AbortHandle { sender };

        (promise, handle)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        task::{Context, Poll, Wake, Waker},
    };

    use super::{Aborted, PromiseAborted, PromiseSettled};
    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug, PartialEq)]
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

    fn cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    fn pending_promise() -> Promise<i32, E> {
        Promise::<i32, E>::lazy(std::future::pending::<Result<i32, E>>())
    }

    /// Counts how many times it is woken, so a test can observe that an abort
    /// arriving while the promise is pending wakes the parked task.
    struct CountingWaker(AtomicUsize);

    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.wake_by_ref();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Sets a shared flag when dropped, so a test can observe whether the
    /// future holding it was dropped.
    struct DropFlag(Arc<AtomicBool>);

    impl Drop for DropFlag {
        fn drop(&mut self) {
            self.0.store(true, Ordering::SeqCst);
        }
    }

    /// A future that never resolves and carries a [`DropFlag`], standing in
    /// for in-flight work that should be cancelled on abort.
    struct NeverReady {
        _flag: DropFlag,
    }

    impl Future for NeverReady {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Pending
        }
    }

    #[test]
    fn abort_rejects_pending_promise() {
        let (mut promise, handle) = pending_promise().abortable();

        assert!(promise.pending(&mut cx()));

        assert_eq!(handle.abort(), Ok(PromiseAborted));

        assert!(promise.ready(&mut cx()));
        assert_eq!(promise.consume(), Some(Err(E::TaskFailed)));
    }

    /// A redundant abort, before the first one is observed, still reports the
    /// promise as aborted: a still-queued request reads as live.
    #[test]
    fn redundant_abort_still_reports_aborted() {
        let (mut promise, handle) = pending_promise().abortable();

        assert_eq!(handle.abort(), Ok(PromiseAborted));
        assert_eq!(handle.abort(), Ok(PromiseAborted));

        assert!(promise.ready(&mut cx()));
        assert_eq!(promise.consume(), Some(Err(E::TaskFailed)));
    }

    #[test]
    fn resolves_when_not_aborted() {
        let (mut promise, _handle) = Promise::<i32, E>::resolve(42).abortable();

        assert!(promise.ready(&mut cx()));
        assert_eq!(promise.consume(), Some(Ok(42)));
    }

    #[test]
    fn dropping_handle_leaves_pending_promise_pending() {
        let (mut promise, handle) = pending_promise().abortable();

        drop(handle);

        assert!(promise.pending(&mut cx()));
        assert!(promise.pending(&mut cx()));
    }

    #[test]
    fn clone_can_abort_after_original_is_dropped() {
        let (mut promise, handle) = pending_promise().abortable();

        let clone = handle.clone();

        drop(handle);

        assert_eq!(clone.abort(), Ok(PromiseAborted));

        assert!(promise.ready(&mut cx()));
        assert_eq!(promise.consume(), Some(Err(E::TaskFailed)));
    }

    /// Once the underlying promise has settled and been observed, a later
    /// abort is too late: it reports [`PromiseSettled`] and the resolved value
    /// stands.
    #[test]
    fn settlement_wins_over_later_abort() {
        let (mut promise, handle) = Promise::<i32, E>::resolve(5).abortable();

        assert!(promise.ready(&mut cx()));

        assert_eq!(handle.abort(), Err(PromiseSettled));

        assert_eq!(promise.consume(), Some(Ok(5)));
    }

    /// A pending abort takes precedence over an underlying promise that is
    /// already resolvable: aborting before the first poll rejects, rather than
    /// surfacing the ready value. This is the imperative `AbortController`
    /// semantic, where `abort` commands cancellation rather than racing.
    #[test]
    fn abort_wins_over_ready_inner() {
        let (mut promise, handle) = Promise::<i32, E>::resolve(5).abortable();

        assert_eq!(handle.abort(), Ok(PromiseAborted));

        assert!(promise.ready(&mut cx()));
        assert_eq!(promise.consume(), Some(Err(E::TaskFailed)));
    }

    /// Aborting drops the underlying future, cancelling poll-driven work.
    #[test]
    fn abort_drops_the_underlying_future() {
        let dropped = Arc::new(AtomicBool::new(false));

        let inner = Promise::<i32, E>::lazy(NeverReady {
            _flag: DropFlag(dropped.clone()),
        });

        let (mut promise, handle) = inner.abortable();

        assert!(promise.pending(&mut cx()));
        assert!(!dropped.load(Ordering::SeqCst));

        assert_eq!(handle.abort(), Ok(PromiseAborted));
        assert!(promise.ready(&mut cx()));

        assert!(
            dropped.load(Ordering::SeqCst),
            "aborting must drop the underlying future"
        );
    }

    #[test]
    fn aborted_error_message() {
        assert_eq!(Aborted.to_string(), "Promise was aborted.");
    }

    /// An abort arriving while the promise is pending wakes the parked task,
    /// so a real executor would re-poll and observe the rejection.
    #[test]
    fn abort_wakes_a_pending_promise() {
        let waker = Arc::new(CountingWaker(AtomicUsize::new(0)));
        let raw = Waker::from(waker.clone());
        let mut cx = Context::from_waker(&raw);

        let (mut promise, handle) = pending_promise().abortable();

        assert!(promise.pending(&mut cx));
        assert_eq!(waker.0.load(Ordering::SeqCst), 0);

        assert_eq!(handle.abort(), Ok(PromiseAborted));

        assert!(
            waker.0.load(Ordering::SeqCst) >= 1,
            "aborting a pending promise must wake the parked task"
        );

        assert!(promise.ready(&mut cx));
        assert_eq!(promise.consume(), Some(Err(E::TaskFailed)));
    }

    /// Once an abort has been observed and the promise has rejected, a later
    /// abort is too late: the promise is no longer abortable.
    #[test]
    fn abort_after_rejection_reports_settled() {
        let (mut promise, handle) = pending_promise().abortable();

        assert_eq!(handle.abort(), Ok(PromiseAborted));
        assert!(promise.ready(&mut cx()));
        assert_eq!(promise.consume(), Some(Err(E::TaskFailed)));
        assert_eq!(handle.abort(), Err(PromiseSettled));
    }

    /// Dropping the wrapped promise leaves nothing to abort, so a later abort
    /// reports the promise as settled.
    #[test]
    fn abort_after_dropping_the_promise_reports_settled() {
        let (promise, handle) = pending_promise().abortable();

        drop(promise);

        assert_eq!(handle.abort(), Err(PromiseSettled));
    }
}
