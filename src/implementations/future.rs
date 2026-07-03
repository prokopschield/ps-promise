use std::{
    future::Future,
    task::Poll::{Pending, Ready},
};

use crate::{Promise, PromiseRejection};

impl<T, E> Future for Promise<T, E>
where
    E: PromiseRejection,
{
    type Output = Result<T, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        this.poll(cx);

        this.consume().map_or_else(|| Pending, Ready)
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        task::{Context, Poll, Waker},
    };

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
    fn panicking_future_polls_to_task_failed() {
        let mut promise: Promise<(), E> = Promise::lazy(async { panic!("boom") });

        let poll = Future::poll(
            Pin::new(&mut promise),
            &mut Context::from_waker(Waker::noop()),
        );

        match poll {
            Poll::Ready(Err(E::TaskFailed(failure @ TaskFailure::Panic(_)))) => {
                assert_eq!(failure.to_string(), "task panicked: boom");
            }
            other => panic!("expected Ready(Err(TaskFailed(Panic(_)))), got {other:?}"),
        }
    }
}
