//! Expansion of `#[once]` functions and `#[once_impl]` blocks.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Block, ImplItemFn, ItemFn, ItemImpl, ReturnType, Signature, Type};

/// What the cache stores, derived from the declared return type.
enum Storage {
    /// Cache the returned value; every call gets a clone of it.
    Owned(TokenStream),
    /// Cache the pointee of a reference return; every call gets a reference to
    /// the cached value.
    Pointee(TokenStream),
}

impl Storage {
    fn of(sig: &Signature) -> Storage {
        match &sig.output {
            ReturnType::Default => Storage::Owned(quote! { () }),
            ReturnType::Type(_, ty) => match &**ty {
                Type::Reference(r) => {
                    let elem = &r.elem;
                    Storage::Pointee(quote! { #elem })
                }
                _ => Storage::Owned(quote! { #ty }),
            },
        }
    }

    fn ty(&self) -> &TokenStream {
        match self {
            Storage::Owned(ty) | Storage::Pointee(ty) => ty,
        }
    }
}

/// Expand a free `#[once]` function.
pub(crate) fn expand_free_fn(input: &ItemFn) -> TokenStream {
    let body = cached_body(&input.sig, &input.block);
    let attrs = without_once(&input.attrs);
    let vis = &input.vis;
    let sig = &input.sig;
    quote! { #(#attrs)* #vis #sig #body }
}

/// Expand an `#[once_impl]` block.
pub(crate) fn expand_impl(input: &ItemImpl) -> TokenStream {
    let self_ty = &input.self_ty;
    let unsafety = &input.unsafety;
    let impl_attrs = &input.attrs;
    // split_for_impl keeps only the `<...>` params here; the trait and self
    // types already carry their own arguments as written by the user
    let (impl_generics, _, where_clause) = input.generics.split_for_impl();

    let impl_head = match &input.trait_ {
        Some((not, path, for_)) => {
            quote! { #unsafety impl #impl_generics #not #path #for_ #self_ty #where_clause }
        }
        None => quote! { #unsafety impl #impl_generics #self_ty #where_clause },
    };

    let mut items = Vec::new();
    for item in &input.items {
        match item {
            syn::ImplItem::Fn(method) if has_once(&method.attrs) => {
                items.push(expand_method(method));
            }
            // keep every non-once member verbatim: functions, consts, type
            // aliases, macro calls, ...
            _ => items.push(quote! { #item }),
        }
    }

    quote! {
        #(#impl_attrs)*
        #impl_head {
            #(#items)*
        }
    }
}

/// Expand an `#[once]` method inside an `#[once_impl]` block.
fn expand_method(input: &ImplItemFn) -> TokenStream {
    let body = cached_body(&input.sig, &input.block);
    let attrs = without_once(&input.attrs);
    let vis = &input.vis;
    let defaultness = &input.defaultness;
    let sig = &input.sig;
    quote! { #(#attrs)* #vis #defaultness #sig #body }
}

/// Build the rewritten function body: a body-local cache static plus the
/// `get_or_init` call.
fn cached_body(sig: &Signature, block: &Block) -> TokenStream {
    let storage = Storage::of(sig);
    let ty = storage.ty();
    let init = match &storage {
        // explicit dereference-and-clone via UFCS, so the clone target cannot
        // be confused by method resolution
        Storage::Pointee(_) => {
            quote! { __ONCE.get_or_init(move || ::core::clone::Clone::clone(#block)) }
        }
        Storage::Owned(_) => quote! { __ONCE.get_or_init(move || #block).clone() },
    };
    quote! {
        {
            // `OnceLock` lives in `std`, not `core` (probed on nightly 1.101)
            static __ONCE: ::std::sync::OnceLock<#ty> = ::std::sync::OnceLock::new();
            #init
        }
    }
}

fn has_once(attrs: &[Attribute]) -> bool {
    attrs.iter().any(|attr| attr.path().is_ident("once"))
}

fn without_once(attrs: &[Attribute]) -> Vec<&Attribute> {
    attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("once"))
        .collect()
}
