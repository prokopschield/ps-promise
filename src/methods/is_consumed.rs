use crate::{Promise, State};

impl<T, E> Promise<T, E> {
    pub const fn is_consumed(&self) -> bool {
        matches!(self.state, State::Consumed)
    }
}
