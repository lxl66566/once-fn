//! Procedural macros for the `once-fn` crate. Use the re-exports from
//! `once_fn`.

mod expand;

use proc_macro::TokenStream;
use syn::{ItemFn, ItemImpl, parse_macro_input};

/// Attribute macro to cache the result of a function, ensuring it only runs
/// once.
///
/// See the `once_fn` crate for documentation.
///
/// # Examples
///
/// ```
/// use once_fn::once;
///
/// #[once]
/// fn foo(b: bool) -> bool {
///     b
/// }
///
/// assert!(foo(true));
/// assert!(foo(false)); // body is not run again
/// ```
#[proc_macro_attribute]
pub fn once(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    expand::expand_free_fn(&input).into()
}

/// Attribute macro to cache the result of functions in a struct impl block or
/// trait impl block.
///
/// See the `once_fn` crate for documentation.
///
/// # Examples
///
/// ```
/// use once_fn::{once, once_impl};
///
/// struct Foo;
///
/// #[once_impl]
/// impl Foo {
///     #[once]
///     pub fn foo(b: bool) -> bool {
///         b
///     }
/// }
///
/// assert!(Foo::foo(true));
/// assert!(Foo::foo(false)); // body is not run again
/// ```
#[proc_macro_attribute]
pub fn once_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);
    expand::expand_impl(&input).into()
}
