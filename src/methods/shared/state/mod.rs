mod methods;

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Mutex,
    },
    task::Waker,
};

use crate::{Promise, PromiseRejection};

pub(super) struct SharedState<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    pub(super) inner: Mutex<Option<Promise<T, E>>>,
    pub(super) wakers: Mutex<HashMap<usize, Waker>>,
    pub(super) next_waiter_id: AtomicUsize,
    pub(super) woke: AtomicBool,
}
