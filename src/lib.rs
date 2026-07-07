//! ECMAScript-style promises: owned futures with typed rejections.
//!
//! A [`Promise<T, E>`](Promise) is a [`Future`] with `Output = Result<T, E>`
//! that owns its computation and remembers its outcome. It can be inspected
//! without consuming the result ([`Promise::peek`], [`Promise::is_resolved`],
//! [`Promise::is_rejected`]), polled manually ([`Promise::poll`],
//! [`Promise::poll_sync`]), consumed ([`Promise::consume`]), or awaited.
//! Awaiting consumes the result; consuming a second time yields
//! [`PromiseRejection::already_consumed`], except for task failures, which
//! replay through [`PromiseRejection::task_failed`] on every consumption.
//! For a promise that can be awaited any number of times, like its
//! ECMAScript counterpart, see [`Promise::shared`].
//!
//! The ECMAScript promise API maps onto:
//!
//! - `Promise.resolve` and `Promise.reject`: [`Promise::resolve`] and
//!   [`Promise::reject`]
//! - `new Promise(executor)`: [`Promise::new`]
//! - `Promise.withResolvers`: [`Promise::with_resolvers`]
//! - `Promise.try`: [`Promise::attempt`] and [`Promise::attempt_async`]
//! - `Promise.all`, `Promise.any`, `Promise.race`, and `Promise.allSettled`:
//!   [`Promise::all`], [`Promise::any`], [`Promise::race`], and
//!   [`Promise::all_settled`]
//! - `then`, `catch`, and `finally`: [`Promise::then`], [`Promise::catch`],
//!   and [`Promise::finally`], plus [`Promise::then_catch`] for the
//!   two-argument `then`, and [`Promise::map`], [`Promise::map_err`],
//!   [`Promise::inspect`], and [`Promise::inspect_err`]
//! - thenable assimilation: [`Promise::flatten`]
//!
//! # Rejections
//!
//! The rejection type `E` implements [`PromiseRejection`], which lets a
//! promise synthesize rejections for consumption after the result was
//! already taken and for task failure. Panics inside a promise body are
//! caught and surface as rejections through
//! [`PromiseRejection::task_failed`] instead of unwinding into the caller.
//! See the trait documentation for the provided escape hatches.
//!
//! # Scheduling
//!
//! Without a runtime feature, every promise is lazy: the wrapped future
//! progresses only while the promise is polled, and dropping the promise
//! drops the future. With the `tokio` or `smol` feature enabled,
//! [`Promise::eager_or_lazy`], and every combinator built on it
//! ([`Promise::then`], [`Promise::map`], [`Promise::catch`], and the rest),
//! spawn the future on the runtime instead (falling back to lazy when only
//! `tokio` is enabled and no runtime context is active); a spawned future
//! runs to completion even if the promise is dropped.
//!
//! Because Cargo unifies features across the whole build graph, any
//! dependency enabling `tokio` or `smol` flips this behavior for every
//! crate in the build. Do not rely on combinator laziness for correctness;
//! when laziness is required, construct the promise with [`Promise::lazy`].
//!
//! # Cargo features
//!
//! - `tokio`: eager scheduling via `tokio::spawn` and tokio-backed timers.
//! - `smol`: eager scheduling via `smol::spawn` and smol-backed timers.
//! - `anyhow`: implements [`PromiseRejection`] for `anyhow::Error`.

#![cfg_attr(docsrs, feature(doc_cfg))]

mod features;
mod gate;
mod implementations;
mod methods;
mod rejection;
mod sync;

use std::{future::Future, pin::Pin};

pub use methods::*;
pub use rejection::*;

/// The boxed future a pending [`Promise`] drives to completion.
pub type BoxedPromiseFuture<T, E> = Pin<Box<dyn Future<Output = Result<T, E>> + Send>>;

/// An owned future with a typed rejection, modeled on the ECMAScript
/// `Promise`.
///
/// `Promise<T, E>` implements [`Future`] with `Output = Result<T, E>`. See
/// the [crate-level documentation](crate) for an overview.
#[must_use = "Await this Promise to observe its outcome, or call `.detach()` to explicitly discard it."]
pub struct Promise<T, E> {
    pub(crate) state: State<T, E>,
}

pub(crate) enum State<T, E> {
    Pending(BoxedPromiseFuture<T, E>),
    Resolved(T),
    Rejected(E),
    Consumed,
    Failed(TaskFailure),
}

/// The README's example, compiled as a doc-test.
#[cfg(doctest)]
#[doc = include_str!("../README.md")]
struct ReadmeDoctests;
