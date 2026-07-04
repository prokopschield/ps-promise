use crate::{Promise, PromiseRejection};

impl<P, E> Promise<P, E>
where
    P: Send + 'static,
    E: PromiseRejection,
{
    /// Flattens a nested promise into the inner promise's outcome.
    ///
    /// Mirrors ECMAScript's thenable assimilation, where a promise resolved
    /// with a thenable adopts its state instead of yielding it. The resolved
    /// value may be any type convertible into a [`Promise`] (a nested
    /// [`Promise`], a `Result`, an `Option`, whose conversion additionally
    /// requires `EO: Default`, or a boxed future); an outer rejection
    /// bypasses the conversion and converts into `EO` via `From`.
    /// The returned promise is scheduled via [`Promise::eager_or_lazy`].
    pub fn flatten<TO, EO>(self) -> Promise<TO, EO>
    where
        P: Into<Promise<TO, EO>>,
        TO: Send + 'static,
        EO: PromiseRejection + From<E>,
    {
        Promise::eager_or_lazy(async move {
            match self.await {
                Ok(inner) => inner.into().await,
                Err(err) => Err(err.into()),
            }
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug, PartialEq)]
    enum Outer {
        AlreadyConsumed,
        Fail,
        TaskFailed,
    }

    impl PromiseRejection for Outer {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(_: TaskFailure) -> Self {
            Self::TaskFailed
        }
    }

    #[derive(Debug, PartialEq)]
    enum E {
        AlreadyConsumed,
        Fail,
        TaskFailed,
        Converted,
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }

        fn task_failed(_: TaskFailure) -> Self {
            Self::TaskFailed
        }
    }

    impl From<Outer> for E {
        fn from(_: Outer) -> Self {
            Self::Converted
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
    fn nested_promise_resolves_to_inner_value() {
        let result = drive(|| {
            Promise::<Promise<i32, E>, E>::resolve(Promise::resolve(7)).flatten::<i32, E>()
        });

        assert_eq!(result, Ok(7));
    }

    #[test]
    fn inner_rejection_propagates() {
        let result = drive(|| {
            Promise::<Promise<i32, E>, E>::resolve(Promise::reject(E::Fail)).flatten::<i32, E>()
        });

        assert_eq!(result, Err(E::Fail));
    }

    #[test]
    fn outer_rejection_short_circuits() {
        let result = drive(|| Promise::<Promise<i32, E>, E>::reject(E::Fail).flatten::<i32, E>());

        assert_eq!(result, Err(E::Fail));
    }

    #[test]
    fn outer_rejection_converts_via_from() {
        let result =
            drive(|| Promise::<Promise<i32, E>, Outer>::reject(Outer::Fail).flatten::<i32, E>());

        assert_eq!(result, Err(E::Converted));
    }

    #[test]
    fn resolved_result_flattens() {
        let result = drive(|| Promise::<Result<i32, E>, E>::resolve(Ok(7)).flatten::<i32, E>());

        assert_eq!(result, Ok(7));
    }

    #[test]
    fn rejected_result_flattens() {
        let result =
            drive(|| Promise::<Result<i32, E>, E>::resolve(Err(E::Fail)).flatten::<i32, E>());

        assert_eq!(result, Err(E::Fail));
    }

    #[test]
    fn pending_inner_promise_is_driven_to_completion() {
        let result = drive(|| {
            Promise::<Promise<i32, E>, E>::resolve(Promise::lazy(async { Ok(7) }))
                .flatten::<i32, E>()
        });

        assert_eq!(result, Ok(7));
    }
}
