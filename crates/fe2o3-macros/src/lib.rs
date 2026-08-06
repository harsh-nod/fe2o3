use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::{format_ident, quote};
use reserved_fe2o3_symbols::{
    KERNEL_PREFIX, KERNEL_REGISTRATION_KIND_KERNEL, KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1,
    KERNEL_REGISTRATION_MAGIC, KERNEL_REGISTRATION_PREFIX, KERNEL_REGISTRATION_VERSION_V1,
    RESERVED_ROOT,
};
use syn::{
    Data, DeriveInput, FnArg, GenericArgument, ItemFn, Meta, Pat, PathArguments, ReturnType, Type,
    Visibility, parse_macro_input,
};

#[proc_macro_derive(DeviceCopy)]
pub fn derive_device_copy(item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    expand_device_copy(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_device_copy(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let core_import = fe2o3_core_import()?;
    expand_device_copy_with_core_import(input, &core_import)
}

fn expand_device_copy_with_core_import(
    input: DeriveInput,
    core_import: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    if !input.generics.params.is_empty() || input.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            input.generics,
            "DeviceCopy can only be derived for non-generic structs",
        ));
    }

    let fields = match input.data {
        Data::Struct(data) => data.fields,
        Data::Enum(data) => {
            return Err(syn::Error::new_spanned(
                data.enum_token,
                "DeviceCopy cannot be derived for enums",
            ));
        }
        Data::Union(data) => {
            return Err(syn::Error::new_spanned(
                data.union_token,
                "DeviceCopy cannot be derived for unions",
            ));
        }
    };

    validate_device_copy_repr(&input.attrs)?;

    let name = input.ident;
    let field_types = fields.iter().map(|field| &field.ty).collect::<Vec<_>>();

    Ok(quote! {
        const _: () = {
            extern crate core as __fe2o3_device_copy_sysroot_core;
            #core_import

            fn __fe2o3_assert_device_copy<
                T: __fe2o3_device_copy_core::DeviceCopy,
            >() {}
            fn __fe2o3_assert_copy<
                T: __fe2o3_device_copy_sysroot_core::marker::Copy,
            >() {}
            fn __fe2o3_assert_send<
                T: __fe2o3_device_copy_sysroot_core::marker::Send,
            >() {}
            fn __fe2o3_assert_sync<
                T: __fe2o3_device_copy_sysroot_core::marker::Sync,
            >() {}
            fn __fe2o3_assert_static<T: 'static>() {}

            #(let _ = __fe2o3_assert_device_copy::<#field_types>;)*
            let _ = __fe2o3_assert_copy::<#name>;
            let _ = __fe2o3_assert_send::<#name>;
            let _ = __fe2o3_assert_sync::<#name>;
            let _ = __fe2o3_assert_static::<#name>;

            const __FE2O3_FIELD_SIZE_SUM: usize = {
                let sum = 0usize;
                #(
                    let sum = match sum.checked_add(
                        __fe2o3_device_copy_sysroot_core::mem::size_of::<#field_types>(),
                    ) {
                        __fe2o3_device_copy_sysroot_core::option::Option::Some(sum) => sum,
                        __fe2o3_device_copy_sysroot_core::option::Option::None => {
                            __fe2o3_device_copy_sysroot_core::panic!(
                                __fe2o3_device_copy_sysroot_core::concat!(
                                    "DeviceCopy derive field-size sum overflow for `",
                                    __fe2o3_device_copy_sysroot_core::stringify!(#name),
                                    "`",
                                ),
                            )
                        }
                    };
                )*
                sum
            };

            __fe2o3_device_copy_sysroot_core::assert!(
                __FE2O3_FIELD_SIZE_SUM
                    == __fe2o3_device_copy_sysroot_core::mem::size_of::<#name>(),
                __fe2o3_device_copy_sysroot_core::concat!(
                    "DeviceCopy cannot be derived for `",
                    __fe2o3_device_copy_sysroot_core::stringify!(#name),
                    "` because its layout contains internal or trailing padding",
                ),
            );

            // SAFETY: On the compiling host target, the generated obligations
            // require every field to be DeviceCopy, the complete struct to satisfy
            // all DeviceCopy supertraits, and every byte to belong to a field.
            unsafe impl __fe2o3_device_copy_core::DeviceCopy for #name {}
        };
    })
}

