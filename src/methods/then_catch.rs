use std::future::Future;

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Chains an asynchronous transformation onto the resolved value and an
    /// asynchronous recovery onto the rejection.
    ///
    /// Mirrors ECMAScript's two-argument `then`: exactly one of the
    /// callbacks runs, and its outcome settles the returned promise. Unlike
    /// `.then(on_fulfilled).catch(on_rejected)`, `on_rejected` does not
    /// observe failures produced by `on_fulfilled`; a rejection returned by
    /// `on_fulfilled`, or a panic in it, rejects the returned promise
    /// directly (a panic via [`PromiseRejection::task_failed`]). Since both
    /// branches supply a callback, neither `TO` nor `EO` needs a `From`
    /// conversion. The returned promise is scheduled via
    /// [`Promise::eager_or_lazy`].
    pub fn then_catch<TO, EO, F, FFut, R, RFut>(
        self,
        on_fulfilled: F,
        on_rejected: R,
    ) -> Promise<TO, EO>
    where
        TO: Send + 'static,
        EO: PromiseRejection,
        F: FnOnce(T) -> FFut + Send + 'static,
        FFut: Future<Output = Result<TO, EO>> + Send + 'static,
        R: FnOnce(E) -> RFut + Send + 'static,
        RFut: Future<Output = Result<TO, EO>> + Send + 'static,
    {
        Promise::eager_or_lazy(async move {
            match self.await {
                Ok(value) => on_fulfilled(value).await,
                Err(err) => on_rejected(err).await,
            }
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

    #[derive(Debug, PartialEq)]
    enum EO {
        AlreadyConsumed,
        Recovered,
        Transformed,
        TaskFailed(String),
    }

    impl PromiseRejection for EO {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(failure: TaskFailure) -> Self {
            Self::TaskFailed(failure.to_string())
        }
    }

    fn drive<T, E, F>(make: F) -> Result<T, E>
    where
        T: Send + 'static,
        E: PromiseRejection,
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
    fn fulfillment_runs_the_first_callback() {
        let result = drive(|| {
            Promise::<i32, E>::resolve(6).then_catch(
                |v| async move { Ok::<String, EO>((v * 7).to_string()) },
                |_| async { Err(EO::Recovered) },
            )
        });

        assert_eq!(result, Ok("42".to_string()));
    }

    #[test]
    fn rejection_runs_the_second_callback() {
        let result = drive(|| {
            Promise::<i32, E>::reject(E::Fail).then_catch(
                |v| async move { Ok::<i32, EO>(v) },
                |err| async move {
                    assert_eq!(err, E::Fail);

                    Ok(0)
                },
            )
        });

        assert_eq!(result, Ok(0));
    }

    #[test]
    fn on_rejected_does_not_observe_failures_produced_by_on_fulfilled() {
        let recovered = Arc::new(AtomicBool::new(false));

        let probe = recovered.clone();

        let result = drive(move || {
            Promise::<i32, E>::resolve(1).then_catch(
                |_| async { Err::<i32, EO>(EO::Transformed) },
                move |_| async move {
                    probe.store(true, Ordering::Relaxed);

                    Ok(0)
                },
            )
        });

        assert_eq!(result, Err(EO::Transformed));
        assert!(!recovered.load(Ordering::Relaxed));
    }

    #[test]
    fn changes_both_types_without_from_impls() {
        let result = drive(|| {
            Promise::<i32, E>::reject(E::Fail).then_catch(
                |v| async move { Ok::<String, EO>(v.to_string()) },
                |_| async { Err(EO::Recovered) },
            )
        });

        assert_eq!(result, Err(EO::Recovered));
    }

    #[test]
    fn panic_in_on_fulfilled_rejects_and_bypasses_on_rejected() {
        let recovered = Arc::new(AtomicBool::new(false));

        let probe = recovered.clone();

        let result = drive(move || {
            Promise::<i32, E>::resolve(1).then_catch(
                |_| async { panic!("then_catch fulfillment panicked") },
                move |_| async move {
                    probe.store(true, Ordering::Relaxed);

                    Ok::<i32, EO>(0)
                },
            )
        });

        match result {
            Err(EO::TaskFailed(msg)) => assert!(msg.contains("then_catch fulfillment panicked")),
            other => panic!("expected TaskFailed rejection, got {other:?}"),
        }

        assert!(!recovered.load(Ordering::Relaxed));
    }
}
