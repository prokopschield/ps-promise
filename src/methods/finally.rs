use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Runs a callback once this [`Promise`] settles, regardless of outcome,
    /// then passes the settled result through.
    ///
    /// Mirrors ECMAScript's `Promise.prototype.finally`: the callback receives
    /// no arguments and cannot alter a resolved value, but if its future
    /// rejects, that rejection takes precedence over the original outcome.
    pub fn finally<F, Fut>(self, f: F) -> Self
    where
        F: FnOnce() -> Fut + Send + 'static,
        Fut: Future<Output = Result<(), E>> + Send + 'static,
    {
        Self::eager_or_lazy(async move {
            let result = self.await;

            f().await?;

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

            while !promise.settle(&mut cx) {}

            promise.consume().expect("promise settled")
        }
    }

    #[test]
    fn runs_callback_on_resolution() {
        let called = Arc::new(AtomicBool::new(false));
        let flag = called.clone();

        let result = drive(move || {
            Promise::<i32, E>::resolve(42).finally(move || async move {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            })
        });

        assert_eq!(result, Ok(42));
        assert!(called.load(Ordering::Relaxed));
    }

    #[test]
    fn runs_callback_on_rejection() {
        let called = Arc::new(AtomicBool::new(false));
        let flag = called.clone();

        let result = drive(move || {
            Promise::<i32, E>::reject(E::Fail).finally(move || async move {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            })
        });

        assert_eq!(result, Err(E::Fail));
        assert!(called.load(Ordering::Relaxed));
    }

    #[test]
    fn callback_rejection_overrides_resolution() {
        let result = drive(|| Promise::<i32, E>::resolve(42).finally(|| async { Err(E::Fail) }));

        assert_eq!(result, Err(E::Fail));
    }

    #[test]
    fn callback_rejection_overrides_original_rejection() {
        let result = drive(|| {
            Promise::<i32, E>::reject(E::AlreadyConsumed).finally(|| async { Err(E::Fail) })
        });

        assert_eq!(result, Err(E::Fail));
    }
}
