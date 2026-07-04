use std::task::Context;

use crate::Promise;

impl<T, E> Promise<T, E> {
    /// Polls the promise's inner future if pending.
    /// Returns `true` if the promise is now settled.
    pub fn settle(&mut self, cx: &mut Context<'_>) -> bool {
        self.poll(cx);

        self.is_settled()
    }
}
