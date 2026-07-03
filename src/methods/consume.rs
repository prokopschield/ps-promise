use std::mem::replace;

use crate::{Promise, PromiseRejection, State};

impl<T, E> Promise<T, E>
where
    E: PromiseRejection,
{
    /// If settled, consumes and returns the result.
    /// Returns `None` if still pending.
    pub fn consume(&mut self) -> Option<Result<T, E>> {
        match replace(&mut self.state, State::Consumed) {
            State::Resolved(val) => Some(Ok(val)),
            State::Rejected(err) => Some(Err(err)),
            State::Consumed => Some(Err(E::already_consumed())),
            State::Failed(failure) => {
                self.state = State::Failed(failure.clone());
                Some(Err(E::task_failed(failure)))
            }
            other @ State::Pending(_) => {
                self.state = other;
                None
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::task::{Context, Waker};

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug)]
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

    #[test]
    fn task_failure_converts_upon_consumption() {
        let mut promise: Promise<(), E> = Promise::lazy(async { panic!("boom") });

        promise.poll(&mut Context::from_waker(Waker::noop()));

        match promise.consume() {
            Some(Err(E::TaskFailed(failure @ TaskFailure::Panic(_)))) => {
                assert_eq!(failure.to_string(), "task panicked: boom");
            }
            other => panic!("expected Err(TaskFailed(Panic(_))), got {other:?}"),
        }
    }
}
