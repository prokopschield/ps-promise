use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    E: PromiseRejection,
{
    pub const fn is_ready(&self) -> bool {
        !self.is_pending()
    }
}
