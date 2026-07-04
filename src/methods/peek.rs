use crate::{Promise, State};

impl<T, E> Promise<T, E> {
    /// If settled, borrows the result without consuming the promise.
    /// Returns `None` while pending, once consumed, and after a task failure;
    /// the corresponding rejection value is constructed only upon consumption.
    ///
    /// Unlike [`Promise::consume`], this leaves the promise untouched; see also
    /// [`Promise::inspect`] for a closure-based alternative.
    pub const fn peek(&self) -> Option<Result<&T, &E>> {
        match &self.state {
            State::Resolved(val) => Some(Ok(val)),
            State::Rejected(err) => Some(Err(err)),
            State::Pending(_) | State::Failed(_) | State::Consumed => None,
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

    #[test]
    fn returns_none_after_task_failure() {
        use std::task::{Context, Waker};

        let mut promise = Promise::<i32, ()>::lazy(async { panic!("boom") });

        promise.poll(&mut Context::from_waker(Waker::noop()));

        assert_eq!(promise.peek(), None);
        assert!(promise.is_rejected());
    }
}