fn fe2o3_core_import() -> syn::Result<proc_macro2::TokenStream> {
    let found = crate_name("fe2o3-core").map_err(|error| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("DeviceCopy derive could not resolve the fe2o3-core crate: {error}"),
        )
    })?;

    Ok(core_import_for(found))
}

fn core_import_for(found: FoundCrate) -> proc_macro2::TokenStream {
    match found {
        FoundCrate::Itself => quote!(
            extern crate self as __fe2o3_device_copy_core;
        ),
        FoundCrate::Name(name) => {
            let ident = format_ident!("{name}");
            quote!(extern crate #ident as __fe2o3_device_copy_core;)
        }
    }
}

#[derive(Clone, Copy)]
enum DeviceCopyRepr {
    C,
    Transparent,
}

fn validate_device_copy_repr(attrs: &[syn::Attribute]) -> syn::Result<DeviceCopyRepr> {
    let mut repr = None;

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("repr")) {
        attr.parse_nested_meta(|meta| {
            let candidate = if meta.path.is_ident("C") {
                DeviceCopyRepr::C
            } else if meta.path.is_ident("transparent") {
                DeviceCopyRepr::Transparent
            } else if meta.path.is_ident("packed") {
                return Err(meta.error("DeviceCopy does not support packed representations"));
            } else {
                return Err(
                    meta.error("DeviceCopy requires exactly #[repr(C)] or #[repr(transparent)]")
                );
            };

            if repr.replace(candidate).is_some() {
                return Err(
                    meta.error("DeviceCopy does not support duplicate or conflicting repr hints")
                );
            }
            Ok(())
        })?;
    }

    repr.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "DeviceCopy requires #[repr(C)] or #[repr(transparent)]",
        )
    })
}

