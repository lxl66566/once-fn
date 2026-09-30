//! Compile-time diagnostics for `#[once]` functions and `#[once_impl]`
//! blocks. Every check explains why the form is rejected.

use syn::{Error, FnArg, ImplItem, ItemImpl, Signature, Type};

use crate::attr::{OnceAttr, has_once};

/// Check a function signature. `in_impl` marks a method inside an
/// `#[once_impl]` block.
pub(crate) fn check_fn(sig: &Signature, cfg: &OnceAttr, in_impl: bool) -> Vec<Error> {
    let mut errors = Vec::new();

    // generic functions are a semantic trap: a `static` in a generic body is
    // shared by all monomorphizations, so every instantiation would share one
    // cache
    if sig.generics.type_params().next().is_some() || sig.generics.const_params().next().is_some() {
        errors.push(Error::new_spanned(
            &sig.generics.params,
            "generic functions are not supported: all monomorphizations would share one cache",
        ));
    } else if let Some(FnArg::Typed(arg)) = sig
        .inputs
        .iter()
        .find(|arg| matches!(arg, FnArg::Typed(pt) if contains_impl_trait(&pt.ty)))
    {
        errors.push(Error::new_spanned(
            arg,
            "impl Trait in argument position is a hidden generic parameter: all monomorphizations \
             would share one cache",
        ));
    }

    if !in_impl {
        if let Some(FnArg::Receiver(receiver)) = sig
            .inputs
            .iter()
            .find(|arg| matches!(arg, FnArg::Receiver(_)))
        {
            errors.push(Error::new_spanned(
                receiver,
                "this looks like an impl method; use `#[once_impl]` on the impl block",
            ));
        }
    }

    if let Some(ty) = return_ty(sig) {
        if contains_impl_trait(ty) {
            errors.push(Error::new_spanned(
                ty,
                "`impl Trait` in return type is not supported: the cache must store a nameable \
                 type",
            ));
        }
        if let Type::Reference(reference) = ty {
            if reference.mutability.is_some() {
                errors.push(Error::new_spanned(
                    reference,
                    "mutable references cannot be cached: they would allow mutation of the cached \
                     value",
                ));
            } else if is_dst(&reference.elem) {
                errors.push(Error::new_spanned(
                    &reference.elem,
                    "the pointee must be `Sized`",
                ));
            }
        }
    }

    if sig.constness.is_some() {
        errors.push(Error::new_spanned(
            sig.constness,
            "const fn is not supported: the cache requires runtime initialization",
        ));
    }

    if cfg.resettable {
        if in_impl {
            errors.push(Error::new_spanned(
                &sig.ident,
                "resettable is not supported inside `#[once_impl]`: the hoisted cache static \
                 could not name `Self`",
            ));
        }
        if sig.asyncness.is_some() {
            errors.push(Error::new_spanned(
                sig.asyncness,
                "resettable is not supported for async functions",
            ));
        }
        if matches!(return_ty(sig), Some(Type::Reference(_))) {
            errors.push(Error::new_spanned(
                &sig.ident,
                "resettable requires an owned return type",
            ));
        }
    }

    errors
}

/// Check an impl block: any `#[once]` method in a generic impl is rejected
/// because the method's body-local static is shared by all monomorphizations.
pub(crate) fn check_impl(imp: &ItemImpl) -> Vec<Error> {
    let generic =
        imp.generics.type_params().next().is_some() || imp.generics.const_params().next().is_some();
    if !generic {
        return Vec::new();
    }
    imp.items
        .iter()
        .filter_map(|item| match item {
            ImplItem::Fn(method) if has_once(&method.attrs) => method
                .attrs
                .iter()
                .find(|attr| attr.path().is_ident("once")),
            _ => None,
        })
        .map(|attr| {
            Error::new_spanned(
                attr,
                "generic impls are not supported: all monomorphizations of `#[once]` methods \
                 would share one cache",
            )
        })
        .collect()
}

/// Fold a list of errors into one so rustc reports them all.
pub(crate) fn into_error(errors: Vec<Error>) -> Option<Error> {
    let mut iter = errors.into_iter();
    let mut first = iter.next()?;
    for error in iter {
        first.combine(error);
    }
    Some(first)
}

fn return_ty(sig: &Signature) -> Option<&Type> {
    match &sig.output {
        syn::ReturnType::Type(_, ty) => Some(ty),
        syn::ReturnType::Default => None,
    }
}

/// Whether a type syntactically ends in an unsized tail.
fn is_dst(ty: &Type) -> bool {
    match ty {
        Type::Slice(_) | Type::TraitObject(_) => true,
        Type::Path(tp) => {
            // bare `str`
            tp.qself.is_none() && tp.path.get_ident().is_some_and(|ident| ident == "str")
        },
        Type::Tuple(t) => t.elems.last().is_some_and(is_dst),
        _ => false,
    }
}

/// Whether a type contains `impl Trait` anywhere.
fn contains_impl_trait(ty: &Type) -> bool {
    match ty {
        Type::ImplTrait(_) => true,
        Type::Reference(r) => contains_impl_trait(&r.elem),
        Type::Ptr(p) => contains_impl_trait(&p.elem),
        Type::Slice(s) => contains_impl_trait(&s.elem),
        Type::Array(a) => contains_impl_trait(&a.elem),
        Type::Paren(p) => contains_impl_trait(&p.elem),
        Type::Tuple(t) => t.elems.iter().any(contains_impl_trait),
        Type::Path(tp) => tp.path.segments.iter().any(|seg| match &seg.arguments {
            syn::PathArguments::AngleBracketed(a) => a.args.iter().any(|arg| match arg {
                syn::GenericArgument::Type(t) => contains_impl_trait(t),
                _ => false,
            }),
            syn::PathArguments::Parenthesized(p) => {
                p.inputs.iter().any(contains_impl_trait)
                    || matches!(&p.output, syn::ReturnType::Type(_, t) if contains_impl_trait(t))
            },
            syn::PathArguments::None => false,
        }),
        _ => false,
    }
}
