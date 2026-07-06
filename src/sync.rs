//! Synchronization primitives, resolved to `loom::sync` when the crate is
//! built with `--cfg loom` and to [`std::sync`] otherwise.
//!
//! Only the primitives whose interleavings the loom models explore are
//! aliased. `Arc` and `Weak` stay [`std::sync`] throughout the crate, since
//! waker construction through [`std::task::Wake`] requires
//! [`std::sync::Arc`].
//!
//! The loom models live in `loom` submodules of the modules they cover and
//! run with `RUSTFLAGS="--cfg loom" cargo test --release --lib loom::`.

#[cfg(loom)]
pub use loom::sync::{atomic, Mutex};

#[cfg(not(loom))]
pub use std::sync::{atomic, Mutex};