#[proc_macro_attribute]
pub fn kernel(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mode = match parse_kernel_mode(attr.into()) {
        Ok(mode) => mode,
        Err(error) => return error.to_compile_error().into(),
    };

    let input = parse_macro_input!(item as ItemFn);

    expand_kernel(input, mode)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KernelMode {
    Basic,
    Typed,
}

fn parse_kernel_mode(attr: proc_macro2::TokenStream) -> syn::Result<KernelMode> {
    if attr.is_empty() {
        return Ok(KernelMode::Basic);
    }

    if let Ok(argument) = syn::parse2::<syn::Ident>(attr.clone())
        && argument == "typed"
    {
        return Ok(KernelMode::Typed);
    }

    Err(syn::Error::new_spanned(
        attr,
        "#[kernel] accepts only #[kernel] or #[kernel(typed)]",
    ))
}

fn expand_kernel(input: ItemFn, mode: KernelMode) -> syn::Result<proc_macro2::TokenStream> {
    if mode == KernelMode::Typed {
        validate_typed_kernel_signature(&input)?;
    }
    validate_kernel_signature(&input)?;

    let device_import = fe2o3_device_import()?;
    let host_import = if mode == KernelMode::Typed {
        Some(fe2o3_host_import()?)
    } else {
        None
    };

    expand_kernel_with_imports(input, mode, &device_import, host_import.as_ref())
}

#[cfg(test)]
fn expand_kernel_with_device_import(
    input: ItemFn,
    device_import: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    expand_kernel_with_imports(input, KernelMode::Basic, device_import, None)
}

fn expand_kernel_with_imports(
    mut input: ItemFn,
    mode: KernelMode,
    device_import: &proc_macro2::TokenStream,
    host_import: Option<&proc_macro2::TokenStream>,
) -> syn::Result<proc_macro2::TokenStream> {
    if mode == KernelMode::Typed {
        validate_typed_kernel_signature(&input)?;
    }
    validate_kernel_signature(&input)?;

    let original_ident = input.sig.ident.clone();
    let original_name = original_ident.to_string();

    if original_name.starts_with(RESERVED_ROOT) {
        return Err(syn::Error::new_spanned(
            original_ident,
            format!("function names starting with `{RESERVED_ROOT}` are reserved by fe2o3"),
        ));
    }

    let internal_ident = format_ident!("{KERNEL_PREFIX}{original_name}");
    let name_marker_ident = format_ident!("__fe2o3_kernel_name_{original_name}");
    let type_marker_ident = format_ident!("__fe2o3_kernel_marker_{original_name}");
    let registration_ident = format_ident!("{KERNEL_REGISTRATION_PREFIX}{original_name}");
    let registration_kind = match mode {
        KernelMode::Basic => KERNEL_REGISTRATION_KIND_KERNEL,
        KernelMode::Typed => KERNEL_REGISTRATION_KIND_TYPED_VECADD_V1,
    };
    let marker_value = syn::LitStr::new(&original_name, original_ident.span());
    let export_value = syn::LitStr::new(&original_name, original_ident.span());
    let argument_types = input
        .sig
        .inputs
        .iter()
        .map(|argument| match argument {
            syn::FnArg::Typed(argument) => (*argument.ty).clone(),
            syn::FnArg::Receiver(_) => unreachable!("free function unexpectedly had a receiver"),
        })
        .collect::<Vec<_>>();
    let visibility = input.vis.clone();
    let safety = input.sig.unsafety;
    let abi = input.sig.abi.clone();
    let output = input.sig.output.clone();
    let function_pointer = quote!(#safety #abi fn(#(#argument_types),*) #output);
    input.sig.ident = internal_ident.clone();

    let typed_module = if mode == KernelMode::Typed {
        let host_import = host_import.expect("typed expansion requires a host import");
        let module_ident = format_ident!("{original_name}_gpu");
        let artifact_start_symbol = syn::LitStr::new(
            &format!("__fe2o3_kernel_artifact_{original_name}_start"),
            original_ident.span(),
        );
        let artifact_end_symbol = syn::LitStr::new(
            &format!("__fe2o3_kernel_artifact_{original_name}_end"),
            original_ident.span(),
        );

        quote! {
            pub mod #module_ident {
                unsafe extern "C" {
                    #[link_name = #artifact_start_symbol]
                    static __FE2O3_ARTIFACT_START: u8;
                    #[link_name = #artifact_end_symbol]
                    static __FE2O3_ARTIFACT_END: u8;
                }

                const _: () = {
                    extern crate core as __fe2o3_kernel_sysroot_core;
                    #host_import

                    // SAFETY: the fe2o3 backend owns these reserved symbols and
                    // binds them to the artifact for this exact generated marker.
                    unsafe impl __fe2o3_kernel_host::__generated::CompilerGeneratedKernelContractV1
                        for super::#type_marker_ident
                    {
                        const PROFILE:
                            __fe2o3_kernel_host::__generated::CompilerGeneratedKernelProfileV1 =
                            __fe2o3_kernel_host::__generated::CompilerGeneratedKernelProfileV1::TypedVecAddF32V1;

                        fn artifact_container_bytes() -> &'static [u8] {
                            let start_pointer =
                                __fe2o3_kernel_sysroot_core::ptr::addr_of!(
                                    __FE2O3_ARTIFACT_START
                                );
                            let end_pointer =
                                __fe2o3_kernel_sysroot_core::ptr::addr_of!(
                                    __FE2O3_ARTIFACT_END
                                );
                            let start_address = start_pointer.addr();
                            let end_address = end_pointer.addr();
                            let __fe2o3_kernel_sysroot_core::option::Option::Some(length) =
                                end_address.checked_sub(start_address)
                            else {
                                return &[];
                            };

                            if length == 0
                                || length
                                    > __fe2o3_kernel_sysroot_core::primitive::isize::MAX as usize
                            {
                                return &[];
                            }

                            // SAFETY: the generated unsafe trait implementation
                            // relies on the backend/linker contract that the ordered
                            // symbols bound one contiguous immutable artifact.
                            unsafe {
                                __fe2o3_kernel_sysroot_core::slice::from_raw_parts(
                                    start_pointer,
                                    length,
                                )
                            }
                        }
                    }
                };
            }
        }
    } else {
        quote! {}
    };

    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        #[unsafe(no_mangle)]
        #input

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        pub const #name_marker_ident: &str = #marker_value;

        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #visibility enum #type_marker_ident {}

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        #[used]
        static #registration_ident: (
            u64,
            u16,
            u16,
            &'static str,
            &'static str,
            #function_pointer,
        ) = (
            #KERNEL_REGISTRATION_MAGIC,
            #KERNEL_REGISTRATION_VERSION_V1,
            #registration_kind,
            #marker_value,
            #export_value,
            #internal_ident,
        );

        const _: () = {
            #device_import

            // SAFETY: every associated value below is generated from the same
            // parsed function and the same collector-visible registration.
            unsafe impl __fe2o3_kernel_device::KernelMarkerV1 for #type_marker_ident {
                type Function = #function_pointer;
                type Registration = (
                    u64,
                    u16,
                    u16,
                    &'static str,
                    &'static str,
                    #function_pointer,
                );

                const LOGICAL_NAME: &'static str = #marker_value;
                const EXPORT_NAME: &'static str = #export_value;
                const FUNCTION: Self::Function = #internal_ident;
                const REGISTRATION: &'static Self::Registration = &#registration_ident;
            }
        };

        #typed_module
    })
}

