use anyhow::anyhow;

use crate::{PromiseRejection, TaskFailure};

impl PromiseRejection for anyhow::Error {
    fn already_consumed() -> Self {
        anyhow!("Promise was consumed and then awaited.")
    }

    fn task_failed(failure: TaskFailure) -> Self {
        Self::new(failure)
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{anyhow, Error};

    use crate::{Promise, PromiseRejection, TaskFailure};

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
    fn task_failed_downcasts_to_the_failure() {
        let err = Error::task_failed(TaskFailure::Timeout);

        assert!(matches!(
            err.downcast_ref::<TaskFailure>(),
            Some(TaskFailure::Timeout)
        ));
    }

    #[test]
    fn task_failed_displays_the_failure_message() {
        let err = Error::task_failed(TaskFailure::Panic("boom".into()));

        assert_eq!(format!("{err}"), "task panicked: boom");
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
