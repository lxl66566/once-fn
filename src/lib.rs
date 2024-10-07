extern crate proc_macro;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ReturnType};

/// Attribute macro to cache the result of a function, ensuring it only runs once.
#[proc_macro_attribute]
pub fn once(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let fn_name = &input.sig.ident;
    let fn_block = &input.block;
    let fn_vis = &input.vis;
    let fn_inputs = &input.sig.inputs;
    let fn_generics = &input.sig.generics;
    let fn_attrs = &input.attrs;

    let fn_return_type = match &input.sig.output {
        ReturnType::Type(_, ty) => quote! { #ty },
        ReturnType::Default => quote! { () },
    };

    let static_var_name = syn::Ident::new(
        &format!("__ONCE_{}", fn_name.to_string().to_uppercase()),
        fn_name.span(),
    );

    let expanded = quote! {
            static #static_var_name: std::sync::OnceLock<#fn_return_type> = std::sync::OnceLock::new();

            #(#fn_attrs)*
            #fn_vis fn #fn_name #fn_generics(#fn_inputs) -> #fn_return_type {
                #static_var_name.get_or_init(move || #fn_block).clone()
            }

    };

    TokenStream::from(expanded)
}