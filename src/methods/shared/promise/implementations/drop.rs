use crate::PromiseRejection;

use super::super::SharedPromise;

impl<T, E> Drop for SharedPromise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    fn drop(&mut self) {
        self.state.remove_waker(self.waiter_id);
    }
}
