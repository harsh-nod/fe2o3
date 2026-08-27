#![feature(proc_macro_tracked_env)]

mod control_flow_v1;

use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize,
    DigestBytes, Dimensions, LaunchContract, Mutability, Name, PointerWidth,
    RustDisjointIndexSpaceV1, RustLayoutEvidenceV1, RustPhysicalComponentKindV1,
    RustPhysicalComponentV1, RustPointerMutabilityV1, RustScalarElementTypeV1,
    RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1, ScalarType, TypeIdentity,
    derive_generated_host_contract_identity_v1,
};
use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::{format_ident, quote};
use reserved_fe2o3_symbols::{
    CRATE_BINDING_ID_ENV_V1, CrateBindingIdV1, GeneratedHostContractIdV3, KERNEL_PREFIX,
    KERNEL_REGISTRATION_KIND_KERNEL, KERNEL_REGISTRATION_KIND_TYPED_GENERAL_LAYOUT_V3,
    KERNEL_REGISTRATION_MAGIC, KERNEL_REGISTRATION_PREFIX, KERNEL_REGISTRATION_VERSION_V1,
    KERNEL_REGISTRATION_VERSION_V3, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
    REFERENCE_BINDING_REGISTRATION_KIND_V1, REFERENCE_BINDING_REGISTRATION_MAGIC_V1,
    REFERENCE_BINDING_REGISTRATION_PREFIX_V1, REFERENCE_BINDING_REGISTRATION_VERSION_V1,
    RESERVED_ROOT, TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3, derive_kernel_binding_id_v1,
    host_kernel_symbol_v1,
};
use syn::{
    Data, DeriveInput, Expr, ExprArray, FnArg, ForeignItem, GenericArgument, ItemFn,
    ItemForeignMod, Lit, LitInt, Meta, Pat, PathArguments, ReturnType, Token, Type, TypePath,
    Visibility, parse::Parse, parse::ParseStream, parse::Parser, parse_macro_input, parse_quote,
    punctuated::Punctuated, visit::Visit,
};

use crate::control_flow_v1::{
    CONTROL_FLOW_REGISTRATION_KIND_V1, CONTROL_FLOW_REGISTRATION_MAGIC_V1,
    CONTROL_FLOW_REGISTRATION_PREFIX_V1, CONTROL_FLOW_REGISTRATION_VERSION_V1,
    ParsedControlFlowOptionsV1, analyze_kernel_control_flow_v1, lower_bounded_for_loops_v1,
    parse_control_flow_options_v1,
};

const SIMULATION_MODE_ENV_V1: &str = "FE2O3_SIMULATION_MODE_V1";
const SIMULATION_ATTEMPT_ENV_V1: &str = "FE2O3_SIMULATION_ATTEMPT_V1";

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

/// Marks a device kernel and emits its collector registration.
///
/// V1 launch declarations use `launch(required = [x, y, z], max = [x, y, z],
/// max_grid = [x, y, z], min_workgroups_per_compute_unit = n)`. Every
/// dimension is nonzero; workgroup products are at most 1024. Exact typed
/// profiles require `required`; `max` may be omitted or identical. `max_grid`
/// defaults to `[u32::MAX, 1, 1]`, and the occupancy field requires `max`.
///
/// Ordinary kernels are safe Rust entry points: their signatures and direct
/// bodies may not contain unsafe Rust syntax. Target assembly is an explicitly
/// separate low-level boundary declared with `unsafe_asm(target = "gfx942",
/// operands(...), options(...), effects(...))` on an `unsafe fn`. The
/// declaration is recorded for later compiler validation and grants no
/// assembly, memory, loading, or launch authority by itself. Raw device FFI is
/// likewise kept behind the separate [`device_import`] and [`device_export`]
/// attributes rather than weakening ordinary kernels.
///
/// Direct source loops and integer `match` expressions require an ordered
/// `control_flow(loop_bounds(...), integer_switches(...))` declaration. Every
/// loop receives one nonzero maximum iteration count and every match receives
/// one fixed-width signed or unsigned discriminant type in lexical order. The
/// macro emits a separate canonical source-CFG sidecar with exact spans and
/// structured break/continue targets. The sidecar is descriptive until a
/// compiler collector authenticates it against MIR.
#[proc_macro_attribute]
pub fn kernel(attr: TokenStream, item: TokenStream) -> TokenStream {
    let options = match parse_kernel_options(attr.into()) {
        Ok(options) => options,
        Err(error) => return error.to_compile_error().into(),
    };

    let input = parse_macro_input!(item as ItemFn);

    expand_kernel(input, options)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

struct CrossCrateImport {
    local_name: syn::Ident,
    marker: TypePath,
}

impl Parse for CrossCrateImport {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let local_name = input.parse()?;
        input.parse::<Token![,]>()?;
        let marker = input.parse()?;
        if !input.is_empty() {
            return Err(input.error("expected a local name followed by one producer marker type"));
        }
        Ok(Self { local_name, marker })
    }
}

#[proc_macro]
pub fn import_kernel(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as CrossCrateImport);
    expand_import_kernel(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_import_kernel(input: CrossCrateImport) -> syn::Result<proc_macro2::TokenStream> {
    let device_import = fe2o3_device_import()?;
    let local_name = input.local_name;
    let marker = input.marker;
    let registration_ident = format_ident!("{KERNEL_REGISTRATION_PREFIX}{local_name}");
    Ok(quote! {
        const _: () = {
            #device_import

            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            #[used]
            static #registration_ident: (
                u64, u16, u16, &'static str, &'static str, &'static str, &'static str,
                <#marker as __fe2o3_kernel_device::KernelMarkerV1>::Function,
            ) = (
                #KERNEL_REGISTRATION_MAGIC,
                <#marker as __fe2o3_kernel_device::CrossCrateTypedKernelV1>::REGISTRATION_VERSION,
                <#marker as __fe2o3_kernel_device::CrossCrateTypedKernelV1>::REGISTRATION_KIND,
                <#marker as __fe2o3_kernel_device::KernelMarkerV1>::LOGICAL_NAME,
                <#marker as __fe2o3_kernel_device::KernelMarkerV1>::EXPORT_NAME,
                <#marker as __fe2o3_kernel_device::CrossCrateTypedKernelV1>::CRATE_BINDING,
                <#marker as __fe2o3_kernel_device::CrossCrateTypedKernelV1>::KERNEL_BINDING,
                <#marker as __fe2o3_kernel_device::KernelMarkerV1>::FUNCTION,
            );
        };
    })
}

#[proc_macro]
pub fn import_device(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as CrossCrateImport);
    expand_import_device(input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

fn expand_import_device(input: CrossCrateImport) -> syn::Result<proc_macro2::TokenStream> {
    let device_import = fe2o3_device_import()?;
    let local_name = input.local_name;
    let marker = input.marker;
    let anchor_ident = format_ident!(
        "{}{}",
        reserved_fe2o3_symbols::CROSS_CRATE_DEVICE_EXPORT_ANCHOR_PREFIX_V1,
        local_name
    );
    let magic = reserved_fe2o3_symbols::CROSS_CRATE_DEVICE_EXPORT_ANCHOR_MAGIC_V1;
    let version = reserved_fe2o3_symbols::CROSS_CRATE_DEVICE_EXPORT_ANCHOR_VERSION_V1;
    Ok(quote! {
        const _: () = {
            #device_import

            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            #[used]
            static #anchor_ident: (
                u64, u16, &'static str,
                <#marker as __fe2o3_kernel_device::CrossCrateDeviceExportV1>::Function,
            ) = (
                #magic,
                #version,
                <#marker as __fe2o3_kernel_device::CrossCrateDeviceExportV1>::CONTRACT_ID,
                <#marker as __fe2o3_kernel_device::CrossCrateDeviceExportV1>::FUNCTION,
            );
        };
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KernelMode {
    Basic,
    Typed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct KernelOptions {
    mode: KernelMode,
    explicit_namespace: Option<CrateBindingIdV1>,
    reference: Option<String>,
    launch: Option<ParsedLaunchBoundsV1>,
    unsafe_assembly: Option<ParsedUnsafeAssemblyV1>,
    control_flow: Option<ParsedControlFlowOptionsV1>,
}

const MAX_TYPED_KERNEL_SYMBOL_STEM_BYTES: usize = 128;
const MAX_WORKGROUP_THREADS_V1: u64 = 1_024;
const MAX_RESIDENT_WORKGROUPS_PER_COMPUTE_UNIT_V1: u16 = 64;
const GENERAL_TYPED_DEFAULT_BLOCK_V1: [u32; 3] = [256, 1, 1];
const GENERAL_TYPED_WAVE64_BLOCK_V1: [u32; 3] = [64, 1, 1];
const GENERAL_TYPED_POINTER_SIZE_V1: u64 = 8;
const GENERAL_TYPED_POINTER_ALIGNMENT_V1: u32 = 8;
const GENERAL_TYPED_SLICE_SIZE_V1: u64 = 16;
const KERNEL_FRONTEND_REGISTRATION_PREFIX_V1: &str = "__fe2o3_kernel_frontend_contract_v1_";
const KERNEL_FRONTEND_REGISTRATION_MAGIC_V1: u64 = u64::from_le_bytes(*b"FE2O3KFA");
const KERNEL_FRONTEND_REGISTRATION_VERSION_V1: u16 = 1;
const KERNEL_FRONTEND_REGISTRATION_KIND_V1: u16 = 1;
const FRONTEND_KERNEL_CONTRACT_MAGIC_V1: [u8; 8] = *b"FE2O3KF\0";

const ASSEMBLY_OPERAND_SGPR_V1: u16 = 0x0001;
const ASSEMBLY_OPERAND_VGPR_V1: u16 = 0x0002;
const ASSEMBLY_OPERAND_IMMEDIATE_V1: u16 = 0x0004;
const ASSEMBLY_OPERAND_ADDRESS_V1: u16 = 0x0008;
const ASSEMBLY_OPTION_NOMEM_V1: u16 = 0x0001;
const ASSEMBLY_OPTION_READONLY_V1: u16 = 0x0002;
const ASSEMBLY_OPTION_PURE_V1: u16 = 0x0004;
const ASSEMBLY_OPTION_PRESERVES_FLAGS_V1: u16 = 0x0008;
const ASSEMBLY_OPTION_NOSTACK_V1: u16 = 0x0010;
const ASSEMBLY_EFFECT_READ_GLOBAL_V1: u16 = 0x0001;
const ASSEMBLY_EFFECT_WRITE_GLOBAL_V1: u16 = 0x0002;
const ASSEMBLY_EFFECT_READ_WORKGROUP_V1: u16 = 0x0004;
const ASSEMBLY_EFFECT_WRITE_WORKGROUP_V1: u16 = 0x0008;
const ASSEMBLY_EFFECT_ATOMIC_V1: u16 = 0x0010;
const ASSEMBLY_EFFECT_BARRIER_V1: u16 = 0x0020;
const ASSEMBLY_EFFECT_CONTROL_FLOW_V1: u16 = 0x0040;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ParsedLaunchBoundsV1 {
    required: Option<[u32; 3]>,
    maximum: Option<[u32; 3]>,
    max_grid: Option<[u32; 3]>,
    min_workgroups_per_compute_unit: Option<u16>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ParsedUnsafeAssemblyV1 {
    target: u16,
    operands: u16,
    options: u16,
    effects: u16,
}

fn parse_kernel_options(attr: proc_macro2::TokenStream) -> syn::Result<KernelOptions> {
    if attr.is_empty() {
        return Ok(KernelOptions {
            mode: KernelMode::Basic,
            explicit_namespace: None,
            reference: None,
            launch: None,
            unsafe_assembly: None,
            control_flow: None,
        });
    }

    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attr.clone())?;
    let mut typed = false;
    let mut explicit_namespace = None;
    let mut reference = None;
    let mut launch = None;
    let mut unsafe_assembly = None;
    let mut control_flow = None;
    for argument in arguments {
        match argument {
            Meta::Path(path) if path.is_ident("typed") && !typed => typed = true,
            Meta::NameValue(value) if value.path.is_ident("namespace") => {
                if explicit_namespace.is_some() {
                    return Err(syn::Error::new_spanned(
                        value,
                        "#[kernel(typed)] accepts at most one explicit namespace",
                    ));
                }
                let Expr::Lit(literal) = &value.value else {
                    return Err(syn::Error::new_spanned(
                        value,
                        "#[kernel(typed)] namespace must be a 64-byte lowercase hexadecimal string literal",
                    ));
                };
                let Lit::Str(literal) = &literal.lit else {
                    return Err(syn::Error::new_spanned(
                        value,
                        "#[kernel(typed)] namespace must be a 64-byte lowercase hexadecimal string literal",
                    ));
                };
                explicit_namespace = Some(
                    CrateBindingIdV1::from_hex(&literal.value())
                        .map_err(|error| syn::Error::new_spanned(literal, error))?,
                );
            }
            Meta::NameValue(value) if value.path.is_ident("reference") => {
                if reference.is_some() {
                    return Err(syn::Error::new_spanned(
                        value,
                        "#[kernel] accepts at most one safe Rust reference function",
                    ));
                }
                let Expr::Path(path) = value.value else {
                    return Err(syn::Error::new_spanned(
                        value,
                        "#[kernel] reference must be a Rust function path",
                    ));
                };
                reference = Some(quote!(#path).to_string());
            }
            Meta::List(list) if list.path.is_ident("launch") => {
                if launch.is_some() {
                    return Err(syn::Error::new_spanned(
                        list,
                        "#[kernel] accepts at most one launch declaration",
                    ));
                }
                launch = Some(parse_launch_bounds_v1(&list)?);
            }
            Meta::List(list) if list.path.is_ident("unsafe_asm") => {
                if unsafe_assembly.is_some() {
                    return Err(syn::Error::new_spanned(
                        list,
                        "#[kernel] accepts at most one unsafe_asm declaration",
                    ));
                }
                unsafe_assembly = Some(parse_unsafe_assembly_v1(&list)?);
            }
            Meta::List(list) if list.path.is_ident("control_flow") => {
                if control_flow.is_some() {
                    return Err(syn::Error::new_spanned(
                        list,
                        "#[kernel] accepts at most one control_flow declaration",
                    ));
                }
                control_flow = Some(parse_control_flow_options_v1(&list)?);
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    argument,
                    "#[kernel] accepts only #[kernel], #[kernel(typed)], namespace, reference, launch(...), unsafe_asm(...), and control_flow(...) declarations",
                ));
            }
        }
    }

    if explicit_namespace.is_some() && !typed {
        return Err(syn::Error::new_spanned(
            attr,
            "#[kernel] namespace requires typed mode",
        ));
    }
    if reference.is_some() && !typed {
        return Err(syn::Error::new_spanned(
            attr,
            "#[kernel] safe Rust reference bindings require typed mode",
        ));
    }
    if launch
        .as_ref()
        .is_some_and(|launch| launch.max_grid.is_some())
        && !typed
    {
        return Err(syn::Error::new_spanned(
            attr,
            "#[kernel] max_grid requires typed mode",
        ));
    }
    Ok(KernelOptions {
        mode: if typed {
            KernelMode::Typed
        } else {
            KernelMode::Basic
        },
        explicit_namespace,
        reference,
        launch,
        unsafe_assembly,
        control_flow,
    })
}

fn parse_launch_bounds_v1(list: &syn::MetaList) -> syn::Result<ParsedLaunchBoundsV1> {
    let mut required = None;
    let mut maximum = None;
    let mut max_grid = None;
    let mut occupancy = None;
    list.parse_nested_meta(|meta| {
        if meta.path.is_ident("required") {
            if required.is_some() {
                return Err(meta.error("launch required dimensions are duplicated"));
            }
            required = Some(parse_workgroup_dimensions_v1(&meta)?);
            return Ok(());
        }
        if meta.path.is_ident("max") {
            if maximum.is_some() {
                return Err(meta.error("launch maximum dimensions are duplicated"));
            }
            maximum = Some(parse_workgroup_dimensions_v1(&meta)?);
            return Ok(());
        }
        if meta.path.is_ident("max_grid") {
            if max_grid.is_some() {
                return Err(meta.error("launch maximum grid dimensions are duplicated"));
            }
            max_grid = Some(parse_grid_dimensions_v1(&meta)?);
            return Ok(());
        }
        if meta.path.is_ident("min_workgroups_per_compute_unit") {
            if occupancy.is_some() {
                return Err(meta.error("launch occupancy constraint is duplicated"));
            }
            let value = meta.value()?.parse::<LitInt>()?.base10_parse::<u16>()?;
            if value == 0 || value > MAX_RESIDENT_WORKGROUPS_PER_COMPUTE_UNIT_V1 {
                return Err(meta.error(
                    "min_workgroups_per_compute_unit must be an integer in 1..=64",
                ));
            }
            occupancy = Some(value);
            return Ok(());
        }
        Err(meta.error(
            "launch supports only required = [x, y, z], max = [x, y, z], max_grid = [x, y, z], and min_workgroups_per_compute_unit = n",
        ))
    })?;

    if required.is_none() && maximum.is_none() {
        return Err(syn::Error::new_spanned(
            list,
            "launch requires required or max workgroup dimensions",
        ));
    }
    if occupancy.is_some() && maximum.is_none() {
        return Err(syn::Error::new_spanned(
            list,
            "min_workgroups_per_compute_unit requires max workgroup dimensions",
        ));
    }
    if let (Some(required), Some(maximum)) = (required, maximum)
        && required
            .into_iter()
            .zip(maximum)
            .any(|(required, maximum)| required > maximum)
    {
        return Err(syn::Error::new_spanned(
            list,
            "required workgroup dimensions exceed max dimensions",
        ));
    }
    Ok(ParsedLaunchBoundsV1 {
        required,
        maximum,
        max_grid,
        min_workgroups_per_compute_unit: occupancy,
    })
}

fn parse_grid_dimensions_v1(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<[u32; 3]> {
    let array = meta.value()?.parse::<ExprArray>()?;
    if array.elems.len() != 3 {
        return Err(syn::Error::new_spanned(
            array,
            "grid dimensions require exactly three integer components",
        ));
    }
    let mut result = [0_u32; 3];
    for (index, expression) in array.elems.iter().enumerate() {
        let Expr::Lit(literal) = expression else {
            return Err(syn::Error::new_spanned(
                expression,
                "grid dimensions must be integer literals",
            ));
        };
        let Lit::Int(integer) = &literal.lit else {
            return Err(syn::Error::new_spanned(
                expression,
                "grid dimensions must be integer literals",
            ));
        };
        result[index] = integer.base10_parse::<u32>()?;
        if result[index] == 0 {
            return Err(syn::Error::new_spanned(
                integer,
                "grid dimensions must be nonzero",
            ));
        }
    }
    Ok(result)
}

fn parse_workgroup_dimensions_v1(meta: &syn::meta::ParseNestedMeta<'_>) -> syn::Result<[u32; 3]> {
    let array = meta.value()?.parse::<ExprArray>()?;
    if array.elems.len() != 3 {
        return Err(syn::Error::new_spanned(
            array,
            "workgroup dimensions require exactly three integer components",
        ));
    }
    let mut result = [0_u32; 3];
    for (index, expression) in array.elems.iter().enumerate() {
        let Expr::Lit(literal) = expression else {
            return Err(syn::Error::new_spanned(
                expression,
                "workgroup dimensions must be integer literals",
            ));
        };
        let Lit::Int(integer) = &literal.lit else {
            return Err(syn::Error::new_spanned(
                expression,
                "workgroup dimensions must be integer literals",
            ));
        };
        result[index] = integer.base10_parse::<u32>()?;
        if result[index] == 0 {
            return Err(syn::Error::new_spanned(
                integer,
                "workgroup dimensions must be nonzero",
            ));
        }
    }
    let volume = result.into_iter().try_fold(1_u64, |volume, component| {
        volume.checked_mul(u64::from(component))
    });
    if volume.is_none_or(|volume| volume > MAX_WORKGROUP_THREADS_V1) {
        return Err(syn::Error::new_spanned(
            array,
            "workgroup dimension product must not exceed 1024",
        ));
    }
    Ok(result)
}

fn parse_unsafe_assembly_v1(list: &syn::MetaList) -> syn::Result<ParsedUnsafeAssemblyV1> {
    let mut target = None;
    let mut operands = None;
    let mut options = None;
    let mut effects = None;
    list.parse_nested_meta(|meta| {
        if meta.path.is_ident("target") {
            if target.is_some() {
                return Err(meta.error("unsafe_asm target is duplicated"));
            }
            let value = meta.value()?.parse::<syn::LitStr>()?;
            if value.value() != "gfx942" {
                return Err(syn::Error::new_spanned(
                    value,
                    "unsafe_asm supports only target = \"gfx942\" in V1",
                ));
            }
            target = Some(1_u16);
            return Ok(());
        }
        if meta.path.is_ident("operands") {
            if operands.is_some() {
                return Err(meta.error("unsafe_asm operands are duplicated"));
            }
            operands = Some(parse_assembly_flags_v1(
                &meta,
                &[
                    ("sgpr", ASSEMBLY_OPERAND_SGPR_V1),
                    ("vgpr", ASSEMBLY_OPERAND_VGPR_V1),
                    ("immediate", ASSEMBLY_OPERAND_IMMEDIATE_V1),
                    ("address", ASSEMBLY_OPERAND_ADDRESS_V1),
                ],
                false,
                "unsafe_asm operands support only sgpr, vgpr, immediate, and address",
            )?);
            return Ok(());
        }
        if meta.path.is_ident("options") {
            if options.is_some() {
                return Err(meta.error("unsafe_asm options are duplicated"));
            }
            options = Some(parse_assembly_flags_v1(
                &meta,
                &[
                    ("nomem", ASSEMBLY_OPTION_NOMEM_V1),
                    ("readonly", ASSEMBLY_OPTION_READONLY_V1),
                    ("pure", ASSEMBLY_OPTION_PURE_V1),
                    ("preserves_flags", ASSEMBLY_OPTION_PRESERVES_FLAGS_V1),
                    ("nostack", ASSEMBLY_OPTION_NOSTACK_V1),
                ],
                true,
                "unsafe_asm options support only nomem, readonly, pure, preserves_flags, and nostack",
            )?);
            return Ok(());
        }
        if meta.path.is_ident("effects") {
            if effects.is_some() {
                return Err(meta.error("unsafe_asm effects are duplicated"));
            }
            effects = Some(parse_assembly_flags_v1(
                &meta,
                &[
                    ("read_global", ASSEMBLY_EFFECT_READ_GLOBAL_V1),
                    ("write_global", ASSEMBLY_EFFECT_WRITE_GLOBAL_V1),
                    ("read_workgroup", ASSEMBLY_EFFECT_READ_WORKGROUP_V1),
                    ("write_workgroup", ASSEMBLY_EFFECT_WRITE_WORKGROUP_V1),
                    ("atomic", ASSEMBLY_EFFECT_ATOMIC_V1),
                    ("barrier", ASSEMBLY_EFFECT_BARRIER_V1),
                    ("control_flow", ASSEMBLY_EFFECT_CONTROL_FLOW_V1),
                ],
                true,
                "unsafe_asm effects support only none, read/write_global, read/write_workgroup, atomic, barrier, and control_flow",
            )?);
            return Ok(());
        }
        Err(meta.error("unsafe_asm requires target, operands(...), options(...), and effects(...)"))
    })?;

    let target =
        target.ok_or_else(|| syn::Error::new_spanned(list, "unsafe_asm target is required"))?;
    let operands = operands
        .ok_or_else(|| syn::Error::new_spanned(list, "unsafe_asm operands are required"))?;
    if operands == 0 {
        return Err(syn::Error::new_spanned(
            list,
            "unsafe_asm must declare at least one operand kind",
        ));
    }
    let options =
        options.ok_or_else(|| syn::Error::new_spanned(list, "unsafe_asm options are required"))?;
    let effects =
        effects.ok_or_else(|| syn::Error::new_spanned(list, "unsafe_asm effects are required"))?;
    validate_assembly_options_and_effects_v1(list, options, effects)?;
    Ok(ParsedUnsafeAssemblyV1 {
        target,
        operands,
        options,
        effects,
    })
}

fn parse_assembly_flags_v1(
    meta: &syn::meta::ParseNestedMeta<'_>,
    supported: &[(&str, u16)],
    allow_none: bool,
    unsupported_message: &'static str,
) -> syn::Result<u16> {
    let mut bits = 0_u16;
    let mut saw_none = false;
    meta.parse_nested_meta(|item| {
        if allow_none && item.path.is_ident("none") {
            if saw_none || bits != 0 {
                return Err(item.error("none conflicts with other declarations"));
            }
            saw_none = true;
            return Ok(());
        }
        let Some((_, bit)) = supported.iter().find(|(name, _)| item.path.is_ident(name)) else {
            return Err(item.error(unsupported_message));
        };
        if saw_none || bits & *bit != 0 {
            return Err(item.error("unsafe_asm declaration is duplicated or conflicting"));
        }
        bits |= *bit;
        Ok(())
    })?;
    Ok(bits)
}

fn validate_assembly_options_and_effects_v1(
    list: &syn::MetaList,
    options: u16,
    effects: u16,
) -> syn::Result<()> {
    if options & ASSEMBLY_OPTION_NOMEM_V1 != 0 && options & ASSEMBLY_OPTION_READONLY_V1 != 0
        || options & ASSEMBLY_OPTION_PURE_V1 != 0
            && options & (ASSEMBLY_OPTION_NOMEM_V1 | ASSEMBLY_OPTION_READONLY_V1) == 0
    {
        return Err(syn::Error::new_spanned(
            list,
            "unsafe_asm nomem/readonly conflict, and pure requires one of them",
        ));
    }
    let memory = ASSEMBLY_EFFECT_READ_GLOBAL_V1
        | ASSEMBLY_EFFECT_WRITE_GLOBAL_V1
        | ASSEMBLY_EFFECT_READ_WORKGROUP_V1
        | ASSEMBLY_EFFECT_WRITE_WORKGROUP_V1
        | ASSEMBLY_EFFECT_ATOMIC_V1
        | ASSEMBLY_EFFECT_BARRIER_V1;
    let writes = ASSEMBLY_EFFECT_WRITE_GLOBAL_V1
        | ASSEMBLY_EFFECT_WRITE_WORKGROUP_V1
        | ASSEMBLY_EFFECT_ATOMIC_V1;
    if options & ASSEMBLY_OPTION_NOMEM_V1 != 0 && effects & memory != 0
        || options & ASSEMBLY_OPTION_READONLY_V1 != 0 && effects & writes != 0
        || options & ASSEMBLY_OPTION_PURE_V1 != 0 && effects & ASSEMBLY_EFFECT_CONTROL_FLOW_V1 != 0
        || effects == 0 && options & ASSEMBLY_OPTION_NOMEM_V1 == 0
    {
        return Err(syn::Error::new_spanned(
            list,
            "unsafe_asm effects conflict with its memory/control-flow options",
        ));
    }
    Ok(())
}

fn expand_kernel(input: ItemFn, options: KernelOptions) -> syn::Result<proc_macro2::TokenStream> {
    validate_kernel_assembly_boundary(&input, options.unsafe_assembly)?;
    if options.mode == KernelMode::Typed {
        validate_typed_kernel_profile_v1(&input, &options)?;
        validate_typed_kernel_symbol_stem(&input.sig.ident)?;
    }
    validate_kernel_source_safety(&input, options.unsafe_assembly.is_some())?;
    validate_kernel_signature(&input)?;

    let crate_binding = if options.mode == KernelMode::Typed {
        Some(resolve_crate_binding(options.explicit_namespace)?)
    } else {
        None
    };

    let device_import = fe2o3_device_import()?;
    let device_path = fe2o3_device_path()?;
    // Every supported source kernel observes the attempt even when its emitted
    // tokens have no simulation-specific host adapter to omit.
    let simulation_mode = simulation_mode_selected_v1()?;
    let omit_host_adapter = options.mode == KernelMode::Typed && simulation_mode;
    let host_import = if options.mode == KernelMode::Typed && !omit_host_adapter {
        Some(fe2o3_host_import()?)
    } else {
        None
    };

    expand_kernel_with_imports(
        input,
        options,
        &device_import,
        Some(&device_path),
        host_import.as_ref(),
        crate_binding,
    )
}

fn simulation_mode_selected_v1() -> syn::Result<bool> {
    let mode = tracked_environment_value_v1(SIMULATION_MODE_ENV_V1)?;
    let attempt = tracked_environment_value_v1(SIMULATION_ATTEMPT_ENV_V1)?;
    if mode.as_deref().is_some_and(simulation_mode_value_v1) {
        match attempt {
            Some(attempt) if simulation_attempt_value_v1(&attempt) => Ok(true),
            Some(_) => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "{SIMULATION_ATTEMPT_ENV_V1} must be exactly 32 lowercase hexadecimal digits and nonzero"
                ),
            )),
            None => Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("{SIMULATION_MODE_ENV_V1}=1 requires {SIMULATION_ATTEMPT_ENV_V1}"),
            )),
        }
    } else if attempt.is_some() {
        Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("{SIMULATION_ATTEMPT_ENV_V1} is only valid with {SIMULATION_MODE_ENV_V1}=1"),
        ))
    } else {
        Ok(false)
    }
}

