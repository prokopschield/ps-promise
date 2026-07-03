use crate::{Promise, State};

impl<T, E> Promise<T, E> {
    /// Returns `true` if the promise settled with a value.
    pub const fn is_resolved(&self) -> bool {
        matches!(self.state, State::Resolved(_))
    }
}
