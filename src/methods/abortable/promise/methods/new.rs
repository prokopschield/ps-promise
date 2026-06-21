use async_channel::{Receiver, RecvError};

use crate::{Promise, PromiseRejection};

use super::super::AbortablePromise;

impl<T, E> AbortablePromise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    pub(in super::super::super) fn new(inner: Promise<T, E>, receiver: Receiver<()>) -> Self {
        Self {
            inner,
            receiver: Promise::lazy(async move { receiver.recv().await.map_err(|RecvError| ()) }),
        }
    }
}
