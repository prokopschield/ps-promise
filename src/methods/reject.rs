use crate::{Promise, PromiseRejection, State};

impl<T, E> Promise<T, E>
where
    E: PromiseRejection,
{
    pub const fn reject(err: E) -> Self {
        Self {
            state: State::Rejected(err),
        }
    }
}
