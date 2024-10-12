#![warn(clippy::cargo)]

extern crate proc_macro;
use heck::ToShoutySnakeCase;
use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ItemImpl, ReturnType, Type, TypeReference};

/// Parse a function that to be called only once.
///
/// # Returns
///
/// `(static var definition, function)`
/// which type is `(proc_macro2::TokenStream, proc_macro2::TokenStream)`
macro_rules! parse_once_fn {
    ($input: expr) => {
        parse_once_fn!($input,)
    };
    ($input: expr, $($additional_ident: expr),*) => {
        {
            let fn_name = &$input.sig.ident;
            let fn_block = &$input.block;
            let fn_vis = &$input.vis;
            let fn_inputs = &$input.sig.inputs;
            let fn_generics = &$input.sig.generics;
            let fn_where_clause = &$input.sig.generics.where_clause;
            let fn_attrs = &$input.attrs.iter().filter(|attr| !attr.path().is_ident("once")).collect::<Vec<_>>();
            let fn_output = &$input.sig.output;
            let fn_async_marker = &$input.sig.asyncness;
            let fn_unsafe_marker = &$input.sig.unsafety;

            let (fn_return_type, fn_return_type_is_ref) = match &$input.sig.output {
                ReturnType::Type(_, ty) => match **ty {
                    Type::Reference(TypeReference { ref elem, .. }) => (quote! { #elem }, true),
                    _ => (quote! { #ty }, false),
                },
                ReturnType::Default => (quote! { () }, false),
            };

            let additional_ident_string = Vec::<String>::from([$($additional_ident.to_string()),*]).join("_");
            let fn_ident_string = fn_name.to_string().to_uppercase();
            let static_var_name_string = format!("__{}", format!("ONCE_{}_{}", additional_ident_string, fn_ident_string).to_shouty_snake_case());
            let static_var_name = syn::Ident::new(
                &static_var_name_string,
                fn_name.span(),
            );

            let static_var_def = quote! {static #static_var_name: ::std::sync::OnceLock<#fn_return_type> = ::std::sync::OnceLock::new();};
            let fn_def = {
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
                    #(#fn_attrs)*
                    #fn_vis #fn_async_marker #fn_unsafe_marker fn #fn_name #fn_generics(#fn_inputs) #fn_output #fn_where_clause {
                        #body
                    }
                }
            };
            (static_var_def, fn_def)
        }
    }
}

/// Attribute macro to cache the result of a function, ensuring it only runs
/// once.
#[proc_macro_attribute]
pub fn once(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);
    let (static_var_def, expanded) = parse_once_fn!(input);
    TokenStream::from(quote! {
        #static_var_def
        #expanded
    })
}

/// Attribute macro to cache the result of functions in a struct impl block or
/// trait impl block.
#[proc_macro_attribute]
pub fn once_impl(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemImpl);

    let struct_name = &input.self_ty;
    let struct_name_quoted = quote!(#struct_name);
    let impl_trait = &input.trait_;
    let trait_name = &input.trait_.as_ref().map(|t| &t.1).map(|t| quote!(#t));
    let impl_attrs = &input.attrs;

    let impl_output = if let Some((not, impl_output, for_)) = impl_trait {
        quote!(impl #not #impl_output #for_ #struct_name)
    } else {
        quote!(impl #struct_name)
    };

    let mut generated_fns = Vec::new();
    let mut generated_statics = Vec::new();
    for item in input.items.iter() {
        if let syn::ImplItem::Fn(method) = item {
            let is_once = method.attrs.iter().any(|attr| attr.path().is_ident("once"));
            if is_once {
                let (static_var_def, fn_def) = parse_once_fn!(
                    method,
                    struct_name_quoted,
                    trait_name.as_ref().unwrap_or(&quote! {})
                );
                generated_fns.push(fn_def);
                generated_statics.push(static_var_def);
            }
        } else {
            generated_fns.push(quote! {item});
        }
    }
    let gen = quote! {
        #(#generated_statics)*

        #(#impl_attrs)*
        #impl_output {
            #(#generated_fns)*
        }
    };

    gen.into()
}
