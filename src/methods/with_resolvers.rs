use std::{fmt::Debug, sync::Arc};

use async_channel::{Receiver, Sender};

use crate::{Promise, PromiseRejection, TaskFailure};

/// Rejection payload produced when every handle returned by
/// `Promise::with_resolvers` is dropped without settling the promise.
#[derive(thiserror::Error, Debug, Clone, Copy, PartialEq, Eq)]
#[error("all resolver handles were dropped without settling the promise")]
pub struct ResolversDropped;

impl<T, E> Promise<T, E>
where
    T: Send + Unpin + 'static,
    E: PromiseRejection,
{
    /// Creates a pending [`Promise`] along with handles that settle it.
    ///
    /// Mirrors ECMAScript's `Promise.withResolvers`: the promise stays
    /// pending until [`Resolve::resolve`] or [`Reject::reject`] is called,
    /// and the first settlement wins. Both handles are clonable, and any
    /// clone may settle the promise. If every handle, including clones, is
    /// dropped without settling, the promise rejects with
    /// [`ResolversDropped`] wrapped in [`TaskFailure::Error`], mapped
    /// through [`PromiseRejection::task_failed`].
    #[must_use = "Dropping the handles rejects the Promise!"]
    pub fn with_resolvers() -> (Self, Resolve<T, E>, Reject<T, E>) {
        let (sender, receiver): (_, Receiver<Result<T, E>>) = async_channel::bounded(1);

        let promise = Self::lazy(async move {
            receiver.recv().await.unwrap_or_else(|_| {
                Err(E::task_failed(TaskFailure::Error(Arc::new(
                    ResolversDropped,
                ))))
            })
        });

        (
            promise,
            Resolve {
                sender: sender.clone(),
            },
            Reject { sender },
        )
    }
}

/// Resolves the [`Promise`] created by `Promise::with_resolvers`.
///
/// Cloning yields another handle to the same promise; any clone may settle
/// it, and the first settlement wins.
pub struct Resolve<T, E> {
    sender: Sender<Result<T, E>>,
}

impl<T, E> Clone for Resolve<T, E> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<T, E> Debug for Resolve<T, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Resolve").finish_non_exhaustive()
    }
}

impl<T, E> Resolve<T, E> {
    /// Settles the associated [`Promise`] with `value`.
    ///
    /// Ignored if the promise is already settled.
    pub fn resolve(self, value: T) {
        let _ = self.sender.try_send(Ok(value));
    }
}

/// Rejects the [`Promise`] created by `Promise::with_resolvers`.
///
/// Cloning yields another handle to the same promise; any clone may settle
/// it, and the first settlement wins.
pub struct Reject<T, E> {
    sender: Sender<Result<T, E>>,
}

impl<T, E> Clone for Reject<T, E> {
    fn clone(&self) -> Self {
        Self {
            sender: self.sender.clone(),
        }
    }
}

impl<T, E> Debug for Reject<T, E> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Reject").finish_non_exhaustive()
    }
}

impl<T, E> Reject<T, E> {
    /// Settles the associated [`Promise`] with `err`.
    ///
    /// Ignored if the promise is already settled.
    pub fn reject(self, err: E) {
        let _ = self.sender.try_send(Err(err));
    }
}

#[cfg(test)]
mod tests {
    use std::task::{Context, Waker};

    use crate::{Promise, PromiseRejection, TaskFailure};

    #[derive(Debug, PartialEq)]
    enum E {
        AlreadyConsumed,
        Fail,
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

    #[test]
    fn resolve_settles_promise() {
        let (mut promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        resolve.resolve(42);
        promise.ready(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(42)));
    }

    #[test]
    fn reject_settles_promise() {
        let (mut promise, _resolve, reject) = Promise::<i32, E>::with_resolvers();

        reject.reject(E::Fail);
        promise.ready(&mut cx());

        assert_eq!(promise.consume(), Some(Err(E::Fail)));
    }

    #[test]
    fn pending_until_settled() {
        let (mut promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        assert!(promise.pending(&mut cx()));

        resolve.resolve(7);

        assert!(promise.ready(&mut cx()));
        assert_eq!(promise.consume(), Some(Ok(7)));
    }

    #[test]
    fn first_settlement_wins() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        resolve.resolve(1);
        reject.reject(E::Fail);
        promise.ready(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(1)));
    }

    #[test]
    fn rejects_when_handles_dropped() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        drop(resolve);
        drop(reject);
        promise.ready(&mut cx());

        assert_eq!(promise.consume(), Some(Err(E::TaskFailed)));
    }

    #[test]
    fn settles_even_if_other_handle_dropped_later() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        resolve.resolve(9);
        drop(reject);
        promise.ready(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(9)));
    }

    #[test]
    fn clone_can_settle_after_originals_are_dropped() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        let clone = resolve.clone();

        drop(resolve);
        drop(reject);

        clone.resolve(3);
        promise.ready(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(3)));
    }

    #[test]
    fn first_settlement_wins_across_clones() {
        let (mut promise, resolve, _reject) = Promise::<i32, E>::with_resolvers();

        let clone = resolve.clone();

        clone.resolve(1);
        resolve.resolve(2);
        promise.ready(&mut cx());

        assert_eq!(promise.consume(), Some(Ok(1)));
    }

    #[test]
    fn live_clone_keeps_the_promise_pending() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        let clone = reject.clone();

        drop(resolve);
        drop(reject);

        assert!(promise.pending(&mut cx()));

        clone.reject(E::Fail);
        promise.ready(&mut cx());

        assert_eq!(promise.consume(), Some(Err(E::Fail)));
    }

    #[test]
    fn rejects_only_when_every_clone_is_dropped() {
        let (mut promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        let resolve_clone = resolve.clone();
        let reject_clone = reject.clone();

        drop(resolve);
        drop(reject);

        assert!(promise.pending(&mut cx()));

        drop(resolve_clone);
        drop(reject_clone);
        promise.ready(&mut cx());

        assert_eq!(promise.consume(), Some(Err(E::TaskFailed)));
    }

    #[test]
    fn debug_output_names_the_handles() {
        let (_promise, resolve, reject) = Promise::<i32, E>::with_resolvers();

        assert_eq!(format!("{resolve:?}"), "Resolve { .. }");
        assert_eq!(format!("{reject:?}"), "Reject { .. }");
    }
}
