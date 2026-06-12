use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    /// Runs a callback with a reference to the resolved value, then passes
    /// the result through unchanged.
    ///
    /// The callback is not invoked on rejection; see [`Promise::inspect_err`].
    pub fn inspect<F>(self, f: F) -> Self
    where
        F: FnOnce(&T) + Send + 'static,
    {
        Self::eager_or_lazy(async move {
            let result = self.await;

            if let Ok(value) = &result {
                f(value);
            }

            result
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::sync::{
        atomic::{AtomicI32, Ordering},
        Arc,
    };

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug, PartialEq)]
    enum E {
        AlreadyConsumed,
        Fail,
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

    fn drive<T, F>(make: F) -> Result<T, E>
    where
        T: Send + Unpin + 'static,
        F: FnOnce() -> Promise<T, E> + Send + 'static,
    {
        #[cfg(feature = "tokio")]
        return tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("build current-thread tokio runtime")
            .block_on(async move { make().await });

        #[cfg(all(feature = "smol", not(feature = "tokio")))]
        return smol::block_on(async move { make().await });

        #[cfg(not(any(feature = "tokio", feature = "smol")))]
        {
            use std::task::{Context, Waker};

            let mut promise = make();
            let mut cx = Context::from_waker(Waker::noop());

            while !promise.ready(&mut cx) {}

            promise.consume().expect("promise settled")
        }
    }

    #[test]
    fn observes_resolved_value() {
        let seen = Arc::new(AtomicI32::new(0));
        let sink = seen.clone();

        let result = drive(move || {
            Promise::<i32, E>::resolve(42).inspect(move |v| sink.store(*v, Ordering::Relaxed))
        });

        assert_eq!(result, Ok(42));
        assert_eq!(seen.load(Ordering::Relaxed), 42);
    }

    #[test]
    fn skipped_on_rejection() {
        let seen = Arc::new(AtomicI32::new(0));
        let sink = seen.clone();

        let result = drive(move || {
            Promise::<i32, E>::reject(E::Fail).inspect(move |v| sink.store(*v, Ordering::Relaxed))
        });

        assert_eq!(result, Err(E::Fail));
        assert_eq!(seen.load(Ordering::Relaxed), 0);
    }
}
