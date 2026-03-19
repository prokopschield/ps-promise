use anyhow::anyhow;

use crate::PromiseRejection;

impl PromiseRejection for anyhow::Error {
    fn already_consumed() -> Self {
        anyhow!("Promise was consumed and then awaited.")
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{anyhow, Error};

    use crate::{Promise, PromiseRejection};

    #[test]
    fn already_consumed_contains_expected_message() {
        let err = Error::already_consumed();
        let message = format!("{err}");

        assert!(
            message.contains("Promise was consumed and then awaited"),
            "Unexpected error message: {message}"
        );
    }

    #[test]
    fn promise_resolves_successfully() {
        let mut p: Promise<&'static str, Error> = Promise::resolve("ok");
        p.poll_sync();

        match p.consume() {
            Some(Ok(val)) => assert_eq!(val, "ok"),
            other => panic!("expected Some(Ok(\"ok\")), got {other:?}"),
        }
    }

    #[test]
    fn promise_rejects_on_double_consume() {
        let mut p: Promise<i32, Error> = Promise::resolve(123);
        p.poll_sync();

        assert!(p.consume().is_some_and(|r| r.is_ok()));

        p.poll_sync();
        let second = p.consume();

        assert!(
            second.is_some_and(|r| r.is_err()),
            "Expected second consume to fail"
        );
    }

    #[test]
    fn promise_rejects_normally() {
        let mut p: Promise<(), Error> = Promise::reject(anyhow!("expected rejection"));
        p.poll_sync();

        match p.consume() {
            Some(Err(err)) => {
                let msg = format!("{err:#}");
                assert!(msg.contains("expected rejection"));
            }
            other => panic!("expected Some(Err(...)), got {other:?}"),
        }
    }
}