fn tracked_environment_value_v1(name: &str) -> syn::Result<Option<String>> {
    match proc_macro::tracked::env_var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("{name} is not valid UTF-8"),
        )),
    }
}

fn simulation_mode_value_v1(value: &str) -> bool {
    value == "1"
}

fn simulation_attempt_value_v1(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.as_bytes().iter().any(|byte| *byte != b'0')
}

fn resolve_crate_binding(
    explicit_namespace: Option<CrateBindingIdV1>,
) -> syn::Result<CrateBindingIdV1> {
    let compiler_binding = match proc_macro::tracked::env_var(CRATE_BINDING_ID_ENV_V1) {
        Ok(value) => Some(CrateBindingIdV1::from_hex(&value).map_err(|error| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("invalid {CRATE_BINDING_ID_ENV_V1}: {error}"),
            )
        })?),
        Err(std::env::VarError::NotPresent) => None,
        Err(std::env::VarError::NotUnicode(_)) => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("{CRATE_BINDING_ID_ENV_V1} is not valid UTF-8"),
            ));
        }
    };
    reconcile_crate_binding_v1(compiler_binding, explicit_namespace)
}

fn reconcile_crate_binding_v1(
    compiler_binding: Option<CrateBindingIdV1>,
    explicit_namespace: Option<CrateBindingIdV1>,
) -> syn::Result<CrateBindingIdV1> {
    match (compiler_binding, explicit_namespace) {
        (Some(compiler), Some(explicit)) if compiler != explicit => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!(
                "compiler-derived crate binding {} disagrees with explicit #[kernel(typed)] namespace {}",
                compiler.to_hex(),
                explicit.to_hex()
            ),
        )),
        (Some(compiler), _) => Ok(compiler),
        (None, Some(explicit)) => Ok(explicit),
        (None, None) => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[kernel(typed)] requires the cargo-fe2o3 rustc wrapper or an explicit 256-bit namespace",
        )),
    }
}

#[cfg(test)]
fn expand_kernel_with_device_import(
    input: ItemFn,
    device_import: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    expand_kernel_with_imports(
        input,
        KernelOptions {
            mode: KernelMode::Basic,
            explicit_namespace: None,
            reference: None,
            launch: None,
            unsafe_assembly: None,
            control_flow: None,
        },
        device_import,
        None,
        None,
        None,
    )
}

fn expand_kernel_with_imports(
    input: ItemFn,
    options: KernelOptions,
    device_import: &proc_macro2::TokenStream,
    device_path: Option<&proc_macro2::TokenStream>,
    host_import: Option<&proc_macro2::TokenStream>,
    crate_binding: Option<CrateBindingIdV1>,
) -> syn::Result<proc_macro2::TokenStream> {
    if options.mode == KernelMode::Typed {
        validate_general_typed_signature_shape_v1(&input, &options)?;
        return expand_general_typed_kernel_with_imports(
            input,
            options,
            device_import,
            device_path,
            host_import,
            crate_binding,
        );
    }

    debug_assert!(host_import.is_none());
    debug_assert!(crate_binding.is_none());
    expand_device_kernel_with_imports(input, options, device_import, device_path)
}

fn expand_device_kernel_with_imports(
    mut input: ItemFn,
    options: KernelOptions,
    device_import: &proc_macro2::TokenStream,
    device_path: Option<&proc_macro2::TokenStream>,
) -> syn::Result<proc_macro2::TokenStream> {
    debug_assert_eq!(options.mode, KernelMode::Basic);
    validate_kernel_assembly_boundary(&input, options.unsafe_assembly)?;
    if options.control_flow.is_some()
        && options
            .unsafe_assembly
            .is_some_and(|assembly| assembly.effects & ASSEMBLY_EFFECT_CONTROL_FLOW_V1 != 0)
    {
        return Err(syn::Error::new_spanned(
            &input.sig,
            "unsafe assembly with control_flow effects cannot participate in a structured control_flow V1 contract",
        ));
    }
    validate_kernel_source_safety(&input, options.unsafe_assembly.is_some())?;
    validate_kernel_signature(&input)?;

    let original_ident = input.sig.ident.clone();
    let original_name = original_ident.to_string();
    let control_flow_contract =
        analyze_kernel_control_flow_v1(&input, options.control_flow.as_ref())?;
    lower_bounded_for_loops_v1(&mut input, options.control_flow.as_ref())?;

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
    let registration_kind = KERNEL_REGISTRATION_KIND_KERNEL;
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
    let device_api = device_path
        .cloned()
        .unwrap_or_else(|| quote!(__fe2o3_kernel_device));
    let fallback_device_import = device_path.is_none().then_some(device_import);
    input.sig.ident = internal_ident.clone();

    let registration_type = quote!((u64, u16, u16, &'static str, &'static str, #function_pointer));
    let registration_value = quote!((
        #KERNEL_REGISTRATION_MAGIC,
        #KERNEL_REGISTRATION_VERSION_V1,
        #registration_kind,
        #marker_value,
        #export_value,
        #internal_ident,
    ));
    let frontend_registration = encode_kernel_frontend_contract_v1(&options).map(|bytes| {
        let registration_ident =
            format_ident!("{KERNEL_FRONTEND_REGISTRATION_PREFIX_V1}{original_name}");
        let bytes_ident =
            format_ident!("__fe2o3_kernel_frontend_contract_bytes_v1_{original_name}");
        let bytes = bytes.iter();
        quote! {
            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            const #bytes_ident: &'static [u8] = &[#(#bytes),*];

            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            #[used]
            static #registration_ident: (
                u64,
                u16,
                u16,
                &'static str,
                &'static [u8],
                #function_pointer,
            ) = (
                #KERNEL_FRONTEND_REGISTRATION_MAGIC_V1,
                #KERNEL_FRONTEND_REGISTRATION_VERSION_V1,
                #KERNEL_FRONTEND_REGISTRATION_KIND_V1,
                #marker_value,
                #bytes_ident,
                #internal_ident,
            );
        }
    });
    let control_flow_registration = control_flow_contract.map(|bytes| {
        let registration_ident =
            format_ident!("{CONTROL_FLOW_REGISTRATION_PREFIX_V1}{original_name}");
        let bytes = bytes.iter();
        quote! {
            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            #[used]
            static #registration_ident: (
                u64,
                u16,
                u16,
                &'static str,
                &'static [u8],
                #function_pointer,
            ) = (
                #CONTROL_FLOW_REGISTRATION_MAGIC_V1,
                #CONTROL_FLOW_REGISTRATION_VERSION_V1,
                #CONTROL_FLOW_REGISTRATION_KIND_V1,
                #marker_value,
                &[#(#bytes),*],
                #internal_ident,
            );
        }
    });
    let reference_registration = expand_reference_registration_v1(
        &original_name,
        original_ident.span(),
        &internal_ident,
        &function_pointer,
        options.reference.as_ref(),
    );

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
        static #registration_ident: #registration_type = #registration_value;

        #frontend_registration
        #control_flow_registration
        #reference_registration

        const _: () = {
            #fallback_device_import

            // SAFETY: every associated value below is generated from the same
            // parsed function and the same collector-visible registration.
            unsafe impl #device_api::KernelMarkerV1 for #type_marker_ident {
                type Function = #function_pointer;
                type Registration = #registration_type;

                const LOGICAL_NAME: &'static str = #marker_value;
                const EXPORT_NAME: &'static str = #export_value;
                const FUNCTION: Self::Function = #internal_ident;
                const REGISTRATION: &'static Self::Registration = &#registration_ident;
            }

        };
    })
}

fn expand_general_typed_kernel_with_imports(
    mut input: ItemFn,
    options: KernelOptions,
    device_import: &proc_macro2::TokenStream,
    device_path: Option<&proc_macro2::TokenStream>,
    host_import: Option<&proc_macro2::TokenStream>,
    crate_binding: Option<CrateBindingIdV1>,
) -> syn::Result<proc_macro2::TokenStream> {
    validate_kernel_assembly_boundary(&input, options.unsafe_assembly)?;
    validate_kernel_source_safety(&input, options.unsafe_assembly.is_some())?;
    if options.control_flow.is_some()
        && options
            .unsafe_assembly
            .is_some_and(|assembly| assembly.effects & ASSEMBLY_EFFECT_CONTROL_FLOW_V1 != 0)
    {
        return Err(syn::Error::new_spanned(
            &input.sig,
            "unsafe assembly with control_flow effects cannot participate in a structured control_flow V1 contract",
        ));
    }
    validate_typed_kernel_symbol_stem(&input.sig.ident)?;
    validate_kernel_signature(&input)?;

    let original_ident = input.sig.ident.clone();
    let original_name = original_ident.to_string();
    if original_name.starts_with(RESERVED_ROOT) {
        return Err(syn::Error::new_spanned(
            original_ident,
            format!("function names starting with `{RESERVED_ROOT}` are reserved by fe2o3"),
        ));
    }

    let crate_binding = crate_binding.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "general typed V1 expansion requires a resolved crate binding",
        )
    })?;
    let kernel_binding = derive_kernel_binding_id_v1(
        crate_binding,
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        &original_name,
        &original_name,
    );
    let model = model_general_typed_signature_v1(&input, &options, kernel_binding.as_bytes())?;
    let generated_host_contract =
        GeneratedHostContractIdV3::from_bytes(*model.generated_host_contract_identity.as_bytes());
    let generated_host_arguments = generated_general_typed_arguments_v1(&input, &model.arguments);
    let generated_worker_v3_adapter = generated_worker_v3_adapter_v1(&input, &model);
    let control_flow_contract =
        analyze_kernel_control_flow_v1(&input, options.control_flow.as_ref())?;
    lower_bounded_for_loops_v1(&mut input, options.control_flow.as_ref())?;

    let returns_kernel_result = is_kernel_result_return(&input.sig.output);
    let internal_ident = format_ident!("__fe2o3_host_kernel_v1_{}", kernel_binding.to_hex());
    let body_ident = format_ident!("__fe2o3_kernel_body_v1_{}", kernel_binding.to_hex());
    let result_device_path = if returns_kernel_result {
        Some(device_path.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "KernelResult expansion requires the resolved fe2o3-device crate path",
            )
        })?)
    } else {
        None
    };
    let name_marker_ident = format_ident!("__fe2o3_kernel_name_{original_name}");
    let type_marker_ident = format_ident!("__fe2o3_kernel_marker_{original_name}");
    let registration_ident = format_ident!("{KERNEL_REGISTRATION_PREFIX}{original_name}");
    let marker_value = syn::LitStr::new(&original_name, original_ident.span());
    let export_value = syn::LitStr::new(&original_name, original_ident.span());
    let crate_binding_hex = syn::LitStr::new(&crate_binding.to_hex(), original_ident.span());
    let kernel_binding_hex = syn::LitStr::new(&kernel_binding.to_hex(), original_ident.span());
    let generated_host_contract_hex =
        syn::LitStr::new(&generated_host_contract.to_hex(), original_ident.span());
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
    let output = if returns_kernel_result {
        ReturnType::Default
    } else {
        input.sig.output.clone()
    };
    let function_pointer = quote!(#safety #abi fn(#(#argument_types),*) #output);
    let device_api = device_path
        .cloned()
        .unwrap_or_else(|| quote!(__fe2o3_kernel_device));
    let fallback_device_import = device_path.is_none().then_some(device_import);
    let argument_names = input
        .sig
        .inputs
        .iter()
        .map(|argument| match argument {
            syn::FnArg::Typed(argument) => match argument.pat.as_ref() {
                Pat::Ident(pattern) => pattern.ident.clone(),
                _ => unreachable!("general typed validation requires identifier patterns"),
            },
            syn::FnArg::Receiver(_) => unreachable!("general typed validation rejects receivers"),
        })
        .collect::<Vec<_>>();
    let helper_input = if returns_kernel_result {
        let mut helper = input.clone();
        helper.vis = Visibility::Inherited;
        helper.sig.ident = body_ident.clone();
        helper.attrs.push(parse_quote!(#[inline(always)]));

        input.sig.ident = internal_ident.clone();
        input.sig.output = ReturnType::Default;
        for argument in &mut input.sig.inputs {
            if let syn::FnArg::Typed(argument) = argument
                && let Pat::Ident(pattern) = argument.pat.as_mut()
            {
                pattern.mutability = None;
            }
        }
        *input.block = parse_quote!({
            let _: #result_device_path::KernelResult = #body_ident(#(#argument_names),*);
        });
        Some(helper)
    } else {
        input.sig.ident = internal_ident.clone();
        None
    };

    let registration_type = quote!((
        u64,
        u16,
        u16,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        #function_pointer,
    ));
    let registration_value = quote!((
        #KERNEL_REGISTRATION_MAGIC,
        #KERNEL_REGISTRATION_VERSION_V3,
        #KERNEL_REGISTRATION_KIND_TYPED_GENERAL_LAYOUT_V3,
        #marker_value,
        #export_value,
        #crate_binding_hex,
        #kernel_binding_hex,
        #TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3,
        #generated_host_contract_hex,
        #internal_ident,
    ));
    let symbol = syn::LitStr::new(
        &host_kernel_symbol_v1(kernel_binding),
        original_ident.span(),
    );
    let export_attribute = quote!(#[unsafe(export_name = #symbol)]);
    let frontend_registration = encode_kernel_frontend_contract_v1(&options).map(|bytes| {
        let registration_ident =
            format_ident!("{KERNEL_FRONTEND_REGISTRATION_PREFIX_V1}{original_name}");
        let bytes_ident =
            format_ident!("__fe2o3_kernel_frontend_contract_bytes_v1_{original_name}");
        let bytes = bytes.iter();
        quote! {
            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            const #bytes_ident: &'static [u8] = &[#(#bytes),*];

            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            #[used]
            static #registration_ident: (
                u64,
                u16,
                u16,
                &'static str,
                &'static [u8],
                #function_pointer,
            ) = (
                #KERNEL_FRONTEND_REGISTRATION_MAGIC_V1,
                #KERNEL_FRONTEND_REGISTRATION_VERSION_V1,
                #KERNEL_FRONTEND_REGISTRATION_KIND_V1,
                #marker_value,
                #bytes_ident,
                #internal_ident,
            );
        }
    });
    let control_flow_registration = control_flow_contract.map(|bytes| {
        let registration_ident =
            format_ident!("{CONTROL_FLOW_REGISTRATION_PREFIX_V1}{original_name}");
        let bytes = bytes.iter();
        quote! {
            #[doc(hidden)]
            #[allow(non_upper_case_globals)]
            #[used]
            static #registration_ident: (
                u64,
                u16,
                u16,
                &'static str,
                &'static [u8],
                #function_pointer,
            ) = (
                #CONTROL_FLOW_REGISTRATION_MAGIC_V1,
                #CONTROL_FLOW_REGISTRATION_VERSION_V1,
                #CONTROL_FLOW_REGISTRATION_KIND_V1,
                #marker_value,
                &[#(#bytes),*],
                #internal_ident,
            );
        }
    });
    let reference_registration = expand_reference_registration_v1(
        &original_name,
        original_ident.span(),
        &internal_ident,
        &function_pointer,
        options.reference.as_ref(),
    );
    let module_ident = format_ident!("{original_name}_gpu");
    let binding_bytes = kernel_binding.as_bytes().into_iter();
    let generated_host_contract_profile_bytes = generated_host_contract.as_bytes().into_iter();
    let typed_module = host_import.map_or_else(
        || quote! {},
        |host_import| quote! {
        #[cfg(not(target_arch = "amdgpu"))]
        pub mod #module_ident {
            extern crate alloc as __fe2o3_kernel_alloc;

            use __fe2o3_kernel_alloc::vec;

            #host_import

            pub type Marker = super::#type_marker_ident;

            #generated_host_arguments
            #generated_worker_v3_adapter

            const _: () = {
                // SAFETY: these constants and the generated argument adapter
                // come from one parsed signature. Worker V3 independently
                // matches the marker binding and complete argument layout to
                // the admitted compiler descriptor before load or dispatch.
                unsafe impl __fe2o3_kernel_host::__generated::CompilerGeneratedKernelExpectationV1
                    for super::#type_marker_ident
                {
                    const PROFILE:
                        __fe2o3_kernel_host::__generated::CompilerGeneratedKernelProfileV1 =
                        __fe2o3_kernel_host::__generated::CompilerGeneratedKernelProfileV1::new(
                            [#(#generated_host_contract_profile_bytes),*]
                        );

                    const KERNEL_BINDING_ID_V1: [u8; 32] = [#(#binding_bytes),*];
                }
            };
        }
    });

    Ok(quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        #helper_input

        #[doc(hidden)]
        #[allow(non_snake_case)]
        #export_attribute
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
        static #registration_ident: #registration_type = #registration_value;

        #frontend_registration
        #control_flow_registration
        #reference_registration

        const _: () = {
            #fallback_device_import

            // SAFETY: every associated value below is generated from the same
            // parsed function and the same collector-visible registration.
            unsafe impl #device_api::KernelMarkerV1 for #type_marker_ident {
                type Function = #function_pointer;
                type Registration = #registration_type;

                const LOGICAL_NAME: &'static str = #marker_value;
                const EXPORT_NAME: &'static str = #export_value;
                const FUNCTION: Self::Function = #internal_ident;
                const REGISTRATION: &'static Self::Registration = &#registration_ident;
            }
        };

        #typed_module
    })
}

fn expand_reference_registration_v1(
    original_name: &str,
    span: proc_macro2::Span,
    internal_ident: &syn::Ident,
    function_pointer: &proc_macro2::TokenStream,
    reference: Option<&String>,
) -> proc_macro2::TokenStream {
    let Some(reference) = reference else {
        return quote! {};
    };
    let reference: syn::ExprPath =
        syn::parse_str(reference).expect("kernel option parsing retained one Rust path");
    let anchor_ident = format_ident!("__fe2o3_kernel_reference_anchor_v1_{original_name}");
    let registration_ident =
        format_ident!("{REFERENCE_BINDING_REGISTRATION_PREFIX_V1}{original_name}");
    let marker_value = syn::LitStr::new(original_name, span);
    quote! {
        #[doc(hidden)]
        #[allow(non_snake_case)]
        fn #anchor_ident() {
            let _ = ::core::hint::black_box(#reference);
        }

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        #[used]
        static #registration_ident: (
            u64,
            u16,
            u16,
            &'static str,
            #function_pointer,
            fn(),
        ) = (
            #REFERENCE_BINDING_REGISTRATION_MAGIC_V1,
            #REFERENCE_BINDING_REGISTRATION_VERSION_V1,
            #REFERENCE_BINDING_REGISTRATION_KIND_V1,
            #marker_value,
            #internal_ident,
            #anchor_ident,
        );
    }
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

fn fe2o3_device_path() -> syn::Result<proc_macro2::TokenStream> {
    let found = crate_name("fe2o3-device").map_err(|error| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("#[kernel] could not resolve the fe2o3-device crate: {error}"),
        )
    })?;

    Ok(device_path_for(found))
}

fn device_path_for(found: FoundCrate) -> proc_macro2::TokenStream {
    match found {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = format_ident!("{name}");
            quote!(::#ident)
        }
    }
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

fn validate_typed_kernel_symbol_stem(ident: &syn::Ident) -> syn::Result<()> {
    let symbol_stem = ident.to_string();
    let mut bytes = symbol_stem.bytes();
    let valid = symbol_stem.len() <= MAX_TYPED_KERNEL_SYMBOL_STEM_BYTES
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_');

    if valid {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            ident,
            format!(
                "#[kernel(typed)] kernel name must be 1 to {MAX_TYPED_KERNEL_SYMBOL_STEM_BYTES} ASCII identifier bytes for backend artifact symbols"
            ),
        ))
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
    validate_exact_vecadd_argument(
        arguments[0],
        1,
        GeneralTypedArgumentKindV1::SharedSlice(GeneralTypedScalarV1::F32),
    )?;
    validate_exact_vecadd_argument(
        arguments[1],
        2,
        GeneralTypedArgumentKindV1::SharedSlice(GeneralTypedScalarV1::F32),
    )?;
    validate_exact_vecadd_argument(
        arguments[2],
        3,
        GeneralTypedArgumentKindV1::ExclusiveSlice(GeneralTypedScalarV1::F32),
    )?;

    Ok(())
}

fn validate_typed_kernel_profile_v1(input: &ItemFn, options: &KernelOptions) -> syn::Result<()> {
    match validate_typed_kernel_signature(input) {
        Ok(()) => validate_fixed_wg256_launch_v1(options.launch.as_ref(), &input.sig),
        Err(exact_error) => {
            if validate_general_typed_signature_shape_v1(input, options).is_err() {
                Err(exact_error)
            } else {
                model_general_typed_signature_v1(input, options, [0; 32]).map(|_| ())
            }
        }
    }
}

fn validate_general_typed_signature_shape_v1(
    input: &ItemFn,
    options: &KernelOptions,
) -> syn::Result<()> {
    let mut signature_options = options.clone();
    signature_options.launch = None;
    model_general_typed_signature_v1(input, &signature_options, [0; 32]).map(|_| ())
}

