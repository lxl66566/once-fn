#![warn(clippy::cargo)]

extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ReturnType, Type, TypeReference};

/// Attribute macro to cache the result of a function, ensuring it only runs
/// once.
#[proc_macro_attribute]
pub fn once(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let fn_name = &input.sig.ident;
    let fn_block = &input.block;
    let fn_vis = &input.vis;
    let fn_inputs = &input.sig.inputs;
    let fn_generics = &input.sig.generics;
    let fn_where_clause = &input.sig.generics.where_clause;
    let fn_attrs = &input.attrs;
    let fn_output = &input.sig.output;
    let fn_async_marker = input.sig.asyncness;
    let fn_unsafe_marker = input.sig.unsafety;

    let (fn_return_type, fn_return_type_is_ref) = match &input.sig.output {
        ReturnType::Type(_, ty) => match **ty {
            Type::Reference(TypeReference { ref elem, .. }) => (quote! { #elem }, true),
            _ => (quote! { #ty }, false),
        },
        ReturnType::Default => (quote! { () }, false),
    };

    let static_var_name = syn::Ident::new(
        &format!("__ONCE_{}", fn_name.to_string().to_uppercase()),
        fn_name.span(),
    );

    let expanded = {
        // use different body if the return type is a reference
        let body = if fn_return_type_is_ref {
            quote! {
                #static_var_name.get_or_init(move || #fn_block.clone())
            }
        } else {
            quote! {
                #static_var_name.get_or_init(move || #fn_block).clone()
            }
        };
        quote! {
            static #static_var_name: ::std::sync::OnceLock<#fn_return_type> = ::std::sync::OnceLock::new();

            #(#fn_attrs)*
            #fn_vis #fn_async_marker #fn_unsafe_marker fn #fn_name #fn_generics(#fn_inputs) #fn_output #fn_where_clause {
                #body
            }
        }
    };

    TokenStream::from(expanded)
}
