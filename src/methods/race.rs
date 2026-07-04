use std::{future::Future, task::Poll};

use crate::{gate::GatedPromise, Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    /// Settles with the outcome of the first promise to settle.
    ///
    /// Mirrors ECMAScript's `Promise.race`, including its footgun: a race
    /// over an empty input is forever pending, and awaiting it parks the
    /// task permanently, since no waker is ever registered. The returned
    /// [`Promise`] is lazy.
    pub fn race<I>(promises: I) -> Self
    where
        I: IntoIterator<Item = Self>,
    {
        Self::lazy(PromiseRace::from(promises))
    }
}

impl<T, E> Future for PromiseRace<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    type Output = Result<T, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        for promise in &mut this.promises {
            promise.poll(cx);

            if let Some(result) = promise.inner.consume() {
                return Poll::Ready(result);
            }
        }

        Poll::Pending
    }
}

pub struct PromiseRace<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
{
    promises: Vec<GatedPromise<T, E>>,
}

impl<I, T, E> From<I> for PromiseRace<T, E>
where
    T: Send + 'static,
    E: PromiseRejection,
    I: IntoIterator<Item = Promise<T, E>>,
{
    fn from(value: I) -> Self {
        Self {
            promises: value.into_iter().map(GatedPromise::new).collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        future::Future,
        pin::Pin,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        task::{Context, Poll, Waker},
    };

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(thiserror::Error, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
    enum E {
        #[error("Promise already consumed.")]
        AlreadyConsumed,
        #[error("The underlying task failed.")]
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

    fn cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    /// Inner future that counts its polls and never settles.
    struct CountPolls {
        polls: Arc<AtomicUsize>,
    }

    impl Future for CountPolls {
        type Output = Result<i32, E>;

        fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
            self.polls.fetch_add(1, Ordering::SeqCst);

            Poll::Pending
        }
    }

    #[test]
    fn children_without_a_wakeup_request_are_not_repolled() {
        let polls = Arc::new(AtomicUsize::new(0));

        let never: Promise<i32, E> = Promise::lazy(CountPolls {
            polls: polls.clone(),
        });
        let (pending, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let mut race = Promise::race([never, pending]);

        assert!(!race.poll_settled(&mut cx()));
        assert!(!race.poll_settled(&mut cx()));
        assert!(!race.poll_settled(&mut cx()));

        assert_eq!(polls.load(Ordering::SeqCst), 1);

        resolve.resolve(7);

        assert!(race.poll_settled(&mut cx()));

        assert_eq!(polls.load(Ordering::SeqCst), 1);
        assert_eq!(race.consume(), Some(Ok(7)));
    }
}
