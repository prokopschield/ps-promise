use async_channel::TrySendError;

use crate::{AbortHandle, PromiseAborted, PromiseSettled};

impl AbortHandle {
    /// Requests that the associated [`Promise`](crate::Promise) abort.
    ///
    /// Aborting is a suggestion, not a command. The request is lodged
    /// unconditionally, and a promise that is still pending when next polled
    /// rejects with [`Aborted`](crate::Aborted), wrapped in
    /// [`TaskFailure::Error`](crate::TaskFailure::Error) and mapped through
    /// [`PromiseRejection::task_failed`](crate::PromiseRejection::task_failed).
    /// A promise that has already settled is unaffected: the request simply
    /// arrives too late.
    ///
    /// The returned value is a **best-effort hint, not a guarantee.** It is
    /// derived from the channel state alone, with no synchronization spent to
    /// make it precise. Absent a concurrent settlement it is accurate, but a
    /// promise settling on another thread at the same moment may cause this
    /// call to report [`PromiseAborted`] for a promise that goes on to deliver
    /// its own result, or [`PromiseSettled`] for one that still aborts. Use it
    /// as advisory; do not rely on it to decide which outcome occurred.
    ///
    /// # Errors
    ///
    /// Returns [`PromiseSettled`] as a hint that the promise appeared to be no
    /// longer abortable, having settled or been dropped. As above, this is
    /// advisory rather than authoritative.
    pub fn abort(&self) -> Result<PromiseAborted, PromiseSettled> {
        match self.sender.try_send(()) {
            Ok(()) | Err(TrySendError::Full(())) => Ok(PromiseAborted),
            Err(TrySendError::Closed(())) => Err(PromiseSettled),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{AbortHandle, PromiseAborted, PromiseSettled};

    /// Builds an [`AbortHandle`] alongside the live receiver that keeps its
    /// channel open, so a test can drive the channel state directly.
    fn live_handle() -> (AbortHandle, async_channel::Receiver<()>) {
        let (sender, receiver) = async_channel::bounded(1);

        (AbortHandle { sender }, receiver)
    }

    /// A first abort on a live channel reports the promise as aborted.
    #[test]
    fn fresh_request_reports_aborted() {
        let (handle, _receiver) = live_handle();

        assert_eq!(handle.abort(), Ok(PromiseAborted));
    }

    /// A redundant abort, while the prior signal is still queued, also reports
    /// the promise as aborted: a still-queued request reads as live.
    #[test]
    fn pending_request_still_reports_aborted() {
        let (handle, _receiver) = live_handle();

        assert_eq!(handle.abort(), Ok(PromiseAborted));
        assert_eq!(handle.abort(), Ok(PromiseAborted));
    }

    /// Once the receiver is gone, the promise has settled and the abort is an
    /// error.
    #[test]
    fn settled_promise_reports_an_error() {
        let (handle, receiver) = live_handle();

        drop(receiver);

        assert_eq!(handle.abort(), Err(PromiseSettled));
    }

    /// A clone shares the same channel, so aborting through it is observed by
    /// the original and reports the promise as aborted.
    #[test]
    fn clones_target_the_same_signal() {
        let (handle, _receiver) = live_handle();

        let clone = handle.clone();

        assert_eq!(clone.abort(), Ok(PromiseAborted));
        assert_eq!(handle.abort(), Ok(PromiseAborted));
    }
}
