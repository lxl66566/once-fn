//! Expansion of `#[once]` functions and `#[once_impl]` blocks.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    Attribute, Block, GenericArgument, ImplItemFn, ItemFn, ItemImpl, PathArguments, PathSegment,
    ReturnType, Signature, Type, TypePath,
};

/// What the cache stores, derived from the declared return type.
enum Storage {
    /// Cache the returned value; every call gets a clone of it.
    Owned(TokenStream),
    /// Cache the pointee of a reference return; every call gets a reference to
    /// the cached value.
    Pointee(TokenStream),
}

impl Storage {
    /// `self_ty` substitutes `Self` in the stored type; it is `None` for free
    /// functions.
    fn of(sig: &Signature, self_ty: Option<&Type>) -> Storage {
        let subst = |ty: &Type| match self_ty {
            Some(self_ty) => subst_self(ty, self_ty),
            None => quote! { #ty },
        };
        match &sig.output {
            ReturnType::Default => Storage::Owned(quote! { () }),
            ReturnType::Type(_, ty) => match &**ty {
                Type::Reference(r) => {
                    let pointee = subst(&r.elem);
                    Storage::Pointee(pointee)
                }
                _ => Storage::Owned(subst(ty)),
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
    let body = cached_body(&input.sig, &input.block, None);
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
                items.push(expand_method(method, self_ty));
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
fn expand_method(input: &ImplItemFn, self_ty: &Type) -> TokenStream {
    let body = cached_body(&input.sig, &input.block, Some(self_ty));
    let attrs = without_once(&input.attrs);
    let vis = &input.vis;
    let defaultness = &input.defaultness;
    let sig = &input.sig;
    quote! { #(#attrs)* #vis #defaultness #sig #body }
}

/// Build the rewritten function body: a body-local cache static plus the
/// `get_or_init` call.
fn cached_body(sig: &Signature, block: &Block, self_ty: Option<&Type>) -> TokenStream {
    let storage = Storage::of(sig, self_ty);
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

/// Replace every `Self` in a type with the impl's self type. Used only in
/// generated static types: a `static` is an independent item, so `Self` inside
/// one is rejected by E0401 even within an impl method body.
fn subst_self(ty: &Type, self_ty: &Type) -> TokenStream {
    match ty {
        Type::Path(tp) => subst_self_path(tp, self_ty),
        Type::Reference(r) => {
            let lifetime = &r.lifetime;
            let mutability = &r.mutability;
            let elem = subst_self(&r.elem, self_ty);
            quote! { & #lifetime #mutability #elem }
        }
        Type::Ptr(p) => {
            let const_token = &p.const_token;
            let mutability = &p.mutability;
            let elem = subst_self(&p.elem, self_ty);
            quote! { * #const_token #mutability #elem }
        }
        Type::Slice(s) => {
            let elem = subst_self(&s.elem, self_ty);
            quote! { [#elem] }
        }
        Type::Array(a) => {
            let elem = subst_self(&a.elem, self_ty);
            let len = &a.len;
            quote! { [#elem; #len] }
        }
        Type::Paren(p) => {
            let elem = subst_self(&p.elem, self_ty);
            quote! { (#elem) }
        }
        Type::Tuple(t) => {
            let elems = t.elems.iter().map(|e| subst_self(e, self_ty));
            quote! { (#(#elems ,)*) }
        }
        _ => quote! { #ty },
    }
}

fn subst_self_path(tp: &TypePath, self_ty: &Type) -> TokenStream {
    if tp.qself.is_some() {
        // `<Self as Trait>::Assoc` and friends stay as written
        return quote! { #tp };
    }
    let leading = &tp.path.leading_colon;
    let mut segments = Vec::new();
    for (i, seg) in tp.path.segments.iter().enumerate() {
        if i == 0 && seg.ident == "Self" {
            // `Self` cannot carry generic arguments in type position
            segments.push(quote! { #self_ty });
        } else {
            segments.push(subst_self_segment(seg, self_ty));
        }
    }
    let mut path = TokenStream::new();
    for (i, seg) in segments.into_iter().enumerate() {
        if i > 0 {
            path.extend(quote! { :: });
        }
        path.extend(seg);
    }
    quote! { #leading #path }
}

fn subst_self_segment(seg: &PathSegment, self_ty: &Type) -> TokenStream {
    let ident = &seg.ident;
    match &seg.arguments {
        PathArguments::None => quote! { #ident },
        PathArguments::AngleBracketed(a) => {
            let colon2 = &a.colon2_token;
            let args = a.args.iter().map(|arg| match arg {
                GenericArgument::Type(t) => subst_self(t, self_ty),
                other => quote! { #other },
            });
            quote! { #ident #colon2 < #(#args),* > }
        }
        PathArguments::Parenthesized(p) => {
            let inputs = p.inputs.iter().map(|t| subst_self(t, self_ty));
            let output = match &p.output {
                ReturnType::Default => quote! {},
                ReturnType::Type(arrow, ty) => {
                    let ty = subst_self(ty, self_ty);
                    quote! { #arrow #ty }
                }
            };
            quote! { #ident (#(#inputs),*) #output }
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
