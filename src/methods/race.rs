use std::{future::Future, pin::pin, task::Poll};

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    pub fn race<I>(promises: I) -> Self
    where
        I: IntoIterator<Item = Self>,
    {
        Self::new(PromiseRace::from(promises))
    }
}

impl<T, E> Future for PromiseRace<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    type Output = Result<T, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        for promise in &mut this.promises {
            if let Poll::Ready(result) = pin!(promise).poll(cx) {
                return Poll::Ready(result);
            }
        }

        Poll::Pending
    }
}

pub struct PromiseRace<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    promises: Vec<Promise<T, E>>,
}

impl<I, T, E> From<I> for PromiseRace<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
    I: IntoIterator<Item = Promise<T, E>>,
{
    fn from(value: I) -> Self {
        Self {
            promises: value.into_iter().collect(),
        }
    }
}