fn fe2o3_device_import() -> syn::Result<proc_macro2::TokenStream> {
    let found = crate_name("fe2o3-device").map_err(|error| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("#[kernel] could not resolve the fe2o3-device crate: {error}"),
        )
    })?;

    Ok(device_import_for(found))
}

fn device_import_for(found: FoundCrate) -> proc_macro2::TokenStream {
    match found {
        FoundCrate::Itself => quote!(
            extern crate self as __fe2o3_kernel_device;
        ),
        FoundCrate::Name(name) => {
            let ident = format_ident!("{name}");
            quote!(extern crate #ident as __fe2o3_kernel_device;)
        }
    }
}

fn fe2o3_host_import() -> syn::Result<proc_macro2::TokenStream> {
    let found = crate_name("fe2o3-host").map_err(|error| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("#[kernel(typed)] could not resolve the fe2o3-host crate: {error}"),
        )
    })?;

    Ok(host_import_for(found))
}

fn host_import_for(found: FoundCrate) -> proc_macro2::TokenStream {
    match found {
        FoundCrate::Itself => quote!(
            extern crate self as __fe2o3_kernel_host;
        ),
        FoundCrate::Name(name) => {
            let ident = format_ident!("{name}");
            quote!(extern crate #ident as __fe2o3_kernel_host;)
        }
    }
}

fn validate_typed_kernel_signature(input: &ItemFn) -> syn::Result<()> {
    let signature = &input.sig;
    let required_signature =
        "#[kernel(typed)] requires `pub fn(&[f32], &[f32], DisjointSlice<f32>)`";

    if !matches!(input.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &input.vis,
            "#[kernel(typed)] requires a public kernel function",
        ));
    }
    if signature.unsafety.is_some() {
        return Err(syn::Error::new_spanned(
            signature.unsafety,
            "#[kernel(typed)] requires a safe kernel function",
        ));
    }
    if signature.constness.is_some() || signature.asyncness.is_some() || signature.abi.is_some() {
        return Err(syn::Error::new_spanned(
            signature,
            "#[kernel(typed)] requires a non-const synchronous Rust function",
        ));
    }
    if signature.variadic.is_some() {
        return Err(syn::Error::new_spanned(
            &signature.variadic,
            required_signature,
        ));
    }
    if !signature.generics.params.is_empty() || signature.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &signature.generics,
            "#[kernel(typed)] does not support generic kernel functions",
        ));
    }
    if !is_unit_return(&signature.output) {
        return Err(syn::Error::new_spanned(
            &signature.output,
            "#[kernel(typed)] requires the unit return type",
        ));
    }
    if signature.inputs.len() != 3 {
        return Err(syn::Error::new_spanned(
            &signature.inputs,
            required_signature,
        ));
    }

    let arguments = signature.inputs.iter().collect::<Vec<_>>();
    validate_typed_argument(arguments[0], 1, is_shared_f32_slice)?;
    validate_typed_argument(arguments[1], 2, is_shared_f32_slice)?;
    validate_typed_argument(arguments[2], 3, is_disjoint_f32_slice)?;

    Ok(())
}

