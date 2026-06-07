mod all;
mod any;
mod catch;
mod consume;
#[cfg(any(feature = "smol", feature = "tokio"))]
mod eager;
mod eager_or_lazy;
mod eager_with;
#[cfg(feature = "smol")]
mod eager_with_smol;
#[cfg(feature = "tokio")]
mod eager_with_tokio;
mod is_consumed;
mod is_pending;
mod is_ready;
mod is_rejected;
mod is_resolved;
mod lazy;
mod map;
mod map_err;
mod new;
mod pending;
mod pending_sync;
mod poll;
mod poll_sync;
mod race;
mod ready;
mod ready_sync;
mod reject;
mod resolve;
mod then;
mod wrap;