fn validate_exact_vecadd_argument(
    argument: &FnArg,
    position: usize,
    expected: GeneralTypedArgumentKindV1,
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

    if parse_general_typed_argument_type_v1(&argument.ty).ok() != Some(expected) {
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

fn is_kernel_result_return(output: &ReturnType) -> bool {
    let ReturnType::Type(_, output) = output else {
        return false;
    };
    let Type::Path(path) = output.as_ref() else {
        return false;
    };
    if path.qself.is_some() {
        return false;
    }
    let Some(segment) = path.path.segments.last() else {
        return false;
    };
    if segment.ident != "KernelResult" {
        return false;
    }
    match &segment.arguments {
        syn::PathArguments::None => true,
        syn::PathArguments::AngleBracketed(arguments) if arguments.args.len() == 1 => {
            matches!(
                arguments.args.first(),
                Some(syn::GenericArgument::Type(Type::Tuple(tuple))) if tuple.elems.is_empty()
            )
        }
        _ => false,
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneralTypedScalarV1 {
    I8,
    U8,
    I16,
    U16,
    I32,
    U32,
    I64,
    U64,
    F32,
    F64,
}

impl GeneralTypedScalarV1 {
    const fn size_alignment(self) -> (u64, u32) {
        match self {
            Self::I8 | Self::U8 => (1, 1),
            Self::I16 | Self::U16 => (2, 2),
            Self::I32 | Self::U32 | Self::F32 => (4, 4),
            Self::I64 | Self::U64 | Self::F64 => (8, 8),
        }
    }

    const fn abi_scalar(self) -> ScalarType {
        match self {
            Self::I8 => ScalarType::I8,
            Self::U8 => ScalarType::U8,
            Self::I16 => ScalarType::I16,
            Self::U16 => ScalarType::U16,
            Self::I32 => ScalarType::I32,
            Self::U32 => ScalarType::U32,
            Self::I64 => ScalarType::I64,
            Self::U64 => ScalarType::U64,
            Self::F32 => ScalarType::F32,
            Self::F64 => ScalarType::F64,
        }
    }

    const fn rust_layout_scalar(self) -> RustScalarElementTypeV1 {
        match self {
            Self::I8 => RustScalarElementTypeV1::I8,
            Self::U8 => RustScalarElementTypeV1::U8,
            Self::I16 => RustScalarElementTypeV1::I16,
            Self::U16 => RustScalarElementTypeV1::U16,
            Self::I32 => RustScalarElementTypeV1::I32,
            Self::U32 => RustScalarElementTypeV1::U32,
            Self::I64 => RustScalarElementTypeV1::I64,
            Self::U64 => RustScalarElementTypeV1::U64,
            Self::F32 => RustScalarElementTypeV1::F32,
            Self::F64 => RustScalarElementTypeV1::F64,
        }
    }

    fn rust_type_tokens(self) -> proc_macro2::TokenStream {
        match self {
            Self::I8 => quote!(i8),
            Self::U8 => quote!(u8),
            Self::I16 => quote!(i16),
            Self::U16 => quote!(u16),
            Self::I32 => quote!(i32),
            Self::U32 => quote!(u32),
            Self::I64 => quote!(i64),
            Self::U64 => quote!(u64),
            Self::F32 => quote!(f32),
            Self::F64 => quote!(f64),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneralTypedArgumentKindV1 {
    Scalar(GeneralTypedScalarV1),
    SharedSlice(GeneralTypedScalarV1),
    ExclusiveSlice(GeneralTypedScalarV1),
    MappedExclusiveSlice(GeneralTypedScalarV1, RustDisjointIndexSpaceV1),
    GlobalMutPointer(GeneralTypedScalarV1),
}

#[allow(dead_code)]
#[derive(Clone, Debug, Eq, PartialEq)]
struct GeneralTypedSignatureModelV1 {
    arguments: Vec<GeneralTypedArgumentKindV1>,
    abi: AbiLayout,
    launch: LaunchContract,
    generated_host_contract_identity: DigestBytes,
}

fn generated_general_typed_arguments_v1(
    input: &ItemFn,
    arguments: &[GeneralTypedArgumentKindV1],
) -> proc_macro2::TokenStream {
    debug_assert_eq!(input.sig.inputs.len(), arguments.len());
    let fields = input
        .sig
        .inputs
        .iter()
        .map(|argument| match argument {
            FnArg::Typed(argument) => match argument.pat.as_ref() {
                Pat::Ident(pattern) => pattern.ident.clone(),
                _ => unreachable!("general typed argument validation requires identifier patterns"),
            },
            FnArg::Receiver(_) => {
                unreachable!("general typed argument validation rejects receivers")
            }
        })
        .collect::<Vec<_>>();
    let field_types = arguments
        .iter()
        .map(|argument| match argument {
            GeneralTypedArgumentKindV1::Scalar(scalar) => scalar.rust_type_tokens(),
            GeneralTypedArgumentKindV1::SharedSlice(scalar) => {
                let scalar = scalar.rust_type_tokens();
                quote!(
                    __fe2o3_kernel_host::__generated::GeneratedReadDeviceSlice<
                        'allocation,
                        #scalar
                    >
                )
            }
            GeneralTypedArgumentKindV1::ExclusiveSlice(scalar) => {
                let scalar = scalar.rust_type_tokens();
                quote!(
                    __fe2o3_kernel_host::__generated::GeneratedReadWriteDeviceSlice<
                        'allocation,
                        #scalar
                    >
                )
            }
            GeneralTypedArgumentKindV1::MappedExclusiveSlice(scalar, _) => {
                let scalar = scalar.rust_type_tokens();
                quote!(
                    __fe2o3_kernel_host::__generated::GeneratedReadWriteDeviceSlice<
                        'allocation,
                        #scalar
                    >
                )
            }
            GeneralTypedArgumentKindV1::GlobalMutPointer(scalar) => {
                let scalar = scalar.rust_type_tokens();
                quote!(GlobalMut<'allocation, #scalar>)
            }
        })
        .collect::<Vec<_>>();
    let retains_borrows = arguments.iter().any(|argument| {
        matches!(
            argument,
            GeneralTypedArgumentKindV1::SharedSlice(_)
                | GeneralTypedArgumentKindV1::ExclusiveSlice(_)
                | GeneralTypedArgumentKindV1::MappedExclusiveSlice(_, _)
                | GeneralTypedArgumentKindV1::GlobalMutPointer(_)
        )
    });
    let global_mut_pointer_support = arguments
        .iter()
        .any(|argument| matches!(argument, GeneralTypedArgumentKindV1::GlobalMutPointer(_)))
        .then(|| {
            quote! {
                /// A generated global-mutable pointer argument must name one
                /// initialized object held under an exclusive host borrow.
                #[derive(Clone, Copy, Debug, Eq, PartialEq)]
                pub struct GlobalMutArgumentError {
                    actual_elements: usize,
                }

                impl GlobalMutArgumentError {
                    /// Number of elements in the rejected device region.
                    pub const fn actual_elements(self) -> usize {
                        self.actual_elements
                    }
                }

                /// Opaque host authority for one global mutable device object.
                ///
                /// Construction consumes an exclusive, type-checked device
                /// region and rejects every extent except one element. The
                /// pointer is not exposed and this capability is not cloneable.
                #[must_use = "a global mutable argument only retains an exclusive device-buffer borrow"]
                #[allow(dead_code)]
                pub struct GlobalMut<
                    'allocation,
                    T: __fe2o3_kernel_host::__generated::GeneratedDeviceScalarV1,
                > {
                    region: __fe2o3_kernel_host::__generated::GeneratedReadWriteDeviceSlice<
                        'allocation,
                        T,
                    >,
                }

                impl<
                    'allocation,
                    T: __fe2o3_kernel_host::__generated::GeneratedDeviceScalarV1,
                > GlobalMut<'allocation, T> {
                    /// Checks and retains one exact global mutable object.
                    pub fn new(
                        region: __fe2o3_kernel_host::__generated::GeneratedReadWriteDeviceSlice<
                            'allocation,
                            T,
                        >,
                    ) -> Result<Self, GlobalMutArgumentError> {
                        if region.len() != 1 {
                            return Err(GlobalMutArgumentError {
                                actual_elements: region.len(),
                            });
                        }
                        Ok(Self { region })
                    }

                    /// The admitted region always contains exactly one object.
                    pub const fn len(&self) -> usize {
                        1
                    }

                    /// Global pointer arguments are never empty after admission.
                    pub const fn is_empty(&self) -> bool {
                        false
                    }
                }
            }
        });

    if retains_borrows {
        quote! {
            #global_mut_pointer_support

            /// Opaque host arguments for this exact kernel signature.
            ///
            /// This value only retains typed values and device-buffer borrows;
            /// it does not pack arguments, authorize a launch, or launch a kernel.
            #[must_use = "generated arguments retain device-buffer borrows but do not launch a kernel"]
            #[allow(dead_code)]
            pub struct Arguments<'allocation> {
                #(#fields: #field_types,)*
            }

            impl<'allocation> Arguments<'allocation> {
                /// Retains the typed host capabilities for this kernel signature.
                #[allow(clippy::too_many_arguments)]
                pub fn new(#(#fields: #field_types),*) -> Self {
                    Self { #(#fields),* }
                }
            }
        }
    } else {
        quote! {
            /// Opaque host arguments for this exact kernel signature.
            ///
            /// This value only retains typed values; it does not pack arguments,
            /// authorize a launch, or launch a kernel.
            #[must_use = "generated arguments are inert and do not launch a kernel"]
            #[allow(dead_code)]
            pub struct Arguments {
                #(#fields: #field_types,)*
            }

            impl Arguments {
                /// Retains the typed host values for this kernel signature.
                #[allow(clippy::too_many_arguments)]
                pub fn new(#(#fields: #field_types),*) -> Self {
                    Self { #(#fields),* }
                }
            }
        }
    }
}

fn generated_worker_v3_adapter_v1(
    input: &ItemFn,
    model: &GeneralTypedSignatureModelV1,
) -> proc_macro2::TokenStream {
    if model.arguments.iter().any(|argument| {
        matches!(
            argument,
            GeneralTypedArgumentKindV1::MappedExclusiveSlice(_, _)
                | GeneralTypedArgumentKindV1::GlobalMutPointer(_)
        )
    }) {
        // Descriptor V1 does not preserve mapped index-space identities or global pointers.
        return quote! {};
    }

    let fields = input
        .sig
        .inputs
        .iter()
        .map(|argument| match argument {
            FnArg::Typed(argument) => match argument.pat.as_ref() {
                Pat::Ident(pattern) => pattern.ident.clone(),
                _ => unreachable!("general typed argument validation requires identifier patterns"),
            },
            FnArg::Receiver(_) => {
                unreachable!("general typed argument validation rejects receivers")
            }
        })
        .collect::<Vec<_>>();
    let scalar_inputs = model
        .arguments
        .iter()
        .zip(&fields)
        .enumerate()
        .filter(|(_, (argument, _))| matches!(argument, GeneralTypedArgumentKindV1::Scalar(_)))
        .map(|(index, (_, field))| quote!(plan.scalar(#index, self.#field)?))
        .collect::<Vec<_>>();
    let memory_arguments = model
        .arguments
        .iter()
        .zip(&fields)
        .enumerate()
        .filter(|(_, (argument, _))| {
            matches!(
                argument,
                GeneralTypedArgumentKindV1::SharedSlice(_)
                    | GeneralTypedArgumentKindV1::ExclusiveSlice(_)
            )
        })
        .map(|(index, (_, field))| quote!(self.#field.bind_argument_pair(plan, #index)?))
        .collect::<Vec<_>>();
    let layout = generated_worker_v3_layout_v1(model);
    let retains_borrows = model.arguments.iter().any(|argument| {
        matches!(
            argument,
            GeneralTypedArgumentKindV1::SharedSlice(_)
                | GeneralTypedArgumentKindV1::ExclusiveSlice(_)
        )
    });
    let arguments_type = if retains_borrows {
        quote!(Arguments<'allocation>)
    } else {
        quote!(Arguments)
    };
    let prepare_impl = if retains_borrows {
        quote! {
            impl<'allocation> Arguments<'allocation> {
                /// Prepares this exact verified Worker V3 invocation for one-shot dispatch.
                #[allow(clippy::type_complexity)]
                pub fn prepare_worker_v3<'loaded, A>(
                    self,
                    executable: &'loaded mut __fe2o3_kernel_host::LoadedWorkerV3HsaExecutableV1<Marker, A>,
                    observed: &__fe2o3_kernel_host::ObservedContext,
                    geometry: __fe2o3_kernel_host::HsaLaunchGeometryV1,
                ) -> Result<
                    __fe2o3_kernel_host::__generated::GeneratedWorkerV3PreparedInvocationV1<
                        'loaded,
                        'allocation,
                        Marker,
                        A,
                        Self,
                    >,
                    __fe2o3_kernel_host::__generated::GeneratedWorkerV3PrepareErrorV1,
                >
                where
                    A: __fe2o3_kernel_host::ReviewedHsaImplicitKernargAdapterV1,
                {
                    executable.prepare_generated_worker_v3_v1(observed, geometry, self)
                }
            }
        }
    } else {
        quote! {
            impl Arguments {
                /// Prepares this exact verified Worker V3 invocation for one-shot dispatch.
                #[allow(clippy::type_complexity)]
                pub fn prepare_worker_v3<'loaded, A>(
                    self,
                    executable: &'loaded mut __fe2o3_kernel_host::LoadedWorkerV3HsaExecutableV1<Marker, A>,
                    observed: &__fe2o3_kernel_host::ObservedContext,
                    geometry: __fe2o3_kernel_host::HsaLaunchGeometryV1,
                ) -> Result<
                    __fe2o3_kernel_host::__generated::GeneratedWorkerV3PreparedInvocationV1<
                        'loaded,
                        'static,
                        Marker,
                        A,
                        Self,
                    >,
                    __fe2o3_kernel_host::__generated::GeneratedWorkerV3PrepareErrorV1,
                >
                where
                    A: __fe2o3_kernel_host::ReviewedHsaImplicitKernargAdapterV1,
                {
                    executable.prepare_generated_worker_v3_v1(observed, geometry, self)
                }
            }
        }
    };
    quote! {
        // SAFETY: the implementation is generated from the same parsed signature, canonical ABI,
        // and marker as the compiler-visible V3 registration. Memory inputs and access records are
        // emitted together by retained, non-forgeable capabilities.
        unsafe impl<'allocation>
            __fe2o3_kernel_host::__generated::CompilerGeneratedWorkerV3ArgumentsV1<
                'allocation,
                Marker,
            > for #arguments_type
        {
            fn generated_argument_layout_v1() -> Result<
                __fe2o3_kernel_host::__generated::CompilerGeneratedArgumentLayoutV1,
                __fe2o3_kernel_host::__generated::GeneratedArgumentLayoutError,
            > {
                #layout
            }

            fn bind_arguments_v1(
                &self,
                plan: &__fe2o3_kernel_host::__generated::GeneratedArgumentPackingPlanV1,
            ) -> Result<
                __fe2o3_kernel_host::__generated::GeneratedWorkerV3ArgumentBindingV1<'allocation>,
                __fe2o3_kernel_host::__generated::GeneratedArgumentPackError,
            > {
                let scalar_inputs = [#(#scalar_inputs),*].into_iter().collect();
                let memory_arguments = [#(#memory_arguments),*].into_iter().collect();
                Ok(
                    __fe2o3_kernel_host::__generated::GeneratedWorkerV3ArgumentBindingV1::
                        from_compiler_generated_parts_v1(scalar_inputs, memory_arguments),
                )
            }
        }

        #prepare_impl
    }
}

fn generated_worker_v3_layout_v1(model: &GeneralTypedSignatureModelV1) -> proc_macro2::TokenStream {
    let fields = model
        .arguments
        .iter()
        .zip(model.abi.fields())
        .map(|(kind, field)| generated_worker_v3_field_v1(*kind, field))
        .collect::<Vec<_>>();
    let size = model.abi.size();
    let alignment = model.abi.alignment();

    quote! {
        __fe2o3_kernel_host::__generated::CompilerGeneratedArgumentLayoutV1::new(
            #size,
            #alignment,
            __fe2o3_kernel_host::__generated::PointerWidth::Bits64,
            [#(#fields),*].into_iter().collect(),
        )
    }
}

fn generated_worker_v3_field_v1(
    kind: GeneralTypedArgumentKindV1,
    field: &AbiField,
) -> proc_macro2::TokenStream {
    let name = field.name().as_str();
    let offset = field.offset();
    let size = field.size();
    let alignment = field.alignment();
    let (scalar, shared, exclusive) = match kind {
        GeneralTypedArgumentKindV1::Scalar(scalar) => (scalar, false, false),
        GeneralTypedArgumentKindV1::SharedSlice(scalar) => (scalar, true, false),
        GeneralTypedArgumentKindV1::ExclusiveSlice(scalar) => (scalar, false, true),
        GeneralTypedArgumentKindV1::MappedExclusiveSlice(_, _)
        | GeneralTypedArgumentKindV1::GlobalMutPointer(_) => {
            unreachable!("unsupported V3 descriptor kinds do not generate an adapter")
        }
    };
    let rust_type = scalar.rust_type_tokens();
    let scalar_ident = match scalar {
        GeneralTypedScalarV1::I8 => format_ident!("I8"),
        GeneralTypedScalarV1::U8 => format_ident!("U8"),
        GeneralTypedScalarV1::I16 => format_ident!("I16"),
        GeneralTypedScalarV1::U16 => format_ident!("U16"),
        GeneralTypedScalarV1::I32 => format_ident!("I32"),
        GeneralTypedScalarV1::U32 => format_ident!("U32"),
        GeneralTypedScalarV1::I64 => format_ident!("I64"),
        GeneralTypedScalarV1::U64 => format_ident!("U64"),
        GeneralTypedScalarV1::F32 => format_ident!("F32"),
        GeneralTypedScalarV1::F64 => format_ident!("F64"),
    };
    let (abi_kind, mutability, access, address_space, type_identity, ownership, alias) =
        if shared || exclusive {
            let (element_size, element_alignment) = scalar.size_alignment();
            let type_identity = if shared {
                quote!(<#rust_type as __fe2o3_kernel_host::__generated::GeneratedDeviceScalarV1>::
                shared_slice_type_identity_v1(
                    __fe2o3_kernel_host::__generated::PointerWidth::Bits64,
                ))
            } else {
                quote!(<#rust_type as __fe2o3_kernel_host::__generated::GeneratedDeviceScalarV1>::
                disjoint_slice_type_identity_v1(
                    __fe2o3_kernel_host::__generated::PointerWidth::Bits64,
                ))
            };
            (
                quote!(__fe2o3_kernel_host::__generated::AbiKind::Slice {
                    element_size: #element_size,
                    element_alignment: #element_alignment,
                }),
                if shared {
                    quote!(__fe2o3_kernel_host::__generated::Mutability::Immutable)
                } else {
                    quote!(__fe2o3_kernel_host::__generated::Mutability::Mutable)
                },
                if shared {
                    quote!(__fe2o3_kernel_host::__generated::Access::ReadOnly)
                } else {
                    quote!(__fe2o3_kernel_host::__generated::Access::ReadWrite)
                },
                quote!(__fe2o3_kernel_host::__generated::AddressSpace::Global),
                type_identity,
                if shared {
                    quote!(__fe2o3_kernel_host::__generated::ArgumentOwnership::SharedBorrow)
                } else {
                    quote!(__fe2o3_kernel_host::__generated::ArgumentOwnership::UniqueBorrow)
                },
                if shared {
                    quote!(__fe2o3_kernel_host::__generated::AliasClass::SharedReadOnly)
                } else {
                    quote!(__fe2o3_kernel_host::__generated::AliasClass::Exclusive)
                },
            )
        } else {
            (
                quote!(__fe2o3_kernel_host::__generated::AbiKind::Scalar(
                    __fe2o3_kernel_host::__generated::ScalarType::#scalar_ident,
                )),
                quote!(__fe2o3_kernel_host::__generated::Mutability::Immutable),
                quote!(__fe2o3_kernel_host::__generated::Access::ByValue),
                quote!(__fe2o3_kernel_host::__generated::AddressSpace::Value),
                quote!(<#rust_type as __fe2o3_kernel_host::__generated::GeneratedDeviceScalarV1>::
                scalar_type_identity_v1(
                    __fe2o3_kernel_host::__generated::PointerWidth::Bits64,
                )),
                quote!(__fe2o3_kernel_host::__generated::ArgumentOwnership::ByValue),
                quote!(__fe2o3_kernel_host::__generated::AliasClass::Value),
            )
        };

    quote! {
        __fe2o3_kernel_host::__generated::AbiField::new(
            __fe2o3_kernel_host::__generated::Name::new(#name)
                .expect("generated Worker V3 argument name is valid"),
            #offset,
            #size,
            #alignment,
            #abi_kind,
            #mutability,
            #access,
            #address_space,
            #type_identity,
            #ownership,
            #alias,
        ).expect("generated Worker V3 ABI field is valid")
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AlphaZetaCov6MacroRoleV1 {
    Alpha,
    Zeta,
}

const SCALAR_GEMM_V1_ARGUMENT_NAMES: &[&str] = &["a", "b", "c", "m", "n", "k"];

fn exact_scalar_gemm_v1(input: &ItemFn, arguments: &[GeneralTypedArgumentKindV1]) -> bool {
    let names = input
        .sig
        .inputs
        .iter()
        .map(|argument| match argument {
            FnArg::Typed(argument) => match argument.pat.as_ref() {
                Pat::Ident(pattern) => pattern.ident.to_string(),
                _ => unreachable!("general typed validation requires identifier patterns"),
            },
            FnArg::Receiver(_) => unreachable!("general typed validation rejects receivers"),
        })
        .collect::<Vec<_>>();

    input.sig.ident == "scalar_gemm_v1"
        && names
            .iter()
            .map(String::as_str)
            .eq(SCALAR_GEMM_V1_ARGUMENT_NAMES.iter().copied())
        && arguments
            == [
                GeneralTypedArgumentKindV1::SharedSlice(GeneralTypedScalarV1::F32),
                GeneralTypedArgumentKindV1::SharedSlice(GeneralTypedScalarV1::F32),
                GeneralTypedArgumentKindV1::ExclusiveSlice(GeneralTypedScalarV1::F32),
                GeneralTypedArgumentKindV1::Scalar(GeneralTypedScalarV1::U32),
                GeneralTypedArgumentKindV1::Scalar(GeneralTypedScalarV1::U32),
                GeneralTypedArgumentKindV1::Scalar(GeneralTypedScalarV1::U32),
            ]
}

impl AlphaZetaCov6MacroRoleV1 {
    const fn argument_names(self) -> &'static [&'static str] {
        match self {
            Self::Alpha => &["scale", "input", "output"],
            Self::Zeta => &["a", "b", "bias", "output"],
        }
    }
}

fn exact_alpha_zeta_cov6_role_v1(
    input: &ItemFn,
    arguments: &[GeneralTypedArgumentKindV1],
) -> Option<AlphaZetaCov6MacroRoleV1> {
    let names = input
        .sig
        .inputs
        .iter()
        .map(|argument| match argument {
            FnArg::Typed(argument) => match argument.pat.as_ref() {
                Pat::Ident(pattern) => pattern.ident.to_string(),
                _ => unreachable!("general typed validation requires identifier patterns"),
            },
            FnArg::Receiver(_) => unreachable!("general typed validation rejects receivers"),
        })
        .collect::<Vec<_>>();

    match (
        input.sig.ident.to_string().as_str(),
        names.as_slice(),
        arguments,
    ) {
        (
            "alpha",
            [scale, input, output],
            [
                GeneralTypedArgumentKindV1::Scalar(GeneralTypedScalarV1::F32),
                GeneralTypedArgumentKindV1::SharedSlice(GeneralTypedScalarV1::F32),
                GeneralTypedArgumentKindV1::ExclusiveSlice(GeneralTypedScalarV1::F32),
            ],
        ) if scale == "scale" && input == "input" && output == "output" => {
            Some(AlphaZetaCov6MacroRoleV1::Alpha)
        }
        (
            "zeta",
            [a, b, bias, output],
            [
                GeneralTypedArgumentKindV1::SharedSlice(GeneralTypedScalarV1::F32),
                GeneralTypedArgumentKindV1::SharedSlice(GeneralTypedScalarV1::F32),
                GeneralTypedArgumentKindV1::Scalar(GeneralTypedScalarV1::F32),
                GeneralTypedArgumentKindV1::ExclusiveSlice(GeneralTypedScalarV1::F32),
            ],
        ) if a == "a" && b == "b" && bias == "bias" && output == "output" => {
            Some(AlphaZetaCov6MacroRoleV1::Zeta)
        }
        _ => None,
    }
}

#[allow(dead_code)]
fn model_general_typed_signature_v1(
    input: &ItemFn,
    options: &KernelOptions,
    kernel_binding: [u8; 32],
) -> syn::Result<GeneralTypedSignatureModelV1> {
    // The rustc collector must independently reconstruct this exact
    // type/layout/effect convention before accepting the emitted identity.
    validate_general_typed_function_shape_v1(input)?;
    let arguments = input
        .sig
        .inputs
        .iter()
        .enumerate()
        .map(|(index, argument)| parse_general_typed_argument_v1(argument, index + 1))
        .collect::<syn::Result<Vec<_>>>()?;
    let scalar_gemm_v1 = exact_scalar_gemm_v1(input, &arguments);
    let alpha_zeta_cov6 = exact_alpha_zeta_cov6_role_v1(input, &arguments);
    let exact_argument_names = if scalar_gemm_v1 {
        Some(SCALAR_GEMM_V1_ARGUMENT_NAMES)
    } else {
        alpha_zeta_cov6.map(AlphaZetaCov6MacroRoleV1::argument_names)
    };
    let abi = general_typed_abi_v1(&arguments, exact_argument_names, &input.sig)?;
    let launch = general_typed_launch_v1(options.launch.as_ref(), &input.sig)?;
    if (scalar_gemm_v1 || alpha_zeta_cov6.is_some())
        && launch.block_size()
            != BlockSize::Exact(general_typed_dimensions_v1(GENERAL_TYPED_DEFAULT_BLOCK_V1))
    {
        return Err(syn::Error::new_spanned(
            &input.sig,
            "the alpha/zeta and scalar_gemm_v1 typed profiles require an exact 256x1x1 launch contract",
        ));
    }
    let logical_name = input.sig.ident.to_string();
    let generated_host_contract_identity = derive_generated_host_contract_identity_v1(
        MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        kernel_binding,
        &logical_name,
        &logical_name,
        &abi,
        &launch,
    );
    Ok(GeneralTypedSignatureModelV1 {
        arguments,
        abi,
        launch,
        generated_host_contract_identity,
    })
}

fn validate_general_typed_function_shape_v1(input: &ItemFn) -> syn::Result<()> {
    let signature = &input.sig;
    if !matches!(input.vis, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            &input.vis,
            "general typed V1 requires a public kernel function",
        ));
    }
    if signature.unsafety.is_some() {
        return Err(syn::Error::new_spanned(
            signature.unsafety,
            "general typed V1 requires a safe kernel function",
        ));
    }
    if signature.constness.is_some() || signature.asyncness.is_some() || signature.abi.is_some() {
        return Err(syn::Error::new_spanned(
            signature,
            "general typed V1 requires a non-const synchronous Rust function",
        ));
    }
    if signature.variadic.is_some() {
        return Err(syn::Error::new_spanned(
            &signature.variadic,
            "general typed V1 does not support variadic functions",
        ));
    }
    if !signature.generics.params.is_empty() || signature.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &signature.generics,
            "general typed V1 does not support generic kernel functions",
        ));
    }
    if !is_unit_return(&signature.output) && !is_kernel_result_return(&signature.output) {
        return Err(syn::Error::new_spanned(
            &signature.output,
            "general typed V1 requires the unit return type or KernelResult",
        ));
    }
    if signature.inputs.is_empty() {
        return Err(syn::Error::new_spanned(
            &signature.inputs,
            "general typed V1 requires at least one kernel argument",
        ));
    }
    Ok(())
}

fn parse_general_typed_argument_v1(
    argument: &FnArg,
    position: usize,
) -> syn::Result<GeneralTypedArgumentKindV1> {
    let FnArg::Typed(argument) = argument else {
        return Err(syn::Error::new_spanned(
            argument,
            "general typed V1 does not support methods",
        ));
    };
    let Pat::Ident(pattern) = argument.pat.as_ref() else {
        return Err(syn::Error::new_spanned(
            &argument.pat,
            format!("general typed V1 argument {position} must use an identifier pattern"),
        ));
    };
    if pattern.by_ref.is_some() || pattern.subpat.is_some() {
        return Err(syn::Error::new_spanned(
            pattern,
            format!("general typed V1 argument {position} must use an identifier pattern"),
        ));
    }
    parse_general_typed_argument_type_v1(&argument.ty).map_err(|_| {
        syn::Error::new_spanned(
            &argument.ty,
            format!(
                "general typed V1 argument {position} must be a supported scalar, `&[T]`, a supported branded `fe2o3_device::DisjointSlice<T, IndexSpace>`, or `fe2o3_device::DeviceGlobalMutPtr<T>`"
            ),
        )
    })
}

fn parse_general_typed_argument_type_v1(ty: &Type) -> Result<GeneralTypedArgumentKindV1, ()> {
    if let Some(scalar) = parse_general_typed_scalar_v1(ty) {
        return Ok(GeneralTypedArgumentKindV1::Scalar(scalar));
    }
    if let Type::Reference(reference) = ty {
        if reference.lifetime.is_some() || reference.mutability.is_some() {
            return Err(());
        }
        let Type::Slice(slice) = reference.elem.as_ref() else {
            return Err(());
        };
        return parse_general_typed_scalar_v1(&slice.elem)
            .map(GeneralTypedArgumentKindV1::SharedSlice)
            .ok_or(());
    }

    let Type::Path(path) = ty else {
        return Err(());
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() {
        return Err(());
    }
    let segments = path.path.segments.iter().collect::<Vec<_>>();
    let (segment, kind) = match segments.as_slice() {
        [segment] if segment.ident == "DisjointSlice" => {
            (*segment, GeneralTypedPointerPathV1::DisjointSlice)
        }
        [segment] if segment.ident == "DeviceGlobalMutPtr" => {
            (*segment, GeneralTypedPointerPathV1::DeviceGlobalMutPointer)
        }
        [namespace, segment]
            if namespace.ident == "fe2o3_device"
                && matches!(namespace.arguments, PathArguments::None)
                && segment.ident == "DisjointSlice" =>
        {
            (*segment, GeneralTypedPointerPathV1::DisjointSlice)
        }
        [namespace, segment]
            if namespace.ident == "fe2o3_device"
                && matches!(namespace.arguments, PathArguments::None)
                && segment.ident == "DeviceGlobalMutPtr" =>
        {
            (*segment, GeneralTypedPointerPathV1::DeviceGlobalMutPointer)
        }
        _ => return Err(()),
    };
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(());
    };
    let valid_arity = match kind {
        GeneralTypedPointerPathV1::DisjointSlice => (1..=2).contains(&arguments.args.len()),
        GeneralTypedPointerPathV1::DeviceGlobalMutPointer => arguments.args.len() == 1,
    };
    if arguments.colon2_token.is_some() || !valid_arity {
        return Err(());
    }
    let Some(GenericArgument::Type(element)) = arguments.args.first() else {
        return Err(());
    };
    let scalar = parse_general_typed_scalar_v1(element).ok_or(())?;
    match kind {
        GeneralTypedPointerPathV1::DisjointSlice => {
            let index_space = if let Some(index_space) = arguments.args.iter().nth(1) {
                let GenericArgument::Type(index_space) = index_space else {
                    return Err(());
                };
                parse_disjoint_index_space_v1(index_space).ok_or(())?
            } else {
                RustDisjointIndexSpaceV1::Index1D
            };
            if index_space == RustDisjointIndexSpaceV1::Index1D {
                Ok(GeneralTypedArgumentKindV1::ExclusiveSlice(scalar))
            } else {
                Ok(GeneralTypedArgumentKindV1::MappedExclusiveSlice(
                    scalar,
                    index_space,
                ))
            }
        }
        GeneralTypedPointerPathV1::DeviceGlobalMutPointer => {
            Ok(GeneralTypedArgumentKindV1::GlobalMutPointer(scalar))
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum GeneralTypedPointerPathV1 {
    DisjointSlice,
    DeviceGlobalMutPointer,
}

fn parse_general_typed_scalar_v1(ty: &Type) -> Option<GeneralTypedScalarV1> {
    let Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    let segment = &path.path.segments[0];
    if !matches!(segment.arguments, PathArguments::None) {
        return None;
    }
    match segment.ident.to_string().as_str() {
        "i8" => Some(GeneralTypedScalarV1::I8),
        "u8" => Some(GeneralTypedScalarV1::U8),
        "i16" => Some(GeneralTypedScalarV1::I16),
        "u16" => Some(GeneralTypedScalarV1::U16),
        "i32" => Some(GeneralTypedScalarV1::I32),
        "u32" => Some(GeneralTypedScalarV1::U32),
        "i64" => Some(GeneralTypedScalarV1::I64),
        "u64" => Some(GeneralTypedScalarV1::U64),
        "f32" => Some(GeneralTypedScalarV1::F32),
        "f64" => Some(GeneralTypedScalarV1::F64),
        _ => None,
    }
}

fn is_index_1d_v1(ty: &Type) -> bool {
    let Type::Path(path) = ty else {
        return false;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() {
        return false;
    }
    let segments = path.path.segments.iter().collect::<Vec<_>>();
    match segments.as_slice() {
        [segment] => segment.ident == "Index1D" && matches!(segment.arguments, PathArguments::None),
        [namespace, segment] => {
            namespace.ident == "fe2o3_device"
                && matches!(namespace.arguments, PathArguments::None)
                && segment.ident == "Index1D"
                && matches!(segment.arguments, PathArguments::None)
        }
        _ => false,
    }
}

fn parse_disjoint_index_space_v1(ty: &Type) -> Option<RustDisjointIndexSpaceV1> {
    if is_index_1d_v1(ty) {
        return Some(RustDisjointIndexSpaceV1::Index1D);
    }
    let Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() || path.path.leading_colon.is_some() {
        return None;
    }
    let segments = path.path.segments.iter().collect::<Vec<_>>();
    let segment = match segments.as_slice() {
        [segment] => *segment,
        [namespace, segment]
            if namespace.ident == "fe2o3_device"
                && matches!(namespace.arguments, PathArguments::None) =>
        {
            *segment
        }
        _ => return None,
    };
    if segment.ident == "GridExclusive" && matches!(segment.arguments, PathArguments::None) {
        return Some(RustDisjointIndexSpaceV1::GridExclusive);
    }
    if segment.ident == "Blocked" {
        let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return None;
        };
        if arguments.colon2_token.is_some() || arguments.args.len() != 3 {
            return None;
        }
        let mut arguments = arguments.args.iter();
        let Some(GenericArgument::Type(base)) = arguments.next() else {
            return None;
        };
        if !is_index_1d_v1(base) {
            return None;
        }
        let lanes_per_block = parse_u64_const_argument_v1(arguments.next()?)?;
        let elements_per_lane = parse_u64_const_argument_v1(arguments.next()?)?;
        return RustDisjointIndexSpaceV1::blocked_index_1d(lanes_per_block, elements_per_lane);
    }
    if segment.ident == "Tiled2D" {
        let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return None;
        };
        if arguments.colon2_token.is_some() || arguments.args.len() != 5 {
            return None;
        }
        let mut arguments = arguments.args.iter();
        let Some(GenericArgument::Type(base)) = arguments.next() else {
            return None;
        };
        if !is_index_1d_v1(base) {
            return None;
        }
        let lanes_per_tile = parse_u64_const_argument_v1(arguments.next()?)?;
        let tile_rows = parse_u64_const_argument_v1(arguments.next()?)?;
        let tile_columns = parse_u64_const_argument_v1(arguments.next()?)?;
        let elements_per_lane = parse_u64_const_argument_v1(arguments.next()?)?;
        return RustDisjointIndexSpaceV1::tiled_2d_index_1d(
            lanes_per_tile,
            tile_rows,
            tile_columns,
            elements_per_lane,
        );
    }
    if segment.ident == "RowStriped2D" {
        let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
            return None;
        };
        if arguments.colon2_token.is_some() || arguments.args.len() != 3 {
            return None;
        }
        let mut arguments = arguments.args.iter();
        let Some(GenericArgument::Type(base)) = arguments.next() else {
            return None;
        };
        if !is_index_1d_v1(base) {
            return None;
        }
        let lanes_per_row = parse_u64_const_argument_v1(arguments.next()?)?;
        let elements_per_lane = parse_u64_const_argument_v1(arguments.next()?)?;
        return RustDisjointIndexSpaceV1::row_striped_2d_index_1d(lanes_per_row, elements_per_lane);
    }
    if segment.ident != "Shifted" {
        return None;
    }
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    if arguments.colon2_token.is_some() || arguments.args.len() != 2 {
        return None;
    }
    let mut arguments = arguments.args.iter();
    let Some(GenericArgument::Type(base)) = arguments.next() else {
        return None;
    };
    if !is_index_1d_v1(base) {
        return None;
    }
    let offset = parse_u64_const_argument_v1(arguments.next()?)?;
    Some(RustDisjointIndexSpaceV1::ShiftedIndex1D { offset })
}

fn parse_u64_const_argument_v1(argument: &GenericArgument) -> Option<u64> {
    let GenericArgument::Const(syn::Expr::Lit(value)) = argument else {
        return None;
    };
    let syn::Lit::Int(value) = &value.lit else {
        return None;
    };
    value.base10_parse().ok()
}

fn general_typed_abi_v1(
    arguments: &[GeneralTypedArgumentKindV1],
    exact_argument_names: Option<&[&str]>,
    span: impl quote::ToTokens,
) -> syn::Result<AbiLayout> {
    let mut offset = 0_u64;
    let mut layout_alignment = 1_u32;
    let mut fields = Vec::with_capacity(arguments.len());
    for (index, argument) in arguments.iter().copied().enumerate() {
        let (size, alignment) = match argument {
            GeneralTypedArgumentKindV1::Scalar(scalar) => scalar.size_alignment(),
            GeneralTypedArgumentKindV1::SharedSlice(_)
            | GeneralTypedArgumentKindV1::ExclusiveSlice(_)
            | GeneralTypedArgumentKindV1::MappedExclusiveSlice(_, _) => (
                GENERAL_TYPED_SLICE_SIZE_V1,
                GENERAL_TYPED_POINTER_ALIGNMENT_V1,
            ),
            GeneralTypedArgumentKindV1::GlobalMutPointer(_) => (
                GENERAL_TYPED_POINTER_SIZE_V1,
                GENERAL_TYPED_POINTER_ALIGNMENT_V1,
            ),
        };
        offset = align_up_v1(offset, alignment).ok_or_else(|| {
            syn::Error::new_spanned(&span, "general typed V1 argument layout overflows")
        })?;
        fields.push(
            general_typed_abi_field_v1(
                exact_argument_names
                    .map_or_else(|| format!("arg{index}"), |names| names[index].to_owned()),
                offset,
                argument,
            )
            .map_err(|error| {
                syn::Error::new_spanned(&span, format!("invalid general typed V1 ABI: {error}"))
            })?,
        );
        offset = offset.checked_add(size).ok_or_else(|| {
            syn::Error::new_spanned(&span, "general typed V1 argument layout overflows")
        })?;
        layout_alignment = layout_alignment.max(alignment);
    }
    let size = align_up_v1(offset, layout_alignment).ok_or_else(|| {
        syn::Error::new_spanned(&span, "general typed V1 argument layout overflows")
    })?;
    AbiLayout::new(size, layout_alignment, PointerWidth::Bits64, fields).map_err(|error| {
        syn::Error::new_spanned(span, format!("invalid general typed V1 ABI: {error}"))
    })
}

fn general_typed_abi_field_v1(
    name: String,
    offset: u64,
    argument: GeneralTypedArgumentKindV1,
) -> Result<AbiField, fe2o3_artifacts::ValidationError> {
    let name = Name::new(name)?;
    let (size, alignment, kind, mutability, access, address_space, ownership, alias_class) =
        match argument {
            GeneralTypedArgumentKindV1::Scalar(scalar) => {
                let (size, alignment) = scalar.size_alignment();
                (
                    size,
                    alignment,
                    AbiKind::Scalar(scalar.abi_scalar()),
                    Mutability::Immutable,
                    Access::ByValue,
                    AddressSpace::Value,
                    ArgumentOwnership::ByValue,
                    AliasClass::Value,
                )
            }
            GeneralTypedArgumentKindV1::SharedSlice(scalar) => (
                GENERAL_TYPED_SLICE_SIZE_V1,
                GENERAL_TYPED_POINTER_ALIGNMENT_V1,
                general_typed_slice_kind_v1(scalar),
                Mutability::Immutable,
                Access::ReadOnly,
                AddressSpace::Global,
                ArgumentOwnership::SharedBorrow,
                AliasClass::SharedReadOnly,
            ),
            GeneralTypedArgumentKindV1::ExclusiveSlice(scalar) => (
                GENERAL_TYPED_SLICE_SIZE_V1,
                GENERAL_TYPED_POINTER_ALIGNMENT_V1,
                general_typed_slice_kind_v1(scalar),
                Mutability::Mutable,
                // A proc macro cannot authenticate effects from the function
                // body. Exclusive slices therefore remain conservative.
                Access::ReadWrite,
                AddressSpace::Global,
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            ),
            GeneralTypedArgumentKindV1::MappedExclusiveSlice(scalar, _) => (
                GENERAL_TYPED_SLICE_SIZE_V1,
                GENERAL_TYPED_POINTER_ALIGNMENT_V1,
                general_typed_slice_kind_v1(scalar),
                Mutability::Mutable,
                Access::ReadWrite,
                AddressSpace::Global,
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            ),
            GeneralTypedArgumentKindV1::GlobalMutPointer(scalar) => {
                let (pointee_size, pointee_alignment) = scalar.size_alignment();
                (
                    GENERAL_TYPED_POINTER_SIZE_V1,
                    GENERAL_TYPED_POINTER_ALIGNMENT_V1,
                    AbiKind::Pointer {
                        pointee_size,
                        pointee_alignment,
                    },
                    Mutability::Mutable,
                    Access::ReadWrite,
                    AddressSpace::Global,
                    ArgumentOwnership::UniqueBorrow,
                    AliasClass::Exclusive,
                )
            }
        };
    AbiField::new(
        name,
        offset,
        size,
        alignment,
        kind,
        mutability,
        access,
        address_space,
        general_typed_type_identity_v1(argument),
        ownership,
        alias_class,
    )
}

fn general_typed_slice_kind_v1(scalar: GeneralTypedScalarV1) -> AbiKind {
    let (element_size, element_alignment) = scalar.size_alignment();
    AbiKind::Slice {
        element_size,
        element_alignment,
    }
}

fn general_typed_type_identity_v1(argument: GeneralTypedArgumentKindV1) -> TypeIdentity {
    match argument {
        GeneralTypedArgumentKindV1::Scalar(scalar) => general_typed_scalar_type_identity_v1(scalar),
        GeneralTypedArgumentKindV1::SharedSlice(scalar) => {
            general_typed_slice_type_identity_v1(scalar, false)
        }
        GeneralTypedArgumentKindV1::ExclusiveSlice(scalar) => {
            general_typed_slice_type_identity_v1(scalar, true)
        }
        GeneralTypedArgumentKindV1::MappedExclusiveSlice(scalar, index_space) => {
            general_typed_mapped_slice_type_identity_v1(scalar, index_space)
        }
        GeneralTypedArgumentKindV1::GlobalMutPointer(scalar) => {
            general_typed_global_mut_pointer_type_identity_v1(scalar)
        }
    }
}

fn general_typed_global_mut_pointer_type_identity_v1(scalar: GeneralTypedScalarV1) -> TypeIdentity {
    let scalar = scalar.rust_layout_scalar();
    let pointer = RustPhysicalComponentV1::new(
        0,
        GENERAL_TYPED_POINTER_SIZE_V1,
        GENERAL_TYPED_POINTER_ALIGNMENT_V1,
        RustPhysicalComponentKindV1::Pointer {
            mutability: RustPointerMutabilityV1::Mut,
            pointee: scalar,
        },
    )
    .expect("the fixed V1 global pointer component is valid");
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::global_mut_pointer(scalar)),
        RustcAbiClassV1::Scalar,
        PointerWidth::Bits64,
        GENERAL_TYPED_POINTER_SIZE_V1,
        GENERAL_TYPED_POINTER_ALIGNMENT_V1,
        vec![pointer],
    )
    .expect("the fixed V1 global pointer layout is valid")
    .type_identity()
}

fn general_typed_scalar_type_identity_v1(scalar: GeneralTypedScalarV1) -> TypeIdentity {
    let scalar = scalar.rust_layout_scalar();
    let size = scalar.size_bytes();
    let alignment = size as u32;
    let component = RustPhysicalComponentV1::new(
        0,
        size,
        alignment,
        RustPhysicalComponentKindV1::Scalar { scalar },
    )
    .expect("the fixed V1 scalar component is valid");
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::scalar(scalar)),
        RustcAbiClassV1::Scalar,
        PointerWidth::Bits64,
        size,
        alignment,
        vec![component],
    )
    .expect("the fixed V1 scalar layout is valid")
    .type_identity()
}

fn general_typed_slice_type_identity_v1(
    scalar: GeneralTypedScalarV1,
    exclusive: bool,
) -> TypeIdentity {
    let source_type = if exclusive {
        RustSourceTypeShapeV1::disjoint_slice(
            scalar.rust_layout_scalar(),
            RustDisjointIndexSpaceV1::Index1D,
        )
    } else {
        RustSourceTypeShapeV1::shared_slice(scalar.rust_layout_scalar())
    };
    let pointer = RustPhysicalComponentV1::new(
        0,
        GENERAL_TYPED_POINTER_SIZE_V1,
        GENERAL_TYPED_POINTER_ALIGNMENT_V1,
        RustPhysicalComponentKindV1::Pointer {
            mutability: if exclusive {
                RustPointerMutabilityV1::Mut
            } else {
                RustPointerMutabilityV1::Const
            },
            pointee: scalar.rust_layout_scalar(),
        },
    )
    .expect("the fixed V1 pointer layout is valid");
    let length = RustPhysicalComponentV1::new(
        GENERAL_TYPED_POINTER_SIZE_V1,
        GENERAL_TYPED_POINTER_SIZE_V1,
        GENERAL_TYPED_POINTER_ALIGNMENT_V1,
        RustPhysicalComponentKindV1::Usize,
    )
    .expect("the fixed V1 usize layout is valid");
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(source_type),
        RustcAbiClassV1::ScalarPair,
        PointerWidth::Bits64,
        GENERAL_TYPED_SLICE_SIZE_V1,
        GENERAL_TYPED_POINTER_ALIGNMENT_V1,
        vec![pointer, length],
    )
    .expect("the fixed V1 slice layout is valid")
    .type_identity()
}

fn general_typed_mapped_slice_type_identity_v1(
    scalar: GeneralTypedScalarV1,
    index_space: RustDisjointIndexSpaceV1,
) -> TypeIdentity {
    let scalar = scalar.rust_layout_scalar();
    let pointer = RustPhysicalComponentV1::new(
        0,
        GENERAL_TYPED_POINTER_SIZE_V1,
        GENERAL_TYPED_POINTER_ALIGNMENT_V1,
        RustPhysicalComponentKindV1::Pointer {
            mutability: RustPointerMutabilityV1::Mut,
            pointee: scalar,
        },
    )
    .expect("the fixed V1 pointer component is valid");
    let length = RustPhysicalComponentV1::new(
        GENERAL_TYPED_POINTER_SIZE_V1,
        GENERAL_TYPED_POINTER_SIZE_V1,
        GENERAL_TYPED_POINTER_ALIGNMENT_V1,
        RustPhysicalComponentKindV1::Usize,
    )
    .expect("the fixed V1 usize component is valid");
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::disjoint_slice(scalar, index_space)),
        RustcAbiClassV1::ScalarPair,
        PointerWidth::Bits64,
        GENERAL_TYPED_SLICE_SIZE_V1,
        GENERAL_TYPED_POINTER_ALIGNMENT_V1,
        vec![pointer, length],
    )
    .expect("the fixed V1 mapped slice layout is valid")
    .type_identity()
}

fn general_typed_launch_v1(
    launch: Option<&ParsedLaunchBoundsV1>,
    span: impl quote::ToTokens,
) -> syn::Result<LaunchContract> {
    let exact_block = match launch {
        None => GENERAL_TYPED_DEFAULT_BLOCK_V1,
        Some(launch) => exact_general_typed_block_v1(launch, &span)?,
    };
    let block_size = BlockSize::Exact(general_typed_dimensions_v1(exact_block));
    let max_grid = launch
        .and_then(|launch| launch.max_grid)
        .unwrap_or([u32::MAX, 1, 1]);
    let max_grid = general_typed_dimensions_v1(max_grid);
    LaunchContract::new(1, block_size, max_grid, 0, 0).map_err(|error| {
        syn::Error::new_spanned(span, format!("invalid general typed V1 launch: {error}"))
    })
}

fn exact_general_typed_block_v1(
    launch: &ParsedLaunchBoundsV1,
    span: impl quote::ToTokens,
) -> syn::Result<[u32; 3]> {
    if launch.min_workgroups_per_compute_unit.is_some() {
        return Err(syn::Error::new_spanned(
            span,
            "general typed V1 does not support launch occupancy constraints",
        ));
    }
    let Some(required) = launch.required else {
        return Err(syn::Error::new_spanned(
            span,
            "general typed V1 explicit launch requires required dimensions",
        ));
    };
    if launch.maximum.is_some_and(|maximum| required != maximum) {
        return Err(syn::Error::new_spanned(
            span,
            "general typed V1 explicit launch requires identical required and max dimensions",
        ));
    }
    if required != GENERAL_TYPED_DEFAULT_BLOCK_V1 && required != GENERAL_TYPED_WAVE64_BLOCK_V1 {
        return Err(syn::Error::new_spanned(
            span,
            "general typed V1 supports only exact 64x1x1 or 256x1x1 launch dimensions",
        ));
    }
    Ok(required)
}

fn validate_fixed_wg256_launch_v1(
    launch: Option<&ParsedLaunchBoundsV1>,
    span: impl quote::ToTokens,
) -> syn::Result<()> {
    let Some(launch) = launch else {
        return Ok(());
    };
    if launch.max_grid.is_some() {
        return Err(syn::Error::new_spanned(
            &span,
            "the typed vecadd V2 profile does not support max_grid",
        ));
    }
    let exact = exact_general_typed_block_v1(launch, &span)?;
    if exact != GENERAL_TYPED_DEFAULT_BLOCK_V1 {
        return Err(syn::Error::new_spanned(
            span,
            "the typed vecadd V2 profile requires an exact 256x1x1 launch contract",
        ));
    }
    Ok(())
}

fn general_typed_dimensions_v1(dimensions: [u32; 3]) -> Dimensions {
    Dimensions::new(dimensions[0], dimensions[1], dimensions[2])
        .expect("macro launch parsing already validates dimensions")
}

const fn align_up_v1(value: u64, alignment: u32) -> Option<u64> {
    let alignment = alignment as u64;
    let mask = alignment - 1;
    match value.checked_add(mask) {
        Some(value) => Some(value & !mask),
        None => None,
    }
}

fn encode_kernel_frontend_contract_v1(options: &KernelOptions) -> Option<Vec<u8>> {
    if options.launch.is_none() && options.unsafe_assembly.is_none() {
        return None;
    }
    let mut bytes = Vec::with_capacity(76);
    bytes.extend_from_slice(&FRONTEND_KERNEL_CONTRACT_MAGIC_V1);
    push_u16(&mut bytes, 1);
    let flags = u16::from(options.launch.is_some())
        | (u16::from(options.unsafe_assembly.is_some()) * 0x0002);
    push_u16(&mut bytes, flags);
    push_u32(&mut bytes, 0);
    push_u32(&mut bytes, 0);
    if let Some(launch) = options.launch {
        let launch_flags = u16::from(launch.required.is_some())
            | (u16::from(launch.maximum.is_some()) * 0x0002)
            | (u16::from(launch.min_workgroups_per_compute_unit.is_some()) * 0x0004)
            | (u16::from(launch.max_grid.is_some()) * 0x0008);
        push_u16(&mut bytes, launch_flags);
        push_u16(&mut bytes, 0);
        push_dimensions(&mut bytes, launch.required);
        push_dimensions(&mut bytes, launch.maximum);
        push_u16(
            &mut bytes,
            launch.min_workgroups_per_compute_unit.unwrap_or(0),
        );
        push_u16(&mut bytes, 0);
        if launch.max_grid.is_some() {
            push_dimensions(&mut bytes, launch.max_grid);
        }
    }
    if let Some(assembly) = options.unsafe_assembly {
        push_u16(&mut bytes, assembly.target);
        push_u16(&mut bytes, assembly.operands);
        push_u16(&mut bytes, assembly.options);
        push_u16(&mut bytes, assembly.effects);
        push_u32(&mut bytes, 0);
    }
    let length = u32::try_from(bytes.len()).expect("V1 kernel contract is bounded below u32");
    bytes[12..16].copy_from_slice(&length.to_le_bytes());
    debug_assert!(bytes.len() <= 76);
    Some(bytes)
}

fn push_dimensions(bytes: &mut Vec<u8>, dimensions: Option<[u32; 3]>) {
    for component in dimensions.unwrap_or([0; 3]) {
        push_u32(bytes, component);
    }
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

#[derive(Default)]
struct KernelAssemblyUseVisitor {
    inline_assembly: Option<proc_macro2::Span>,
    unsupported_assembly: Option<proc_macro2::Span>,
}

impl<'ast> Visit<'ast> for KernelAssemblyUseVisitor {
    fn visit_macro(&mut self, invocation: &'ast syn::Macro) {
        let Some(name) = invocation
            .path
            .segments
            .last()
            .map(|segment| &segment.ident)
        else {
            return;
        };
        if name == "asm" {
            self.inline_assembly.get_or_insert(name.span());
        } else if name == "global_asm" || name == "llvm_asm" {
            self.unsupported_assembly.get_or_insert(name.span());
        }
        syn::visit::visit_macro(self, invocation);
    }
}

#[derive(Default)]
struct KernelUnsafeSyntaxVisitor {
    first_use: Option<(proc_macro2::Span, &'static str)>,
}

impl KernelUnsafeSyntaxVisitor {
    fn record(&mut self, span: proc_macro2::Span, diagnostic: &'static str) {
        self.first_use.get_or_insert((span, diagnostic));
    }
}

impl<'ast> Visit<'ast> for KernelUnsafeSyntaxVisitor {
    fn visit_expr_unsafe(&mut self, expression: &'ast syn::ExprUnsafe) {
        self.record(
            expression.unsafe_token.span,
            "unsafe blocks are not allowed in ordinary #[kernel] bodies; use safe fe2o3 device APIs or an explicitly declared low-level provider boundary",
        );
        syn::visit::visit_expr_unsafe(self, expression);
    }

    fn visit_item_fn(&mut self, function: &'ast ItemFn) {
        if let Some(unsafety) = function.sig.unsafety {
            self.record(
                unsafety.span,
                "unsafe function declarations are not allowed inside ordinary #[kernel] bodies",
            );
        }
        syn::visit::visit_item_fn(self, function);
    }

    fn visit_item_foreign_mod(&mut self, foreign: &'ast ItemForeignMod) {
        if let Some(unsafety) = foreign.unsafety {
            self.record(
                unsafety.span,
                "unsafe extern blocks are not allowed inside ordinary #[kernel] bodies; declare raw device FFI with device_import/device_export",
            );
        }
        syn::visit::visit_item_foreign_mod(self, foreign);
    }

    fn visit_item_impl(&mut self, implementation: &'ast syn::ItemImpl) {
        if let Some(unsafety) = implementation.unsafety {
            self.record(
                unsafety.span,
                "unsafe impl declarations are not allowed inside ordinary #[kernel] bodies",
            );
        }
        syn::visit::visit_item_impl(self, implementation);
    }

    fn visit_item_trait(&mut self, trait_item: &'ast syn::ItemTrait) {
        if let Some(unsafety) = trait_item.unsafety {
            self.record(
                unsafety.span,
                "unsafe trait declarations are not allowed inside ordinary #[kernel] bodies",
            );
        }
        syn::visit::visit_item_trait(self, trait_item);
    }

    fn visit_impl_item_fn(&mut self, function: &'ast syn::ImplItemFn) {
        if let Some(unsafety) = function.sig.unsafety {
            self.record(
                unsafety.span,
                "unsafe method declarations are not allowed inside ordinary #[kernel] bodies",
            );
        }
        syn::visit::visit_impl_item_fn(self, function);
    }

    fn visit_trait_item_fn(&mut self, function: &'ast syn::TraitItemFn) {
        if let Some(unsafety) = function.sig.unsafety {
            self.record(
                unsafety.span,
                "unsafe method declarations are not allowed inside ordinary #[kernel] bodies",
            );
        }
        syn::visit::visit_trait_item_fn(self, function);
    }

    fn visit_foreign_item_fn(&mut self, function: &'ast syn::ForeignItemFn) {
        if let Some(unsafety) = function.sig.unsafety {
            self.record(
                unsafety.span,
                "unsafe foreign functions are not allowed inside ordinary #[kernel] bodies; declare raw device FFI with device_import/device_export",
            );
        }
        syn::visit::visit_foreign_item_fn(self, function);
    }

    fn visit_type_bare_fn(&mut self, function: &'ast syn::TypeBareFn) {
        if let Some(unsafety) = function.unsafety {
            self.record(
                unsafety.span,
                "unsafe function pointer types are not allowed in ordinary #[kernel] signatures or bodies",
            );
        }
        syn::visit::visit_type_bare_fn(self, function);
    }
}

fn validate_kernel_source_safety(
    input: &ItemFn,
    declared_unsafe_assembly: bool,
) -> syn::Result<()> {
    if declared_unsafe_assembly {
        return Ok(());
    }
    if let Some(unsafety) = input.sig.unsafety {
        return Err(syn::Error::new(
            unsafety.span,
            "ordinary #[kernel] functions must be safe; use unsafe_asm(...) only for an explicitly declared low-level assembly kernel",
        ));
    }

    let mut visitor = KernelUnsafeSyntaxVisitor::default();
    visitor.visit_generics(&input.sig.generics);
    for argument in &input.sig.inputs {
        visitor.visit_fn_arg(argument);
    }
    visitor.visit_return_type(&input.sig.output);
    visitor.visit_block(&input.block);
    if let Some((span, diagnostic)) = visitor.first_use {
        return Err(syn::Error::new(span, diagnostic));
    }
    Ok(())
}

fn validate_kernel_assembly_boundary(
    input: &ItemFn,
    declaration: Option<ParsedUnsafeAssemblyV1>,
) -> syn::Result<()> {
    let mut visitor = KernelAssemblyUseVisitor::default();
    visitor.visit_block(&input.block);
    if let Some(span) = visitor.unsupported_assembly {
        return Err(syn::Error::new(
            span,
            "#[kernel] supports only local asm! declarations; global_asm! and llvm_asm! fail closed",
        ));
    }
    if let Some(span) = visitor.inline_assembly
        && declaration.is_none()
    {
        return Err(syn::Error::new(
            span,
            "asm! reachable directly from a kernel requires an explicit unsafe_asm(...) declaration",
        ));
    }
    if declaration.is_some() && input.sig.unsafety.is_none() {
        return Err(syn::Error::new_spanned(
            &input.sig,
            "unsafe_asm(...) requires an unsafe kernel function",
        ));
    }
    Ok(())
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

#[proc_macro_attribute]
pub fn device_export(attr: TokenStream, item: TokenStream) -> TokenStream {
    let options = match parse_device_ffi_options(attr.into()) {
        Ok(options) => options,
        Err(error) => return error.to_compile_error().into(),
    };
    let input = parse_macro_input!(item as ItemFn);
    expand_device_export(input, options)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_attribute]
pub fn device_import(attr: TokenStream, item: TokenStream) -> TokenStream {
    let options = match parse_device_ffi_options(attr.into()) {
        Ok(options) => options,
        Err(error) => return error.to_compile_error().into(),
    };
    let input = parse_macro_input!(item as ItemForeignMod);
    expand_device_import(input, options)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct DeviceFfiOptions {
    symbol: String,
    target: String,
    code_object: u16,
    effects: String,
    semantic: String,
}

const DEVICE_FFI_OPTION_HELP: &str = "device FFI requires `symbol = \"...\", target = \"gfx...\", code_object = 4|5|6, effects = \"...\", semantic = \"<64 lowercase hex bytes>\"`";

fn parse_device_ffi_options(tokens: proc_macro2::TokenStream) -> syn::Result<DeviceFfiOptions> {
    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(tokens)?;
    let mut symbol = None;
    let mut target = None;
    let mut code_object = None;
    let mut effects = None;
    let mut semantic = None;

    for argument in arguments {
        let Meta::NameValue(value) = argument else {
            return Err(syn::Error::new_spanned(argument, DEVICE_FFI_OPTION_HELP));
        };
        let slot = if value.path.is_ident("symbol") {
            &mut symbol
        } else if value.path.is_ident("target") {
            &mut target
        } else if value.path.is_ident("effects") {
            &mut effects
        } else if value.path.is_ident("semantic") {
            &mut semantic
        } else if value.path.is_ident("code_object") {
            if code_object.is_some() {
                return Err(syn::Error::new_spanned(
                    value,
                    "duplicate device FFI option",
                ));
            }
            let Expr::Lit(literal) = &value.value else {
                return Err(syn::Error::new_spanned(value, DEVICE_FFI_OPTION_HELP));
            };
            let Lit::Int(literal) = &literal.lit else {
                return Err(syn::Error::new_spanned(value, DEVICE_FFI_OPTION_HELP));
            };
            let parsed = literal.base10_parse::<u16>()?;
            if !matches!(parsed, 4..=6) {
                return Err(syn::Error::new_spanned(
                    literal,
                    "device FFI code_object must be exactly 4, 5, or 6",
                ));
            }
            code_object = Some(parsed);
            continue;
        } else {
            return Err(syn::Error::new_spanned(value, DEVICE_FFI_OPTION_HELP));
        };
        if slot.is_some() {
            return Err(syn::Error::new_spanned(
                value,
                "duplicate device FFI option",
            ));
        }
        let Expr::Lit(literal) = &value.value else {
            return Err(syn::Error::new_spanned(value, DEVICE_FFI_OPTION_HELP));
        };
        let Lit::Str(literal) = &literal.lit else {
            return Err(syn::Error::new_spanned(value, DEVICE_FFI_OPTION_HELP));
        };
        *slot = Some(literal.value());
    }

    let options = DeviceFfiOptions {
        symbol: symbol.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), DEVICE_FFI_OPTION_HELP)
        })?,
        target: target.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), DEVICE_FFI_OPTION_HELP)
        })?,
        code_object: code_object.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), DEVICE_FFI_OPTION_HELP)
        })?,
        effects: effects.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), DEVICE_FFI_OPTION_HELP)
        })?,
        semantic: semantic.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), DEVICE_FFI_OPTION_HELP)
        })?,
    };
    reserved_fe2o3_symbols::validate_device_ffi_symbol_v1(&options.symbol)
        .map_err(|error| device_ffi_grammar_diagnostic(error, &options.effects))?;
    validate_device_ffi_target(&options.target)?;
    reserved_fe2o3_symbols::parse_device_ffi_effects_v1(&options.effects)
        .map_err(|error| device_ffi_grammar_diagnostic(error, &options.effects))?;
    validate_lower_hex_256(&options.semantic, "semantic identity")?;
    Ok(options)
}

