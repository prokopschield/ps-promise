use crate::Promise;

impl<T, E> Promise<T, E> {
    pub const fn resolve(value: T) -> Self {
        Self::Resolved(value)
    }
}
