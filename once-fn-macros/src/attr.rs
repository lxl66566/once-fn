//! Parsing of attribute arguments.

use proc_macro2::TokenStream;
use syn::{Attribute, Token, parse::Parser, punctuated::Punctuated, spanned::Spanned};

/// Arguments of `#[once]`.
#[derive(Default)]
pub(crate) struct OnceAttr {
    /// `#[once(resettable)]`: allow resetting the cache at runtime.
    pub(crate) resettable: bool,
}

const UNEXPECTED_ARG: &str = "unexpected attribute argument, expected `resettable`";

/// Parse the argument list of the `once` attribute macro.
pub(crate) fn parse_once_args(args: TokenStream) -> syn::Result<OnceAttr> {
    if args.is_empty() {
        return Ok(OnceAttr::default());
    }
    let idents = Punctuated::<syn::Ident, Token![,]>::parse_terminated
        .parse2(args.clone())
        .map_err(|_| syn::Error::new(args.span(), UNEXPECTED_ARG))?;
    let mut attr = OnceAttr::default();
    for ident in &idents {
        if ident != "resettable" {
            return Err(syn::Error::new(ident.span(), UNEXPECTED_ARG));
        }
        attr.resettable = true;
    }
    Ok(attr)
}

/// Parse the `#[once(...)]` attribute of a method inside an `#[once_impl]`
/// block.
pub(crate) fn parse_once_attr(attr: &Attribute) -> syn::Result<OnceAttr> {
    let mut once = OnceAttr::default();
    // `#[once]` without arguments
    if matches!(attr.meta, syn::Meta::Path(_)) {
        return Ok(once);
    }
    attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("resettable") {
            once.resettable = true;
            Ok(())
        } else {
            Err(meta.error(UNEXPECTED_ARG))
        }
    })?;
    Ok(once)
}

pub(crate) fn has_once(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("once"))
}
