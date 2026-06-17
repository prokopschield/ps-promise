use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    /// Runs a blocking closure on a thread pool and wraps the result in a
    /// [`Promise`].
    ///
    /// Dispatch is selected at compile time based on which runtime features
    /// are enabled, with a runtime check when `tokio` is on:
    ///
    /// - `tokio` enabled and called from within a tokio runtime context
    ///   (detected via `tokio::runtime::Handle::try_current`): dispatches to
    ///   `tokio::task::spawn_blocking`; a panic in the closure is mapped to
    ///   [`TaskFailure::Panic`](crate::TaskFailure::Panic), and any other
    ///   `JoinError` (runtime shutdown, cancellation) to
    ///   [`TaskFailure::Error`](crate::TaskFailure::Error).
    /// - Otherwise, with `smol` enabled: dispatches to `smol::unblock`.
    /// - Otherwise: dispatches to `blocking::unblock`, which uses the
    ///   `blocking` crate's runtime-independent thread pool.
    ///
    /// The closure is scheduled synchronously during this call; the outer
    /// [`Promise`] must still be polled (or awaited) to receive the outcome.
    pub fn unblock<F>(f: F) -> Self
    where
        F: FnOnce() -> Result<T, E> + Send + 'static,
    {
        #[cfg(feature = "tokio")]
        if tokio::runtime::Handle::try_current().is_ok() {
            let handle = tokio::task::spawn_blocking(f);

            return Self::lazy(async move {
                match handle.await {
                    Ok(inner) => inner,
                    Err(join_err) => Err(E::task_failed(crate::TaskFailure::from(join_err))),
                }
            });
        }

        #[cfg(feature = "smol")]
        return Self::lazy(smol::unblock(f));

        #[cfg(not(feature = "smol"))]
        return Self::lazy(blocking::unblock(f));
    }
}
