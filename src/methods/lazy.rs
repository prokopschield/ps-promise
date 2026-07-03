use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    E: PromiseRejection,
{
    /// Wraps a [`Future`] in a [`Promise`] without driving it.
    ///
    /// The future does not make progress until the [`Promise`] is polled.
    pub fn lazy<F>(future: F) -> Self
    where
        F: Future<Output = Result<T, E>> + Send + 'static,
    {
        Self::Pending(Box::pin(future))
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

    #[derive(Debug)]
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
    fn does_not_poll_on_construction() {
        struct Counter {
            count: Arc<AtomicUsize>,
        }

        impl Future for Counter {
            type Output = Result<(), E>;
            fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
                self.count.fetch_add(1, Ordering::Relaxed);
                Poll::Ready(Ok(()))
            }
        }

        let count = Arc::new(AtomicUsize::new(0));

        let mut promise: Promise<(), E> = Promise::lazy(Counter {
            count: count.clone(),
        });

        assert_eq!(count.load(Ordering::Relaxed), 0);
        assert!(promise.is_pending());

        promise.poll(&mut cx());

        assert_eq!(count.load(Ordering::Relaxed), 1);
    }
}
