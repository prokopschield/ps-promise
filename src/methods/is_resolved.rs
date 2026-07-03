use crate::Promise;

impl<T, E> Promise<T, E> {
    pub const fn is_resolved(&self) -> bool {
        matches!(self, Self::Resolved(_))
    }
}
