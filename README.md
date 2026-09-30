# once-fn

Make a function run only once: the first call runs the body and caches its result, and every later call returns the cached result without running the body again. Sync functions, `async fn` and methods in impl blocks are all supported.

## Example

Sync function:

```rust
use once_fn::once;

#[once]
fn foo(b: bool) -> bool {
    b
}

assert!(foo(true)); // runs the body and caches `true`
assert!(foo(false)); // returns the cached `true`, the body is not run again
```

`async fn`, runtime-agnostic (no task is spawned, no thread blocks):

```rust
use once_fn::once;

#[once]
async fn answer() -> u32 {
    std::future::ready(()).await; // any async work
    42
}

// call it like any async fn: `answer().await`
```

The async cache is [`AsyncOnceCell`](https://docs.rs/once-fn/latest/once_fn/struct.AsyncOnceCell.html), a runtime-agnostic asynchronous once cell that is also exported for direct use.

Methods in impl blocks, including trait impls; `-> Self` and reference returns work:

```rust
use once_fn::{once, once_impl};

struct Foo;

#[once_impl]
impl Foo {
    #[once]
    pub fn foo(b: bool) -> bool {
        b
    }
}

assert!(Foo::foo(true));
assert!(Foo::foo(false)); // cached
```

## Storage

The declared return type decides what the cache stores:

- Owned return (default): the return type must implement `Clone`; every call returns a fresh clone of the cached value.
- `-> &T`: the cache stores the pointee, which must be `Sized + Clone`, and every call returns a reference into the cache. The returned reference does not point at the value the first caller passed in (the pointee is cloned into the cache); all calls return references to the same cached slot.

  ```rust
  use once_fn::once;

  #[once]
  fn first(input: &u32) -> &u32 {
      input
  }

  let one = 1;
  let a = first(&one);
  let two = 2;
  assert_eq!(first(&two), a); // later arguments are ignored
  ```

- `-> Arc<T>`: the cache stores the `Arc` and each call bumps the reference counter, so `T` does not need `Clone` and callers share one allocation. This is the recommended pattern for large values.

  ```rust
  use std::sync::Arc;

  use once_fn::once;

  struct Big([u8; 4096]); // does not implement Clone

  #[once]
  fn big() -> Arc<Big> {
      Arc::new(Big([0; 4096]))
  }

  let a = big();
  let b = big();
  assert!(Arc::ptr_eq(&a, &b));
  ```

- `#[once(resettable)]` (sync free functions with owned returns only) switches to a resettable cache and generates a `<name>_reset` function with the same visibility. Resetting drops the old value, so the next call runs the body again.

  ```rust
  use once_fn::once;

  #[once(resettable)]
  fn stamp() -> u32 {
      // expensive computation
      7
  }

  assert_eq!(stamp(), 7);
  assert_eq!(stamp(), 7); // cached
  stamp_reset();
  assert_eq!(stamp(), 7); // runs the body again
  ```

## Semantics

- Only the first call runs the body. Later calls still evaluate their arguments at the call site, but the body does not run, so the arguments have no effect on the result.
- If the first call panics, nothing is cached and the next call runs the body again; the body of a once fn may therefore run more than once in the presence of panics or cancellations. For `async fn`, dropping the initializing future before it finishes (task abort, timeout) or a panic in the body rolls the cache back the same way.
- Concurrent first calls run the body exactly once. The sync path blocks inside `get_or_init` until the value is available; the async path parks concurrent callers as futures and wakes them when the value is stored, so no thread blocks.
- `Err` is cached like any other return value: after a first call that returns `Err(..)`, every later call returns the same `Err` again. Use `#[once(resettable)]` if failures must be retried.
- The cached value lives until process exit and is never dropped; `<name>_reset` is the only way to drop it.
- A once method's cache is one static per function, shared across all instances of the type: `a.load()` and `b.load()` return the same cached value.
- Reentrancy: calling a once fn again while its body is running on the same thread (or awaiting an async once fn from inside its own initializing body) panics with a clear message instead of deadlocking. Cross-thread reentrancy, where thread A is initializing and thread B re-enters and the two end up waiting on each other, is not detected and still deadlocks.
- The async initializer runs inside the first caller's task context: nothing is spawned and no thread blocks, but synchronous stretches of the body run on that task. Awaiting other once fns inside the body is supported (each function has its own cell).

## Limitations

The following forms are rejected at compile time with precise diagnostics:

- Type or const generic functions, and generic impls containing `#[once]` methods (lifetime-only generics are fine): one cache would be shared by all monomorphizations. `impl Trait` in argument or return position is rejected for the same reason.
- `const fn`: the cache requires runtime initialization.
- `-> &mut T` returns: they would allow mutation of the cached value.
- DST pointees such as `-> &str`, `-> &[T]` or `-> &dyn Trait`: the pointee must be `Sized`.
- `#[once(resettable)]` on `async fn`, on reference returns, or inside `#[once_impl]`.
- Applying `#[once]` to a whole impl block (use `#[once_impl]` instead), a free function with a `self` receiver (use `#[once_impl]`), unrecognized attribute arguments, and arguments passed to `#[once_impl]` itself.

See [tests](./once-fn/tests/) for more examples.

## MSRV

1.85 (edition 2024).

## Why not

- `cached::proc_macro::once`: the closest equivalent; it also caches a single value and ignores later arguments. It offers TTL expiry (`ttl_secs`), `force_refresh`, skipping `None`/`Err` results and companion functions, which this crate does not. In exchange, once-fn supports reference returns, applies to whole impl blocks via `#[once_impl]` while preserving the other members, can drop and recompute the cache at runtime via `#[once(resettable)]`, and rejects unsupported forms with precise compile-time diagnostics; `cached` requires the return type to be owned and `Clone`.
- `fn-once`: a `FnOnce`-style "call at most once" macro for a different purpose, not result caching, and it ships no documentation.