fn validate_typed_argument(
    argument: &FnArg,
    position: usize,
    type_matches: fn(&Type) -> bool,
) -> syn::Result<()> {
    let FnArg::Typed(argument) = argument else {
        return Err(syn::Error::new_spanned(
            argument,
            "#[kernel(typed)] does not support methods",
        ));
    };

    let Pat::Ident(pattern) = argument.pat.as_ref() else {
        return Err(syn::Error::new_spanned(
            &argument.pat,
            format!("#[kernel(typed)] argument {position} must use an identifier pattern"),
        ));
    };
    if pattern.by_ref.is_some() || pattern.subpat.is_some() {
        return Err(syn::Error::new_spanned(
            pattern,
            format!("#[kernel(typed)] argument {position} must use an identifier pattern"),
        ));
    }

    if !type_matches(&argument.ty) {
        let required_type = if position < 3 {
            "&[f32]"
        } else {
            "DisjointSlice<f32>"
        };
        return Err(syn::Error::new_spanned(
            &argument.ty,
            format!("#[kernel(typed)] argument {position} must have exact type `{required_type}`"),
        ));
    }

    Ok(())
}

fn is_unit_return(output: &ReturnType) -> bool {
    match output {
        ReturnType::Default => true,
        ReturnType::Type(_, output) => {
            matches!(output.as_ref(), Type::Tuple(tuple) if tuple.elems.is_empty())
        }
    }
}

fn is_shared_f32_slice(ty: &Type) -> bool {
    let Type::Reference(reference) = ty else {
        return false;
    };
    if reference.lifetime.is_some() || reference.mutability.is_some() {
        return false;
    }
    let Type::Slice(slice) = reference.elem.as_ref() else {
        return false;
    };

    is_exact_f32(&slice.elem)
}

fn is_disjoint_f32_slice(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return false;
    }
    let segment = &path.path.segments[0];
    if segment.ident != "DisjointSlice" {
        return false;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return false;
    };
    if arguments.colon2_token.is_some() || arguments.args.len() != 1 {
        return false;
    }

    matches!(arguments.args.first(), Some(GenericArgument::Type(ty)) if is_exact_f32(ty))
}

fn is_exact_f32(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    path.qself.is_none()
        && path.path.leading_colon.is_none()
        && path.path.segments.len() == 1
        && path.path.segments[0].ident == "f32"
        && matches!(path.path.segments[0].arguments, PathArguments::None)
}

