# once-fn

Make a function run only once: every later call returns the cached result of the first run. Sync functions, `async fn` and methods in impl blocks are all supported.

## Example

```rust
use once_fn::once;

#[once]
fn foo(b: bool) -> bool {
    b
}

assert!(foo(true));  // runs the body and caches `true`
assert!(foo(false)); // returns the cached `true`, the body is not run again
```

`async fn` works the same way, runtime-agnostic — the cache is [`AsyncOnceCell`](https://docs.rs/once-fn/latest/once_fn/struct.AsyncOnceCell.html), also exported for direct use. For impl blocks, apply `#[once_impl]` to the block and `#[once]` to the methods; `-> Self` and reference returns work.

## Storage

The declared return type decides what the cache stores and what callers get:

| Form | Callers get | Requires |
| --- | --- | --- |
| `-> T` (default) | a fresh clone of the cached value, every call | `T: Clone` |
| `-> &T` | a reference into the cache, every call the same slot | `T: Sized + Clone` |
| `-> Arc<T>` | a cheap clone (refcount bump); shares one allocation | nothing on `T` |
| `#[once(by_ref)]` | `&'static T` into the cache, no clone at all | `T: 'static` |
| `#[once(resettable)]` | like the default, plus a `<name>_reset()` that drops the cache so the next call reruns | sync free fn, owned return |

```rust
use once_fn::once;

struct Big([u8; 4096]); // does not implement Clone

#[once(by_ref)]
fn big() -> Big {
    Big([0; 4096])
}

let a: &'static Big = big();
assert!(std::ptr::eq(a, big())); // every call borrows the same cached slot
```

`#[once(by_ref)]` is the `LazyLock` pattern without the static boilerplate: the attribute rewrites the declared `-> T` into `-> &'static T`, while the body still returns an owned `T` exactly as written. Prefer `-> Arc<T>` when callers must own the value.

## Semantics

- Only the first call runs the body; arguments of later calls have no effect on the result.
- A failed first attempt caches nothing — a panic, or an async initializer cancelled before it finishes — so the next call runs the body again.
- Concurrent first calls run the body exactly once: sync callers block inside `get_or_init`, async callers park as futures and are woken. The async initializer runs inside the first caller's task; nothing is spawned.
- The cached value lives until process exit; `<name>_reset` is the only way to drop it.
- Reentering a once fn while its body is initializing panics with a clear message instead of deadlocking. Cross-thread reentrancy is not detected and still deadlocks.

## Limitations

Rejected at compile time, with precise diagnostics: generic functions and generic impls (lifetime-only generics are fine), `impl Trait` in argument or return position, `const fn`, `-> &mut T`, DST pointees (`-> &str`, `-> &[T]`), `resettable` on `async fn`/reference returns/inside `#[once_impl]`, and `by_ref` on reference or non-`'static` returns or combined with `resettable`.

See [tests](./once-fn/tests/) for more examples.

## Why not

- `cached::proc_macro::once`: no async fn, no reference returns, no impl blocks. (It does offer TTL expiry and skipping `None`/`Err` results, which this crate does not.)
- `fn-once`: barely documented; its own example does not compile.

## MSRV

1.85
