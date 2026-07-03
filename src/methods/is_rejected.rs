use crate::{Promise, State};

impl<T, E> Promise<T, E> {
    /// Returns `true` if the promise settled with a rejection or a task failure.
    ///
    /// After a task failure, [`Promise::peek`] still returns `None`; the
    /// corresponding rejection value is constructed only upon consumption.
    pub const fn is_rejected(&self) -> bool {
        matches!(self.state, State::Rejected(_) | State::Failed(_))
    }
}
