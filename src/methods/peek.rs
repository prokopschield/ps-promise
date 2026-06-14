use crate::{Promise, PromiseRejection};

impl<T, E> Promise<T, E>
where
    T: Unpin,
    E: PromiseRejection,
{
    /// If settled, borrows the result without consuming the promise.
    /// Returns `None` while pending and once consumed.
    ///
    /// Unlike [`Promise::consume`], this leaves the promise untouched; see also
    /// [`Promise::inspect`] for a callback-based alternative.
    pub const fn peek(&self) -> Option<Result<&T, &E>> {
        match self {
            Self::Resolved(val) => Some(Ok(val)),
            Self::Rejected(err) => Some(Err(err)),
            Self::Pending(_) | Self::Consumed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::Promise;

    #[test]
    fn peeks_resolved_value() {
        let promise = Promise::<i32, ()>::resolve(42);

        assert_eq!(promise.peek(), Some(Ok(&42)));
        assert!(promise.is_resolved());
    }

    #[test]
    fn peeks_rejected_value() {
        let promise = Promise::<i32, ()>::reject(());

        assert_eq!(promise.peek(), Some(Err(&())));
        assert!(promise.is_rejected());
    }

    #[test]
    fn returns_none_while_pending() {
        let promise = Promise::<i32, ()>::lazy(async { Ok(42) });

        assert_eq!(promise.peek(), None);
        assert!(promise.is_pending());
    }
}
