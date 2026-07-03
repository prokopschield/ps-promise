use crate::Promise;

impl<T, E> Promise<T, E> {
    pub const fn is_consumed(&self) -> bool {
        matches!(self, Self::Consumed)
    }
}
