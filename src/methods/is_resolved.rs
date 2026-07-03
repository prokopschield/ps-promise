use crate::{Promise, State};

impl<T, E> Promise<T, E> {
    pub const fn is_resolved(&self) -> bool {
        matches!(self.state, State::Resolved(_))
    }
}
