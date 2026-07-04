use std::fmt::Debug;

use crate::{Promise, State};

impl<T, E> Debug for Promise<T, E>
where
    T: Debug,
    E: Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut fmt = f.debug_struct("Promise");

        match &self.state {
            State::Pending(_) => fmt.field("state", &format_args!("Pending")),
            State::Resolved(value) => fmt.field("resolved", value),
            State::Rejected(err) => fmt.field("rejected", err),
            State::Consumed => fmt.field("state", &format_args!("Consumed")),
            State::Failed(failure) => fmt.field("failed", failure),
        };

        fmt.finish()
    }
}
