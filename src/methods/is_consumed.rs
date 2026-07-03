use crate::{Promise, State};

impl<T, E> Promise<T, E> {
    /// Returns `true` if the settled result has already been consumed.
    ///
    /// A promise is consumed by [`Promise::consume`] or by being awaited
    /// to completion.
    pub const fn is_consumed(&self) -> bool {
        matches!(self.state, State::Consumed)
    }
}
