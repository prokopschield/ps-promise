use crate::Promise;

impl<T, E> Promise<T, E> {
    pub const fn is_ready(&self) -> bool {
        !self.is_pending()
    }
}
