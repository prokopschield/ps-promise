use crate::Promise;

impl<T, E> Promise<T, E> {
    /// Polls the promise's inner future using a no-op waker.
    /// Returns `true` if the promise is now resolved or rejected.
    pub fn ready_sync(&mut self) -> bool {
        self.poll_sync();

        self.is_ready()
    }
}
