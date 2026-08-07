#![feature(proc_macro_tracked_env)]

mod control_flow_v1;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::{format_ident, quote};
use reserved_fe2o3_symbols::{
    CRATE_BINDING_ID_ENV_V1, CrateBindingIdV1, KERNEL_PREFIX, KERNEL_REGISTRATION_KIND_KERNEL,
    KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2, KERNEL_REGISTRATION_MAGIC,
    KERNEL_REGISTRATION_PREFIX, KERNEL_REGISTRATION_VERSION_V1, KERNEL_REGISTRATION_VERSION_V2,
    RESERVED_ROOT, TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2, artifact_length_symbol_v1,
    artifact_pointer_symbol_v1, derive_kernel_binding_id_v1, host_kernel_symbol_v1,
};
use syn::{
    Data, DeriveInput, Expr, ExprArray, FnArg, ForeignItem, GenericArgument, ItemFn,
    ItemForeignMod, Lit, LitInt, Meta, Pat, PathArguments, ReturnType, Token, Type, Visibility,
    parse::Parser, parse_macro_input, punctuated::Punctuated, visit::Visit,
};

use crate::control_flow_v1::{
    CONTROL_FLOW_REGISTRATION_KIND_V1, CONTROL_FLOW_REGISTRATION_MAGIC_V1,
    CONTROL_FLOW_REGISTRATION_PREFIX_V1, CONTROL_FLOW_REGISTRATION_VERSION_V1,
    ParsedControlFlowOptionsV1, analyze_kernel_control_flow_v1, parse_control_flow_options_v1,
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

/// Marks a device kernel and emits its collector registration.
///
/// V1 launch declarations use `launch(required = [x, y, z], max = [x, y, z],
/// min_workgroups_per_compute_unit = n)`. Each dimension is nonzero and the
/// product is at most 1024. The occupancy field requires `max`.
///
/// Target assembly must be declared with `unsafe_asm(target = "gfx942",
/// operands(...), options(...), effects(...))` on an `unsafe fn`. The
/// declaration is recorded for later compiler validation and grants no
/// assembly, memory, loading, or launch authority by itself.
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum KernelMode {
    Basic,
    Typed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct KernelOptions {
    mode: KernelMode,
    explicit_namespace: Option<CrateBindingIdV1>,
    launch: Option<ParsedLaunchBoundsV1>,
    unsafe_assembly: Option<ParsedUnsafeAssemblyV1>,
    control_flow: Option<ParsedControlFlowOptionsV1>,
}

const MAX_TYPED_KERNEL_SYMBOL_STEM_BYTES: usize = 128;
const MAX_WORKGROUP_THREADS_V1: u64 = 1_024;
const MAX_RESIDENT_WORKGROUPS_PER_COMPUTE_UNIT_V1: u16 = 64;
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
            launch: None,
            unsafe_assembly: None,
            control_flow: None,
        });
    }

    let arguments = Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attr.clone())?;
    let mut typed = false;
    let mut explicit_namespace = None;
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
                    "#[kernel] accepts only #[kernel], #[kernel(typed)], namespace, launch(...), unsafe_asm(...), and control_flow(...) declarations",
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

    Ok(KernelOptions {
        mode: if typed {
            KernelMode::Typed
        } else {
            KernelMode::Basic
        },
        explicit_namespace,
        launch,
        unsafe_assembly,
        control_flow,
    })
}

fn parse_launch_bounds_v1(list: &syn::MetaList) -> syn::Result<ParsedLaunchBoundsV1> {
    let mut required = None;
    let mut maximum = None;
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
            "launch supports only required = [x, y, z], max = [x, y, z], and min_workgroups_per_compute_unit = n",
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
        min_workgroups_per_compute_unit: occupancy,
    })
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
        validate_typed_kernel_signature(&input)?;
        validate_typed_kernel_symbol_stem(&input.sig.ident)?;
    }
    validate_kernel_signature(&input)?;

    let crate_binding = if options.mode == KernelMode::Typed {
        Some(resolve_crate_binding(options.explicit_namespace)?)
    } else {
        None
    };

    let device_import = fe2o3_device_import()?;
    let host_import = if options.mode == KernelMode::Typed {
        Some(fe2o3_host_import()?)
    } else {
        None
    };

    expand_kernel_with_imports(
        input,
        options,
        &device_import,
        host_import.as_ref(),
        crate_binding,
    )
}

