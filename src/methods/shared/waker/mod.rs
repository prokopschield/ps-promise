mod implementations;
mod methods;

use std::sync::Weak;

use crate::PromiseRejection;

use super::state::SharedState;

pub(super) struct SharedWaker<T, E>
where
    E: PromiseRejection,
{
    state: Weak<SharedState<T, E>>,
}
