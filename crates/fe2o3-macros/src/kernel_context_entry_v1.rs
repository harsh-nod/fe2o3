//! Logical context entry generation; ordinary kernel bodies are never translated.

use fe2o3_rustc_front::{
    KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1, KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1,
    KERNEL_CONTEXT_FRONTEND_REGISTRATION_PREFIX_V1,
    KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1, KernelContextFrontendContractV1,
    encode_kernel_context_frontend_contract_v1,
};
use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    FnArg, GenericArgument, ItemFn, Pat, PathArguments, ReturnType, Type, TypePath, Visibility,
    parse_quote,
};

use crate::KernelOptions;

pub(super) struct ContextEntryV1 {
    logical: ItemFn,
    source_type: TypePath,
}

pub(super) struct ContextEntryItems<'a> {
    pub physical_root: &'a syn::Ident,
    pub logical_helper: &'a syn::Ident,
    pub nominal_marker: &'a syn::Ident,
}

pub(super) fn validate_typed_profile_v1(
    input: &ItemFn,
    options: &KernelOptions,
) -> syn::Result<()> {
    let physical = physical_signature_v1(input, options)?;
    if physical.sig.inputs.len() == input.sig.inputs.len() {
        crate::validate_typed_kernel_profile_v1(input, options)
    } else {
        crate::model_general_typed_signature_v1(&physical, options, [0; 32]).map(|_| ())
    }
}

pub(super) fn physical_signature_v1(
    input: &ItemFn,
    options: &KernelOptions,
) -> syn::Result<ItemFn> {
    let mut physical = input.clone();
    ContextEntryV1::take(&mut physical, options)?;
    Ok(physical)
}

fn context_path(ty: &Type) -> Option<&TypePath> {
    match ty {
        Type::Path(path) if path.path.segments.last()?.ident == "KernelContext" => Some(path),
        Type::Reference(reference) => context_path(&reference.elem),
        Type::Paren(paren) => context_path(&paren.elem),
        Type::Group(group) => context_path(&group.elem),
        _ => None,
    }
}

impl ContextEntryV1 {
    pub(super) fn take(input: &mut ItemFn, options: &KernelOptions) -> syn::Result<Option<Self>> {
        let mut source_type = None;
        for (index, argument) in input.sig.inputs.iter().enumerate() {
            let FnArg::Typed(argument) = argument else {
                continue;
            };
            let Some(path) = context_path(&argument.ty) else {
                continue;
            };
            if index != 0 || source_type.is_some() {
                return Err(syn::Error::new_spanned(
                    argument,
                    "KernelContext must be the first and only logical context argument",
                ));
            }
            let valid_lifetime = path.path.segments.last().is_some_and(|segment| {
                matches!(&segment.arguments, PathArguments::AngleBracketed(args)
                    if args.colon2_token.is_none() && args.args.len() == 1
                        && matches!(args.args.first(), Some(GenericArgument::Lifetime(lifetime))
                            if lifetime.ident == "_"))
            });
            if !matches!(argument.ty.as_ref(), Type::Path(_))
                || path.qself.is_some()
                || !valid_lifetime
            {
                return Err(syn::Error::new_spanned(
                    &argument.ty,
                    "logical context requires KernelContext<'_> by value without explicit brands",
                ));
            }
            source_type = Some(path.clone());
        }
        let Some(source_type) = source_type else {
            return Ok(None);
        };
        if options.control_flow.is_some() || options.unsafe_assembly.is_some() {
            return Err(syn::Error::new_spanned(
                &input.sig,
                "logical context entry does not yet support control_flow or unsafe_asm declarations",
            ));
        }
        if input.sig.unsafety.is_some() || input.sig.abi.is_some() {
            return Err(syn::Error::new_spanned(
                &input.sig,
                "logical context entry requires a safe Rust function",
            ));
        }
        for attr in &input.attrs {
            if ![
                "doc", "allow", "warn", "deny", "forbid", "expect", "inline", "cold", "must_use",
            ]
            .iter()
            .any(|name| attr.path().is_ident(name))
            {
                return Err(syn::Error::new_spanned(
                    attr,
                    "this attribute is not supported on a logical context entry",
                ));
            }
        }
        for argument in &input.sig.inputs {
            let FnArg::Typed(argument) = argument else {
                return Err(syn::Error::new_spanned(
                    argument,
                    "logical context entry requires a free function",
                ));
            };
            if !argument.attrs.is_empty()
                || !matches!(argument.pat.as_ref(), Pat::Ident(pattern)
                    if pattern.by_ref.is_none() && pattern.subpat.is_none())
            {
                return Err(syn::Error::new_spanned(
                    argument,
                    "logical context entry arguments require plain identifier patterns without attributes",
                ));
            }
        }
        let logical = input.clone();
        input.sig.inputs = input.sig.inputs.iter().skip(1).cloned().collect();
        Ok(Some(Self {
            logical,
            source_type,
        }))
    }

