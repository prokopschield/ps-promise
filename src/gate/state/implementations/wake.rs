use std::{
    sync::{Arc, PoisonError},
    task::Wake,
};

use crate::sync::atomic::Ordering;

use super::super::GateState;

/// Records the inner promise's wakeup request and notifies the stored outer
/// waker, if any.
///
/// The request flag is set before the waker is taken, so a concurrent poll of
/// the wrapper either observes the flag or is notified through the waker it
/// just stored; the wakeup is never lost. Taking the waker drops repeated
/// wakes: without an intervening poll of the wrapper, only the first wake
/// notifies the consumer.
impl Wake for GateState {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.woke.store(true, Ordering::Release);

        let waker = self
            .waker
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take();

        if let Some(waker) = waker {
            waker.wake();
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        task::{Wake, Waker},
    };

    use super::super::super::GateState;

    struct CountingWaker {
        count: AtomicUsize,
    }

    impl Wake for CountingWaker {
        fn wake(self: Arc<Self>) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.count.fetch_add(1, Ordering::Relaxed);
        }
    }

    fn counting_waker() -> (Arc<CountingWaker>, Waker) {
        let inner = Arc::new(CountingWaker {
            count: AtomicUsize::new(0),
        });

        let waker = Waker::from(inner.clone());

        (inner, waker)
    }

    #[test]
    fn wake_sets_the_request_and_notifies_the_stored_waker() {
        let state = Arc::new(GateState::new());

        assert!(state.take_wake_request());
        assert!(!state.take_wake_request());

        let (counter, waker) = counting_waker();

        state.register(&waker);

        let gate_waker = Waker::from(state.clone());

        gate_waker.wake_by_ref();

        assert!(state.take_wake_request());
        assert_eq!(counter.count.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn repeated_wakes_without_a_repoll_notify_once() {
        let state = Arc::new(GateState::new());

        let (counter, waker) = counting_waker();

        state.register(&waker);

        let gate_waker = Waker::from(state);

        gate_waker.wake_by_ref();
        gate_waker.wake_by_ref();
        gate_waker.wake();

        assert_eq!(counter.count.load(Ordering::SeqCst), 1);
    }
}
