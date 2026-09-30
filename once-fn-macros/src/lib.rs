//! Procedural macros for the `once-fn` crate. Use the re-exports from
//! `once_fn`.

mod attr;
mod check;
mod expand;

use proc_macro::TokenStream;
use syn::{ItemFn, ItemImpl};

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
///
/// `async fn` is supported: the awaited result is cached.
///
/// ```
/// use once_fn::once;
///
/// #[once]
/// async fn bar(b: bool) -> bool {
///     std::future::ready(()).await;
///     b
/// }
///
/// fn block_on<F: std::future::Future>(fut: F) -> F::Output {
///     use std::sync::Arc;
///     use std::task::{Context, Poll, Wake, Waker};
///     use std::thread::{self, Thread};
///
///     struct ThreadWaker(Thread);
///
///     impl Wake for ThreadWaker {
///         fn wake(self: Arc<Self>) {
///             self.0.unpark();
///         }
///     }
///
///     let mut fut = Box::pin(fut);
///     let waker = Waker::from(Arc::new(ThreadWaker(thread::current())));
///     let mut cx = Context::from_waker(&waker);
///     loop {
///         match fut.as_mut().poll(&mut cx) {
///             Poll::Ready(v) => return v,
///             Poll::Pending => thread::park(),
///         }
///     }
/// }
///
/// assert!(block_on(bar(true)));
/// assert!(block_on(bar(false))); // body is not run again
/// ```
///
/// `#[once(resettable)]` switches to a resettable cache and generates a
/// `<function>_reset` companion function with the same visibility: calling it
/// drops the cached value, so the next call runs the body again.
///
/// ```
/// use once_fn::once;
///
/// #[once(resettable)]
/// fn stamped() -> usize {
///     7
/// }
///
/// assert_eq!(stamped(), 7);
/// stamped_reset();
/// assert_eq!(stamped(), 7); // runs the body again
/// ```
#[proc_macro_attribute]
pub fn once(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_once(attr, item)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

/// Attribute macro to cache the result of functions in a struct impl block or
/// trait impl block.
///
/// Every non-`#[once]` member of the block is preserved verbatim; `#[once]`
/// methods may return `Self`, and each once method's cache is shared across
/// all instances of the type.
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
pub fn once_impl(attr: TokenStream, item: TokenStream) -> TokenStream {
    expand_once_impl(attr, item)
        .unwrap_or_else(|e| e.to_compile_error())
        .into()
}

fn expand_once(attr: TokenStream, item: TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    let cfg = attr::parse_once_args(attr.into())?;

    let tokens: proc_macro2::TokenStream = item.into();
    let fn_item = match syn::parse2::<ItemFn>(tokens.clone()) {
        Ok(fn_item) => fn_item,
        Err(fn_err) => {
            // `#[once]` on a whole impl block is a common mistake
            if syn::parse2::<ItemImpl>(tokens).is_ok() {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "apply `#[once]` to methods inside an `#[once_impl]` block",
                ));
            }
            return Err(fn_err);
        }
    };

    if let Some(error) = check::into_error(check::check_fn(&fn_item.sig, &cfg, false)) {
        return Err(error);
    }

    Ok(expand::expand_free_fn(&fn_item, &cfg))
}

fn expand_once_impl(attr: TokenStream, item: TokenStream) -> syn::Result<proc_macro2::TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            proc_macro2::TokenStream::from(attr),
            "`once_impl` does not accept any attribute arguments",
        ));
    }

    let input: ItemImpl = syn::parse(item)?;

    let mut errors = check::check_impl(&input);
    for item in &input.items {
        if let syn::ImplItem::Fn(method) = item {
            if let Some(once_attr) = method.attrs.iter().find(|a| a.path().is_ident("once")) {
                let cfg = attr::parse_once_attr(once_attr)?;
                errors.extend(check::check_fn(&method.sig, &cfg, true));
            }
        }
    }
    if let Some(error) = check::into_error(errors) {
        return Err(error);
    }

    Ok(expand::expand_impl(&input))
}
