use anyhow::anyhow;

use crate::PromiseRejection;

impl PromiseRejection for anyhow::Error {
    fn already_consumed() -> Self {
        anyhow!("Promise was consumed and then awaited.")
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{anyhow, Error, Result};

    use crate::{Promise, PromiseRejection};

    #[test]
    fn already_consumed_contains_expected_message() {
        let err = Error::already_consumed();
        let message = format!("{err}");

        assert!(
            message.contains("Promise was consumed and then awaited"),
            "Unexpected error message: {}",
            message
        );
    }

    /// Helper function to block on the promise without external executors.
    /// This assumes that `Promise` resolves synchronously.
    fn await_promise<T, E>(promise: &mut Promise<T, E>) -> Result<T, E>
    where
        T: Unpin,
        E: PromiseRejection,
    {
        use std::{
            future::Future,
            pin::pin,
            task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
        };

        // Construct a minimal no‑op waker
        fn no_op(_: *const ()) {}
        fn raw_waker() -> RawWaker {
            RawWaker::new(
                std::ptr::null(),
                &RawWakerVTable::new(|_| raw_waker(), no_op, no_op, no_op),
            )
        }
        let waker = unsafe { Waker::from_raw(raw_waker()) };
        let mut cx = Context::from_waker(&waker);

        let mut pinned = pin!(promise);

        match pinned.as_mut().poll(&mut cx) {
            Poll::Ready(res) => res,
            Poll::Pending => panic!("Promise did not complete immediately."),
        }
    }

    /// Test that a directly resolved promise returns its value on first await.
    #[test]
    fn promise_resolves_successfully_first_await() -> Result<()> {
        // Manually construct a resolved promise — should yield immediately.
        let mut p: Promise<&'static str, Error> = Promise::resolve("ok");
        let result = await_promise(&mut p)?;
        assert_eq!(result, "ok");
        Ok(())
    }

    /// Test that a promise already consumed once rejects with the custom `anyhow` error.
    #[test]
    fn promise_rejects_on_double_await() {
        let mut p: Promise<i32, Error> = Promise::resolve(123);

        // First await succeeds.
        let result = await_promise(&mut p);
        assert!(result.is_ok());
        assert_eq!(result.unwrap(), 123);

        // Second await should produce the custom rejection.
        let second = await_promise(&mut p);
        assert!(second.is_err(), "Expected second await to fail");

        let err = second.unwrap_err();
        let msg = format!("{err:#}");
        assert!(
            msg.contains("Promise was consumed and then awaited."),
            "Unexpected error message: {msg}"
        );
    }

    /// Test that a rejected promise surfaces its error normally the first time.
    #[test]
    fn promise_rejects_normally() {
        let mut p: Promise<(), Error> = Promise::reject(anyhow!("expected rejection"));
        let res = await_promise(&mut p);
        assert!(res.is_err());
        let msg = format!("{:#}", res.unwrap_err());
        assert!(msg.contains("expected rejection"));
    }
}
