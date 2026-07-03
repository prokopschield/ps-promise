use std::mem::replace;

use crate::{Promise, PromiseRejection, State};

impl<T, E> Promise<T, E>
where
    E: PromiseRejection,
{
    /// If settled, consumes and returns the result.
    /// Returns `None` if still pending.
    pub fn consume(&mut self) -> Option<Result<T, E>> {
        match replace(&mut self.state, State::Consumed) {
            State::Resolved(val) => Some(Ok(val)),
            State::Rejected(err) => Some(Err(err)),
            State::Consumed => Some(Err(E::already_consumed())),
            other @ State::Pending(_) => {
                self.state = other;
                None
            }
        }
    }
}
