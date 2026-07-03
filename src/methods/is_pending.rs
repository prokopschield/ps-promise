use crate::Promise;

impl<T, E> Promise<T, E> {
    pub const fn is_pending(&self) -> bool {
        matches!(self, Self::Pending(_))
    }
}
