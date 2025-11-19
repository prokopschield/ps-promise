use std::{future::Future, mem::replace, task::Poll};

use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    pub fn any<I>(promises: I) -> Promise<T, Vec<E>>
    where
        I: IntoIterator<Item = Self>,
    {
        Promise::new(PromiseAny::from(promises))
    }
}

impl<T, E> Future for PromiseAny<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    type Output = Result<T, Vec<E>>;

    fn poll(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Self::Output> {
        let this = self.get_mut();

        let mut n_pending = 0;

        for promise in &mut this.promises {
            if promise.ready(cx) {
                match promise {
                    Promise::Resolved(_) => {
                        if let Promise::Resolved(val) = replace(promise, Promise::Consumed) {
                            return Poll::Ready(Ok(val));
                        }
                    }

                    // If it is Consumed or Rejected, we leave it in the list to collect later.
                    Promise::Consumed | Promise::Rejected(_) => {}

                    // Should not happen if ready() returned true, but handle safely
                    Promise::Pending(_) => n_pending += 1,
                }
            } else {
                // ready() returned false -> promise is Pending
                n_pending += 1;
            }
        }

        if n_pending > 0 {
            return Poll::Pending;
        }

        // Pending promises return Poll::Pending,
        // Resolved promises return earlier,
        // Rejected | Consumed promises remain.

        let mut errors = Vec::new();

        for promise in this.promises.drain(..) {
            if let Promise::Rejected(err) = promise {
                errors.push(err);
            }
        }

        Poll::Ready(Err(errors))
    }
}

pub struct PromiseAny<T, E>
where
    T: Send + Unpin + Sync + 'static,
    E: PromiseRejection,
{
    promises: Vec<Promise<T, E>>,
}

impl<I, T, E> From<I> for PromiseAny<T, E>
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

#[cfg(test)]
mod tests {
    use std::task::{Context, Waker};

    use crate::{Promise, PromiseRejection};

    #[derive(thiserror::Error, Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
    enum E {
        #[error("Promise already consumed.")]
        AlreadyConsumed,
        #[error("Code: {0}")]
        Code(i32),
    }

    impl PromiseRejection for E {
        fn already_consumed() -> Self {
            Self::AlreadyConsumed
        }
    }

    fn cx() -> Context<'static> {
        Context::from_waker(Waker::noop())
    }

    #[test]
    fn empty() {
        let mut all: Promise<(), Vec<E>> = Promise::any([]);

        all.ready(&mut cx());

        if let Promise::Rejected(v) = all {
            assert!(v.is_empty(), "Result vector is not empty!")
        } else {
            panic!("Invalid state for empty input: {:?}", all);
        }
    }

    #[test]
    fn resolving() {
        let mut all = Promise::any([
            Promise::new(async { Err(E::Code(1)) }),
            Promise::new(async { Ok(2) }),
            Promise::new(async { Err(E::Code(3)) }),
        ]);

        all.ready(&mut cx());

        if let Promise::Resolved(v) = all {
            assert_eq!(v, 2);
        } else {
            panic!("Expected Resolved(2), got {:?}", all);
        }
    }

    #[test]
    fn rejecting() {
        let mut all: Promise<(), Vec<E>> = Promise::any([
            Promise::new(async { Err(E::Code(1)) }),
            Promise::new(async { Err(E::Code(2)) }),
            Promise::new(async { Err(E::Code(3)) }),
        ]);

        all.ready(&mut cx());

        if let Promise::Rejected(v) = all {
            assert_eq!(v, [1, 2, 3].map(E::Code));
        } else {
            panic!("Expected Rejected([1,2,3]), got {:?}", all);
        }
    }
}
