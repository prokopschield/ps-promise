use crate::{Promise, PromiseRejection, State};

impl<T, E> Promise<T, E>
where
    E: PromiseRejection,
{
    /// Creates a [`Promise`] already rejected with `err`.
    ///
    /// Mirrors ECMAScript's `Promise.reject`.
    pub const fn reject(err: E) -> Self {
        Self {
            state: State::Rejected(err),
        }
    }
}
