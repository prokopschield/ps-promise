use crate::Promise;

impl<T, E> Promise<T, E> {
    pub const fn is_rejected(&self) -> bool {
        matches!(self, Self::Rejected(_))
    }
}