fn resolve_crate_binding(
    explicit_namespace: Option<CrateBindingIdV1>,
) -> syn::Result<CrateBindingIdV1> {
    match proc_macro::tracked::env_var(CRATE_BINDING_ID_ENV_V1) {
        Ok(value) => CrateBindingIdV1::from_hex(&value).map_err(|error| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("invalid {CRATE_BINDING_ID_ENV_V1}: {error}"),
            )
        }),
        Err(std::env::VarError::NotPresent) => explicit_namespace.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[kernel(typed)] requires the cargo-fe2o3 rustc wrapper or an explicit 256-bit namespace",
            )
        }),
        Err(std::env::VarError::NotUnicode(_)) => Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("{CRATE_BINDING_ID_ENV_V1} is not valid UTF-8"),
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
            launch: None,
            unsafe_assembly: None,
            control_flow: None,
        },
        device_import,
        None,
        None,
    )
}

fn expand_kernel_with_imports(
    mut input: ItemFn,
    options: KernelOptions,
    device_import: &proc_macro2::TokenStream,
    host_import: Option<&proc_macro2::TokenStream>,
    crate_binding: Option<CrateBindingIdV1>,
) -> syn::Result<proc_macro2::TokenStream> {
    let mode = options.mode;
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
    if mode == KernelMode::Typed {
        validate_typed_kernel_signature(&input)?;
        validate_typed_kernel_symbol_stem(&input.sig.ident)?;
    }
    validate_kernel_signature(&input)?;

    let original_ident = input.sig.ident.clone();
    let original_name = original_ident.to_string();
    let control_flow_contract =
        analyze_kernel_control_flow_v1(&input, options.control_flow.as_ref())?;

    if original_name.starts_with(RESERVED_ROOT) {
        return Err(syn::Error::new_spanned(
            original_ident,
            format!("function names starting with `{RESERVED_ROOT}` are reserved by fe2o3"),
        ));
    }

    let kernel_binding = crate_binding.map(|crate_binding| {
        derive_kernel_binding_id_v1(
            crate_binding,
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            &original_name,
            &original_name,
        )
    });
    let internal_ident = match kernel_binding {
        Some(binding) => format_ident!("__fe2o3_host_kernel_v1_{}", binding.to_hex()),
        None => format_ident!("{KERNEL_PREFIX}{original_name}"),
    };
    let name_marker_ident = format_ident!("__fe2o3_kernel_name_{original_name}");
    let type_marker_ident = format_ident!("__fe2o3_kernel_marker_{original_name}");
    let registration_ident = format_ident!("{KERNEL_REGISTRATION_PREFIX}{original_name}");
    let registration_kind = match mode {
        KernelMode::Basic => KERNEL_REGISTRATION_KIND_KERNEL,
        KernelMode::Typed => KERNEL_REGISTRATION_KIND_TYPED_VECADD_LAYOUT_V2,
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

    let (registration_type, registration_value) = match (mode, crate_binding, kernel_binding) {
        (KernelMode::Basic, None, None) => (
            quote!((u64, u16, u16, &'static str, &'static str, #function_pointer)),
            quote!((
                #KERNEL_REGISTRATION_MAGIC,
                #KERNEL_REGISTRATION_VERSION_V1,
                #registration_kind,
                #marker_value,
                #export_value,
                #internal_ident,
            )),
        ),
        (KernelMode::Typed, Some(crate_binding), Some(kernel_binding)) => {
            let crate_binding = syn::LitStr::new(&crate_binding.to_hex(), original_ident.span());
            let kernel_binding = syn::LitStr::new(&kernel_binding.to_hex(), original_ident.span());
            (
                quote!((
                    u64,
                    u16,
                    u16,
                    &'static str,
                    &'static str,
                    &'static str,
                    &'static str,
                    #function_pointer,
                )),
                quote!((
                    #KERNEL_REGISTRATION_MAGIC,
                    #KERNEL_REGISTRATION_VERSION_V2,
                    #registration_kind,
                    #marker_value,
                    #export_value,
                    #crate_binding,
                    #kernel_binding,
                    #internal_ident,
                )),
            )
        }
        _ => unreachable!("kernel mode and binding identity must agree"),
    };
    let export_attribute = match kernel_binding {
        Some(binding) => {
            let symbol = syn::LitStr::new(&host_kernel_symbol_v1(binding), original_ident.span());
            quote!(#[unsafe(export_name = #symbol)])
        }
        None => quote!(#[unsafe(no_mangle)]),
    };
    let frontend_registration = encode_kernel_frontend_contract_v1(&options).map(|bytes| {
        let registration_ident =
            format_ident!("{KERNEL_FRONTEND_REGISTRATION_PREFIX_V1}{original_name}");
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
                #KERNEL_FRONTEND_REGISTRATION_MAGIC_V1,
                #KERNEL_FRONTEND_REGISTRATION_VERSION_V1,
                #KERNEL_FRONTEND_REGISTRATION_KIND_V1,
                #marker_value,
                &[#(#bytes),*],
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

    let typed_module = if let Some(kernel_binding) = kernel_binding {
        let host_import = host_import.expect("typed expansion requires a host import");
        let module_ident = format_ident!("{original_name}_gpu");
        let artifact_pointer_ident =
            format_ident!("{}", artifact_pointer_symbol_v1(kernel_binding));
        let artifact_length_ident = format_ident!("{}", artifact_length_symbol_v1(kernel_binding));
        let binding_bytes = kernel_binding.as_bytes().into_iter();

        quote! {
            pub mod #module_ident {
                unsafe extern "C" {
                    fn #artifact_pointer_ident() -> *const u8;
                    fn #artifact_length_ident() -> usize;
                }

                #host_import

                pub type Kernel =
                    __fe2o3_kernel_host::__generated::GeneratedVecAddKernelV1<
                        super::#type_marker_ident,
                    >;
                pub type Prepared<'loaded, 'allocation> =
                    __fe2o3_kernel_host::__generated::GeneratedVecAddPreparedV1<
                        'loaded,
                        'allocation,
                        super::#type_marker_ident,
                    >;

                const _: () = {
                    // SAFETY: the fe2o3 backend owns these reserved symbols and
                    // binds them to the artifact for this exact generated marker.
                    unsafe impl __fe2o3_kernel_host::__generated::CompilerGeneratedKernelContractV1
                        for super::#type_marker_ident
                    {
                        const PROFILE:
                            __fe2o3_kernel_host::__generated::CompilerGeneratedKernelProfileV1 =
                            __fe2o3_kernel_host::__generated::CompilerGeneratedKernelProfileV1::TypedVecAddF32RustcLayoutV2;

                        const KERNEL_BINDING_ID_V1: [u8; 32] = [#(#binding_bytes),*];

                        fn artifact_container_bytes() -> &'static [u8] {
                            // SAFETY: the generated unsafe trait implementation relies
                            // on the backend/linker contract that this accessor pair
                            // returns one exact, immutable, program-lifetime artifact.
                            unsafe {
                                __fe2o3_kernel_host::__generated::artifact_bytes_from_backend_v1(
                                    #artifact_pointer_ident(),
                                    #artifact_length_ident(),
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

        const _: () = {
            #device_import

            // SAFETY: every associated value below is generated from the same
            // parsed function and the same collector-visible registration.
            unsafe impl __fe2o3_kernel_device::KernelMarkerV1 for #type_marker_ident {
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

fn encode_kernel_frontend_contract_v1(options: &KernelOptions) -> Option<Vec<u8>> {
    if options.launch.is_none() && options.unsafe_assembly.is_none() {
        return None;
    }
    let mut bytes = Vec::with_capacity(64);
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
            | (u16::from(launch.min_workgroups_per_compute_unit.is_some()) * 0x0004);
        push_u16(&mut bytes, launch_flags);
        push_u16(&mut bytes, 0);
        push_dimensions(&mut bytes, launch.required);
        push_dimensions(&mut bytes, launch.maximum);
        push_u16(
            &mut bytes,
            launch.min_workgroups_per_compute_unit.unwrap_or(0),
        );
        push_u16(&mut bytes, 0);
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
    debug_assert!(bytes.len() <= 64);
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
    let symbol = syn::LitStr::new(&options.symbol, function_ident.span());
    let marker = syn::LitStr::new(&marker, function_ident.span());
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
        static #registration_ident: #registration

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
        DeviceFfiOptions, KernelMode, KernelOptions, canonical_device_ffi_signature,
        core_import_for, device_ffi_contract, device_import_for,
        expand_device_copy_with_core_import, expand_device_export_with_import,
        expand_device_import_with_import, expand_kernel_with_device_import,
        expand_kernel_with_imports, host_import_for, parse_device_ffi_options,
        parse_kernel_options, validate_generated_device_ffi_contract_grammar,
        validate_kernel_assembly_boundary, validate_typed_kernel_signature,
        validate_typed_kernel_symbol_stem,
    };
    use proc_macro_crate::FoundCrate;
    use reserved_fe2o3_symbols::{
        TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2, artifact_length_symbol_v1,
        artifact_pointer_symbol_v1, derive_crate_binding_id_v1, derive_kernel_binding_id_v1,
        host_kernel_symbol_v1,
    };
    use syn::{ItemFn, ItemForeignMod, parse_quote};

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
                launch: None,
                unsafe_assembly: None,
                control_flow: None,
            }
        );

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
        let expansion = expand_kernel_with_imports(input, options, &device_import, None, None)
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
    fn typed_kernel_emits_guarded_embedded_artifact_contract() {
        let input = parse_quote! {
            pub fn vecadd(a: &[f32], b: &[f32], mut c: DisjointSlice<f32>) {
                let _ = (a, b, &mut c);
            }
        };
        let device_import = device_import_for(FoundCrate::Name("gpu_device".to_string()));
        let host_import = host_import_for(FoundCrate::Name("gpu_host".to_string()));
        let crate_binding = derive_crate_binding_id_v1("fixture", ["metadata"]);
        let kernel_binding = derive_kernel_binding_id_v1(
            crate_binding,
            TYPED_VECADD_F32_LAYOUT_PROFILE_TAG_V2,
            "vecadd",
            "vecadd",
        );

        let expansion = expand_kernel_with_imports(
            input,
            KernelOptions {
                mode: KernelMode::Typed,
                explicit_namespace: None,
                launch: None,
                unsafe_assembly: None,
                control_flow: None,
            },
            &device_import,
            Some(&host_import),
            Some(crate_binding),
        )
        .unwrap()
        .to_string();

        assert!(expansion.contains("pub mod vecadd_gpu"));
        assert!(expansion.contains("2u16 , 3u16"));
        assert!(!expansion.contains("1u16 , 1u16"));
        assert!(expansion.contains(&format!(
            "fn {} () -> * const u8",
            artifact_pointer_symbol_v1(kernel_binding)
        )));
        assert!(expansion.contains(&format!(
            "fn {} () -> usize",
            artifact_length_symbol_v1(kernel_binding)
        )));
        assert!(expansion.contains(&host_kernel_symbol_v1(kernel_binding)));
        assert!(expansion.contains(&crate_binding.to_hex()));
        assert!(expansion.contains(&kernel_binding.to_hex()));
        assert!(expansion.contains("extern crate gpu_host as __fe2o3_kernel_host"));
        assert!(expansion.contains(
            "pub type Kernel = __fe2o3_kernel_host :: __generated :: GeneratedVecAddKernelV1"
        ));
        assert!(expansion.contains(
            "pub type Prepared < 'loaded , 'allocation > = __fe2o3_kernel_host :: __generated :: GeneratedVecAddPreparedV1"
        ));
        assert!(expansion.contains(
            "unsafe impl __fe2o3_kernel_host :: __generated :: CompilerGeneratedKernelContractV1"
        ));
        assert!(expansion.contains(
            "const PROFILE : __fe2o3_kernel_host :: __generated :: CompilerGeneratedKernelProfileV1 = __fe2o3_kernel_host :: __generated :: CompilerGeneratedKernelProfileV1 :: TypedVecAddF32RustcLayoutV2"
        ));
        assert!(expansion.contains("const KERNEL_BINDING_ID_V1 : [u8 ; 32]"));
        assert!(expansion.contains("fn artifact_container_bytes () -> & 'static [u8]"));
        assert!(
            expansion
                .contains("__fe2o3_kernel_host :: __generated :: artifact_bytes_from_backend_v1")
        );
        assert!(!expansion.contains("artifact_vecadd_start"));
        assert!(!expansion.contains("artifact_vecadd_end"));
        assert!(!expansion.contains("from_raw_parts"));
        assert!(!expansion.contains("checked_sub"));
        assert!(!expansion.contains("KernelParams"));
        assert!(!expansion.contains("launch_unchecked"));
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