    pub(super) fn expand(
        self,
        physical: &mut ItemFn,
        device_path: Option<&TokenStream>,
        items: ContextEntryItems<'_>,
        function_pointer: &TokenStream,
        discard_kernel_result: bool,
    ) -> syn::Result<TokenStream> {
        let ContextEntryItems {
            physical_root: root,
            logical_helper: helper,
            nominal_marker: marker,
        } = items;
        let device = device_path.ok_or_else(|| {
            syn::Error::new_spanned(
                &physical.sig,
                "logical context expansion requires the resolved fe2o3-device crate path",
            )
        })?;
        let mut logical = self.logical;
        let name = logical.sig.ident.to_string();
        let label = syn::LitStr::new(&name, logical.sig.ident.span());
        logical.sig.ident = helper.clone();
        logical.vis = Visibility::Inherited;
        logical.attrs.retain(|attr| !attr.path().is_ident("inline"));
        logical.attrs.push(parse_quote!(#[inline(never)]));
        physical
            .attrs
            .retain(|attr| !attr.path().is_ident("expect"));
        let FnArg::Typed(context) = &mut logical.sig.inputs[0] else {
            unreachable!("validated logical context argument")
        };
        *context.ty = parse_quote!(#device::KernelContext<'_, #marker>);
        let mut argument_names = Vec::new();
        for (ordinal, argument) in physical.sig.inputs.iter_mut().enumerate() {
            let FnArg::Typed(argument) = argument else {
                unreachable!("validated physical argument")
            };
            let Pat::Ident(pattern) = argument.pat.as_mut() else {
                unreachable!("validated physical identifier")
            };
            pattern.mutability = None;
            pattern.ident = format_ident!("__fe2o3_context_argument_{ordinal}");
            argument_names.push(pattern.ident.clone());
        }
        physical.sig.ident = root.clone();
        let call = quote!(#helper(
            #device::KernelContext::<'_, #marker>::__compiler_issue(),
            #(#argument_names),*
        ));
        if discard_kernel_result {
            physical.sig.output = ReturnType::Default;
            *physical.block = parse_quote!({
                let _: #device::KernelResult = #call;
            });
        } else {
            *physical.block = parse_quote!({ #call });
        }

        // Invariance rejects substitutions, including never-to-any coercions.
        let mut checked_type = self.source_type;
        let PathArguments::AngleBracketed(args) = &mut checked_type
            .path
            .segments
            .last_mut()
            .expect("validated context path")
            .arguments
        else {
            unreachable!("validated context lifetime")
        };
        args.args[0] = GenericArgument::Lifetime(parse_quote!('__fe2o3));
        let contract = KernelContextFrontendContractV1::for_generated_kernel(
            root.to_string(),
            helper.to_string(),
            marker.to_string(),
        )
        .map_err(|error| syn::Error::new_spanned(root, error.to_string()))?;
        let bytes = encode_kernel_context_frontend_contract_v1(&contract);
        let registration = format_ident!("{KERNEL_CONTEXT_FRONTEND_REGISTRATION_PREFIX_V1}{name}");
        let bytes_name = format_ident!("__fe2o3_kernel_context_contract_bytes_v1_{name}");
        Ok(quote! {
            #[doc(hidden)]
            #[allow(non_snake_case)]
            #logical

            const _: () = {
                #[allow(dead_code)]
                fn check<'__fe2o3>(
                    value: ::core::marker::PhantomData<fn(#checked_type) -> #checked_type>,
                ) -> ::core::marker::PhantomData<
                    fn(#device::KernelContext<'__fe2o3>) -> #device::KernelContext<'__fe2o3>
                > {
                    value
                }
            };

            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            const #bytes_name: &'static [u8] = &[#(#bytes),*];

            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            #[used]
            static #registration: (u64, u16, u16, &'static str, &'static [u8], #function_pointer) = (
                #KERNEL_CONTEXT_FRONTEND_REGISTRATION_MAGIC_V1,
                #KERNEL_CONTEXT_FRONTEND_REGISTRATION_VERSION_V1,
                #KERNEL_CONTEXT_FRONTEND_REGISTRATION_KIND_V1,
                #label,
                #bytes_name,
                #root,
            );
        })
    }
}

#[cfg(test)]
#[path = "kernel_context_entry_tests_v1.rs"]
mod tests;