fn expand_device_export(
    input: ItemFn,
    options: DeviceFfiOptions,
) -> syn::Result<proc_macro2::TokenStream> {
    let device_import = fe2o3_device_import()?;
    expand_device_export_with_import(input, options, &device_import)
}

fn expand_device_export_with_import(
    input: ItemFn,
    options: DeviceFfiOptions,
    device_import: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    validate_device_ffi_signature(&input.sig, &input.vis, true)?;
    validate_device_ffi_attributes(&input.attrs)?;
    let physical_abi = canonical_device_ffi_signature(&input.sig)?;
    validate_generated_device_ffi_contract_grammar(&options, &physical_abi)?;
    let direction = reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_EXPORT_V1;
    let contract = device_ffi_contract(direction, &options, &physical_abi);
    let marker = reserved_fe2o3_symbols::device_ffi_marker_v1(
        contract,
        device_ffi_fields(direction, &options, &physical_abi),
    );
    let contract_hex = contract.to_hex();
    let registration_ident = format_ident!(
        "{}{}",
        reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_PREFIX_V1,
        contract_hex
    );
    let function_ident = &input.sig.ident;
    let export_marker_ident = format_ident!("__fe2o3_device_export_marker_{}", function_ident);
    let visibility = input.vis.clone();
    let inputs = input.sig.inputs.iter().map(|argument| match argument {
        FnArg::Typed(argument) => argument.ty.as_ref(),
        FnArg::Receiver(_) => unreachable!("device FFI methods were rejected"),
    });
    let output = &input.sig.output;
    let function_pointer = quote!(unsafe extern "C" fn(#(#inputs),*) #output);
    let symbol = syn::LitStr::new(&options.symbol, function_ident.span());
    let marker = syn::LitStr::new(&marker, function_ident.span());
    let contract_literal = syn::LitStr::new(&contract_hex, function_ident.span());
    let registration = device_ffi_registration_tokens(
        direction,
        &options,
        &physical_abi,
        &contract_hex,
        quote!(#function_ident),
        &input.sig,
    );
    let abi_assertions = device_ffi_abi_assertions(&input.sig, device_import)?;

    Ok(quote! {
        #[doc = #marker]
        #[unsafe(export_name = #symbol)]
        #input

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        #[used]
        #visibility static #registration_ident: #registration

        #[doc(hidden)]
        #[allow(non_camel_case_types)]
        #visibility enum #export_marker_ident {}

        const _: () = {
            #device_import

            // SAFETY: both values are generated from the same parsed export.
            unsafe impl __fe2o3_kernel_device::CrossCrateDeviceExportV1
                for #export_marker_ident
            {
                type Function = #function_pointer;
                const CONTRACT_ID: &'static str = #contract_literal;
                const FUNCTION: Self::Function = #function_ident;
            }
        };

        #abi_assertions
    })
}

fn expand_device_import(
    input: ItemForeignMod,
    options: DeviceFfiOptions,
) -> syn::Result<proc_macro2::TokenStream> {
    let device_import = fe2o3_device_import()?;
    expand_device_import_with_import(input, options, &device_import)
}

fn expand_device_import_with_import(
    mut input: ItemForeignMod,
    options: DeviceFfiOptions,
    device_import: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    if input.unsafety.is_none() {
        return Err(syn::Error::new_spanned(
            &input.abi,
            "#[device_import] requires an `unsafe extern \"C\"` block",
        ));
    }
    validate_c_abi(input.abi.name.as_ref(), &input.abi)?;
    if input.items.len() != 1 {
        return Err(syn::Error::new_spanned(
            &input,
            "#[device_import] requires exactly one foreign function declaration",
        ));
    }
    let inherited_abi = input.abi.clone();
    let ForeignItem::Fn(function) = &mut input.items[0] else {
        return Err(syn::Error::new_spanned(
            &input.items[0],
            "#[device_import] accepts only a foreign function declaration",
        ));
    };
    let mut effective_signature = function.sig.clone();
    effective_signature.abi = Some(inherited_abi);
    validate_device_ffi_signature(&effective_signature, &function.vis, false)?;
    validate_device_ffi_attributes(&function.attrs)?;
    let physical_abi = canonical_device_ffi_signature(&function.sig)?;
    validate_generated_device_ffi_contract_grammar(&options, &physical_abi)?;
    let direction = reserved_fe2o3_symbols::DEVICE_FFI_DIRECTION_IMPORT_V1;
    let contract = device_ffi_contract(direction, &options, &physical_abi);
    let marker = reserved_fe2o3_symbols::device_ffi_marker_v1(
        contract,
        device_ffi_fields(direction, &options, &physical_abi),
    );
    let contract_hex = contract.to_hex();
    let registration_ident = format_ident!(
        "{}{}",
        reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_PREFIX_V1,
        contract_hex
    );
    let function_ident = function.sig.ident.clone();
    let symbol = syn::LitStr::new(&options.symbol, function_ident.span());
    let marker = syn::LitStr::new(&marker, function_ident.span());
    function.attrs.push(syn::parse_quote!(#[doc = #marker]));
    function
        .attrs
        .push(syn::parse_quote!(#[link_name = #symbol]));
    let registration = device_ffi_registration_tokens(
        direction,
        &options,
        &physical_abi,
        &contract_hex,
        quote!(#function_ident),
        &function.sig,
    );
    let abi_assertions = device_ffi_abi_assertions(&function.sig, device_import)?;

    Ok(quote! {
        #input

        #[doc(hidden)]
        #[allow(non_upper_case_globals)]
        #[used]
        static #registration_ident: #registration

        #abi_assertions
    })
}

fn device_ffi_fields<'a>(
    direction: u16,
    options: &'a DeviceFfiOptions,
    physical_abi: &'a str,
) -> reserved_fe2o3_symbols::DeviceFfiContractFieldsV1<'a> {
    reserved_fe2o3_symbols::DeviceFfiContractFieldsV1 {
        direction,
        symbol: &options.symbol,
        calling_convention: "C",
        code_object_version: options.code_object,
        target: &options.target,
        physical_abi,
        effects: &options.effects,
        semantic_identity: &options.semantic,
    }
}

fn device_ffi_contract(
    direction: u16,
    options: &DeviceFfiOptions,
    physical_abi: &str,
) -> reserved_fe2o3_symbols::DeviceFfiContractIdV1 {
    reserved_fe2o3_symbols::derive_device_ffi_contract_id_v1(device_ffi_fields(
        direction,
        options,
        physical_abi,
    ))
}

fn device_ffi_registration_tokens(
    direction: u16,
    options: &DeviceFfiOptions,
    physical_abi: &str,
    contract_hex: &str,
    function: proc_macro2::TokenStream,
    signature: &syn::Signature,
) -> proc_macro2::TokenStream {
    let magic = reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_MAGIC_V1;
    let version = reserved_fe2o3_symbols::DEVICE_FFI_REGISTRATION_VERSION_V1;
    let contract = syn::LitStr::new(contract_hex, signature.ident.span());
    let symbol = syn::LitStr::new(&options.symbol, signature.ident.span());
    let target = syn::LitStr::new(&options.target, signature.ident.span());
    let physical_abi = syn::LitStr::new(physical_abi, signature.ident.span());
    let effects = syn::LitStr::new(&options.effects, signature.ident.span());
    let semantic = syn::LitStr::new(&options.semantic, signature.ident.span());
    let code_object = options.code_object;
    let inputs = signature.inputs.iter().map(|argument| match argument {
        FnArg::Typed(argument) => argument.ty.as_ref(),
        FnArg::Receiver(_) => unreachable!("device FFI methods were rejected"),
    });
    let output = &signature.output;
    quote! {
        (
            u64, u16, u16, &'static str, &'static str, &'static str, u16,
            &'static str, &'static str, &'static str, &'static str,
            unsafe extern "C" fn(#(#inputs),*) #output,
        ) = (
            #magic, #version, #direction, #contract, #symbol, "C", #code_object,
            #target, #physical_abi, #effects, #semantic, #function,
        );
    }
}

fn device_ffi_abi_assertions(
    signature: &syn::Signature,
    device_import: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let inputs = signature.inputs.iter().map(|argument| match argument {
        FnArg::Typed(argument) => argument.ty.as_ref(),
        FnArg::Receiver(_) => unreachable!("device FFI methods were rejected"),
    });
    let outputs = match &signature.output {
        ReturnType::Default => Vec::new(),
        ReturnType::Type(_, ty) if matches!(ty.as_ref(), Type::Tuple(tuple) if tuple.elems.is_empty()) => {
            Vec::new()
        }
        ReturnType::Type(_, ty) => vec![ty.as_ref()],
    };
    Ok(quote! {
        const _: () = {
            #device_import
            fn __fe2o3_assert_device_ffi_abi_v1<T: __fe2o3_kernel_device::DeviceFfiAbiTypeV1>() {}
            #(let _ = __fe2o3_assert_device_ffi_abi_v1::<#inputs>;)*
            #(let _ = __fe2o3_assert_device_ffi_abi_v1::<#outputs>;)*
        };
    })
}

fn validate_device_ffi_signature(
    signature: &syn::Signature,
    visibility: &Visibility,
    export: bool,
) -> syn::Result<()> {
    if !matches!(visibility, Visibility::Public(_)) {
        return Err(syn::Error::new_spanned(
            visibility,
            "device FFI declarations must be public",
        ));
    }
    if export && signature.unsafety.is_none() {
        return Err(syn::Error::new_spanned(
            signature,
            "#[device_export] requires `pub unsafe extern \"C\" fn`; the attribute grants no safe-call authority",
        ));
    }
    if signature.constness.is_some() || signature.asyncness.is_some() {
        return Err(syn::Error::new_spanned(
            signature,
            "device FFI declarations cannot be const or async",
        ));
    }
    if signature.variadic.is_some() {
        return Err(syn::Error::new_spanned(
            signature,
            "variadic device FFI is unsupported",
        ));
    }
    if !signature.generics.params.is_empty() || signature.generics.where_clause.is_some() {
        return Err(syn::Error::new_spanned(
            &signature.generics,
            "device FFI roots require a concrete, nongeneric instance identity",
        ));
    }
    validate_c_abi(
        signature.abi.as_ref().and_then(|abi| abi.name.as_ref()),
        signature,
    )?;
    if signature.inputs.len() > reserved_fe2o3_symbols::MAX_DEVICE_FFI_ARGUMENTS_V1 {
        return Err(syn::Error::new_spanned(
            &signature.inputs,
            format!(
                "device FFI has more than {} physical arguments",
                reserved_fe2o3_symbols::MAX_DEVICE_FFI_ARGUMENTS_V1
            ),
        ));
    }
    for argument in &signature.inputs {
        let FnArg::Typed(argument) = argument else {
            return Err(syn::Error::new_spanned(
                argument,
                "device FFI methods are unsupported",
            ));
        };
        canonical_device_ffi_type(&argument.ty)?;
    }
    if let ReturnType::Type(_, result) = &signature.output
        && !matches!(result.as_ref(), Type::Tuple(tuple) if tuple.elems.is_empty())
    {
        canonical_device_ffi_type(result)?;
    }
    Ok(())
}

fn validate_c_abi<T: quote::ToTokens>(name: Option<&syn::LitStr>, tokens: T) -> syn::Result<()> {
    if name.is_some_and(|name| name.value() == "C") {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            tokens,
            "device FFI requires the non-unwinding `extern \"C\"` calling convention",
        ))
    }
}

fn validate_device_ffi_attributes(attributes: &[syn::Attribute]) -> syn::Result<()> {
    for attribute in attributes {
        let forbidden = [
            "no_mangle",
            "export_name",
            "link_name",
            "target_feature",
            "naked",
            "track_caller",
        ]
        .into_iter()
        .any(|name| attribute.path().is_ident(name));
        if forbidden || attribute.path().is_ident("unsafe") {
            return Err(syn::Error::new_spanned(
                attribute,
                "device FFI macros exclusively control symbol, ABI, and unwind-relevant attributes",
            ));
        }
    }
    Ok(())
}

fn canonical_device_ffi_signature(signature: &syn::Signature) -> syn::Result<String> {
    let mut value = String::from("C(");
    for (index, argument) in signature.inputs.iter().enumerate() {
        if index != 0 {
            value.push(',');
        }
        let FnArg::Typed(argument) = argument else {
            unreachable!("device FFI methods were rejected");
        };
        value.push_str(&canonical_device_ffi_type(&argument.ty)?);
    }
    value.push_str(")->");
    match &signature.output {
        ReturnType::Default => value.push_str("unit[size=0,align=1]"),
        ReturnType::Type(_, result) if matches!(result.as_ref(), Type::Tuple(tuple) if tuple.elems.is_empty()) =>
        {
            value.push_str("unit[size=0,align=1]");
        }
        ReturnType::Type(_, result) => value.push_str(&canonical_device_ffi_type(result)?),
    }
    if value.len() > reserved_fe2o3_symbols::MAX_DEVICE_FFI_PHYSICAL_ABI_BYTES_V1 {
        return Err(syn::Error::new_spanned(
            signature,
            "device FFI physical ABI exceeds its bounded canonical representation",
        ));
    }
    Ok(value)
}

fn canonical_device_ffi_type(ty: &Type) -> syn::Result<String> {
    let Type::Path(path) = ty else {
        return Err(unsupported_device_ffi_type(ty));
    };
    if path.qself.is_some() {
        return Err(unsupported_device_ffi_type(ty));
    }
    let segment = path.path.segments.last().expect("type path is nonempty");
    if path.path.segments.len() == 1 && matches!(segment.arguments, PathArguments::None) {
        let (size, name) = match segment.ident.to_string().as_str() {
            "i8" => (1, "i8"),
            "u8" => (1, "u8"),
            "i16" => (2, "i16"),
            "u16" => (2, "u16"),
            "i32" => (4, "i32"),
            "u32" => (4, "u32"),
            "i64" => (8, "i64"),
            "u64" => (8, "u64"),
            "f32" => (4, "f32"),
            "f64" => (8, "f64"),
            _ => return Err(unsupported_device_ffi_type(ty)),
        };
        return Ok(format!("{name}[size={size},align={size}]"));
    }

    let (mutable, address_space) = match segment.ident.to_string().as_str() {
        "DeviceGlobalConstPtr" => (false, "global"),
        "DeviceGlobalMutPtr" => (true, "global"),
        "DeviceConstantPtr" => (false, "constant"),
        "DeviceWorkgroupConstPtr" => (false, "workgroup"),
        "DeviceWorkgroupMutPtr" => (true, "workgroup"),
        "DevicePrivateConstPtr" => (false, "private"),
        "DevicePrivateMutPtr" => (true, "private"),
        _ => return Err(unsupported_device_ffi_type(ty)),
    };
    let PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(unsupported_device_ffi_type(ty));
    };
    if arguments.args.len() != 1 {
        return Err(unsupported_device_ffi_type(ty));
    }
    let Some(GenericArgument::Type(element)) = arguments.args.first() else {
        return Err(unsupported_device_ffi_type(ty));
    };
    let element = canonical_device_ffi_type(element)?;
    let Some(element) = element.split('[').next() else {
        return Err(unsupported_device_ffi_type(ty));
    };
    if element.contains("ptr") {
        return Err(unsupported_device_ffi_type(ty));
    }
    Ok(format!(
        "{}_ptr<{address_space},{element}>[size=8,align=8,as={address_space}]",
        if mutable { "mut" } else { "const" },
    ))
}

fn unsupported_device_ffi_type<T: quote::ToTokens>(ty: T) -> syn::Error {
    syn::Error::new_spanned(
        ty,
        "unsupported device FFI type; use fixed-width scalars or fe2o3 address-space pointer wrappers (references, aggregates, trait objects, and function pointers are rejected)",
    )
}

fn validate_generated_device_ffi_contract_grammar(
    options: &DeviceFfiOptions,
    physical_abi: &str,
) -> syn::Result<()> {
    reserved_fe2o3_symbols::validate_device_ffi_contract_grammar_v1(
        &options.symbol,
        physical_abi,
        &options.effects,
    )
    .map(|_| ())
    .map_err(|error| device_ffi_grammar_diagnostic(error, &options.effects))
}

fn device_ffi_grammar_diagnostic(
    error: reserved_fe2o3_symbols::DeviceFfiGrammarError,
    effects: &str,
) -> syn::Error {
    use reserved_fe2o3_symbols::DeviceFfiGrammarError;

    let message = match error {
        DeviceFfiGrammarError::InvalidDirection => {
            "device FFI direction is not canonical".to_owned()
        }
        DeviceFfiGrammarError::InvalidSymbol => {
            "device FFI symbol is empty, too long, or contains noncanonical bytes".to_owned()
        }
        DeviceFfiGrammarError::InvalidPhysicalAbi => {
            "device FFI physical ABI is not canonical".to_owned()
        }
        DeviceFfiGrammarError::TooManyPhysicalAbiArguments => format!(
            "device FFI has more than {} physical arguments",
            reserved_fe2o3_symbols::MAX_DEVICE_FFI_ARGUMENTS_V1
        ),
        DeviceFfiGrammarError::InvalidEffects
            if effects.len() > reserved_fe2o3_symbols::MAX_DEVICE_FFI_EFFECT_BYTES_V1 =>
        {
            "device FFI effects are too long".to_owned()
        }
        DeviceFfiGrammarError::InvalidEffects => {
            "device FFI effects must use unique, canonically sorted V1 effect names".to_owned()
        }
        DeviceFfiGrammarError::EffectAbiMismatch(effect) => format!(
            "device FFI effect `{}` has no compatible physical pointer argument",
            effect.as_str()
        ),
        _ => "device FFI contract uses unsupported V1 grammar".to_owned(),
    };
    syn::Error::new(proc_macro2::Span::call_site(), message)
}

fn validate_device_ffi_target(target: &str) -> syn::Result<()> {
    if target.len() > reserved_fe2o3_symbols::MAX_DEVICE_FFI_TARGET_BYTES_V1 {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "device FFI target is too long",
        ));
    }
    let mut parts = target.split(':');
    let processor = parts.next().unwrap_or_default();
    if !processor.starts_with("gfx")
        || processor.len() <= 3
        || !processor[3..]
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "device FFI target must be a canonical concrete `gfx...` target",
        ));
    }
    let mut previous = None;
    let mut names = std::collections::BTreeSet::new();
    for feature in parts {
        let valid = matches!(feature, "sramecc+" | "sramecc-" | "xnack+" | "xnack-");
        if !valid
            || previous.is_some_and(|previous: &str| previous >= feature)
            || !names.insert(&feature[..feature.len() - 1])
        {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "device FFI target features must be unique and canonically sorted",
            ));
        }
        previous = Some(feature);
    }
    Ok(())
}

