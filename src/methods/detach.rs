use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    /// Spawns this [`Promise`] on the ambient runtime to run to completion in
    /// the background, discarding its outcome.
    ///
    /// This is fire-and-forget: it consumes the [`Promise`], satisfying the
    /// `#[must_use]` obligation without an `.await`. The inner future runs to
    /// completion regardless of this call returning, and its `Ok`/`Err` result
    /// is dropped. Panics are caught by the [`Promise`] itself and turned into
    /// a rejection, which is likewise dropped, so a detached [`Promise`] never
    /// propagates a panic to the executor.
    ///
    /// Dispatch is selected at compile time based on which runtime features
    /// are enabled, with a runtime check when both are on:
    ///
    /// - Only `tokio` enabled: spawns via [`tokio::spawn`] and drops the
    ///   `JoinHandle`, which detaches the task.
    /// - Only `smol` enabled: spawns via [`smol::spawn`] and calls
    ///   [`Task::detach`](smol::Task::detach); without this the task would be
    ///   cancelled on drop.
    /// - Both enabled: dispatches to the tokio path when called from within a
    ///   tokio runtime context (detected via
    ///   `tokio::runtime::Handle::try_current`), otherwise to the smol path.
    ///
    /// Requires at least one of the `tokio` or `smol` features; if neither is
    /// enabled this method does not exist and call sites fail to compile.
    pub fn detach(self) {
        #[cfg(all(feature = "tokio", feature = "smol"))]
        if tokio::runtime::Handle::try_current().is_ok() {
            drop(tokio::spawn(self));
        } else {
            smol::spawn(self).detach();
        }

        #[cfg(all(feature = "tokio", not(feature = "smol")))]
        drop(tokio::spawn(self));

        #[cfg(all(feature = "smol", not(feature = "tokio")))]
        smol::spawn(self).detach();
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

    #[cfg(feature = "tokio")]
    #[test]
    fn runs_detached_via_tokio() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .expect("build current-thread tokio runtime");

        let ran = Arc::new(AtomicBool::new(false));
        let flag = ran.clone();

        rt.block_on(async move {
            Promise::<(), E>::lazy(async move {
                flag.store(true, Ordering::Relaxed);
                Ok(())
            })
            .detach();

            for _ in 0..5 {
                tokio::task::yield_now().await;
            }
        });

        assert!(
            ran.load(Ordering::Relaxed),
            "detached promise must run without being awaited"
        );
    }

    #[cfg(all(feature = "smol", not(feature = "tokio")))]
    #[test]
    fn runs_detached_via_smol() {
        let ran = Arc::new(AtomicBool::new(false));
        let inner_flag = ran.clone();
        let wait_flag = ran.clone();

        smol::block_on(async move {
            Promise::<(), E>::lazy(async move {
                inner_flag.store(true, Ordering::Relaxed);
                Ok(())
            })
            .detach();

            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);

            while !wait_flag.load(Ordering::Relaxed) && std::time::Instant::now() < deadline {
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
        });

        assert!(
            ran.load(Ordering::Relaxed),
            "detached promise must run without being awaited"
        );
    }
}
