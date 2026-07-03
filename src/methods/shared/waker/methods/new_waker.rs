use std::{sync::Arc, task::Waker};

use crate::PromiseRejection;

use super::super::super::state::SharedState;
use super::super::SharedWaker;

impl<T, E> SharedWaker<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    pub(in crate::methods::shared) fn new_waker(state: &Arc<SharedState<T, E>>) -> Waker {
        Waker::from(Arc::new(Self {
            state: Arc::downgrade(state),
        }))
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        task::{Context, Poll, Waker},
    };

    use crate::{Promise, PromiseRejection, TaskFailure};

    use super::super::super::SharedWaker;

    #[derive(Debug, Clone, PartialEq)]
    enum E {
        AlreadyConsumed,
        TaskFailed,
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(_: TaskFailure) -> Self {
            Self::TaskFailed
        }
    }

    fn poll_with<F: Future + Unpin>(future: &mut F, waker: &Waker) -> Poll<F::Output> {
        Pin::new(future).poll(&mut Context::from_waker(waker))
    }

    struct CountingWaker {
        count: AtomicUsize,
    }

    impl std::task::Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    #[test]
    fn new_waker_wakes_single_parked_consumer() {
        let (promise, _resolve, _reject) = Promise::<i32, E>::with_resolvers();
        let shared = promise.shared();

        let cw = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let task_waker = Waker::from(cw.clone());

        let mut consumer = shared.clone();

        assert_eq!(poll_with(&mut consumer, &task_waker), Poll::Pending);
        assert_eq!(cw.count.load(Ordering::SeqCst), 0);

        let w = SharedWaker::<i32, E>::new_waker(&shared.state);

        w.wake();

        assert!(cw.count.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn new_waker_fans_out_to_multiple_parked_consumers() {
        let (promise, _resolve, _reject) = Promise::<i32, E>::with_resolvers();
        let shared = promise.shared();

        let cw_a = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let cw_b = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });

        let task_waker_a = Waker::from(cw_a.clone());
        let task_waker_b = Waker::from(cw_b.clone());

        let mut consumer_a = shared.clone();
        let mut consumer_b = shared.clone();

        assert_eq!(poll_with(&mut consumer_a, &task_waker_a), Poll::Pending);
        assert_eq!(poll_with(&mut consumer_b, &task_waker_b), Poll::Pending);

        let w = SharedWaker::<i32, E>::new_waker(&shared.state);

        w.wake();

        assert!(cw_a.count.load(Ordering::SeqCst) >= 1);
        assert!(cw_b.count.load(Ordering::SeqCst) >= 1);
    }

    #[test]
    fn new_waker_constructs_and_wakes_while_shared_alive() {
        let (promise, _resolve, _reject) = Promise::<i32, E>::with_resolvers();
        let shared = promise.shared();

        let cw = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });
        let task_waker = Waker::from(cw);

        let mut consumer = shared.clone();

        assert_eq!(poll_with(&mut consumer, &task_waker), Poll::Pending);

        let w = SharedWaker::<i32, E>::new_waker(&shared.state);

        w.wake_by_ref();

        drop(w);
        drop(shared);
    }
}
