use crate::{Promise, State};

impl<T, E> Promise<T, E> {
    /// Returns `true` if the promise settled with a task failure.
    ///
    /// Task failures are a subset of rejections; [`Promise::is_rejected`]
    /// also returns `true` for them.
    pub const fn is_failed(&self) -> bool {
        matches!(self.state, State::Failed(_))
    }
}

#[cfg(test)]
mod tests {
    use std::task::{Context, Waker};

    use crate::Promise;

    #[test]
    fn task_failure_is_failed() {
        let mut promise = Promise::<i32, ()>::lazy(async { panic!("boom") });

        promise.poll(&mut Context::from_waker(Waker::noop()));

        assert!(promise.is_failed());
        assert!(promise.is_rejected());
    }

    #[test]
    fn other_states_are_not_failed() {
        let mut consumed = Promise::<i32, ()>::resolve(0);

        consumed.consume();

        assert!(!Promise::<i32, ()>::resolve(42).is_failed());
        assert!(!Promise::<i32, ()>::reject(()).is_failed());
        assert!(!Promise::<i32, ()>::lazy(async { Ok(42) }).is_failed());
        assert!(!consumed.is_failed());
    }
}
