use std::time::Duration;

use crate::{Promise, PromiseRejection};

impl<E> Promise<(), E>
where
    E: PromiseRejection,
{
    /// Resolves with `()` once `duration` has elapsed.
    ///
    /// Dispatch is selected at the first poll, based on which runtime
    /// features are enabled, with a runtime check when `tokio` is on:
    ///
    /// - `tokio` enabled and the first poll occurs within a tokio runtime
    ///   context (detected via `tokio::runtime::Handle::try_current`): uses
    ///   `tokio::time::sleep`.
    /// - Otherwise, with `smol` enabled: uses `smol::Timer`.
    /// - Otherwise: parks a thread on the `blocking` crate's pool.
    ///
    /// The returned [`Promise`] is lazy; the timer starts when it is first
    /// polled. On a tokio runtime whose time driver is disabled,
    /// `tokio::time::sleep` is unusable, and the timer falls back to
    /// `smol::Timer` or the `blocking` pool as above.
    pub fn sleep(duration: Duration) -> Self {
        Self::lazy(async move {
            #[cfg(feature = "tokio")]
            if tokio::runtime::Handle::try_current().is_ok() {
                let timer = Promise::<(), ()>::lazy(async move {
                    tokio::time::sleep(duration).await;

                    Ok(())
                });

                if timer.await.is_ok() {
                    return Ok(());
                }
            }

            #[cfg(feature = "smol")]
            return {
                smol::Timer::after(duration).await;

                Ok(())
            };

            #[cfg(not(feature = "smol"))]
            return {
                blocking::unblock(move || std::thread::sleep(duration)).await;

                Ok(())
            };
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use std::time::{Duration, Instant};

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug, PartialEq)]
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

    const NAP: Duration = Duration::from_millis(50);

    #[cfg(feature = "tokio")]
    #[test]
    fn sleeps_via_tokio() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("build current-thread tokio runtime");

        let start = Instant::now();
        let result = rt.block_on(async { Promise::<(), E>::sleep(NAP).await });

        assert_eq!(result, Ok(()));
        assert!(start.elapsed() >= NAP);
    }

    /// The timer is selected at the first poll, not at construction: a
    /// promise constructed inside a tokio runtime context but polled outside
    /// of one must fall back to a portable timer instead of panicking in
    /// `tokio::time::sleep`.
    #[cfg(feature = "tokio")]
    #[test]
    fn selects_the_timer_at_first_poll() {
        use std::task::{Context, Waker};

        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_time()
            .build()
            .expect("build current-thread tokio runtime");

        let mut promise = {
            let _guard = rt.enter();

            Promise::<(), E>::sleep(NAP)
        };

        let mut cx = Context::from_waker(Waker::noop());

        let start = Instant::now();

        while !promise.poll_settled(&mut cx) {}

        assert_eq!(promise.consume(), Some(Ok(())));
        assert!(start.elapsed() >= NAP);
    }

    /// On a tokio runtime whose time driver is disabled, the timer falls
    /// back to a portable timer instead of surfacing the panic raised by
    /// `tokio::time::sleep`.
    #[cfg(feature = "tokio")]
    #[test]
    fn falls_back_without_a_time_driver() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("build current-thread tokio runtime");

        let start = Instant::now();
        let result = rt.block_on(async { Promise::<(), E>::sleep(NAP).await });

        assert_eq!(result, Ok(()));
        assert!(start.elapsed() >= NAP);
    }

    #[cfg(all(feature = "smol", not(feature = "tokio")))]
    #[test]
    fn sleeps_via_smol() {
        let start = Instant::now();
        let result = smol::block_on(Promise::<(), E>::sleep(NAP));

        assert_eq!(result, Ok(()));
        assert!(start.elapsed() >= NAP);
    }

    #[cfg(not(any(feature = "tokio", feature = "smol")))]
    #[test]
    fn sleeps_via_blocking_pool() {
        use std::task::{Context, Waker};

        let mut promise = Promise::<(), E>::sleep(NAP);
        let mut cx = Context::from_waker(Waker::noop());

        let start = Instant::now();

        while !promise.poll_settled(&mut cx) {}

        assert_eq!(promise.consume(), Some(Ok(())));
        assert!(start.elapsed() >= NAP);
    }
}
