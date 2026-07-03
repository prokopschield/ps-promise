use crate::{Promise, PromiseRejection, State};

impl<T, E> Promise<T, E>
where
    E: PromiseRejection,
{
    /// Creates a [`Promise`] already resolved with `value`.
    ///
    /// Mirrors ECMAScript's `Promise.resolve`.
    pub const fn resolve(value: T) -> Self {
        Self {
            state: State::Resolved(value),
        }
    }
}
