//! Synchronization primitives, re-exported through a crate-local alias.
//!
//! The concurrency machinery in `src/methods/shared/` and `src/gate/` names
//! its `Mutex` and atomics through this module, so an alternative
//! implementation can be substituted with a `cfg` switch.

pub use std::sync::{atomic, Mutex};
