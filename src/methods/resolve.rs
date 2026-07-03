use crate::{Promise, PromiseRejection, State};

impl<T, E> Promise<T, E>
where
    E: PromiseRejection,
{
    pub const fn resolve(value: T) -> Self {
        Self {
            state: State::Resolved(value),
        }
    }
}
