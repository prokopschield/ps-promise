use std::fmt::Debug;

use crate::Promise;

impl<T, E> Debug for Promise<T, E>
where
    T: Unpin + Debug,
    E: Unpin + Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut fmt = f.debug_struct("Promise");

        match self {
            Self::Pending(_) => fmt.field("state", &"Pending"),
            Self::Resolved(value) => fmt.field("resolved", value),
            Self::Rejected(err) => fmt.field("rejected", err),
            Self::Consumed => fmt.field("state", &"Consumed"),
        };

        fmt.finish()
    }
}
