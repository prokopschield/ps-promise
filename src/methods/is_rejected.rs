use crate::{Promise, State};

impl<T, E> Promise<T, E> {
    pub const fn is_rejected(&self) -> bool {
        matches!(self.state, State::Rejected(_))
    }
}
