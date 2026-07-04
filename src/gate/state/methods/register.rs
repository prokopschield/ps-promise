use std::{sync::PoisonError, task::Waker};

use super::super::GateState;

impl GateState {
    /// Stores `waker` as the outer waker to notify on the next wakeup
    /// request, replacing any previously stored waker.
    pub(in crate::gate) fn register(&self, waker: &Waker) {
        let mut guard = self.waker.lock().unwrap_or_else(PoisonError::into_inner);

        match guard.as_mut() {
            Some(existing) => existing.clone_from(waker),
            None => *guard = Some(waker.clone()),
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
    fn keeps_only_the_most_recent_waker() {
        let state = Arc::new(GateState::new());

        let (counter_a, waker_a) = counting_waker();
        let (counter_b, waker_b) = counting_waker();

        state.register(&waker_a);
        state.register(&waker_b);

        Waker::from(state).wake();

        assert_eq!(counter_a.count.load(Ordering::SeqCst), 0);
        assert_eq!(counter_b.count.load(Ordering::SeqCst), 1);
    }
}
