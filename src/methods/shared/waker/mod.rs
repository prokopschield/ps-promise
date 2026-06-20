mod implementations;
mod methods;

use std::sync::Weak;

use crate::PromiseRejection;

use super::state::SharedState;

#[derive(Clone, Debug, Default)]
pub(super) struct SharedWaker<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    state: Weak<SharedState<T, E>>,
}
