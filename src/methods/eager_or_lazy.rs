use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    /// Wraps a [`Future`] in a [`Promise`], eagerly scheduled when a runtime
    /// feature is enabled and lazy otherwise.
    ///
    /// - Any of the `tokio` or `smol` features enabled: delegates to
    ///   `Promise::eager`, which dispatches to the available runtime.
    /// - Neither feature enabled: delegates to [`Promise::lazy`]; the future
    ///   only progresses when the [`Promise`] is polled.
    ///
    /// Library authors use this to remain runtime-agnostic: the call site
    /// gains JS-style eager scheduling whenever the binary enables a runtime,
    /// and degrades to lazy scheduling otherwise without forcing a runtime
    /// choice onto downstream crates.
    pub fn eager_or_lazy(future: impl Future<Output = Result<T, E>> + Send + 'static) -> Self {
        #[cfg(any(feature = "tokio", feature = "smol"))]
        return Self::eager(future);

        #[cfg(not(any(feature = "tokio", feature = "smol")))]
        return Self::lazy(future);
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug)]
    #[allow(dead_code)]
    enum E {
        AlreadyConsumed,
        TaskFailed(TaskFailure),
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(failure: TaskFailure) -> Self {
            Self::TaskFailed(failure)
        }
    }

    #[cfg(not(any(feature = "tokio", feature = "smol")))]
    #[test]
    fn falls_back_to_lazy_without_runtime() {
        use std::{
            future::Future,
            pin::Pin,
            sync::{
                atomic::{AtomicUsize, Ordering},
                Arc,
            },
            task::{Context, Poll, Waker},
        };

        struct Counter {
            count: Arc<AtomicUsize>,
        }

        impl Future for Counter {
            type Output = Result<(), E>;

            fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
                self.count.fetch_add(1, Ordering::Relaxed);
                Poll::Ready(Ok(()))
            }
        }

        let count = Arc::new(AtomicUsize::new(0));

        let mut promise: Promise<(), E> = Promise::eager_or_lazy(Counter {
            count: count.clone(),
        });

        assert_eq!(count.load(Ordering::Relaxed), 0);
        assert!(promise.is_pending());

        promise.poll(&mut Context::from_waker(Waker::noop()));

        assert_eq!(count.load(Ordering::Relaxed), 1);
    }

    #[cfg(feature = "tokio")]
    #[test]
    fn resolves_value_via_tokio() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("build current-thread tokio runtime");

        let result =
            rt.block_on(async { Promise::<i32, E>::eager_or_lazy(async { Ok(42) }).await });

        assert!(matches!(result, Ok(42)));
    }

    #[cfg(all(feature = "smol", not(feature = "tokio")))]
    #[test]
    fn resolves_value_via_smol() {
        let result =
            smol::block_on(async { Promise::<i32, E>::eager_or_lazy(async { Ok(42) }).await });

        assert!(matches!(result, Ok(42)));
    }
}
