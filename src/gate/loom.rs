//! Loom models for the [`GateState`](super::state::GateState) wake protocol.
//!
//! Compiled only under `--cfg loom`; run with
//! `RUSTFLAGS="--cfg loom" cargo test --release --lib loom::`.

#![allow(clippy::expect_used)]

use std::{
    sync::Arc,
    task::{Wake, Waker},
};

use loom::sync::atomic::{AtomicBool, Ordering};

use super::state::GateState;

/// An outer waker that records whether it was notified.
struct NotifyWaker {
    notified: AtomicBool,
}

impl Wake for NotifyWaker {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.notified.store(true, Ordering::SeqCst);
    }
}

/// A wake racing a poll of the wrapper is never lost: the poll either
/// observes the wakeup request or is notified through the waker it just
/// stored, exactly as documented on `Wake for GateState`.
#[test]
fn a_wake_racing_a_poll_is_never_lost() {
    loom::model(|| {
        let state = Arc::new(GateState::new());

        assert!(state.take_wake_request());

        let gate_waker = Waker::from(state.clone());

        let signaller = loom::thread::spawn(move || gate_waker.wake());

        let notify = Arc::new(NotifyWaker {
            notified: AtomicBool::new(false),
        });
        let outer = Waker::from(notify.clone());

        state.register(&outer);

        let observed = state.take_wake_request();

        signaller
            .join()
            .expect("the signalling thread must not panic");

        assert!(observed || notify.notified.load(Ordering::SeqCst));
    });
}
