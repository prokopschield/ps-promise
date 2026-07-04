use crate::Promise;

impl<T, E> Promise<T, E> {
    /// Polls the promise's inner future using a no-op waker.
    /// Returns `true` if the promise is still pending.
    pub fn poll_pending_sync(&mut self) -> bool {
        self.poll_sync();

        self.is_pending()
    }
}