fn validate_lower_hex_256(value: &str, field: &str) -> syn::Result<()> {
    if value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && value.bytes().any(|byte| byte != b'0')
    {
        Ok(())
    } else {
        Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("device FFI {field} must contain exactly 64 lowercase hexadecimal bytes"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        DeviceFfiOptions, GeneralTypedArgumentKindV1, GeneralTypedScalarV1, KernelMode,
        KernelOptions, canonical_device_ffi_signature, core_import_for, device_ffi_contract,
        device_import_for, device_path_for, expand_device_copy_with_core_import,
        expand_device_export_with_import, expand_device_import_with_import,
        expand_kernel_with_device_import, expand_kernel_with_imports,
        general_typed_global_mut_pointer_type_identity_v1, generated_general_typed_arguments_v1,
        generated_worker_v3_adapter_v1, host_import_for, model_general_typed_signature_v1,
        parse_device_ffi_options, parse_kernel_options, reconcile_crate_binding_v1,
        simulation_attempt_value_v1, simulation_mode_value_v1,
        validate_generated_device_ffi_contract_grammar, validate_kernel_assembly_boundary,
        validate_kernel_source_safety, validate_typed_kernel_profile_v1,
        validate_typed_kernel_signature, validate_typed_kernel_symbol_stem,
    };
    use fe2o3_artifacts::{
        AbiKind, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize, Mutability,
        PointerWidth, RustLayoutEvidenceV1, RustPhysicalComponentKindV1, RustPhysicalComponentV1,
        RustScalarElementTypeV1, RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1,
        ScalarType,
    };
    use proc_macro_crate::FoundCrate;
    use quote::{ToTokens, quote};
    use reserved_fe2o3_symbols::{
        CrateBindingIdV1, GeneratedHostContractIdV3, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
        TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3, derive_crate_binding_id_v1,
        derive_kernel_binding_id_v1, host_kernel_symbol_v1,
    };
    use syn::{Expr, FnArg, Item, ItemFn, ItemForeignMod, Type, Visibility, parse_quote};

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }

    fn ffi_options() -> DeviceFfiOptions {
        DeviceFfiOptions {
            symbol: "reviewed_helper".to_owned(),
            target: "gfx942".to_owned(),
            code_object: 5,
            effects: "read_global,write_global".to_owned(),
            semantic: "1111111111111111111111111111111111111111111111111111111111111111".to_owned(),
        }
    }

    #[test]
    fn compiler_and_explicit_crate_bindings_must_agree_exactly() {
        let compiler = CrateBindingIdV1::from_bytes([0x11; 32]);
        let explicit = CrateBindingIdV1::from_bytes([0x22; 32]);

        assert_eq!(
            reconcile_crate_binding_v1(Some(compiler), None).unwrap(),
            compiler
        );
        assert_eq!(
            reconcile_crate_binding_v1(None, Some(explicit)).unwrap(),
            explicit
        );
        assert_eq!(
            reconcile_crate_binding_v1(Some(compiler), Some(compiler)).unwrap(),
            compiler
        );
        let mismatch = reconcile_crate_binding_v1(Some(compiler), Some(explicit)).unwrap_err();
        let diagnostic = mismatch.to_string();
        assert!(diagnostic.contains("compiler-derived crate binding"));
        assert!(diagnostic.contains(&compiler.to_hex()));
        assert!(diagnostic.contains(&explicit.to_hex()));
        assert!(
            reconcile_crate_binding_v1(None, None)
                .unwrap_err()
                .to_string()
                .contains("requires the cargo-fe2o3 rustc wrapper")
        );
    }

    #[test]
    fn device_ffi_options_and_physical_abi_are_canonical() {
        let parsed = parse_device_ffi_options(quote::quote!(
            symbol = "reviewed_helper",
            target = "gfx942",
            code_object = 5,
            effects = "read_global,write_global",
            semantic = "1111111111111111111111111111111111111111111111111111111111111111"
        ))
        .unwrap();
        assert_eq!(parsed, ffi_options());

        let function: ItemFn = parse_quote!(
            pub unsafe extern "C" fn helper(
                input: DeviceGlobalConstPtr<f32>,
                output: DeviceGlobalMutPtr<f32>,
                count: u64,
            ) -> u32 {
                0
            }
        );
        assert_eq!(
            canonical_device_ffi_signature(&function.sig).unwrap(),
            "C(const_ptr<global,f32>[size=8,align=8,as=global],mut_ptr<global,f32>[size=8,align=8,as=global],u64[size=8,align=8])->u32[size=4,align=4]"
        );
    }

    #[test]
    fn macro_uses_the_shared_device_ffi_golden_corpus() {
        const CORPUS: &str =
            include_str!("../../reserved-fe2o3-symbols/tests/data/device_ffi_grammar_v1.tsv");
        for line in CORPUS.lines().filter(|line| !line.starts_with('#')) {
            let fields = line.split('\t').collect::<Vec<_>>();
            assert_eq!(fields.len(), 7, "malformed corpus row: {line}");
            let [name, expected, direction, symbol, abi, effects, golden] = fields.as_slice()
            else {
                unreachable!("field count was checked")
            };
            let direction = match reserved_fe2o3_symbols::parse_device_ffi_direction_v1(direction) {
                Ok(direction) => direction,
                Err(_) => {
                    assert_eq!(*expected, "direction", "{name}");
                    continue;
                }
            };
            let options = DeviceFfiOptions {
                symbol: (*symbol).to_owned(),
                target: "gfx942".to_owned(),
                code_object: 5,
                effects: (*effects).to_owned(),
                semantic: "1111111111111111111111111111111111111111111111111111111111111111"
                    .to_owned(),
            };
            match validate_generated_device_ffi_contract_grammar(&options, abi) {
                Ok(()) => {
                    assert_eq!(*expected, "ok", "{name}");
                    assert_eq!(
                        device_ffi_contract(direction.tag(), &options, abi).to_hex(),
                        *golden,
                        "{name}"
                    );
                }
                Err(error) => assert_eq!(
                    macro_grammar_error_class(&error.to_string()),
                    *expected,
                    "{name}: {error}"
                ),
            }
        }
    }

    #[test]
    fn shared_grammar_adapter_preserves_proc_macro_diagnostics() {
        let mut options = ffi_options();
        options.symbol = "bad|symbol".to_owned();
        assert_eq!(
            validate_generated_device_ffi_contract_grammar(&options, "C()->unit[size=0,align=1]")
                .unwrap_err()
                .to_string(),
            "device FFI symbol is empty, too long, or contains noncanonical bytes"
        );

        let mut options = ffi_options();
        options.effects = "write_global".to_owned();
        assert_eq!(
            validate_generated_device_ffi_contract_grammar(
                &options,
                "C(const_ptr<global,u32>[size=8,align=8,as=global])->unit[size=0,align=1]"
            )
            .unwrap_err()
            .to_string(),
            "device FFI effect `write_global` has no compatible physical pointer argument"
        );
    }

    fn macro_grammar_error_class(message: &str) -> &'static str {
        if message.contains("symbol is empty") {
            "symbol"
        } else if message.contains("physical ABI") {
            "physical_abi"
        } else if message.contains("effects must use") {
            "effects"
        } else if message.contains("effect `") {
            "effect_abi"
        } else {
            panic!("unexpected macro grammar diagnostic: {message}")
        }
    }

    #[test]
    fn device_ffi_expansions_bind_symbol_and_registration() {
        let device_import = device_import_for(FoundCrate::Name("device".to_owned()));
        let export: ItemFn = parse_quote!(
            pub unsafe extern "C" fn helper(value: u32) -> u32 {
                value
            }
        );
        let expanded = expand_device_export_with_import(
            export,
            DeviceFfiOptions {
                effects: "none".to_owned(),
                ..ffi_options()
            },
            &device_import,
        )
        .unwrap()
        .to_string();
        assert!(expanded.contains("export_name = \"reviewed_helper\""));
        assert!(expanded.contains("__fe2o3_device_ffi_registration_v1_"));
        assert!(expanded.contains("__fe2o3_device_ffi_v1|2|"));

        let import: ItemForeignMod = parse_quote!(
            unsafe extern "C" {
                pub fn helper(value: u32) -> u32;
            }
        );
        let expanded = expand_device_import_with_import(
            import,
            DeviceFfiOptions {
                effects: "none".to_owned(),
                ..ffi_options()
            },
            &device_import,
        )
        .unwrap()
        .to_string();
        assert!(expanded.contains("link_name = \"reviewed_helper\""));
        assert!(expanded.contains("__fe2o3_device_ffi_v1|1|"));
    }

    #[test]
    fn malformed_device_ffi_options_fail_closed() {
        for options in [
            quote::quote!(
                symbol = "bad|symbol",
                target = "gfx942",
                code_object = 5,
                effects = "none",
                semantic = "1111111111111111111111111111111111111111111111111111111111111111"
            ),
            quote::quote!(
                symbol = "valid",
                target = "gfx942:xnack-:sramecc+",
                code_object = 5,
                effects = "none",
                semantic = "1111111111111111111111111111111111111111111111111111111111111111"
            ),
            quote::quote!(
                symbol = "valid",
                target = "gfx942",
                code_object = 7,
                effects = "none",
                semantic = "1111111111111111111111111111111111111111111111111111111111111111"
            ),
            quote::quote!(
                symbol = "valid",
                target = "gfx942",
                code_object = 5,
                effects = "write_global,read_global",
                semantic = "1111111111111111111111111111111111111111111111111111111111111111"
            ),
            quote::quote!(
                symbol = "valid",
                target = "gfx942",
                code_object = 5,
                effects = "none",
                semantic = "ABC"
            ),
        ] {
            assert!(parse_device_ffi_options(options).is_err());
        }
    }

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
            parse_kernel_options(quote::quote!()).unwrap(),
            KernelOptions {
                mode: KernelMode::Basic,
                explicit_namespace: None,
                reference: None,
                launch: None,
                unsafe_assembly: None,
                control_flow: None,
            }
        );
        assert_eq!(
            parse_kernel_options(quote::quote!(typed)).unwrap(),
            KernelOptions {
                mode: KernelMode::Typed,
                explicit_namespace: None,
                reference: None,
                launch: None,
                unsafe_assembly: None,
                control_flow: None,
            }
        );
        let reference =
            parse_kernel_options(quote::quote!(typed, reference = crate::cpu_reference)).unwrap();
        assert_eq!(
            reference.reference.as_deref(),
            Some("crate :: cpu_reference"),
        );
        assert!(parse_kernel_options(quote::quote!(reference = cpu_reference)).is_err());
        assert!(parse_kernel_options(quote::quote!(typed, reference = |value| value)).is_err());
        assert!(parse_kernel_options(quote::quote!(qualification_worker_v2)).is_err());
        assert!(parse_kernel_options(quote::quote!(typed, qualification_worker_v2)).is_err());

        let explicit = parse_kernel_options(quote::quote!(
            typed,
            namespace = "0000000000000000000000000000000000000000000000000000000000000001"
        ))
        .unwrap();
        assert_eq!(explicit.mode, KernelMode::Typed);
        assert_eq!(
            explicit.explicit_namespace.unwrap().to_hex(),
            "0000000000000000000000000000000000000000000000000000000000000001"
        );

        for rejected in [quote::quote!(other), quote::quote!(typed = true)] {
            assert!(
                parse_kernel_options(rejected)
                    .unwrap_err()
                    .to_string()
                    .contains("#[kernel] accepts only")
            );
        }
        assert!(
            parse_kernel_options(quote::quote!(typed, namespace = "not-a-binding"))
                .unwrap_err()
                .to_string()
                .contains("64 lowercase hexadecimal")
        );
    }

    #[test]
    fn launch_and_unsafe_assembly_options_encode_canonically() {
        let options = parse_kernel_options(quote::quote!(
            launch(
                required = [256, 1, 1],
                max = [256, 1, 1],
                min_workgroups_per_compute_unit = 2
            ),
            unsafe_asm(
                target = "gfx942",
                operands(sgpr, immediate),
                options(nomem, pure, nostack),
                effects(none)
            )
        ))
        .unwrap();
        assert_eq!(options.mode, KernelMode::Basic);
        assert_eq!(options.launch.unwrap().required, Some([256, 1, 1]));
        assert_eq!(options.launch.unwrap().maximum, Some([256, 1, 1]));
        assert_eq!(
            options.launch.unwrap().min_workgroups_per_compute_unit,
            Some(2)
        );
        assert_eq!(options.unsafe_assembly.unwrap().target, 1);

        let bytes = super::encode_kernel_frontend_contract_v1(&options).unwrap();
        let hex = bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(
            hex,
            "4645324f334b46000100030040000000000000000700000000010000010000000100000000010000010000000100000002000000010005001500000000000000"
        );
    }

    #[test]
    fn launch_parser_rejects_malformed_duplicate_and_conflicting_bounds() {
        let rejected = [
            quote::quote!(launch()),
            quote::quote!(launch(required = [64, 1])),
            quote::quote!(launch(required = [0, 1, 1])),
            quote::quote!(launch(max = [1025, 1, 1])),
            quote::quote!(launch(required = [64, 1, 1], max_grid = [1, 1])),
            quote::quote!(launch(required = [64, 1, 1], max_grid = [0, 1, 1])),
            quote::quote!(launch(
                required = [64, 1, 1],
                max_grid = [1, 1, 1],
                max_grid = [1, 1, 1]
            )),
            quote::quote!(launch(required = [64, 2, 1], max = [32, 2, 1])),
            quote::quote!(launch(
                required = [64, 1, 1],
                min_workgroups_per_compute_unit = 2
            )),
            quote::quote!(launch(
                max = [64, 1, 1],
                min_workgroups_per_compute_unit = 0
            )),
            quote::quote!(launch(required = [64, 1, 1], required = [64, 1, 1])),
            quote::quote!(launch(max = [64, 1, 1]), launch(max = [64, 1, 1])),
        ];
        for attributes in rejected {
            assert!(parse_kernel_options(attributes).is_err());
        }
    }

    #[test]
    fn unsafe_assembly_parser_rejects_unknown_or_ambiguous_authority() {
        let rejected = [
            quote::quote!(unsafe_asm(
                target = "gfx1100",
                operands(sgpr),
                options(nomem),
                effects(none)
            )),
            quote::quote!(unsafe_asm(
                target = "gfx942",
                operands(memory),
                options(nomem),
                effects(none)
            )),
            quote::quote!(unsafe_asm(
                target = "gfx942",
                operands(sgpr),
                options(may_unwind),
                effects(none)
            )),
            quote::quote!(unsafe_asm(
                target = "gfx942",
                operands(sgpr, sgpr),
                options(nomem),
                effects(none)
            )),
            quote::quote!(unsafe_asm(
                target = "gfx942",
                operands(sgpr),
                options(nomem, readonly),
                effects(none)
            )),
            quote::quote!(unsafe_asm(
                target = "gfx942",
                operands(address),
                options(readonly),
                effects(write_global)
            )),
            quote::quote!(unsafe_asm(
                target = "gfx942",
                operands(sgpr),
                options(nomem),
                effects(none, barrier)
            )),
        ];
        for attributes in rejected {
            assert!(parse_kernel_options(attributes).is_err());
        }
    }

    #[test]
    fn assembly_use_requires_an_explicit_unsafe_kernel_boundary() {
        let direct_asm: ItemFn = parse_quote! {
            fn kernel() { unsafe { core::arch::asm!("nop") } }
        };
        assert!(
            validate_kernel_assembly_boundary(&direct_asm, None)
                .unwrap_err()
                .to_string()
                .contains("requires an explicit unsafe_asm")
        );

        let declaration = parse_kernel_options(quote::quote!(unsafe_asm(
            target = "gfx942",
            operands(sgpr),
            options(nomem),
            effects(none)
        )))
        .unwrap()
        .unsafe_assembly;
        assert!(
            validate_kernel_assembly_boundary(&direct_asm, declaration)
                .unwrap_err()
                .to_string()
                .contains("requires an unsafe kernel function")
        );
        let unsafe_direct_asm: ItemFn = parse_quote! {
            unsafe fn kernel() { unsafe { core::arch::asm!("nop") } }
        };
        validate_kernel_assembly_boundary(&unsafe_direct_asm, declaration).unwrap();

        let unsupported: ItemFn = parse_quote! {
            unsafe fn kernel() { core::arch::global_asm!("nop"); }
        };
        assert!(
            validate_kernel_assembly_boundary(&unsupported, declaration)
                .unwrap_err()
                .to_string()
                .contains("global_asm!")
        );
    }

    #[test]
    fn ordinary_kernel_source_requires_safe_rust() {
        let safe: ItemFn = parse_quote! {
            fn kernel(value: u32) -> u32 { value + 1 }
        };
        validate_kernel_source_safety(&safe, false).unwrap();

        let rejected: Vec<(ItemFn, &str)> = vec![
            (
                parse_quote!(
                    unsafe fn kernel() {}
                ),
                "ordinary #[kernel] functions must be safe",
            ),
            (
                parse_quote!(
                    fn kernel() {
                        unsafe {}
                    }
                ),
                "unsafe blocks are not allowed in ordinary #[kernel] bodies",
            ),
            (
                parse_quote!(
                    fn kernel() {
                        unsafe fn helper() {}
                    }
                ),
                "unsafe function declarations are not allowed inside ordinary #[kernel] bodies",
            ),
            (
                parse_quote!(
                    fn kernel(callback: unsafe fn()) {
                        let _ = callback;
                    }
                ),
                "unsafe function pointer types are not allowed in ordinary #[kernel] signatures",
            ),
        ];
        for (input, expected) in rejected {
            assert!(
                validate_kernel_source_safety(&input, false)
                    .unwrap_err()
                    .to_string()
                    .contains(expected)
            );
        }
    }

    #[test]
    fn declared_unsafe_assembly_keeps_a_separate_low_level_boundary() {
        let input: ItemFn = parse_quote! {
            unsafe fn kernel() { unsafe { core::arch::asm!("nop") } }
        };
        validate_kernel_source_safety(&input, true).unwrap();
    }

    #[test]
    fn attributed_kernel_emits_a_separate_bound_sidecar() {
        let input = parse_quote! {
            pub unsafe fn bounded(value: u32) -> u32 { value }
        };
        let options = parse_kernel_options(quote::quote!(
            launch(max = [256, 1, 1], min_workgroups_per_compute_unit = 2),
            unsafe_asm(
                target = "gfx942",
                operands(sgpr),
                options(nomem),
                effects(none)
            )
        ))
        .unwrap();
        let device_import = device_import_for(FoundCrate::Name("gpu_device".to_string()));
        let expansion =
            expand_kernel_with_imports(input, options, &device_import, None, None, None)
                .unwrap()
                .to_string();
        assert!(expansion.contains("static __fe2o3_kernel_registration_bounded"));
        assert!(expansion.contains("static __fe2o3_kernel_frontend_contract_v1_bounded"));
        assert!(expansion.contains(&format!(
            "{}u64",
            super::KERNEL_FRONTEND_REGISTRATION_MAGIC_V1
        )));
        assert!(expansion.contains("1u16 , 1u16"));

        let legacy: ItemFn = parse_quote! { pub fn legacy() {} };
        let legacy = expand_kernel_with_device_import(legacy, &device_import)
            .unwrap()
            .to_string();
        assert!(!legacy.contains("__fe2o3_kernel_frontend_contract_v1_legacy"));
    }

    #[test]
    fn simulation_mode_selector_is_closed_to_exact_one() {
        assert!(simulation_mode_value_v1("1"));
        for hostile in ["", "0", "true", "TRUE", "01", "attacker", "1 "] {
            assert!(!simulation_mode_value_v1(hostile), "accepted {hostile:?}");
        }
        assert!(simulation_attempt_value_v1(
            "0123456789abcdef0123456789abcdef"
        ));
        for hostile in [
            "",
            "0",
            "00000000000000000000000000000000",
            "0123456789ABCDEF0123456789abcdef",
            "0123456789abcdef0123456789abcdeg",
            "0123456789abcdef0123456789abcdef00",
        ] {
            assert!(
                !simulation_attempt_value_v1(hostile),
                "accepted {hostile:?}"
            );
        }
    }

    #[test]
    fn typed_simulation_expansion_retains_device_contracts_without_host_adapters() {
        let device_import = device_import_for(FoundCrate::Name("gpu_device".to_owned()));
        let device_path = device_path_for(FoundCrate::Name("gpu_device".to_owned()));
        let crate_binding = derive_crate_binding_id_v1("fixture", ["simulation-device-only"]);
        let options = parse_kernel_options(quote!(typed)).unwrap();

        let general: ItemFn = parse_quote! {
            pub fn fill(mut output: DisjointSlice<u32>) {
                let _ = &mut output;
            }
        };
        let general = expand_kernel_with_imports(
            general,
            options.clone(),
            &device_import,
            Some(&device_path),
            None,
            Some(crate_binding),
        )
        .unwrap()
        .to_string();
        assert!(general.contains("static __fe2o3_kernel_registration_fill"));
        assert!(general.contains("3u16 , 4u16"));
        assert!(general.contains(TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3));
        assert!(general.contains("KernelMarkerV1 for __fe2o3_kernel_marker_fill"));
        assert!(general.contains("unsafe impl :: gpu_device :: KernelMarkerV1"));
        assert!(!general.contains("extern crate gpu_device as __fe2o3_kernel_device"));
        assert!(general.contains("fn __fe2o3_host_kernel_v1_"));
        assert!(!general.contains("pub mod fill_gpu"));
        assert!(!general.contains("__fe2o3_kernel_host"));
        assert!(!general.contains("semantic_witness_from_backend_v1"));

        let vecadd: ItemFn = parse_quote! {
            pub fn vecadd(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
                let _ = (a, b, &mut c);
            }
        };
        let vecadd = expand_kernel_with_imports(
            vecadd,
            options,
            &device_import,
            Some(&device_path),
            None,
            Some(crate_binding),
        )
        .unwrap()
        .to_string();
        assert!(vecadd.contains("static __fe2o3_kernel_registration_vecadd"));
        assert!(vecadd.contains("3u16 , 4u16"));
        assert!(vecadd.contains(TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3));
        assert!(vecadd.contains("unsafe impl :: gpu_device :: KernelMarkerV1"));
        assert!(!vecadd.contains("extern crate gpu_device as __fe2o3_kernel_device"));
        assert!(vecadd.contains("fn __fe2o3_host_kernel_v1_"));
        assert!(!vecadd.contains("pub mod vecadd_gpu"));
        assert!(!vecadd.contains("__fe2o3_kernel_host"));
        assert!(!vecadd.contains("artifact_bytes_from_backend_v1"));
    }

    #[test]
    fn exact_vecadd_uses_the_single_worker_v3_host_contract() {
        let input: ItemFn = parse_quote! {
            pub fn vecadd(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
                let _ = (a, b, &mut c);
            }
        };
        let device_import = device_import_for(FoundCrate::Name("gpu_device".to_string()));
        let host_import = host_import_for(FoundCrate::Name("gpu_host".to_string()));
        let crate_binding = derive_crate_binding_id_v1("fixture", ["worker-v3-token-golden"]);

        let expansion = expand_kernel_with_imports(
            input,
            parse_kernel_options(quote!(typed)).unwrap(),
            &device_import,
            None,
            Some(&host_import),
            Some(crate_binding),
        )
        .unwrap()
        .to_string();

        assert!(expansion.contains("CompilerGeneratedWorkerV3ArgumentsV1"));
        assert!(expansion.contains("prepare_worker_v3"));
        assert!(expansion.contains(TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3));
        assert!(expansion.contains("pub struct Arguments"));
        for retired in [
            "CompilerGeneratedKernelContractV1",
            "artifact_container_bytes",
            "GeneratedVecAddKernelV1",
            "GeneratedVecAddPreparedV1",
            "qualification_worker_v2",
        ] {
            assert!(!expansion.contains(retired), "retained `{retired}`");
        }
    }

    #[test]
    fn general_typed_host_arguments_exactly_follow_the_validated_signature() {
        let mixed: ItemFn = parse_quote! {
            pub fn mixed(
                scale: f64,
                input: &[u16],
                output: DisjointSlice<i32, Index1D>,
            ) {}
        };
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let model = model_general_typed_signature_v1(&mixed, &options, [0x31; 32]).unwrap();
        let generated = generated_general_typed_arguments_v1(&mixed, &model.arguments);
        let generated_text = generated.to_string();
        assert!(!generated_text.contains("* const"));
        assert!(!generated_text.contains("* mut"));
        assert!(!generated_text.contains("from_raw"));
        let file: syn::File = syn::parse2(generated).unwrap();
        let arguments = file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Struct(item) if item.ident == "Arguments" => Some(item),
                _ => None,
            })
            .expect("generated Arguments struct");
        assert!(matches!(arguments.vis, Visibility::Public(_)));
        assert_eq!(arguments.generics.params.len(), 1);
        let fields = arguments.fields.iter().collect::<Vec<_>>();
        assert_eq!(fields.len(), 3);
        assert!(
            fields
                .iter()
                .all(|field| matches!(field.vis, Visibility::Inherited))
        );
        assert_eq!(fields[0].ident.as_ref().unwrap(), "scale");
        assert_eq!(fields[0].ty.to_token_stream().to_string(), "f64");
        assert_eq!(fields[1].ident.as_ref().unwrap(), "input");
        assert_eq!(
            fields[1].ty.to_token_stream().to_string(),
            "__fe2o3_kernel_host :: __generated :: GeneratedReadDeviceSlice < 'allocation , u16 >"
        );
        assert_eq!(fields[2].ident.as_ref().unwrap(), "output");
        assert_eq!(
            fields[2].ty.to_token_stream().to_string(),
            "__fe2o3_kernel_host :: __generated :: GeneratedReadWriteDeviceSlice < 'allocation , i32 >"
        );

        let constructor = file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Impl(item) => item.items.iter().find_map(|item| match item {
                    syn::ImplItem::Fn(function) if function.sig.ident == "new" => Some(function),
                    _ => None,
                }),
                _ => None,
            })
            .expect("generated Arguments::new constructor");
        assert!(matches!(constructor.vis, Visibility::Public(_)));
        let constructor_types = constructor
            .sig
            .inputs
            .iter()
            .filter_map(|argument| match argument {
                FnArg::Typed(argument) => Some(argument.ty.to_token_stream().to_string()),
                FnArg::Receiver(_) => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            constructor_types,
            fields
                .iter()
                .map(|field| field.ty.to_token_stream().to_string())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            file.items
                .iter()
                .filter_map(|item| match item {
                    Item::Impl(item) => Some(&item.items),
                    _ => None,
                })
                .flatten()
                .filter(|item| matches!(item, syn::ImplItem::Fn(_)))
                .count(),
            1
        );

        let scalars: ItemFn = parse_quote! {
            pub fn scalars(
                i8_value: i8,
                u8_value: u8,
                i16_value: i16,
                u16_value: u16,
                i32_value: i32,
                u32_value: u32,
                i64_value: i64,
                u64_value: u64,
                f32_value: f32,
                f64_value: f64,
            ) {}
        };
        let model = model_general_typed_signature_v1(&scalars, &options, [0x32; 32]).unwrap();
        let generated = generated_general_typed_arguments_v1(&scalars, &model.arguments);
        let file: syn::File = syn::parse2(generated).unwrap();
        let arguments = file
            .items
            .iter()
            .find_map(|item| match item {
                Item::Struct(item) if item.ident == "Arguments" => Some(item),
                _ => None,
            })
            .expect("scalar-only Arguments struct");
        assert!(arguments.generics.params.is_empty());
        assert_eq!(
            arguments
                .fields
                .iter()
                .map(|field| field.ty.to_token_stream().to_string())
                .collect::<Vec<_>>(),
            [
                "i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64", "f32", "f64"
            ]
        );
    }

    #[test]
    fn canonical_general_arguments_generate_typed_worker_v3_dispatch() {
        let input: ItemFn = parse_quote! {
            pub fn mixed(
                scale: f64,
                input: &[u16],
                output: DisjointSlice<i32, Index1D>,
            ) {}
        };
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let model = model_general_typed_signature_v1(&input, &options, [0x35; 32]).unwrap();
        let generated = generated_worker_v3_adapter_v1(&input, &model).to_string();

        for expected in [
            "CompilerGeneratedWorkerV3ArgumentsV1",
            "prepare_worker_v3",
            "plan . scalar (0usize , self . scale)",
            "self . input . bind_argument_pair (plan , 1usize)",
            "self . output . bind_argument_pair (plan , 2usize)",
            "GeneratedWorkerV3ArgumentBindingV1",
            "ScalarType :: F64",
            "element_size : 2u64",
            "element_size : 4u64",
        ] {
            assert!(
                generated.contains(expected),
                "missing `{expected}` in {generated}"
            );
        }
        assert!(!generated.contains("from_raw"));
        assert!(!generated.contains("* const"));
        assert!(!generated.contains("* mut"));
    }

    #[test]
    fn worker_v3_dispatch_generation_fails_closed_for_unrepresented_descriptor_kinds() {
        let options = parse_kernel_options(quote!(typed)).unwrap();
        for input in [
            parse_quote! {
                pub fn shifted(output: DisjointSlice<f32, Shifted<Index1D, 7>>) {}
            },
            parse_quote! {
                pub fn global(target: fe2o3_device::DeviceGlobalMutPtr<u32>) {}
            },
        ] {
            let model = model_general_typed_signature_v1(&input, &options, [0x36; 32]).unwrap();
            assert!(generated_worker_v3_adapter_v1(&input, &model).is_empty());
        }
    }

    #[test]
    fn scalar_only_worker_v3_arguments_use_a_static_allocation_lifetime() {
        let input: ItemFn = parse_quote! {
            pub fn scalar(value: u32) {}
        };
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let model = model_general_typed_signature_v1(&input, &options, [0x37; 32]).unwrap();
        let generated = generated_worker_v3_adapter_v1(&input, &model).to_string();

        assert!(generated.contains("for Arguments"));
        assert!(generated.contains("'static , Marker , A , Self"));
        assert!(!generated.contains("for Arguments < 'allocation >"));
    }

    #[test]
    fn general_typed_contract_retains_disjoint_mapping_brands() {
        let identity: ItemFn = parse_quote! {
            pub fn identity(output: DisjointSlice<f32, Index1D>) {}
        };
        let shifted: ItemFn = parse_quote! {
            pub fn shifted(output: DisjointSlice<f32, Shifted<Index1D, 7>>) {}
        };
        let grid_exclusive: ItemFn = parse_quote! {
            pub fn grid_exclusive(output: DisjointSlice<f32, GridExclusive>) {}
        };
        let blocked: ItemFn = parse_quote! {
            pub fn blocked(output: DisjointSlice<f32, Blocked<Index1D, 16, 4>>) {}
        };
        let blocked_other: ItemFn = parse_quote! {
            pub fn blocked_other(output: DisjointSlice<f32, Blocked<Index1D, 16, 2>>) {}
        };
        let tiled: ItemFn = parse_quote! {
            pub fn tiled(output: DisjointSlice<f32, Tiled2D<Index1D, 64, 16, 16, 4>>) {}
        };
        let tiled_other: ItemFn = parse_quote! {
            pub fn tiled_other(output: DisjointSlice<f32, Tiled2D<Index1D, 32, 8, 16, 4>>) {}
        };
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let identity = model_general_typed_signature_v1(&identity, &options, [0x71; 32])
            .unwrap()
            .abi
            .fields()[0]
            .type_identity();
        let shifted = model_general_typed_signature_v1(&shifted, &options, [0x72; 32])
            .unwrap()
            .abi
            .fields()[0]
            .type_identity();
        let grid_exclusive =
            model_general_typed_signature_v1(&grid_exclusive, &options, [0x73; 32])
                .unwrap()
                .abi
                .fields()[0]
                .type_identity();
        let blocked = model_general_typed_signature_v1(&blocked, &options, [0x74; 32])
            .unwrap()
            .abi
            .fields()[0]
            .type_identity();
        let blocked_other = model_general_typed_signature_v1(&blocked_other, &options, [0x75; 32])
            .unwrap()
            .abi
            .fields()[0]
            .type_identity();
        let tiled = model_general_typed_signature_v1(&tiled, &options, [0x77; 32])
            .unwrap()
            .abi
            .fields()[0]
            .type_identity();
        let tiled_other = model_general_typed_signature_v1(&tiled_other, &options, [0x78; 32])
            .unwrap()
            .abi
            .fields()[0]
            .type_identity();

        assert_ne!(identity, shifted);
        assert_ne!(identity, grid_exclusive);
        assert_ne!(shifted, grid_exclusive);
        assert_ne!(identity, blocked);
        assert_ne!(shifted, blocked);
        assert_ne!(grid_exclusive, blocked);
        assert_ne!(blocked, blocked_other);
        assert_ne!(blocked, tiled);
        assert_ne!(tiled, tiled_other);

        for invalid in [
            parse_quote! {
                pub fn zero_lanes(output: DisjointSlice<f32, Blocked<Index1D, 0, 4>>) {}
            },
            parse_quote! {
                pub fn zero_elements(output: DisjointSlice<f32, Blocked<Index1D, 16, 0>>) {}
            },
            parse_quote! {
                pub fn wrong_base(output: DisjointSlice<f32, Blocked<GridExclusive, 16, 4>>) {}
            },
            parse_quote! {
                pub fn zero_tile(output: DisjointSlice<f32, Tiled2D<Index1D, 64, 0, 16, 4>>) {}
            },
            parse_quote! {
                pub fn incomplete_tile(output: DisjointSlice<f32, Tiled2D<Index1D, 64, 8, 16, 4>>) {}
            },
            parse_quote! {
                pub fn wrong_tile_base(
                    output: DisjointSlice<f32, Tiled2D<GridExclusive, 64, 16, 16, 4>>,
                ) {}
            },
        ] {
            assert!(model_general_typed_signature_v1(&invalid, &options, [0x76; 32]).is_err());
        }
    }

    #[test]
    fn general_typed_kernels_emit_exact_v3_expectation_only_registrations() {
        let alpha: ItemFn = parse_quote! {
            pub fn alpha(scale: f32, input: &[f32], output: DisjointSlice<f32>) {
                let _ = (scale, input, output);
            }
        };
        let zeta: ItemFn = parse_quote! {
            pub fn zeta(
                a: &[f32],
                b: &[f32],
                bias: f32,
                output: DisjointSlice<f32>,
            ) {
                let _ = (a, b, bias, output);
            }
        };
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let device_import = device_import_for(FoundCrate::Name("gpu_device".to_string()));
        let host_import = host_import_for(FoundCrate::Name("gpu_host".to_string()));
        let crate_binding = derive_crate_binding_id_v1("fixture", ["general-v3-golden"]);

        let expand = |input: ItemFn, crate_binding| {
            let name = input.sig.ident.to_string();
            let binding = derive_kernel_binding_id_v1(
                crate_binding,
                MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                &name,
                &name,
            );
            let model =
                model_general_typed_signature_v1(&input, &options, binding.as_bytes()).unwrap();
            let contract = GeneratedHostContractIdV3::from_bytes(
                *model.generated_host_contract_identity.as_bytes(),
            );
            let expansion = expand_kernel_with_imports(
                input,
                options.clone(),
                &device_import,
                None,
                Some(&host_import),
                Some(crate_binding),
            )
            .unwrap();
            (expansion, binding, contract)
        };

        let alpha_model = model_general_typed_signature_v1(
            &alpha,
            &options,
            derive_kernel_binding_id_v1(
                crate_binding,
                MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                "alpha",
                "alpha",
            )
            .as_bytes(),
        )
        .unwrap();
        let zeta_model = model_general_typed_signature_v1(
            &zeta,
            &options,
            derive_kernel_binding_id_v1(
                crate_binding,
                MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
                "zeta",
                "zeta",
            )
            .as_bytes(),
        )
        .unwrap();
        assert_eq!(
            alpha_model
                .abi
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["scale", "input", "output"]
        );
        assert_eq!(
            zeta_model
                .abi
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "bias", "output"]
        );

        let (alpha_expansion, alpha_binding, alpha_contract) = expand(alpha.clone(), crate_binding);
        let (zeta_expansion, zeta_binding, zeta_contract) = expand(zeta, crate_binding);
        for (name, expansion, binding, contract) in [
            ("alpha", &alpha_expansion, alpha_binding, alpha_contract),
            ("zeta", &zeta_expansion, zeta_binding, zeta_contract),
        ] {
            let file: syn::File = syn::parse2(expansion.clone()).unwrap();
            let registration_name = format!("__fe2o3_kernel_registration_{name}");
            let registration = file
                .items
                .iter()
                .find_map(|item| match item {
                    Item::Static(item) if item.ident == registration_name.as_str() => Some(item),
                    _ => None,
                })
                .expect("general typed registration static");
            let Type::Tuple(registration_type) = registration.ty.as_ref() else {
                panic!("V3 registration type must be a tuple");
            };
            let Expr::Tuple(registration_value) = registration.expr.as_ref() else {
                panic!("V3 registration value must be a tuple");
            };
            assert_eq!(registration_type.elems.len(), 10);
            assert_eq!(registration_value.elems.len(), 10);
            assert_eq!(
                registration_value.elems[1].to_token_stream().to_string(),
                "3u16"
            );
            assert_eq!(
                registration_value.elems[2].to_token_stream().to_string(),
                "4u16"
            );
            assert_eq!(
                registration_value.elems[7].to_token_stream().to_string(),
                format!("\"{TYPED_GENERAL_RUSTC_LAYOUT_PROFILE_TAG_V3}\"")
            );
            assert_eq!(
                registration_value.elems[8].to_token_stream().to_string(),
                format!("\"{}\"", contract.to_hex())
            );
            assert_eq!(
                registration_value.elems[9].to_token_stream().to_string(),
                format!("__fe2o3_host_kernel_v1_{}", binding.to_hex())
            );

            let expansion = expansion.to_string();
            assert!(expansion.contains(&format!("pub mod {name}_gpu")));
            assert!(expansion.contains("extern crate alloc as __fe2o3_kernel_alloc"));
            assert!(expansion.contains("use __fe2o3_kernel_alloc :: vec"));
            assert!(expansion.contains(&format!(
                "pub type Marker = super :: __fe2o3_kernel_marker_{name}"
            )));
            assert!(expansion.contains("pub struct Arguments < 'allocation >"));
            assert!(expansion.contains("pub fn new"));
            assert!(expansion.contains("CompilerGeneratedKernelExpectationV1"));
            assert!(expansion.contains("CompilerGeneratedKernelProfileV1 :: new"));
            assert!(expansion.contains(&binding.to_hex()));
            assert!(expansion.contains(&contract.to_hex()));
            assert!(expansion.contains(&host_kernel_symbol_v1(binding)));
            assert!(!expansion.contains("__fe2o3_semantic_witness_v1_"));
            assert!(!expansion.contains("fn semantic_witness_v1"));
            assert!(!expansion.contains("semantic_witness_from_backend_v1"));
            assert!(expansion.contains("CompilerGeneratedWorkerV3ArgumentsV1"));
            assert!(expansion.contains("pub fn prepare_worker_v3"));
            assert!(expansion.contains("prepare_generated_worker_v3_v1"));
            assert!(!expansion.contains("CompilerGeneratedAlphaZetaCov6ArgumentsV1"));
            assert!(!expansion.contains("AlphaZetaCov6DispatchIdentityV1 :: new"));
            assert!(!expansion.contains("KernelId :: from_bytes"));
            assert!(expansion.contains("CompilerGeneratedArgumentLayoutV1 :: new"));
            assert!(expansion.contains("from_compiler_generated_parts_v1"));
            assert!(!expansion.contains("prepare_generated_alpha_zeta_cov6_selected_kernel_v1"));
            assert!(!expansion.contains("GeneratedAlphaZetaCov6PrepareResultV1"));
            assert!(!expansion.contains("plan . slice"));
            assert!(!expansion.contains("from_raw"));
            assert!(!expansion.contains("CompilerGeneratedKernelContractV1"));
            assert!(!expansion.contains("artifact_container_bytes"));
            assert!(!expansion.contains("__fe2o3_artifact_v1_"));
            assert!(!expansion.contains("GeneratedVecAdd"));
            assert!(!expansion.contains("pub type Kernel"));
            assert!(!expansion.contains("pub type Prepared"));
            assert_eq!(contract.to_hex().len(), 64);
            assert!(
                contract
                    .to_hex()
                    .bytes()
                    .all(|byte| { byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte) })
            );
        }

        assert_ne!(alpha_binding, zeta_binding);
        assert_ne!(alpha_contract, zeta_contract);

        let changed_signature: ItemFn = parse_quote! {
            pub fn alpha(scale: f64, input: &[f32], output: DisjointSlice<f32>) {}
        };
        let (changed_signature_expansion, changed_signature_binding, changed_signature_contract) =
            expand(changed_signature, crate_binding);
        assert_eq!(alpha_binding, changed_signature_binding);
        assert_ne!(alpha_contract, changed_signature_contract);
        let changed_signature_expansion = changed_signature_expansion.to_string();
        assert!(!changed_signature_expansion.contains("CompilerGeneratedAlphaZetaCov6ArgumentsV1"));
        assert!(changed_signature_expansion.contains("CompilerGeneratedWorkerV3ArgumentsV1"));
        assert!(changed_signature_expansion.contains("pub fn prepare_worker_v3"));
        assert!(!changed_signature_expansion.contains("prepare_generated_alpha_zeta_cov6"));

        let renamed: ItemFn = parse_quote! {
            pub fn renamed(scale: f32, input: &[f32], output: DisjointSlice<f32>) {}
        };
        let (renamed_expansion, renamed_binding, renamed_contract) = expand(renamed, crate_binding);
        assert_ne!(alpha_binding, renamed_binding);
        assert_ne!(alpha_contract, renamed_contract);
        let renamed_expansion = renamed_expansion.to_string();
        assert!(!renamed_expansion.contains("CompilerGeneratedAlphaZetaCov6ArgumentsV1"));
        assert!(renamed_expansion.contains("CompilerGeneratedWorkerV3ArgumentsV1"));
        assert!(renamed_expansion.contains("pub fn prepare_worker_v3"));
        assert!(!renamed_expansion.contains("prepare_generated_alpha_zeta_cov6"));

        let other_crate_binding = derive_crate_binding_id_v1("fixture", ["other-binding"]);
        let (_, rebound_binding, rebound_contract) = expand(alpha, other_crate_binding);
        assert_ne!(alpha_binding, rebound_binding);
        assert_ne!(alpha_contract, rebound_contract);
    }

    #[test]
    fn general_typed_kernel_result_uses_a_unit_abi_wrapper() {
        let input: ItemFn = parse_quote! {
            pub fn checked(
                input: &[f32],
                output: DisjointSlice<f32>,
            ) -> KernelResult {
                let _ = input.first().ok_or(KernelError::OutOfBounds)?;
                let _ = output;
                Ok(())
            }
        };
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let device_import = device_import_for(FoundCrate::Name("gpu_device".to_string()));
        let device_path = device_path_for(FoundCrate::Name("gpu_device".to_string()));
        let host_import = host_import_for(FoundCrate::Name("gpu_host".to_string()));
        let crate_binding = derive_crate_binding_id_v1("fixture", ["kernel-result"]);
        let binding = derive_kernel_binding_id_v1(
            crate_binding,
            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            "checked",
            "checked",
        );

        let expansion = expand_kernel_with_imports(
            input,
            options,
            &device_import,
            Some(&device_path),
            Some(&host_import),
            Some(crate_binding),
        )
        .unwrap()
        .to_string();

        assert!(expansion.contains(&format!("fn __fe2o3_kernel_body_v1_{}", binding.to_hex())));
        assert!(expansion.contains("inline (always)"));
        assert!(expansion.contains("-> KernelResult"));
        assert!(expansion.contains(&format!("fn __fe2o3_host_kernel_v1_{}", binding.to_hex())));
        assert!(expansion.contains("let _ : :: gpu_device :: KernelResult"));
        assert!(!expansion.contains("fn (& [f32] , DisjointSlice < f32 >) -> KernelResult"));
    }

    #[test]
    fn scalar_gemm_v1_host_contract_matches_the_normative_rustc_identity() {
        let input: ItemFn = parse_quote! {
            pub fn scalar_gemm_v1(
                a: &[f32],
                b: &[f32],
                c: DisjointSlice<f32>,
                m: u32,
                n: u32,
                k: u32,
            ) {}
        };
        let options = parse_kernel_options(quote!(
            typed,
            launch(required = [256, 1, 1], max = [256, 1, 1])
        ))
        .unwrap();
        let crate_binding = CrateBindingIdV1::from_hex(
            "53bf3c83481a081d4ab0e2b32039f9c89be5de3937a84aca0c40800c8d6b0413",
        )
        .unwrap();
        let kernel_binding = derive_kernel_binding_id_v1(
            crate_binding,
            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            "scalar_gemm_v1",
            "scalar_gemm_v1",
        );
        let model =
            model_general_typed_signature_v1(&input, &options, kernel_binding.as_bytes()).unwrap();

        assert_eq!(model.abi.size(), 64);
        assert_eq!(model.abi.alignment(), 8);
        assert_eq!(
            model
                .abi
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c", "m", "n", "k"]
        );
        assert_eq!(
            model
                .abi
                .fields()
                .iter()
                .map(|field| field.offset())
                .collect::<Vec<_>>(),
            [0, 16, 32, 48, 52, 56]
        );
        assert_eq!(
            hex(model.generated_host_contract_identity.as_bytes()),
            "55108b826b645a8f8cb648c046504786a7034df7bdddc0fbcb5f7698d2205ca0"
        );

        let lookalike: ItemFn = parse_quote! {
            pub fn scalar_gemm_lookalike(
                a: &[f32],
                b: &[f32],
                c: DisjointSlice<f32>,
                m: u32,
                n: u32,
                k: u32,
            ) {}
        };
        let lookalike_binding = derive_kernel_binding_id_v1(
            crate_binding,
            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            "scalar_gemm_lookalike",
            "scalar_gemm_lookalike",
        );
        let lookalike =
            model_general_typed_signature_v1(&lookalike, &options, lookalike_binding.as_bytes())
                .unwrap();
        assert_eq!(
            lookalike
                .abi
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2", "arg3", "arg4", "arg5"]
        );
    }

    #[test]
    fn scalar_gemm_v1_emits_only_the_generic_worker_v3_adapter() {
        let input: ItemFn = parse_quote! {
            pub fn scalar_gemm_v1(
                a: &[f32],
                b: &[f32],
                c: DisjointSlice<f32>,
                m: u32,
                n: u32,
                k: u32,
            ) {}
        };
        let options = parse_kernel_options(quote!(
            typed,
            launch(required = [256, 1, 1], max = [256, 1, 1])
        ))
        .unwrap();
        let crate_binding = CrateBindingIdV1::from_hex(
            "53bf3c83481a081d4ab0e2b32039f9c89be5de3937a84aca0c40800c8d6b0413",
        )
        .unwrap();
        let expansion = expand_kernel_with_imports(
            input,
            options,
            &device_import_for(FoundCrate::Name("gpu_device".to_owned())),
            None,
            Some(&host_import_for(FoundCrate::Name("gpu_host".to_owned()))),
            Some(crate_binding),
        )
        .unwrap()
        .to_string();
        assert!(expansion.contains("CompilerGeneratedWorkerV3ArgumentsV1"));
        assert!(expansion.contains("prepare_worker_v3"));
        assert!(expansion.contains("GeneratedReadDeviceSlice"));
        assert!(expansion.contains("GeneratedReadWriteDeviceSlice"));
        for retired in [
            "GeneratedScalarGemmV1ReadDeviceSlice",
            "GeneratedScalarGemmV1ReadWriteDeviceSlice",
            "CompilerGeneratedScalarGemmV1Arguments",
            "prepare_generated_scalar_gemm_v1",
            "WorkerV2PrerequisiteAuthenticatorV1",
        ] {
            assert!(!expansion.contains(retired), "retained `{retired}`");
        }
    }

    #[test]
    fn typed_kernel_symbol_stem_matches_the_backend_contract() {
        let valid = parse_quote!(vecadd);
        validate_typed_kernel_symbol_stem(&valid).unwrap();

        let raw: syn::Ident = parse_quote!(r#type);
        let unicode = syn::Ident::new("v\u{e9}cadd", proc_macro2::Span::call_site());
        let long = syn::Ident::new(&"a".repeat(129), proc_macro2::Span::call_site());
        let expected = "#[kernel(typed)] kernel name must be 1 to 128 ASCII identifier bytes for backend artifact symbols";

        for rejected in [&raw, &unicode, &long] {
            assert_eq!(
                validate_typed_kernel_symbol_stem(rejected)
                    .unwrap_err()
                    .to_string(),
                expected,
            );
        }
    }

    #[test]
    fn basic_kernel_keeps_accepting_non_ascii_identifiers() {
        let mut input: ItemFn = parse_quote! {
            pub fn placeholder() {}
        };
        input.sig.ident = syn::Ident::new("v\u{e9}cadd", input.sig.ident.span());
        let device_import = device_import_for(FoundCrate::Name("gpu_device".to_string()));

        let expansion = expand_kernel_with_device_import(input, &device_import)
            .unwrap()
            .to_string();

        assert!(expansion.contains("fe2o3_kernel_v\u{e9}cadd"));
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
    fn rejected_general_fallback_preserves_exact_vecadd_diagnostics() {
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let cases: Vec<ItemFn> = vec![
            parse_quote! {
                pub fn alias(a: &Floats, b: &[f32], c: DisjointSlice<f32>) {}
            },
            parse_quote! {
                pub fn raw(a: *const f32, b: &[f32], c: DisjointSlice<f32>) {}
            },
            parse_quote! {
                pub fn unsupported_second(
                    a: &[f32],
                    b: *const f32,
                    c: DisjointSlice<f32>,
                ) {}
            },
            parse_quote! {
                pub fn unsupported_third(a: &[f32], b: &[f32], c: *mut f32) {}
            },
            parse_quote! {
                pub fn empty() {}
            },
        ];

        for input in cases {
            let exact = validate_typed_kernel_signature(&input)
                .unwrap_err()
                .to_string();
            let selected = validate_typed_kernel_profile_v1(&input, &options)
                .unwrap_err()
                .to_string();
            assert_eq!(
                selected, exact,
                "diagnostic owner changed for {}",
                input.sig.ident
            );
        }
    }

    #[test]
    fn general_typed_model_constructs_scalar_slice_abi_and_default_launch() {
        let input: ItemFn = parse_quote!(
            pub fn alpha(scale: u32, input: &[f32], output: fe2o3_device::DisjointSlice<f32>) {}
        );
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let model = model_general_typed_signature_v1(&input, &options, [0x41; 32]).unwrap();

        assert_eq!(
            model.arguments,
            vec![
                GeneralTypedArgumentKindV1::Scalar(GeneralTypedScalarV1::U32),
                GeneralTypedArgumentKindV1::SharedSlice(GeneralTypedScalarV1::F32),
                GeneralTypedArgumentKindV1::ExclusiveSlice(GeneralTypedScalarV1::F32),
            ]
        );
        assert_eq!(model.abi.size(), 40);
        assert_eq!(model.abi.alignment(), 8);
        assert_eq!(
            model
                .abi
                .fields()
                .iter()
                .map(|field| (field.name().as_str(), field.offset(), field.size()))
                .collect::<Vec<_>>(),
            vec![("arg0", 0, 4), ("arg1", 8, 16), ("arg2", 24, 16)]
        );

        let scalar = &model.abi.fields()[0];
        assert_eq!(scalar.kind(), AbiKind::Scalar(ScalarType::U32));
        assert_eq!(scalar.mutability(), Mutability::Immutable);
        assert_eq!(scalar.access(), Access::ByValue);
        assert_eq!(scalar.address_space(), AddressSpace::Value);
        assert_eq!(scalar.ownership(), ArgumentOwnership::ByValue);
        assert_eq!(scalar.alias_class(), AliasClass::Value);

        let shared = &model.abi.fields()[1];
        assert_eq!(
            shared.kind(),
            AbiKind::Slice {
                element_size: 4,
                element_alignment: 4,
            }
        );
        assert_eq!(shared.mutability(), Mutability::Immutable);
        assert_eq!(shared.access(), Access::ReadOnly);
        assert_eq!(shared.address_space(), AddressSpace::Global);
        assert_eq!(shared.ownership(), ArgumentOwnership::SharedBorrow);
        assert_eq!(shared.alias_class(), AliasClass::SharedReadOnly);

        let exclusive = &model.abi.fields()[2];
        assert_eq!(exclusive.mutability(), Mutability::Mutable);
        assert_eq!(exclusive.access(), Access::ReadWrite);
        assert_eq!(exclusive.ownership(), ArgumentOwnership::UniqueBorrow);
        assert_eq!(exclusive.alias_class(), AliasClass::Exclusive);
        assert_ne!(shared.type_identity(), exclusive.type_identity());

        assert_eq!(model.launch.rank(), 1);
        assert_eq!(
            model.launch.block_size(),
            BlockSize::Exact(fe2o3_artifacts::Dimensions::new(256, 1, 1).unwrap())
        );
        assert_eq!(model.launch.max_grid().x(), u32::MAX);
        assert_eq!(model.launch.max_grid().y(), 1);
        assert_eq!(model.launch.max_grid().z(), 1);
    }

    #[test]
    fn general_typed_model_binds_finite_rank_one_max_grid() {
        let input: ItemFn = parse_quote!(
            pub fn bounded(input: &[u8], output: DisjointSlice<f32>) {}
        );
        let default_options = parse_kernel_options(quote!(
            typed,
            launch(required = [64, 1, 1], max = [64, 1, 1])
        ))
        .unwrap();
        let bounded_options = parse_kernel_options(quote!(
            typed,
            launch(
                required = [64, 1, 1],
                max = [64, 1, 1],
                max_grid = [1, 1, 1]
            )
        ))
        .unwrap();
        let default =
            model_general_typed_signature_v1(&input, &default_options, [0x42; 32]).unwrap();
        let bounded =
            model_general_typed_signature_v1(&input, &bounded_options, [0x42; 32]).unwrap();
        assert_eq!(
            bounded.launch.max_grid(),
            fe2o3_artifacts::Dimensions::new(1, 1, 1).unwrap()
        );
        assert_ne!(
            bounded.generated_host_contract_identity,
            default.generated_host_contract_identity
        );

        let wrong_rank = parse_kernel_options(quote!(
            typed,
            launch(required = [64, 1, 1], max_grid = [1, 2, 1])
        ))
        .unwrap();
        assert!(model_general_typed_signature_v1(&input, &wrong_rank, [0x42; 32]).is_err());
    }

    #[test]
    fn general_typed_wg64_expansion_binds_host_identity_and_sidecar_to_renamed_body() {
        let input: ItemFn = parse_quote! {
            pub fn lds_slice(a: &[u16], b: &[u16], output: DisjointSlice<f32>) {
                let _ = (a, b, output);
            }
        };
        let options = parse_kernel_options(quote!(
            typed,
            launch(required = [64, 1, 1], max = [64, 1, 1])
        ))
        .unwrap();
        let default_options = parse_kernel_options(quote!(typed)).unwrap();
        let crate_binding = derive_crate_binding_id_v1("fixture", ["wg64-expansion"]);
        let kernel_binding = derive_kernel_binding_id_v1(
            crate_binding,
            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            "lds_slice",
            "lds_slice",
        );
        let model =
            model_general_typed_signature_v1(&input, &options, kernel_binding.as_bytes()).unwrap();
        let default_model =
            model_general_typed_signature_v1(&input, &default_options, kernel_binding.as_bytes())
                .unwrap();
        assert_eq!(
            model.launch.block_size(),
            BlockSize::Exact(fe2o3_artifacts::Dimensions::new(64, 1, 1).unwrap())
        );
        assert_ne!(
            model.generated_host_contract_identity, default_model.generated_host_contract_identity,
            "the generated host identity must bind the exact workgroup"
        );

        let device_import = device_import_for(FoundCrate::Name("gpu_device".to_string()));
        let host_import = host_import_for(FoundCrate::Name("gpu_host".to_string()));
        let expansion = expand_kernel_with_imports(
            input,
            options,
            &device_import,
            None,
            Some(&host_import),
            Some(crate_binding),
        )
        .unwrap();
        let syntax: syn::File = syn::parse2(expansion).unwrap();
        let internal_name = format!("__fe2o3_host_kernel_v1_{}", kernel_binding.to_hex());
        assert!(
            syntax.items.iter().any(
                |item| matches!(item, Item::Fn(function) if function.sig.ident == internal_name)
            )
        );

        let registration = syntax
            .items
            .iter()
            .find_map(|item| match item {
                Item::Static(item) if item.ident == "__fe2o3_kernel_registration_lds_slice" => {
                    Some(item)
                }
                _ => None,
            })
            .expect("generated kernel registration");
        let Expr::Tuple(registration) = registration.expr.as_ref() else {
            panic!("kernel registration is not a tuple");
        };
        assert_eq!(
            registration.elems[9].to_token_stream().to_string(),
            internal_name
        );

        let frontend = syntax
            .items
            .iter()
            .find_map(|item| match item {
                Item::Static(item)
                    if item.ident == "__fe2o3_kernel_frontend_contract_v1_lds_slice" =>
                {
                    Some(item)
                }
                _ => None,
            })
            .expect("generated frontend registration");
        let Expr::Tuple(frontend) = frontend.expr.as_ref() else {
            panic!("frontend registration is not a tuple");
        };
        assert_eq!(
            frontend.elems[5].to_token_stream().to_string(),
            internal_name
        );
        let bytes_name = "__fe2o3_kernel_frontend_contract_bytes_v1_lds_slice";
        assert_eq!(frontend.elems[4].to_token_stream().to_string(), bytes_name);
        let bytes = syntax
            .items
            .iter()
            .find_map(|item| match item {
                Item::Const(item) if item.ident == bytes_name => Some(item),
                _ => None,
            })
            .expect("generated frontend byte-slice constant");
        let Expr::Reference(bytes) = bytes.expr.as_ref() else {
            panic!("frontend bytes are not referenced");
        };
        let Expr::Array(bytes) = bytes.expr.as_ref() else {
            panic!("frontend bytes are not an array");
        };
        let bytes = bytes
            .elems
            .iter()
            .map(|byte| match byte {
                Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(byte),
                    ..
                }) => byte.base10_parse::<u8>().unwrap(),
                _ => panic!("frontend contract contains a non-byte expression"),
            })
            .collect::<Vec<_>>();
        assert_eq!(
            bytes,
            [
                70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64,
                0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
            ]
        );
    }

    #[test]
    fn required_only_exact_launches_preserve_wg64_and_wg256_compatibility() {
        let input: ItemFn = parse_quote! {
            pub fn general(input: &[u16], output: DisjointSlice<f32>) {
                let _ = (input, output);
            }
        };
        for dimensions in [[64_u32, 1, 1], [256_u32, 1, 1]] {
            let x = dimensions[0];
            let options = parse_kernel_options(
                syn::parse_str::<syn::MetaList>(&format!("launch(required = [{x}, 1, 1])"))
                    .unwrap()
                    .to_token_stream(),
            )
            .unwrap();
            let model = model_general_typed_signature_v1(&input, &options, [0x63; 32]).unwrap();
            assert_eq!(
                model.launch.block_size(),
                BlockSize::Exact(fe2o3_artifacts::Dimensions::new(dimensions[0], 1, 1).unwrap())
            );
            let bytes = super::encode_kernel_frontend_contract_v1(&options).unwrap();
            assert_eq!(&bytes[20..22], &1_u16.to_le_bytes());
            assert_eq!(&bytes[24..28], &dimensions[0].to_le_bytes());
            assert_eq!(&bytes[36..48], &[0; 12]);
        }

        let vecadd: ItemFn = parse_quote! {
            pub fn vecadd(a: &[f32], b: &[f32], output: DisjointSlice<f32>) {
                let _ = (a, b, output);
            }
        };
        let wg256_required_only =
            parse_kernel_options(quote!(typed, launch(required = [256, 1, 1]))).unwrap();
        validate_typed_kernel_profile_v1(&vecadd, &wg256_required_only).unwrap();

        let alpha: ItemFn = parse_quote! {
            pub fn alpha(scale: f32, input: &[f32], output: DisjointSlice<f32>) {
                let _ = (scale, input, output);
            }
        };
        model_general_typed_signature_v1(&alpha, &wg256_required_only, [0x64; 32]).unwrap();
    }

    #[test]
    fn fixed_typed_profiles_reject_wg64_without_falling_back() {
        let options = parse_kernel_options(quote!(
            typed,
            launch(required = [64, 1, 1], max = [64, 1, 1])
        ))
        .unwrap();
        let vecadd: ItemFn = parse_quote! {
            pub fn vecadd(a: &[f32], b: &[f32], output: DisjointSlice<f32>) {
                let _ = (a, b, output);
            }
        };
        assert!(
            validate_typed_kernel_profile_v1(&vecadd, &options)
                .unwrap_err()
                .to_string()
                .contains("typed vecadd V2 profile requires an exact 256x1x1")
        );

        let alpha: ItemFn = parse_quote! {
            pub fn alpha(scale: f32, input: &[f32], output: DisjointSlice<f32>) {
                let _ = (scale, input, output);
            }
        };
        assert!(
            model_general_typed_signature_v1(&alpha, &options, [0x64; 32])
                .unwrap_err()
                .to_string()
                .contains("alpha/zeta and scalar_gemm_v1 typed profiles require")
        );
    }

    #[test]
    fn general_typed_model_constructs_nontrivial_abi_under_fixed_launch() {
        let input: ItemFn = parse_quote!(
            pub fn beta(
                seed: i8,
                coefficient: f64,
                input: &[u16],
                output: DisjointSlice<u32, Index1D>,
            ) {
            }
        );
        let options = parse_kernel_options(quote!(
            typed,
            launch(required = [256, 1, 1], max = [256, 1, 1])
        ))
        .unwrap();
        let model = model_general_typed_signature_v1(&input, &options, [0x52; 32]).unwrap();

        assert_eq!(model.abi.size(), 48);
        assert_eq!(
            model
                .abi
                .fields()
                .iter()
                .map(|field| (field.offset(), field.size(), field.alignment()))
                .collect::<Vec<_>>(),
            vec![(0, 1, 1), (8, 8, 8), (16, 16, 8), (32, 16, 8)]
        );
        assert_eq!(model.launch.rank(), 1);
        assert_eq!(
            model.launch.block_size(),
            BlockSize::Exact(fe2o3_artifacts::Dimensions::new(256, 1, 1).unwrap())
        );
        assert_eq!(model.launch.max_grid().y(), 1);
        assert_eq!(model.launch.max_grid().z(), 1);
    }

    #[test]
    fn general_typed_model_accepts_every_bounded_scalar_spelling() {
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let scalar_spellings = [
            "i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64", "f32", "f64",
        ];
        let arguments = scalar_spellings
            .iter()
            .enumerate()
            .map(|(index, scalar)| format!("scalar{index}: {scalar}"))
            .collect::<Vec<_>>()
            .join(", ");
        let input: ItemFn = syn::parse_str(&format!("pub fn scalars({arguments}) {{}}"))
            .expect("all bounded scalar spellings parse");
        let model = model_general_typed_signature_v1(&input, &options, [0xa1; 32]).unwrap();
        assert_eq!(model.arguments.len(), scalar_spellings.len());
        assert!(
            model
                .arguments
                .iter()
                .all(|argument| matches!(argument, GeneralTypedArgumentKindV1::Scalar(_)))
        );
    }

    #[test]
    fn general_typed_global_mut_pointer_binds_source_abi_and_host_authority() {
        let input: ItemFn = syn::parse_quote! {
            pub fn atomic(target: fe2o3_device::DeviceGlobalMutPtr<u32>) {
                let _ = target;
            }
        };
        let options = parse_kernel_options(quote!(
            typed,
            launch(required = [64, 1, 1], max = [64, 1, 1])
        ))
        .unwrap();
        let model = model_general_typed_signature_v1(&input, &options, [0xa3; 32]).unwrap();
        assert_eq!(
            model.arguments,
            vec![GeneralTypedArgumentKindV1::GlobalMutPointer(
                GeneralTypedScalarV1::U32
            )]
        );
        let field = &model.abi.fields()[0];
        assert_eq!(
            field.kind(),
            AbiKind::Pointer {
                pointee_size: 4,
                pointee_alignment: 4,
            }
        );
        assert_eq!(field.mutability(), Mutability::Mutable);
        assert_eq!(field.access(), Access::ReadWrite);
        assert_eq!(field.address_space(), AddressSpace::Global);
        assert_eq!(field.ownership(), ArgumentOwnership::UniqueBorrow);
        assert_eq!(field.alias_class(), AliasClass::Exclusive);

        let generated = generated_general_typed_arguments_v1(&input, &model.arguments).to_string();
        for marker in [
            "pub struct GlobalMut",
            "GeneratedReadWriteDeviceSlice",
            "region . len () != 1",
            "GlobalMut < 'allocation , u32 >",
        ] {
            assert!(
                generated.contains(marker),
                "missing generated marker {marker}"
            );
        }
        assert!(!generated.contains("as_raw"));

        let f32_identity =
            general_typed_global_mut_pointer_type_identity_v1(GeneralTypedScalarV1::F32);
        assert_ne!(field.type_identity(), f32_identity);
    }

    #[test]
    fn general_typed_scalar_identities_match_shared_artifact_goldens() {
        let scalar_spellings = [
            "i8", "u8", "i16", "u16", "i32", "u32", "i64", "u64", "f32", "f64",
        ];
        let shared_scalars = [
            RustScalarElementTypeV1::I8,
            RustScalarElementTypeV1::U8,
            RustScalarElementTypeV1::I16,
            RustScalarElementTypeV1::U16,
            RustScalarElementTypeV1::I32,
            RustScalarElementTypeV1::U32,
            RustScalarElementTypeV1::I64,
            RustScalarElementTypeV1::U64,
            RustScalarElementTypeV1::F32,
            RustScalarElementTypeV1::F64,
        ];
        let expected_identities = [
            (
                "ff65d4e4147594edf8151ba77165309fa936203a650ca2daecbde51ce29a4e69",
                "d1081bb401a18da25bf6fa63fb3e769d3ffcbf9c847f7bf509d4210fcedd730f",
            ),
            (
                "1d706983e50eae90f37a598e592d6e0b3806fd8abb04631303af81d1e80ce210",
                "8bddfa68ddac6f85a8c30d7ef47cb27a0a4046c47cfd390436dadc6a6696e68f",
            ),
            (
                "24372d467042ac8437546c3439a3e00c2daa262f958e51582b5087c0f7c4e86c",
                "af357f6ec867517b376d8a9ae7a6f479bbccf4e7afb8f5ecae80a5149928ae9f",
            ),
            (
                "e5377f8d3cdb2409256d5addba15c46e53e2196b23aaf23dbe6b19a71fa95c95",
                "2528593b47fa823e89ae33b2205400360e1b48bb5bb53248ae061dcdce3645ae",
            ),
            (
                "e6786cf029d616cbb3ff5c317a47d55fd84e72ced23b7f610830d03b6103a93a",
                "1acb3481b8e99bb29cef03cc2777bf9e6f3cf1d3584da4ee456c478faf69cdaa",
            ),
            (
                "e312b413d7890a7147b229b57a42d7935d015dee58f0fb610d46999e62659a08",
                "801df7d2b519e75f693078558936feb7813b577c6306110c21c0075b7fceddb2",
            ),
            (
                "dd857c35102cc0d85917be7a380912b70fc3f9bbdab1fb86ff1b69b218b61683",
                "0936d0e4b71167ba7e1e1846e81e3d77a77c9c6c630d6c4c795ce08d66d05df4",
            ),
            (
                "4919da5956e23a23e11be9653162fa6fed5b2f90a1e11ef6b83ad232270ee8a4",
                "3071ad4ec69848183edd068ff74bd280c5fd1987d46cbd703bf9b99fa81e794f",
            ),
            (
                "42301591dde145200e107c459da19b8bdfd11aba362338fdbd5a4dd258c76df2",
                "8c1c71a7931f3627ae1031cab818406af8161716dc51b1c1ea208e405e17fb16",
            ),
            (
                "2073ec8b971717ac59ef1291b84b1662aba314a26f62af3196c0ad6812a162b6",
                "05bbfb5846ec168daf9ab3abb0f86a4d85de88b3f241dffaa20d1bf7835a6f9d",
            ),
        ];
        let arguments = scalar_spellings
            .iter()
            .enumerate()
            .map(|(index, scalar)| format!("scalar{index}: {scalar}"))
            .collect::<Vec<_>>()
            .join(", ");
        let input: ItemFn = syn::parse_str(&format!("pub fn scalars({arguments}) {{}}"))
            .expect("all bounded scalar spellings parse");
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let model = model_general_typed_signature_v1(&input, &options, [0xa2; 32]).unwrap();
        let mut actual_identities = Vec::new();

        for (field, scalar) in model.abi.fields().iter().zip(shared_scalars) {
            let size = scalar.size_bytes();
            let alignment = size as u32;
            let component = RustPhysicalComponentV1::new(
                0,
                size,
                alignment,
                RustPhysicalComponentKindV1::Scalar { scalar },
            )
            .unwrap();
            let independent = RustLayoutEvidenceV1::new(
                RustTypeEvidenceV1::new(RustSourceTypeShapeV1::scalar(scalar)),
                RustcAbiClassV1::Scalar,
                PointerWidth::Bits64,
                size,
                alignment,
                vec![component],
            )
            .unwrap()
            .type_identity();

            assert_eq!(field.type_identity(), independent);
            actual_identities.push((
                hex(independent.rust_type().bytes().as_bytes()),
                hex(independent.layout().bytes().as_bytes()),
            ));
        }
        assert_eq!(
            actual_identities,
            expected_identities
                .map(|(rust_type, layout)| (rust_type.to_owned(), layout.to_owned()))
        );
    }

    #[test]
    fn exclusive_slice_effect_is_conservative_and_body_independent() {
        let empty_body: ItemFn = parse_quote!(
            pub fn exclusive(output: DisjointSlice<u32>) {}
        );
        let consuming_body: ItemFn = parse_quote!(
            pub fn exclusive(output: DisjointSlice<u32>) {
                let _ = output;
            }
        );
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let empty = model_general_typed_signature_v1(&empty_body, &options, [0xb1; 32]).unwrap();
        let consuming =
            model_general_typed_signature_v1(&consuming_body, &options, [0xb1; 32]).unwrap();

        assert_eq!(empty.abi.fields()[0].access(), Access::ReadWrite);
        assert_eq!(
            empty.generated_host_contract_identity,
            consuming.generated_host_contract_identity
        );
        assert!(validate_typed_kernel_signature(&empty_body).is_err());
    }

    #[test]
    fn general_typed_contract_identity_binds_signature_name_and_binding() {
        let alpha: ItemFn = parse_quote!(
            pub fn alpha(scale: u32, input: &[f32], output: DisjointSlice<f32>) {}
        );
        let renamed: ItemFn = parse_quote!(
            pub fn renamed(scale: u32, input: &[f32], output: DisjointSlice<f32>) {}
        );
        let changed_scalar: ItemFn = parse_quote!(
            pub fn alpha(scale: u64, input: &[f32], output: DisjointSlice<f32>) {}
        );
        let changed_effect: ItemFn = parse_quote!(
            pub fn alpha(scale: u32, input: &[f32], output: &[f32]) {}
        );
        let default_options = parse_kernel_options(quote!(typed)).unwrap();
        let explicit_options =
            parse_kernel_options(quote!(typed, launch(required = [256, 1, 1]))).unwrap();
        let identity = |input: &ItemFn, options: &KernelOptions, binding| {
            model_general_typed_signature_v1(input, options, binding)
                .unwrap()
                .generated_host_contract_identity
        };
        let baseline = identity(&alpha, &default_options, [0x61; 32]);

        assert_ne!(baseline, identity(&renamed, &default_options, [0x61; 32]));
        assert_ne!(
            baseline,
            identity(&changed_scalar, &default_options, [0x61; 32])
        );
        assert_ne!(
            baseline,
            identity(&changed_effect, &default_options, [0x61; 32])
        );
        assert_eq!(baseline, identity(&alpha, &explicit_options, [0x61; 32]));
        assert_ne!(baseline, identity(&alpha, &default_options, [0x62; 32]));
        assert_eq!(baseline, identity(&alpha, &default_options, [0x61; 32]));
    }

    #[test]
    fn general_typed_model_rejects_unsupported_function_and_argument_shapes() {
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let cases: Vec<ItemFn> = vec![
            parse_quote!(
                fn private(value: u32) {}
            ),
            parse_quote!(
                pub unsafe fn unsafe_kernel(value: u32) {}
            ),
            parse_quote!(
                pub const fn constant(value: u32) {}
            ),
            parse_quote!(
                pub async fn asynchronous(value: u32) {}
            ),
            parse_quote!(
                pub extern "C" fn foreign(value: u32) {}
            ),
            parse_quote!(
                pub fn generic<T>(value: T) {}
            ),
            parse_quote!(
                pub fn lifetime<'a>(value: &'a [u32]) {}
            ),
            parse_quote!(
                pub fn result(value: u32) -> u32 {
                    value
                }
            ),
            parse_quote!(
                pub fn empty() {}
            ),
            parse_quote!(
                pub fn raw(value: *const u32) {}
            ),
            parse_quote!(
                pub fn mutable(value: &mut [u32]) {}
            ),
            parse_quote!(
                pub fn aggregate(value: (u32, u32)) {}
            ),
            parse_quote!(
                pub fn named(value: UserAggregate) {}
            ),
            parse_quote!(
                pub fn alias(value: ScalarAlias) {}
            ),
            parse_quote!(
                pub fn primitive_f16(value: f16) {}
            ),
            parse_quote!(
                pub fn array(value: &[UserElement]) {}
            ),
            parse_quote!(
                pub fn wrong_index(value: DisjointSlice<u32, Index2D>) {}
            ),
            parse_quote!(
                pub fn aliased_slice(value: gpu::DisjointSlice<u32>) {}
            ),
            parse_quote!(
                pub fn receiver(self: Box<Self>) {}
            ),
            parse_quote!(
                pub fn pattern((left, right): (u32, u32)) {
                    let _ = (left, right);
                }
            ),
        ];

        for input in cases {
            assert!(
                model_general_typed_signature_v1(&input, &options, [0x71; 32]).is_err(),
                "unexpectedly accepted {}",
                input.sig.ident,
            );
        }
    }

    #[test]
    fn general_typed_model_enforces_the_canonical_field_bound() {
        let options = parse_kernel_options(quote!(typed)).unwrap();
        let arguments = (0..64)
            .map(|index| format!("arg{index}: u8"))
            .collect::<Vec<_>>()
            .join(", ");
        let accepted: ItemFn = syn::parse_str(&format!("pub fn bounded({arguments}) {{}}"))
            .expect("valid bounded function");
        assert_eq!(
            model_general_typed_signature_v1(&accepted, &options, [0x81; 32])
                .unwrap()
                .abi
                .fields()
                .len(),
            64
        );

        let arguments = (0..65)
            .map(|index| format!("arg{index}: u8"))
            .collect::<Vec<_>>()
            .join(", ");
        let rejected: ItemFn = syn::parse_str(&format!("pub fn excessive({arguments}) {{}}"))
            .expect("valid excessive function");
        assert!(model_general_typed_signature_v1(&rejected, &options, [0x81; 32]).is_err());
    }

    #[test]
    fn general_typed_model_fails_closed_on_unrepresentable_launch_options() {
        let input: ItemFn = parse_quote!(
            pub fn kernel(value: u32) {}
        );
        for options in [
            parse_kernel_options(quote!(typed, launch(max = [256, 1, 1]))).unwrap(),
            parse_kernel_options(quote!(
                typed,
                launch(required = [32, 2, 1], max = [32, 2, 1])
            ))
            .unwrap(),
            parse_kernel_options(quote!(
                typed,
                launch(required = [128, 1, 1], max = [128, 1, 1])
            ))
            .unwrap(),
            parse_kernel_options(quote!(
                typed,
                launch(required = [64, 1, 1], max = [128, 1, 1])
            ))
            .unwrap(),
            parse_kernel_options(quote!(
                typed,
                launch(max = [128, 1, 1], min_workgroups_per_compute_unit = 2)
            ))
            .unwrap(),
        ] {
            let error = model_general_typed_signature_v1(&input, &options, [0x91; 32])
                .expect_err("unsupported launch contract must fail closed");
            assert!(error.to_string().contains("general typed V1"));
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
