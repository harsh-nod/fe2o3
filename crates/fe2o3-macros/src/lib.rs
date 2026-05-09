use proc_macro::TokenStream;
use quote::{format_ident, quote};
use reserved_fe2o3_symbols::{KERNEL_PREFIX, RESERVED_ROOT};
use syn::{GenericParam, ItemFn, parse_macro_input};

#[proc_macro_attribute]
pub fn kernel(attr: TokenStream, item: TokenStream) -> TokenStream {
    if !attr.is_empty() {
        return syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[kernel] does not accept arguments yet",
        )
        .to_compile_error()
        .into();
    }

    let mut input = parse_macro_input!(item as ItemFn);
    let original_ident = input.sig.ident.clone();
    let original_name = original_ident.to_string();

    if original_name.starts_with(RESERVED_ROOT) {
        return syn::Error::new_spanned(
            original_ident,
            format!("function names starting with `{RESERVED_ROOT}` are reserved by fe2o3"),
        )
        .to_compile_error()
        .into();
    }

    if input
        .sig
        .generics
        .params
        .iter()
        .any(|param| matches!(param, GenericParam::Type(_)))
    {
        return syn::Error::new_spanned(
            input.sig.ident,
            "generic kernels are not implemented in the fe2o3 MVP",
        )
        .to_compile_error()
        .into();
    }

    let internal_ident = format_ident!("{KERNEL_PREFIX}{original_name}");
    let marker_ident = format_ident!("__fe2o3_kernel_name_{original_name}");
    input.sig.ident = internal_ident;

    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        #[unsafe(no_mangle)]
        #input

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        pub const #marker_ident: &str = stringify!(#original_ident);
    }
    .into()
}
