mod methods;

use std::{collections::HashMap, task::Waker};

use crate::{
    sync::{
        atomic::{AtomicBool, AtomicUsize},
        Mutex,
    },
    Promise, PromiseRejection,
};

pub(super) struct SharedState<T, E>
where
    E: PromiseRejection,
{
    pub(super) inner: Mutex<Option<Promise<T, E>>>,
    pub(super) wakers: Mutex<HashMap<usize, Waker>>,
    pub(super) next_waiter_id: AtomicUsize,
    pub(super) woke: AtomicBool,
}
