use crate::Promise;

impl<T, E> Promise<T, E> {
    /// Returns `true` if the promise is no longer pending: it has settled,
    /// or its result has been consumed.
    pub const fn is_ready(&self) -> bool {
        !self.is_pending()
    }
}