fn validate_kernel_signature(input: &ItemFn) -> syn::Result<()> {
    let signature = &input.sig;

    if signature.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            signature.asyncness,
            "async kernels cannot be represented by the v1 kernel marker contract",
        ));
    }
    if signature.variadic.is_some() {
        return Err(syn::Error::new_spanned(
            &signature.variadic,
            "variadic kernels cannot be represented by the v1 kernel marker contract",
        ));
    }
    if !signature.generics.params.is_empty() || signature.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &signature.generics,
            "generic kernels cannot be represented by the v1 kernel marker contract",
        ));
    }

    for argument in &signature.inputs {
        match argument {
            FnArg::Receiver(receiver) => {
                return Err(syn::Error::new_spanned(
                    receiver,
                    "kernel methods cannot be represented by the v1 kernel marker contract",
                ));
            }
            FnArg::Typed(argument) if matches!(argument.ty.as_ref(), Type::ImplTrait(_)) => {
                return Err(syn::Error::new_spanned(
                    &argument.ty,
                    "impl Trait kernel arguments cannot be represented by the v1 kernel marker contract",
                ));
            }
            FnArg::Typed(_) => {}
        }
    }

    if let ReturnType::Type(_, output) = &signature.output
        && matches!(output.as_ref(), Type::ImplTrait(_))
    {
        return Err(syn::Error::new_spanned(
            output,
            "impl Trait kernel results cannot be represented by the v1 kernel marker contract",
        ));
    }

    for attr in &input.attrs {
        let controls_export = attr.path().is_ident("no_mangle")
            || attr.path().is_ident("export_name")
            || (attr.path().is_ident("unsafe")
                && attr.parse_args::<Meta>().is_ok_and(|meta| {
                    meta.path().is_ident("no_mangle") || meta.path().is_ident("export_name")
                }));
        if controls_export {
            return Err(syn::Error::new_spanned(
                attr,
                "#[kernel] controls the exported symbol name; remove no_mangle or export_name",
            ));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        KernelMode, core_import_for, device_import_for, expand_device_copy_with_core_import,
        expand_kernel_with_device_import, expand_kernel_with_imports, host_import_for,
        parse_kernel_mode, validate_typed_kernel_signature,
    };
    use proc_macro_crate::FoundCrate;
    use syn::{ItemFn, parse_quote};

    #[test]
    fn derive_emits_all_safety_obligations() {
        let input = parse_quote! {
            #[repr(C)]
            struct Pair {
                left: u32,
                right: u32,
            }
        };

        let core_import = core_import_for(FoundCrate::Name("fe2o3_core".to_string()));
        let expansion = expand_device_copy_with_core_import(input, &core_import)
            .unwrap()
            .to_string();

        assert!(expansion.contains("extern crate core as __fe2o3_device_copy_sysroot_core"));
        assert!(expansion.contains("extern crate fe2o3_core as __fe2o3_device_copy_core"));
        assert!(expansion.contains("__fe2o3_assert_device_copy"));
        assert!(expansion.contains("__fe2o3_assert_copy"));
        assert!(expansion.contains("__fe2o3_assert_send"));
        assert!(expansion.contains("__fe2o3_assert_sync"));
        assert!(expansion.contains("__fe2o3_assert_static"));
        assert!(expansion.contains("checked_add"));
        assert!(expansion.contains("size_of"));
        assert!(expansion.contains("__fe2o3_device_copy_sysroot_core :: concat"));
        assert!(expansion.contains("__fe2o3_device_copy_sysroot_core :: stringify"));
        assert!(!expansion.contains(":: core"));
    }

    #[test]
    fn renamed_dependency_path_is_used_in_generated_impl() {
        let input = parse_quote! {
            #[repr(C)]
            struct Pair(u32);
        };
        let core_import = core_import_for(FoundCrate::Name("renamed_core".to_string()));

        let expansion = expand_device_copy_with_core_import(input, &core_import)
            .unwrap()
            .to_string();

        assert!(expansion.contains("extern crate renamed_core as __fe2o3_device_copy_core"));
        assert!(expansion.contains("unsafe impl __fe2o3_device_copy_core :: DeviceCopy"));
        assert!(!expansion.contains("extern crate fe2o3_core as __fe2o3_device_copy_core"));
    }

    #[test]
    fn core_package_import_binds_self_inside_fe2o3_core() {
        assert_eq!(
            core_import_for(FoundCrate::Itself).to_string(),
            "extern crate self as __fe2o3_device_copy_core ;"
        );
    }

    #[test]
    fn kernel_emits_v1_registration_directly_associated_with_export() {
        let input = parse_quote! {
            pub fn saxpy(alpha: f32, input: &[f32]) -> f32 {
                alpha + input[0]
            }
        };

        let device_import = device_import_for(FoundCrate::Name("renamed_device".to_string()));
        let expansion = expand_kernel_with_device_import(input, &device_import)
            .unwrap()
            .to_string();

        assert!(expansion.contains("fn fe2o3_kernel_saxpy"));
        assert!(expansion.contains("unsafe (no_mangle)"));
        assert!(expansion.contains("enum __fe2o3_kernel_marker_saxpy"));
        assert!(expansion.contains("static __fe2o3_kernel_registration_saxpy"));
        assert!(expansion.contains("# [used]"));
        assert!(expansion.contains("5643655966792762694u64"));
        assert!(expansion.contains("1u16 , 1u16"));
        assert!(!expansion.contains("1u16 , 2u16"));
        assert!(expansion.contains("\"saxpy\" , \"saxpy\" , fe2o3_kernel_saxpy"));
        assert!(expansion.contains("extern crate renamed_device as __fe2o3_kernel_device"));
        assert!(expansion.contains(
            "unsafe impl __fe2o3_kernel_device :: KernelMarkerV1 for __fe2o3_kernel_marker_saxpy"
        ));
        assert!(expansion.contains("type Function = fn (f32 , & [f32]) -> f32"));
        assert!(expansion.contains(
            "const REGISTRATION : & 'static Self :: Registration = & __fe2o3_kernel_registration_saxpy"
        ));
    }

    #[test]
    fn kernel_marker_uses_self_when_expanding_inside_device_crate() {
        assert_eq!(
            device_import_for(FoundCrate::Itself).to_string(),
            "extern crate self as __fe2o3_kernel_device ;"
        );
    }

    #[test]
    fn kernel_mode_accepts_only_empty_or_typed() {
        assert_eq!(
            parse_kernel_mode(quote::quote!()).unwrap(),
            KernelMode::Basic
        );
        assert_eq!(
            parse_kernel_mode(quote::quote!(typed)).unwrap(),
            KernelMode::Typed
        );

        for rejected in [
            quote::quote!(other),
            quote::quote!(typed,),
            quote::quote!(typed = true),
        ] {
            assert_eq!(
                parse_kernel_mode(rejected).unwrap_err().to_string(),
                "#[kernel] accepts only #[kernel] or #[kernel(typed)]"
            );
        }
    }

    #[test]
    fn typed_kernel_emits_guarded_embedded_artifact_contract() {
        let input = parse_quote! {
            pub fn vecadd(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
                let _ = (a, b, &mut c);
            }
        };
        let device_import = device_import_for(FoundCrate::Name("gpu_device".to_string()));
        let host_import = host_import_for(FoundCrate::Name("gpu_host".to_string()));

        let expansion = expand_kernel_with_imports(
            input,
            KernelMode::Typed,
            &device_import,
            Some(&host_import),
        )
        .unwrap()
        .to_string();

        assert!(expansion.contains("pub mod vecadd_gpu"));
        assert!(expansion.contains("1u16 , 2u16"));
        assert!(!expansion.contains("1u16 , 1u16"));
        assert!(expansion.contains("__fe2o3_kernel_artifact_vecadd_start"));
        assert!(expansion.contains("__fe2o3_kernel_artifact_vecadd_end"));
        assert!(expansion.contains("extern crate gpu_host as __fe2o3_kernel_host"));
        assert!(expansion.contains(
            "unsafe impl __fe2o3_kernel_host :: __generated :: CompilerGeneratedKernelContractV1"
        ));
        assert!(expansion.contains(
            "const PROFILE : __fe2o3_kernel_host :: __generated :: CompilerGeneratedKernelProfileV1 = __fe2o3_kernel_host :: __generated :: CompilerGeneratedKernelProfileV1 :: TypedVecAddF32V1"
        ));
        assert!(expansion.contains("fn artifact_container_bytes () -> & 'static [u8]"));
        assert!(expansion.contains("checked_sub"));
        assert!(expansion.contains("length == 0"));
        assert!(expansion.contains("primitive :: isize :: MAX"));
        assert!(expansion.contains("slice :: from_raw_parts"));
        assert!(!expansion.contains("KernelParams"));
        assert!(!expansion.contains("launch_unchecked"));
    }

    #[test]
    fn typed_kernel_requires_the_exact_vecadd_profile() {
        let cases: Vec<(ItemFn, &str)> = vec![
            (
                parse_quote!(
                    fn private(a: &[f32], b: &[f32], c: DisjointSlice<f32>) {}
                ),
                "#[kernel(typed)] requires a public kernel function",
            ),
            (
                parse_quote!(
                    pub unsafe fn unsafe_kernel(a: &[f32], b: &[f32], c: DisjointSlice<f32>) {}
                ),
                "#[kernel(typed)] requires a safe kernel function",
            ),
            (
                parse_quote!(
                    pub fn generic<T>(a: &[f32], b: &[f32], c: DisjointSlice<f32>) {}
                ),
                "#[kernel(typed)] does not support generic kernel functions",
            ),
            (
                parse_quote!(
                    pub fn result(a: &[f32], b: &[f32], c: DisjointSlice<f32>) -> Result<(), ()> {
                        Ok(())
                    }
                ),
                "#[kernel(typed)] requires the unit return type",
            ),
            (
                parse_quote!(
                    pub fn count(a: &[f32], b: &[f32]) {}
                ),
                "#[kernel(typed)] requires `pub fn(&[f32], &[f32], DisjointSlice<f32>)`",
            ),
            (
                parse_quote!(
                    pub fn alias(a: &Floats, b: &[f32], c: DisjointSlice<f32>) {}
                ),
                "#[kernel(typed)] argument 1 must have exact type `&[f32]`",
            ),
            (
                parse_quote!(
                    pub fn element(a: &[u32], b: &[f32], c: DisjointSlice<f32>) {}
                ),
                "#[kernel(typed)] argument 1 must have exact type `&[f32]`",
            ),
            (
                parse_quote!(
                    pub fn order(a: &[f32], b: DisjointSlice<f32>, c: &[f32]) {}
                ),
                "#[kernel(typed)] argument 2 must have exact type `&[f32]`",
            ),
            (
                parse_quote!(
                    pub fn raw(a: *const f32, b: &[f32], c: DisjointSlice<f32>) {}
                ),
                "#[kernel(typed)] argument 1 must have exact type `&[f32]`",
            ),
            (
                parse_quote!(
                    pub fn output(a: &[f32], b: &[f32], c: DisjointSlice<u32>) {}
                ),
                "#[kernel(typed)] argument 3 must have exact type `DisjointSlice<f32>`",
            ),
        ];

        for (input, expected) in cases {
            assert_eq!(
                validate_typed_kernel_signature(&input)
                    .unwrap_err()
                    .to_string(),
                expected,
                "unexpected diagnostic for {}",
                input.sig.ident,
            );
        }
    }

    #[test]
    fn kernel_marker_rejects_unrepresentable_signatures() {
        let device_import = device_import_for(FoundCrate::Name("device".to_string()));
        let cases: Vec<ItemFn> = vec![
            parse_quote!(
                async fn asynchronous() {}
            ),
            parse_quote!(
                fn generic<T>(value: T) {}
            ),
            parse_quote!(
                fn argument(value: impl Copy) {}
            ),
            parse_quote!(
                fn result() -> impl Copy {
                    1u32
                }
            ),
        ];

        for input in cases {
            assert!(expand_kernel_with_device_import(input, &device_import).is_err());
        }
    }
}
