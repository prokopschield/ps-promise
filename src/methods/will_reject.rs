use crate::{Promise, State};

impl<T, E> Promise<T, E> {
    /// Returns `true` if consuming the promise will yield a rejection: it
    /// settled with a rejection or a task failure, or its result has already
    /// been consumed.
    ///
    /// A return value of `false` is definitive only once the promise has
    /// settled: a pending promise may still reject when polled.
    pub const fn will_reject(&self) -> bool {
        matches!(
            self.state,
            State::Rejected(_) | State::Consumed | State::Failed(_)
        )
    }
}

#[cfg(test)]
mod tests {
    use std::task::{Context, Waker};

    use crate::Promise;

    #[test]
    fn rejected_consumed_and_failed_will_reject() {
        let mut consumed = Promise::<i32, ()>::resolve(0);

        consumed.consume();

        let mut failed = Promise::<i32, ()>::lazy(async { panic!("boom") });

        failed.poll(&mut Context::from_waker(Waker::noop()));

        assert!(Promise::<i32, ()>::reject(()).will_reject());
        assert!(consumed.will_reject());
        assert!(failed.will_reject());
    }

    #[test]
    fn pending_and_resolved_will_not_reject() {
        assert!(!Promise::<i32, ()>::lazy(async { Ok(42) }).will_reject());
        assert!(!Promise::<i32, ()>::resolve(42).will_reject());
    }
}
