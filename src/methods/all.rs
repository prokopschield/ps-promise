use std::{future::Future, mem::take, pin::pin, task::Poll};

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    pub fn all<I>(promises: I) -> Promise<Vec<T>, E>
    where
        I: IntoIterator<Item = Self>,
    {
        Promise::new(PromiseAll::from(promises))
    }
}

impl<T, E> Future for PromiseAll<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    type Output = Result<Vec<T>, E>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        let mut ready = true;

        for promise in &mut this.promises {
            if !promise.ready(cx) {
                ready = false;
            }
        }

        if !ready {
            return Poll::Pending;
        }

        let mut promises = take(&mut this.promises).into_iter();
        let mut values = Vec::new();

        loop {
            let Some(mut promise) = promises.next() else {
                break;
            };

            match Future::poll(pin!(&mut promise), cx) {
                Poll::Ready(Ok(value)) => values.push(value),
                Poll::Ready(Err(err)) => return Poll::Ready(Err(err)),
                Poll::Pending => {
                    let resolved = values.into_iter().map(Promise::resolve);

                    this.promises.extend(resolved);
                    this.promises.push(promise);
                    this.promises.extend(promises);

                    return Poll::Pending;
                }
            }
        }

        Poll::Ready(Ok(values))
    }
}

pub struct PromiseAll<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    promises: Vec<Promise<T, E>>,
}

impl<I, T, E> From<I> for PromiseAll<T, E>
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
