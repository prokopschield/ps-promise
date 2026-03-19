use std::task::Context;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    /// Polls the promise's inner future if pending.
    /// Returns `true` if the promise is now resolved or rejected.
    pub fn ready(&mut self, cx: &mut Context<'_>) -> bool {
        self.poll(cx);

        self.is_ready()
    }
}
