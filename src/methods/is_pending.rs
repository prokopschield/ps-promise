use crate::{Promise, State};

impl<T, E> Promise<T, E> {
    /// Returns `true` if the promise has not yet settled.
    pub const fn is_pending(&self) -> bool {
        matches!(self.state, State::Pending(_))
    }
}
