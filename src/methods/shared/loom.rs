//! Loom models for the [`SharedPromise`](super::SharedPromise) wake protocol.
//!
//! Compiled only under `--cfg loom`; run with
//! `RUSTFLAGS="--cfg loom" cargo test --release --lib loom::`.

#![allow(clippy::expect_used)]

use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

use loom::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use crate::{Promise, PromiseRejection, TaskFailure};

use super::waker::SharedWaker;

#[derive(Debug, Clone, PartialEq)]
enum E {
    AlreadyConsumed,
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

/// Wakes its own waker and stays pending on its first poll, then resolves on
/// its second poll.
struct SelfWakeOnce {
    polls: usize,
}

impl Future for SelfWakeOnce {
    type Output = Result<i32, E>;

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        self.polls += 1;

        if self.polls == 1 {
            cx.waker().wake_by_ref();

            return Poll::Pending;
        }

        Poll::Ready(Ok(42))
    }
}

/// Resolves once `flag` is observed set, and stays pending otherwise.
struct FlagFuture {
    flag: Arc<AtomicBool>,
}

impl Future for FlagFuture {
    type Output = Result<i32, E>;

    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if self.flag.load(Ordering::Acquire) {
            Poll::Ready(Ok(7))
        } else {
            Poll::Pending
        }
    }
}

/// Polls `future` to completion, yielding to the loom scheduler between
/// polls.
///
/// Spin-polls instead of parking through `loom::future::block_on`: a thread
/// terminating while another sits parked in a loom notify wait trips a loom
/// defect (tokio-rs/loom#249), which two concurrent `block_on` consumers
/// reach. Wake delivery to a parked consumer is modeled separately in
/// [`an_external_wake_racing_a_poll_is_never_lost`].
fn drive<F: Future + Unpin>(mut future: F) -> F::Output {
    let mut cx = Context::from_waker(Waker::noop());

    loop {
        if let Poll::Ready(output) = Pin::new(&mut future).poll(&mut cx) {
            return output;
        }

        loom::thread::yield_now();
    }
}

/// Two consumers race on the inner-promise mutex and the `woke` flag; both
/// must observe the settled result.
///
/// The racing consumer hands its result back through a mutex slot instead of
/// a join: `JoinHandle::join` parks on a loom notify, reaching the same loom
/// defect `drive` avoids (tokio-rs/loom#249). The spin-driven consumers blow
/// up the unbounded state space, so this model runs preemption-bounded.
#[test]
fn two_consumers_race_to_drive_the_inner_promise() {
    let mut builder = loom::model::Builder::new();

    builder.preemption_bound = Some(2);

    builder.check(|| {
        let shared = Promise::lazy(SelfWakeOnce { polls: 0 }).shared();
        let consumer = shared.clone();

        let result = Arc::new(loom::sync::Mutex::new(None));
        let slot = result.clone();

        loom::thread::spawn(move || {
            let value = drive(consumer);

            *slot.lock().expect("the result slot must not be poisoned") = Some(value);
        });

        assert_eq!(drive(shared), Ok(42));

        loop {
            let raced = result
                .lock()
                .expect("the result slot must not be poisoned")
                .take();

            if let Some(value) = raced {
                assert_eq!(value, Ok(42));

                break;
            }

            loom::thread::yield_now();
        }
    });
}

/// A shared-waker wake firing concurrently with a poll is never lost: the
/// consumer settles instead of parking forever, which loom would report as a
/// deadlock.
#[test]
fn an_external_wake_racing_a_poll_is_never_lost() {
    loom::model(|| {
        let flag = Arc::new(AtomicBool::new(false));

        let shared = Promise::lazy(FlagFuture { flag: flag.clone() }).shared();
        let state = shared.state.clone();

        let signaller = loom::thread::spawn(move || {
            flag.store(true, Ordering::Release);

            SharedWaker::new_waker(&state).wake();
        });

        assert_eq!(loom::future::block_on(shared), Ok(7));

        signaller
            .join()
            .expect("the signalling thread must not panic");
    });
}
