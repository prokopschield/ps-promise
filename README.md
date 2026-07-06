# ps-promise

ECMAScript-style promises for Rust: owned futures with typed rejections.

A `Promise<T, E>` is a `Future` with `Output = Result<T, E>` that owns its
computation and remembers its outcome. It can be inspected without consuming
the result (`peek`, `is_resolved`, `is_rejected`), polled manually (`poll`,
`poll_sync`), consumed (`consume`), or awaited. Awaiting consumes the result;
consuming a second time yields a synthesized `already_consumed` rejection,
except for task failures, which replay through `task_failed` on every
consumption.
`Promise::shared` produces a handle that, like an ECMAScript promise, can be
awaited any number of times.

## Example

```rust
use ps_promise::{Promise, PromiseRejection, TaskFailure};

#[derive(Debug)]
enum Error {
    AlreadyConsumed,
    TaskFailed(TaskFailure),
}

impl PromiseRejection for Error {
    fn already_consumed() -> Self {
        Self::AlreadyConsumed
    }

    fn task_failed(failure: TaskFailure) -> Self {
        Self::TaskFailed(failure)
    }
}

async fn demo() -> Result<(), Error> {
    let promise: Promise<i32, Error> =
        Promise::lazy(async { Ok(21) }).map(|n| async move { n * 2 });

    assert_eq!(promise.await?, 42);

    Ok(())
}
```

## ECMAScript correspondence

| ECMAScript                | ps-promise                                   |
| ------------------------- | -------------------------------------------- |
| `Promise.resolve(value)`  | `Promise::resolve(value)`                    |
| `Promise.reject(error)`   | `Promise::reject(error)`                     |
| `new Promise(executor)`   | `Promise::new(executor)`                     |
| `Promise.withResolvers()` | `Promise::with_resolvers()`                  |
| `Promise.try(func)`       | `Promise::attempt`, `Promise::attempt_async` |
| `Promise.all(promises)`   | `Promise::all(promises)`                     |
| `Promise.any(promises)`   | `Promise::any(promises)`                     |
| `Promise.race(promises)`  | `Promise::race(promises)`                    |
| `Promise.allSettled(ps)`  | `Promise::all_settled(ps)`                   |
| `promise.then(f)`         | `promise.then(f)`, `promise.map(f)`          |
| `promise.then(f, g)`      | `promise.then_catch(f, g)`                   |
| `promise.catch(f)`        | `promise.catch(f)`, `promise.map_err(f)`     |
| `promise.finally(f)`      | `promise.finally(f)`                         |
| thenable assimilation     | `promise.flatten()`                          |
| awaiting more than once   | `promise.shared()`                           |

Beyond the ECMAScript surface, the crate provides the scheduling
constructors `lazy`, `eager`, and `eager_or_lazy`, plus `timeout`, `sleep`,
`abortable`, `detach`, `unblock` (offloading blocking work), `wrap`, `zip`,
`inspect`, and `inspect_err`.

## Rejections

The rejection type implements the `PromiseRejection` trait, which lets a
promise synthesize rejections in two cases no user value covers: consumption
after the result was already taken, and failure of the underlying task,
e.g. a panic (caught inside the promise body rather than unwinding into the
caller) or a cancellation. Escape hatches exist for error types that do not
model these cases: `()` discards all information, `Vec<E>` lifts rejections
element-wise, `WrappedPromiseRejection<E>` wraps an arbitrary error type,
and the `anyhow` feature implements the trait for `anyhow::Error`.

## Scheduling and Cargo features

Without a runtime feature, every promise is lazy: the wrapped future
progresses only while the promise is polled, and dropping the promise drops
the future. Enabling a runtime feature makes `Promise::eager_or_lazy`, and
every combinator built on it, spawn the future instead (falling back to lazy
when only `tokio` is enabled and no runtime context is active); a spawned
future runs to completion even if the promise is dropped. Because Cargo unifies
features across the whole build graph, any dependency enabling a runtime
feature flips this behavior for every crate in the build; when laziness is
required for correctness, construct the promise with `Promise::lazy`.

- `tokio`: eager scheduling via `tokio::spawn` and tokio-backed timers.
- `smol`: eager scheduling via `smol::spawn` and smol-backed timers.
- `anyhow`: implements `PromiseRejection` for `anyhow::Error`.

## License

GPL-3.0-or-later
