//! Cache the result of a function so that its body runs only once.
//!
//! The [`once`] attribute caches the result of the annotated function: the
//! first call runs the body, and every later call returns a clone of the cached
//! result without running the body again.
//!
//! # Examples
//!
//! ```
//! use once_fn::once;
//!
//! #[once]
//! pub fn foo(b: bool) -> bool {
//!     b
//! }
//!
//! assert!(foo(true)); // runs the body and caches `true`
//! assert!(foo(false)); // returns the cached `true`, body is not run again
//! ```
//!
//! Reference returns are supported: the cache stores the pointee, and each call
//! returns a reference to the cached value.
//!
//! ```
//! use once_fn::once;
//!
//! #[once]
//! pub fn foo2(b: &bool) -> &bool {
//!     b
//! }
//!
//! let b = true;
//! assert_eq!(foo2(&b), &b);
//! ```
//!
//! In impl blocks, apply [`once_impl`] to the block and [`once`] to the
//! methods.
//!
//! ```
//! use once_fn::{once, once_impl};
//!
//! struct Foo;
//!
//! #[once_impl]
//! impl Foo {
//!     #[once]
//!     pub fn foo(b: bool) -> bool {
//!         b
//!     }
//! }
//!
//! assert!(Foo::foo(true));
//! assert!(Foo::foo(false)); // cached
//! ```
//!
//! Methods inside `#[once_impl]` may return `Self`.
//!
//! ```
//! use once_fn::{once, once_impl};
//!
//! #[derive(Clone, Debug, PartialEq, Eq)]
//! struct Config {
//!     value: u32,
//! }
//!
//! #[once_impl]
//! impl Config {
//!     #[once]
//!     fn first(x: u32) -> Self {
//!         Config { value: x }
//!     }
//! }
//!
//! assert_eq!(Config::first(1), Config { value: 1 });
//! assert_eq!(Config::first(2), Config { value: 1 }); // cached from the first call
//! ```
//!
//! Returning [`std::sync::Arc`] is the pattern for large or non-[`Clone`]
//! values: the cache stores the `Arc`, so every call only bumps the reference
//! counter and the pointee type needs no `Clone`.
//!
//! ```
//! use std::sync::Arc;
//!
//! use once_fn::once;
//!
//! struct Big([u8; 4096]); // does not implement Clone
//!
//! #[once]
//! fn big() -> Arc<Big> {
//!     Arc::new(Big([0; 4096]))
//! }
//!
//! let a = big();
//! let b = big();
//! assert!(Arc::ptr_eq(&a, &b)); // both calls share one allocation
//! assert_eq!(a.0.len(), 4096);
//! ```
//!
//! `#[once(resettable)]` switches to a resettable cache and generates a
//! companion `<function>_reset` function with the same visibility: calling it
//! drops the cached value, so the next call runs the body again.
//!
//! ```
//! use std::sync::atomic::{AtomicUsize, Ordering};
//!
//! use once_fn::once;
//!
//! static RUNS: AtomicUsize = AtomicUsize::new(0);
//!
//! #[once(resettable)]
//! fn stamped() -> usize {
//!     RUNS.fetch_add(1, Ordering::SeqCst) + 1
//! }
//!
//! assert_eq!(stamped(), 1);
//! assert_eq!(stamped(), 1); // cached
//! stamped_reset();
//! assert_eq!(stamped(), 2); // runs again
//! ```
//!
//! # Async
//!
//! `async fn` is supported: the body runs to completion once and the awaited
//! result is cached; every later call returns a clone without re-running the
//! body. If the first call is cancelled before the body finishes, or the body
//! panics, the next call runs the body again.
//!
//! ```
//! use once_fn::once;
//!
//! #[once]
//! async fn answer() -> u32 {
//!     tokio::task::yield_now().await;
//!     42
//! }
//!
//! #[tokio::main]
//! async fn main() {
//!     assert_eq!(answer().await, 42);
//!     assert_eq!(answer().await, 42); // cached
//! }
//! ```
//!
//! The async cache is [`AsyncOnceCell`], a runtime-agnostic asynchronous once
//! cell that can also be used directly.
//!
//! # Panics
//!
//! If a once function is called again while its body is running (directly or
//! indirectly, on the same thread), the reentrant call panics with a clear
//! message instead of deadlocking on the cache. An async once fn awaited again
//! from inside its own initializing body panics the same way.
//!
//! # Limitations
//!
//! The cached value is returned by cloning, so the declared return type must
//! implement [`Clone`]; for a reference return, the pointee must implement
//! `Clone`.
//!
//! The following forms are rejected at compile time:
//!
//! - generic functions and generic impls (lifetime-only generics are fine): the
//!   cache is one static shared by all monomorphizations, so caching would
//!   silently mix instantiations; `impl Trait` in argument or return position
//!   is rejected for the same reason.
//! - `const fn`, `-> &mut T`, and unsized pointees (`-> &str`, `-> &[T]`, `->
//!   &dyn Trait`).
//! - `#[once(resettable)]` on an `async fn`.

mod async_once_cell;

#[doc(inline)]
pub use async_once_cell::AsyncOnceCell;
#[doc(inline)]
pub use once_fn_macros::{once, once_impl};
