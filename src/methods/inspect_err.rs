use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Runs a callback with a reference to the rejection, then passes the
    /// result through unchanged.
    ///
    /// The callback is not invoked on resolution; see [`Promise::inspect`].
    pub fn inspect_err<F>(self, f: F) -> Self
    where
        F: FnOnce(&E) + Send + 'static,
    {
        Self::eager_or_lazy(async move {
            let result = self.await;

            if let Err(err) = &result {
                f(err);
            }

            result
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
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
        T: Send + 'static,
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

            while !promise.poll_settled(&mut cx) {}

            promise.consume().expect("promise settled")
        }
    }

    #[test]
    fn observes_rejection() {
        let seen = Arc::new(AtomicBool::new(false));
        let sink = seen.clone();

        let result = drive(move || {
            Promise::<i32, E>::reject(E::Fail).inspect_err(move |err| {
                assert_eq!(*err, E::Fail);
                sink.store(true, Ordering::Relaxed);
            })
        });

        assert_eq!(result, Err(E::Fail));
        assert!(seen.load(Ordering::Relaxed));
    }

    #[test]
    fn skipped_on_resolution() {
        let seen = Arc::new(AtomicBool::new(false));
        let sink = seen.clone();

        let result = drive(move || {
            Promise::<i32, E>::resolve(42).inspect_err(move |_| sink.store(true, Ordering::Relaxed))
        });

        assert_eq!(result, Ok(42));
        assert!(!seen.load(Ordering::Relaxed));
    }
}
