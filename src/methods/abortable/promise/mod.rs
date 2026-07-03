mod implementations;
mod methods;

use crate::{Promise, PromiseRejection};

pub(in super::super) struct AbortablePromise<T, E>
where
    E: PromiseRejection,
{
    inner: Promise<T, E>,
    receiver: Promise<(), ()>,
}
