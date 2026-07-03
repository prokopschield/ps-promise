use crate::{Promise, State};

impl<T, E> Promise<T, E> {
    pub const fn is_pending(&self) -> bool {
        matches!(self.state, State::Pending(_))
    }
}
