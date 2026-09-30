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
//! # Limitations
//!
//! The cached value is returned by cloning, so the declared return type must
//! implement [`Clone`]; for a reference return, the pointee must implement
//! `Clone`.

#[doc(inline)]
pub use once_fn_macros::{once, once_impl};
