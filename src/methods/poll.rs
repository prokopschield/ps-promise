use std::{
    panic::{catch_unwind, AssertUnwindSafe},
    task::{Context, Poll},
};

use crate::{Promise, State, TaskFailure};

impl<T, E> Promise<T, E> {
    /// Attempts to advance this [`Promise`] using the provided execution [`Context`].
    ///
    /// This performs exactly one poll of the underlying future if the
    /// [`Promise`] is pending, and does nothing otherwise.
    pub fn poll(&mut self, cx: &mut Context<'_>) {
        let State::Pending(future) = &mut self.state else {
            return;
        };

        let mut future = AssertUnwindSafe(future);
        let mut cx = AssertUnwindSafe(cx);

        let poll = match catch_unwind(move || future.as_mut().poll(&mut cx)) {
            Ok(poll) => poll,
            Err(panic) => return self.state = State::Failed(TaskFailure::from(panic)),
        };

        match poll {
            Poll::Ready(Ok(value)) => self.state = State::Resolved(value),
            Poll::Ready(Err(err)) => self.state = State::Rejected(err),
            Poll::Pending => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicBool, AtomicUsize, Ordering},
            Arc,
        },
        task::{Context, Poll, Waker},
    };

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum E {
        AlreadyConsumed,
        Fail,
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(_: TaskFailure) -> Self {
            Self::Fail
        }
    }

    fn cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    #[test]
    fn resolves_ready_future() {
        let mut promise: Promise<i32, E> = Promise::lazy(async { Ok(42) });
        promise.poll(&mut cx());
        match promise.consume() {
            Some(Ok(v)) => assert_eq!(v, 42),
            other => panic!("expected Resolved(42), got {other:?}"),
        }
    }

    #[test]
    fn rejects_ready_future() {
        let mut promise: Promise<(), E> = Promise::lazy(async { Err(E::Fail) });
        promise.poll(&mut cx());
        match promise.consume() {
            Some(Err(E::Fail)) => {}
            other => panic!("expected Rejected(Fail), got {other:?}"),
        }
    }

    #[test]
    fn stays_pending() {
        struct Never;

        impl Future for Never {
            type Output = Result<(), E>;
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                Poll::Pending
            }
        }

        let mut promise: Promise<(), E> = Promise::lazy(Never);
        promise.poll(&mut cx());
        assert!(promise.is_pending());
    }

    #[test]
    fn resolved_is_identity() {
        let mut promise: Promise<i32, E> = Promise::resolve(99);
        promise.poll(&mut cx());
        match promise.consume() {
            Some(Ok(v)) => assert_eq!(v, 99),
            other => panic!("expected Resolved(99), got {other:?}"),
        }
    }

    #[test]
    fn rejected_is_identity() {
        let mut promise: Promise<(), E> = Promise::reject(E::Fail);
        promise.poll(&mut cx());
        match promise.consume() {
            Some(Err(E::Fail)) => {}
            other => panic!("expected Rejected(Fail), got {other:?}"),
        }
    }

    #[test]
    fn consumed_is_identity() {
        let mut promise: Promise<(), E> = Promise::resolve(());

        assert!(promise.consume().is_some());

        promise.poll(&mut cx());
        assert!(promise.is_consumed());
    }

    #[test]
    fn pending_then_resolves() {
        struct Delayed {
            ready: Arc<AtomicBool>,
        }

        impl Future for Delayed {
            type Output = Result<i32, E>;
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.ready.load(Ordering::Relaxed) {
                    Poll::Ready(Ok(7))
                } else {
                    Poll::Pending
                }
            }
        }

        let ready = Arc::new(AtomicBool::new(false));

        let mut promise: Promise<i32, E> = Promise::lazy(Delayed {
            ready: ready.clone(),
        });

        promise.poll(&mut cx());
        assert!(promise.is_pending());

        ready.store(true, Ordering::Relaxed);

        promise.poll(&mut cx());
        match promise.consume() {
            Some(Ok(v)) => assert_eq!(v, 7),
            other => panic!("expected Resolved(7), got {other:?}"),
        }
    }

    #[test]
    fn pending_then_rejects() {
        struct Delayed {
            ready: Arc<AtomicBool>,
        }

        impl Future for Delayed {
            type Output = Result<(), E>;
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                if self.ready.load(Ordering::Relaxed) {
                    Poll::Ready(Err(E::Fail))
                } else {
                    Poll::Pending
                }
            }
        }

        let ready = Arc::new(AtomicBool::new(false));

        let mut promise: Promise<(), E> = Promise::lazy(Delayed {
            ready: ready.clone(),
        });

        promise.poll(&mut cx());
        assert!(promise.is_pending());

        ready.store(true, Ordering::Relaxed);

        promise.poll(&mut cx());
        match promise.consume() {
            Some(Err(E::Fail)) => {}
            other => panic!("expected Rejected(Fail), got {other:?}"),
        }
    }

    #[test]
    fn waker_is_forwarded() {
        struct StoreWaker {
            woken: Arc<AtomicBool>,
        }

        impl Future for StoreWaker {
            type Output = Result<(), E>;
            fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
                let waker = cx.waker().clone();
                let flag = self.woken.clone();
                waker.wake();
                flag.store(true, Ordering::Relaxed);
                Poll::Pending
            }
        }

        let woken = Arc::new(AtomicBool::new(false));

        let mut promise: Promise<(), E> = Promise::lazy(StoreWaker {
            woken: woken.clone(),
        });
        promise.poll(&mut cx());

        assert!(
            woken.load(Ordering::Relaxed),
            "waker should have been invoked"
        );
    }

    #[test]
    fn polls_exactly_once() {
        struct Counter {
            count: Arc<AtomicUsize>,
        }

        impl Future for Counter {
            type Output = Result<(), E>;
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                self.count.fetch_add(1, Ordering::Relaxed);
                Poll::Pending
            }
        }

        let count = Arc::new(AtomicUsize::new(0));

        let mut promise: Promise<(), E> = Promise::lazy(Counter {
            count: count.clone(),
        });
        promise.poll(&mut cx());

        assert_eq!(count.load(Ordering::Relaxed), 1);
    }
}
