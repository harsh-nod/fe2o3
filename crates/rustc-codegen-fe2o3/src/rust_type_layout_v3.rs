//! Compiler-authoritative Rust type/layout facts for the bounded general typed profile.

use std::fmt;

use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize,
    Dimensions, LaunchContract, MAX_ABI_FIELDS, Mutability, Name, PointerWidth,
    RustDisjointIndexSpaceV1, RustLayoutEvidenceV1, RustPhysicalComponentKindV1,
    RustPhysicalComponentV1, RustPointerMutabilityV1, RustScalarElementTypeV1,
    RustSourceTypeShapeV1, RustTypeEvidenceV1, RustcAbiClassV1, ScalarType, TypeIdentity,
};
use rustc_abi::{BackendRepr, ExternAbi, HasDataLayout, Primitive};
use rustc_hir::def::DefKind;
use rustc_hir::{Mutability as HirMutability, Safety};
use rustc_middle::ty::layout::{LayoutCx, LayoutOf};
use rustc_middle::ty::{
    FloatTy, Instance, InstanceKind, IntTy, Ty, TyCtxt, TyKind, TypingEnv, UintTy,
};
use rustc_span::Symbol;

use crate::trusted_device_items::{self, TrustedDeviceItem};

const INDEX_1D_DIAGNOSTIC_ITEM: &str = "fe2o3_device_thread_index_1d";
const POINTER_BYTES: u64 = 8;
const POINTER_ALIGNMENT: u32 = 8;
const SLICE_BYTES: u64 = 16;
const DEFAULT_WORKGROUP: [u32; 3] = [256, 1, 1];
const WAVE64_WORKGROUP: [u32; 3] = [64, 1, 1];

const ALPHA_ARGUMENT_KINDS: [GeneralTypedArgumentKindV3; 3] = [
    GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
];
const ALPHA_ARGUMENT_NAMES: [&str; 3] = ["scale", "input", "output"];
const ZETA_ARGUMENT_KINDS: [GeneralTypedArgumentKindV3; 4] = [
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
];
const ZETA_ARGUMENT_NAMES: [&str; 4] = ["a", "b", "bias", "output"];
const SCALAR_GEMM_V1_ARGUMENT_KINDS: [GeneralTypedArgumentKindV3; 6] = [
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::SharedSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::DisjointSlice(RustScalarElementTypeV1::F32),
    GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U32),
    GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U32),
    GeneralTypedArgumentKindV3::Scalar(RustScalarElementTypeV1::U32),
];
const SCALAR_GEMM_V1_ARGUMENT_NAMES: [&str; 6] = ["a", "b", "c", "m", "n", "k"];

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum GeneralTypedArgumentKindV3 {
    Scalar(RustScalarElementTypeV1),
    SharedSlice(RustScalarElementTypeV1),
    DisjointSlice(RustScalarElementTypeV1),
}

impl GeneralTypedArgumentKindV3 {
    pub(crate) const fn scalar(self) -> RustScalarElementTypeV1 {
        match self {
            Self::Scalar(scalar) | Self::SharedSlice(scalar) | Self::DisjointSlice(scalar) => {
                scalar
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneralTypedArgumentV3 {
    kind: GeneralTypedArgumentKindV3,
    layout: RustLayoutEvidenceV1,
}

impl GeneralTypedArgumentV3 {
    pub(crate) const fn kind(&self) -> GeneralTypedArgumentKindV3 {
        self.kind
    }

    pub(crate) const fn layout(&self) -> &RustLayoutEvidenceV1 {
        &self.layout
    }

    pub(crate) fn type_identity(&self) -> TypeIdentity {
        self.layout.type_identity()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneralTypedKernelContractV3 {
    arguments: Vec<GeneralTypedArgumentV3>,
    abi: AbiLayout,
    launch: LaunchContract,
}

impl GeneralTypedKernelContractV3 {
    pub(crate) fn arguments(&self) -> &[GeneralTypedArgumentV3] {
        &self.arguments
    }

    pub(crate) const fn abi(&self) -> &AbiLayout {
        &self.abi
    }

    pub(crate) const fn launch(&self) -> &LaunchContract {
        &self.launch
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneralTypedExtractError(String);

impl GeneralTypedExtractError {
    fn new(reason: impl Into<String>) -> Self {
        Self(reason.into())
    }
}

impl fmt::Display for GeneralTypedExtractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for GeneralTypedExtractError {}

/// Reconstructs the complete general typed contract from rustc semantic and layout facts.
pub(crate) fn extract_general_typed_kernel_v3<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    logical_name: &str,
    export_name: &str,
    launch: &LaunchContract,
) -> Result<GeneralTypedKernelContractV3, GeneralTypedExtractError> {
    if !matches!(instance.def, InstanceKind::Item(_)) {
        return Err(GeneralTypedExtractError::new(format!(
            "expected an ordinary function item, found {:?}",
            instance.def
        )));
    }
    let def_id = instance.def_id();
    if tcx.generics_of(def_id).count() != 0 || !instance.args.is_empty() {
        return Err(GeneralTypedExtractError::new(
            "general typed kernels must be nongeneric function items",
        ));
    }

    let typing_env = TypingEnv::fully_monomorphized();
    let instance_ty = instance.ty(tcx, typing_env);
    let (signature_def_id, args) = match *instance_ty.kind() {
        TyKind::FnDef(signature_def_id, args) if signature_def_id == def_id => {
            (signature_def_id, args)
        }
        _ => {
            return Err(GeneralTypedExtractError::new(format!(
                "registered target is not its own function definition: `{instance_ty}`"
            )));
        }
    };
    let signature = tcx
        .instantiate_bound_regions_with_erased(tcx.fn_sig(signature_def_id).instantiate(tcx, args));
    if signature.safety != Safety::Safe {
        return Err(GeneralTypedExtractError::new(
            "general typed kernels must be safe functions",
        ));
    }
    if signature.abi != ExternAbi::Rust || signature.c_variadic {
        return Err(GeneralTypedExtractError::new(
            "general typed kernels must use the non-variadic Rust ABI",
        ));
    }
    if signature.output() != tcx.types.unit {
        return Err(GeneralTypedExtractError::new(format!(
            "general typed kernels must return unit, found `{}`",
            signature.output()
        )));
    }
    if signature.inputs().is_empty() {
        return Err(GeneralTypedExtractError::new(
            "general typed kernels require at least one argument",
        ));
    }
    if signature.inputs().len() > MAX_ABI_FIELDS {
        return Err(GeneralTypedExtractError::new(format!(
            "general typed kernel argument count {} exceeds maximum {MAX_ABI_FIELDS}",
            signature.inputs().len()
        )));
    }

    let layout_cx = LayoutCx::new(tcx, typing_env);
    require_64_bit_target(tcx, &layout_cx)?;
    let trusted_index = trusted_index1d_type(tcx)?;
    let mut arguments = Vec::with_capacity(signature.inputs().len());
    for (index, ty) in signature.inputs().iter().copied().enumerate() {
        arguments.push(extract_argument(tcx, &layout_cx, ty, trusted_index, index)?);
    }
    validate_general_typed_launch_v3(logical_name, export_name, &arguments, launch)?;
    let abi = build_abi(logical_name, export_name, &arguments)?;
    Ok(GeneralTypedKernelContractV3 {
        arguments,
        abi,
        launch: launch.clone(),
    })
}

fn validate_general_typed_launch_v3(
    logical_name: &str,
    export_name: &str,
    arguments: &[GeneralTypedArgumentV3],
    launch: &LaunchContract,
) -> Result<(), GeneralTypedExtractError> {
    let BlockSize::Exact(dimensions) = launch.block_size() else {
        return Err(GeneralTypedExtractError::new(
            "general typed V3 requires an exact workgroup",
        ));
    };
    let dimensions = [dimensions.x(), dimensions.y(), dimensions.z()];
    if launch.rank() != 1
        || (dimensions != DEFAULT_WORKGROUP && dimensions != WAVE64_WORKGROUP)
        || launch.max_grid()
            != Dimensions::new(u32::MAX, 1, 1)
                .map_err(|error| GeneralTypedExtractError::new(error.to_string()))?
        || launch.static_shared_memory_bytes() != 0
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(GeneralTypedExtractError::new(
            "general typed V3 supports only exact 64x1x1 or 256x1x1 launch contracts",
        ));
    }
    if exact_argument_names(logical_name, export_name, arguments).is_some()
        && dimensions != DEFAULT_WORKGROUP
    {
        return Err(GeneralTypedExtractError::new(
            "the alpha/zeta and scalar_gemm_v1 V3 profiles require exact 256x1x1 launch",
        ));
    }
    Ok(())
}

fn extract_argument<'tcx>(
    tcx: TyCtxt<'tcx>,
    layout_cx: &LayoutCx<'tcx>,
    ty: Ty<'tcx>,
    trusted_index: Ty<'tcx>,
    index: usize,
) -> Result<GeneralTypedArgumentV3, GeneralTypedExtractError> {
    let argument = || format!("argument {}", index + 1);
    if let Some(scalar) = scalar_type(ty) {
        let layout = scalar_layout(layout_cx, ty, scalar, &argument())?;
        return Ok(GeneralTypedArgumentV3 {
            kind: GeneralTypedArgumentKindV3::Scalar(scalar),
            layout,
        });
    }

    if let TyKind::Ref(_, pointee, HirMutability::Not) = *ty.kind()
        && let TyKind::Slice(element) = *pointee.kind()
    {
        let scalar = scalar_type(element).ok_or_else(|| {
            GeneralTypedExtractError::new(format!(
                "{} has unsupported shared-slice element type `{element}`",
                argument()
            ))
        })?;
        let layout = slice_layout(
            layout_cx,
            ty,
            scalar,
            false,
            RustSourceTypeShapeV1::shared_slice(scalar),
            &argument(),
        )?;
        return Ok(GeneralTypedArgumentV3 {
            kind: GeneralTypedArgumentKindV3::SharedSlice(scalar),
            layout,
        });
    }

    if let TyKind::Adt(definition, args) = *ty.kind() {
        if trusted_device_items::classify(tcx, definition.did())
            != Some(TrustedDeviceItem::DisjointSlice)
        {
            return Err(GeneralTypedExtractError::new(format!(
                "{} uses untrusted or unsupported aggregate type `{ty}`",
                argument()
            )));
        }
        let [Some(element), Some(index_space)] = [
            args.first().and_then(|arg| arg.as_type()),
            args.get(1).and_then(|arg| arg.as_type()),
        ] else {
            return Err(GeneralTypedExtractError::new(format!(
                "{} has malformed genuine DisjointSlice arguments",
                argument()
            )));
        };
        if args.len() != 2 {
            return Err(GeneralTypedExtractError::new(format!(
                "{} has malformed genuine DisjointSlice arguments",
                argument()
            )));
        }
        let index_space = disjoint_index_space_v1(tcx, index_space, trusted_index, &argument())?;
        let scalar = scalar_type(element).ok_or_else(|| {
            GeneralTypedExtractError::new(format!(
                "{} has unsupported DisjointSlice element type `{element}`",
                argument()
            ))
        })?;
        let layout = slice_layout(
            layout_cx,
            ty,
            scalar,
            true,
            RustSourceTypeShapeV1::disjoint_slice(scalar, index_space),
            &argument(),
        )?;
        return Ok(GeneralTypedArgumentV3 {
            kind: GeneralTypedArgumentKindV3::DisjointSlice(scalar),
            layout,
        });
    }

    Err(GeneralTypedExtractError::new(format!(
        "{} has unsupported type `{ty}`; only bounded scalars, shared slices, and genuine DisjointSlice values are accepted",
        argument()
    )))
}

fn disjoint_index_space_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    ty: Ty<'tcx>,
    trusted_index1d: Ty<'tcx>,
    argument: &str,
) -> Result<RustDisjointIndexSpaceV1, GeneralTypedExtractError> {
    if ty == trusted_index1d {
        return Ok(RustDisjointIndexSpaceV1::Index1D);
    }
    let TyKind::Adt(definition, arguments) = *ty.kind() else {
        return Err(GeneralTypedExtractError::new(format!(
            "{argument} uses unsupported disjoint index space `{ty}`"
        )));
    };
    match trusted_device_items::classify(tcx, definition.did()) {
        Some(TrustedDeviceItem::ShiftedIndexSpace) => {
            let Some(base) = arguments.first().and_then(|argument| argument.as_type()) else {
                return Err(GeneralTypedExtractError::new(format!(
                    "{argument} has malformed genuine Shifted index-space arguments"
                )));
            };
            let Some(offset) = arguments.get(1).and_then(|argument| argument.as_const()) else {
                return Err(GeneralTypedExtractError::new(format!(
                    "{argument} has malformed genuine Shifted index-space arguments"
                )));
            };
            if arguments.len() != 2 || base != trusted_index1d {
                return Err(GeneralTypedExtractError::new(format!(
                    "{argument} supports only genuine Shifted<Index1D, N>, found `{ty}`"
                )));
            }
            let offset = offset.try_to_target_usize(tcx).ok_or_else(|| {
                GeneralTypedExtractError::new(format!(
                    "{argument} has a non-value or out-of-range Shifted offset"
                ))
            })?;
            Ok(RustDisjointIndexSpaceV1::ShiftedIndex1D { offset })
        }
        Some(TrustedDeviceItem::GridExclusiveIndexSpace) if arguments.is_empty() => {
            Ok(RustDisjointIndexSpaceV1::GridExclusive)
        }
        Some(TrustedDeviceItem::GridExclusiveIndexSpace) => Err(GeneralTypedExtractError::new(
            format!("{argument} has malformed genuine GridExclusive arguments"),
        )),
        Some(TrustedDeviceItem::BlockedIndexSpace) => {
            let Some(base) = arguments.first().and_then(|argument| argument.as_type()) else {
                return Err(GeneralTypedExtractError::new(format!(
                    "{argument} has malformed genuine Blocked index-space arguments"
                )));
            };
            let lanes = arguments
                .get(1)
                .and_then(|argument| argument.as_const())
                .and_then(|value| value.try_to_target_usize(tcx));
            let elements = arguments
                .get(2)
                .and_then(|argument| argument.as_const())
                .and_then(|value| value.try_to_target_usize(tcx));
            let (Some(lanes), Some(elements)) = (lanes, elements) else {
                return Err(GeneralTypedExtractError::new(format!(
                    "{argument} has non-value or out-of-range Blocked dimensions"
                )));
            };
            if arguments.len() != 3 || base != trusted_index1d || lanes == 0 || elements == 0 {
                return Err(GeneralTypedExtractError::new(format!(
                    "{argument} supports only genuine nonzero Blocked<Index1D, L, E>, found `{ty}`"
                )));
            }
            lanes.checked_mul(elements).ok_or_else(|| {
                GeneralTypedExtractError::new(format!(
                    "{argument} has overflowing Blocked dimensions"
                ))
            })?;
            Ok(RustDisjointIndexSpaceV1::blocked_index_1d(lanes, elements)
                .expect("validated nonzero blocked dimensions"))
        }
        Some(TrustedDeviceItem::Tiled2DIndexSpace) => {
            let Some(base) = arguments.first().and_then(|argument| argument.as_type()) else {
                return Err(GeneralTypedExtractError::new(format!(
                    "{argument} has malformed genuine Tiled2D index-space arguments"
                )));
            };
            let dimensions = (1..5)
                .map(|index| {
                    arguments
                        .get(index)
                        .and_then(|argument| argument.as_const())
                        .and_then(|value| value.try_to_target_usize(tcx))
                })
                .collect::<Option<Vec<_>>>();
            let Some(dimensions) = dimensions else {
                return Err(GeneralTypedExtractError::new(format!(
                    "{argument} has non-value or out-of-range Tiled2D dimensions"
                )));
            };
            if arguments.len() != 5 || base != trusted_index1d {
                return Err(GeneralTypedExtractError::new(format!(
                    "{argument} supports only genuine Tiled2D<Index1D, L, R, C, E>, found `{ty}`"
                )));
            }
            RustDisjointIndexSpaceV1::tiled_2d_index_1d(
                dimensions[0],
                dimensions[1],
                dimensions[2],
                dimensions[3],
            )
            .ok_or_else(|| {
                GeneralTypedExtractError::new(format!(
                    "{argument} has invalid or overflowing Tiled2D geometry `{ty}`"
                ))
            })
        }
        _ => Err(GeneralTypedExtractError::new(format!(
            "{argument} uses unsupported or untrusted disjoint index space `{ty}`"
        ))),
    }
}

fn scalar_type(ty: Ty<'_>) -> Option<RustScalarElementTypeV1> {
    match *ty.kind() {
        TyKind::Int(IntTy::I8) => Some(RustScalarElementTypeV1::I8),
        TyKind::Uint(UintTy::U8) => Some(RustScalarElementTypeV1::U8),
        TyKind::Int(IntTy::I16) => Some(RustScalarElementTypeV1::I16),
        TyKind::Uint(UintTy::U16) => Some(RustScalarElementTypeV1::U16),
        TyKind::Int(IntTy::I32) => Some(RustScalarElementTypeV1::I32),
        TyKind::Uint(UintTy::U32) => Some(RustScalarElementTypeV1::U32),
        TyKind::Int(IntTy::I64) => Some(RustScalarElementTypeV1::I64),
        TyKind::Uint(UintTy::U64) => Some(RustScalarElementTypeV1::U64),
        TyKind::Float(FloatTy::F32) => Some(RustScalarElementTypeV1::F32),
        TyKind::Float(FloatTy::F64) => Some(RustScalarElementTypeV1::F64),
        TyKind::Float(FloatTy::F16 | FloatTy::F128)
        | TyKind::Int(IntTy::Isize | IntTy::I128)
        | TyKind::Uint(UintTy::Usize | UintTy::U128) => None,
        _ => None,
    }
}

fn scalar_layout<'tcx>(
    layout_cx: &LayoutCx<'tcx>,
    ty: Ty<'tcx>,
    scalar: RustScalarElementTypeV1,
    argument: &str,
) -> Result<RustLayoutEvidenceV1, GeneralTypedExtractError> {
    let layout = layout_cx.layout_of(ty).map_err(|error| {
        GeneralTypedExtractError::new(format!("failed to lay out {argument}: {error}"))
    })?;
    let BackendRepr::Scalar(component) = layout.backend_repr else {
        return Err(GeneralTypedExtractError::new(format!(
            "{argument} does not have rustc scalar ABI"
        )));
    };
    if !primitive_matches(component.primitive(), scalar)
        || layout.size.bytes() != scalar.size_bytes()
        || layout.align.abi.bytes() != scalar.size_bytes()
    {
        return Err(GeneralTypedExtractError::new(format!(
            "{argument} scalar layout disagrees with its semantic type"
        )));
    }
    let alignment = u32::try_from(layout.align.abi.bytes())
        .map_err(|_| GeneralTypedExtractError::new(format!("{argument} alignment exceeds u32")))?;
    let component = RustPhysicalComponentV1::new(
        0,
        layout.size.bytes(),
        alignment,
        RustPhysicalComponentKindV1::Scalar { scalar },
    )
    .map_err(|error| {
        GeneralTypedExtractError::new(format!("invalid {argument} evidence: {error}"))
    })?;
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::scalar(scalar)),
        RustcAbiClassV1::Scalar,
        PointerWidth::Bits64,
        layout.size.bytes(),
        alignment,
        vec![component],
    )
    .map_err(|error| GeneralTypedExtractError::new(format!("invalid {argument} evidence: {error}")))
}

fn primitive_matches(primitive: Primitive, scalar: RustScalarElementTypeV1) -> bool {
    match (primitive, scalar) {
        (Primitive::Int(integer, true), RustScalarElementTypeV1::I8) => integer.size().bits() == 8,
        (Primitive::Int(integer, false), RustScalarElementTypeV1::U8) => integer.size().bits() == 8,
        (Primitive::Int(integer, true), RustScalarElementTypeV1::I16) => {
            integer.size().bits() == 16
        }
        (Primitive::Int(integer, false), RustScalarElementTypeV1::U16) => {
            integer.size().bits() == 16
        }
        (Primitive::Int(integer, true), RustScalarElementTypeV1::I32) => {
            integer.size().bits() == 32
        }
        (Primitive::Int(integer, false), RustScalarElementTypeV1::U32) => {
            integer.size().bits() == 32
        }
        (Primitive::Int(integer, true), RustScalarElementTypeV1::I64) => {
            integer.size().bits() == 64
        }
        (Primitive::Int(integer, false), RustScalarElementTypeV1::U64) => {
            integer.size().bits() == 64
        }
        (Primitive::Float(float), RustScalarElementTypeV1::F32) => float.size().bits() == 32,
        (Primitive::Float(float), RustScalarElementTypeV1::F64) => float.size().bits() == 64,
        _ => false,
    }
}

fn slice_layout<'tcx>(
    layout_cx: &LayoutCx<'tcx>,
    ty: Ty<'tcx>,
    scalar: RustScalarElementTypeV1,
    mutable: bool,
    source_type: RustSourceTypeShapeV1,
    argument: &str,
) -> Result<RustLayoutEvidenceV1, GeneralTypedExtractError> {
    let layout = layout_cx.layout_of(ty).map_err(|error| {
        GeneralTypedExtractError::new(format!("failed to lay out {argument}: {error}"))
    })?;
    let BackendRepr::ScalarPair(pointer, length) = layout.backend_repr else {
        return Err(GeneralTypedExtractError::new(format!(
            "{argument} does not have rustc scalar-pair ABI"
        )));
    };
    let pointer_ok = matches!(pointer.primitive(), Primitive::Pointer(address_space) if address_space.0 == 0)
        && pointer.size(layout_cx).bytes() == POINTER_BYTES
        && pointer.align(layout_cx).abi.bytes() == u64::from(POINTER_ALIGNMENT);
    let length_ok = matches!(length.primitive(), Primitive::Int(integer, false) if integer.size().bits() == 64)
        && length.size(layout_cx).bytes() == POINTER_BYTES
        && length.align(layout_cx).abi.bytes() == u64::from(POINTER_ALIGNMENT);
    if !pointer_ok
        || !length_ok
        || layout.size.bytes() != SLICE_BYTES
        || layout.align.abi.bytes() != u64::from(POINTER_ALIGNMENT)
    {
        return Err(GeneralTypedExtractError::new(format!(
            "{argument} does not have the exact 64-bit pointer/usize slice ABI"
        )));
    }
    let pointer_component = RustPhysicalComponentV1::new(
        0,
        POINTER_BYTES,
        POINTER_ALIGNMENT,
        RustPhysicalComponentKindV1::Pointer {
            mutability: if mutable {
                RustPointerMutabilityV1::Mut
            } else {
                RustPointerMutabilityV1::Const
            },
            pointee: scalar,
        },
    )
    .map_err(|error| {
        GeneralTypedExtractError::new(format!("invalid {argument} evidence: {error}"))
    })?;
    let length_component = RustPhysicalComponentV1::new(
        POINTER_BYTES,
        POINTER_BYTES,
        POINTER_ALIGNMENT,
        RustPhysicalComponentKindV1::Usize,
    )
    .map_err(|error| {
        GeneralTypedExtractError::new(format!("invalid {argument} evidence: {error}"))
    })?;
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(source_type),
        RustcAbiClassV1::ScalarPair,
        PointerWidth::Bits64,
        SLICE_BYTES,
        POINTER_ALIGNMENT,
        vec![pointer_component, length_component],
    )
    .map_err(|error| GeneralTypedExtractError::new(format!("invalid {argument} evidence: {error}")))
}

fn build_abi(
    logical_name: &str,
    export_name: &str,
    arguments: &[GeneralTypedArgumentV3],
) -> Result<AbiLayout, GeneralTypedExtractError> {
    let exact_names = exact_argument_names(logical_name, export_name, arguments);
    let mut offset = 0_u64;
    let mut layout_alignment = 1_u32;
    let mut fields = Vec::with_capacity(arguments.len());
    for (index, argument) in arguments.iter().enumerate() {
        let (size, alignment) = argument_size_alignment(argument.kind);
        offset = align_up(offset, alignment)
            .ok_or_else(|| GeneralTypedExtractError::new("general typed ABI offset overflow"))?;
        let name =
            exact_names.map_or_else(|| format!("arg{index}"), |names| names[index].to_owned());
        fields.push(build_abi_field(&name, index, offset, argument)?);
        offset = offset
            .checked_add(size)
            .ok_or_else(|| GeneralTypedExtractError::new("general typed ABI size overflow"))?;
        layout_alignment = layout_alignment.max(alignment);
    }
    let size = align_up(offset, layout_alignment)
        .ok_or_else(|| GeneralTypedExtractError::new("general typed ABI tail padding overflow"))?;
    AbiLayout::new(size, layout_alignment, PointerWidth::Bits64, fields).map_err(|error| {
        GeneralTypedExtractError::new(format!("invalid general typed ABI: {error}"))
    })
}

fn exact_argument_names(
    logical_name: &str,
    export_name: &str,
    arguments: &[GeneralTypedArgumentV3],
) -> Option<&'static [&'static str]> {
    let argument_kinds = arguments
        .iter()
        .map(|argument| argument.kind())
        .collect::<Vec<_>>();
    match (logical_name, export_name) {
        ("alpha", "alpha") if argument_kinds == ALPHA_ARGUMENT_KINDS => {
            Some(ALPHA_ARGUMENT_NAMES.as_slice())
        }
        ("zeta", "zeta") if argument_kinds == ZETA_ARGUMENT_KINDS => {
            Some(ZETA_ARGUMENT_NAMES.as_slice())
        }
        ("scalar_gemm_v1", "scalar_gemm_v1") if argument_kinds == SCALAR_GEMM_V1_ARGUMENT_KINDS => {
            Some(SCALAR_GEMM_V1_ARGUMENT_NAMES.as_slice())
        }
        _ => None,
    }
}

fn build_abi_field(
    name: &str,
    index: usize,
    offset: u64,
    argument: &GeneralTypedArgumentV3,
) -> Result<AbiField, GeneralTypedExtractError> {
    let scalar = argument.kind.scalar();
    let (size, alignment, kind, mutability, access, address_space, ownership, alias) =
        match argument.kind {
            GeneralTypedArgumentKindV3::Scalar(_) => (
                scalar.size_bytes(),
                scalar.size_bytes() as u32,
                AbiKind::Scalar(artifact_scalar(scalar)),
                Mutability::Immutable,
                Access::ByValue,
                AddressSpace::Value,
                ArgumentOwnership::ByValue,
                AliasClass::Value,
            ),
            GeneralTypedArgumentKindV3::SharedSlice(_) => (
                SLICE_BYTES,
                POINTER_ALIGNMENT,
                AbiKind::Slice {
                    element_size: scalar.size_bytes(),
                    element_alignment: scalar.size_bytes() as u32,
                },
                Mutability::Immutable,
                Access::ReadOnly,
                AddressSpace::Global,
                ArgumentOwnership::SharedBorrow,
                AliasClass::SharedReadOnly,
            ),
            GeneralTypedArgumentKindV3::DisjointSlice(_) => (
                SLICE_BYTES,
                POINTER_ALIGNMENT,
                AbiKind::Slice {
                    element_size: scalar.size_bytes(),
                    element_alignment: scalar.size_bytes() as u32,
                },
                Mutability::Mutable,
                Access::ReadWrite,
                AddressSpace::Global,
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            ),
        };
    AbiField::new(
        Name::new(name).map_err(|error| GeneralTypedExtractError::new(error.to_string()))?,
        offset,
        size,
        alignment,
        kind,
        mutability,
        access,
        address_space,
        argument.type_identity(),
        ownership,
        alias,
    )
    .map_err(|error| GeneralTypedExtractError::new(format!("invalid argument {index}: {error}")))
}

fn artifact_scalar(scalar: RustScalarElementTypeV1) -> ScalarType {
    match scalar {
        RustScalarElementTypeV1::I8 => ScalarType::I8,
        RustScalarElementTypeV1::U8 => ScalarType::U8,
        RustScalarElementTypeV1::I16 => ScalarType::I16,
        RustScalarElementTypeV1::U16 => ScalarType::U16,
        RustScalarElementTypeV1::I32 => ScalarType::I32,
        RustScalarElementTypeV1::U32 => ScalarType::U32,
        RustScalarElementTypeV1::I64 => ScalarType::I64,
        RustScalarElementTypeV1::U64 => ScalarType::U64,
        RustScalarElementTypeV1::F32 => ScalarType::F32,
        RustScalarElementTypeV1::F64 => ScalarType::F64,
        RustScalarElementTypeV1::F16 => unreachable!("general typed V3 rejects f16"),
        _ => unreachable!("unknown scalar schema is not admitted by general typed V3"),
    }
}

fn argument_size_alignment(kind: GeneralTypedArgumentKindV3) -> (u64, u32) {
    match kind {
        GeneralTypedArgumentKindV3::Scalar(scalar) => {
            (scalar.size_bytes(), scalar.size_bytes() as u32)
        }
        GeneralTypedArgumentKindV3::SharedSlice(_)
        | GeneralTypedArgumentKindV3::DisjointSlice(_) => (SLICE_BYTES, POINTER_ALIGNMENT),
    }
}

fn align_up(value: u64, alignment: u32) -> Option<u64> {
    let mask = u64::from(alignment) - 1;
    value.checked_add(mask).map(|value| value & !mask)
}

fn require_64_bit_target<'tcx>(
    tcx: TyCtxt<'tcx>,
    layout_cx: &LayoutCx<'tcx>,
) -> Result<(), GeneralTypedExtractError> {
    if layout_cx.data_layout().pointer_size().bits() != 64 {
        return Err(GeneralTypedExtractError::new(
            "general typed V3 requires 64-bit pointers",
        ));
    }
    let usize_layout = layout_cx.layout_of(tcx.types.usize).map_err(|error| {
        GeneralTypedExtractError::new(format!("failed to lay out usize: {error}"))
    })?;
    if usize_layout.size.bytes() != POINTER_BYTES
        || !matches!(usize_layout.backend_repr, BackendRepr::Scalar(scalar) if matches!(scalar.primitive(), Primitive::Int(integer, false) if integer.size().bits() == 64))
    {
        return Err(GeneralTypedExtractError::new(
            "general typed V3 requires unsigned 64-bit usize",
        ));
    }
    Ok(())
}

fn trusted_index1d_type<'tcx>(tcx: TyCtxt<'tcx>) -> Result<Ty<'tcx>, GeneralTypedExtractError> {
    let marker = tcx
        .get_diagnostic_item(Symbol::intern(INDEX_1D_DIAGNOSTIC_ITEM))
        .ok_or_else(|| {
            GeneralTypedExtractError::new(format!(
                "missing trusted diagnostic item `{INDEX_1D_DIAGNOSTIC_ITEM}`"
            ))
        })?;
    if tcx.def_kind(marker) != DefKind::Fn {
        return Err(GeneralTypedExtractError::new(
            "Index1D diagnostic item does not resolve to a function",
        ));
    }
    if trusted_device_items::classify(tcx, marker) != Some(TrustedDeviceItem::ThreadIndex1d) {
        let reason = trusted_device_items::rejected_provider(tcx, marker).map_or_else(
            || "provider is not registered as trusted".to_owned(),
            |rejection| rejection.reason,
        );
        return Err(GeneralTypedExtractError::new(format!(
            "Index1D diagnostic item does not resolve to the trusted function: {reason}"
        )));
    }
    let signature =
        tcx.instantiate_bound_regions_with_erased(tcx.fn_sig(marker).instantiate_identity());
    if !signature.inputs().is_empty()
        || signature.safety != Safety::Safe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
    {
        return Err(GeneralTypedExtractError::new(
            "trusted Index1D function has an unexpected signature",
        ));
    }
    let TyKind::Adt(thread_index, args) = *signature.output().kind() else {
        return Err(GeneralTypedExtractError::new(
            "trusted Index1D function has an unexpected return type",
        ));
    };
    if trusted_device_items::classify(tcx, thread_index.did())
        != Some(TrustedDeviceItem::ThreadIndex)
        || args.len() != 1
    {
        return Err(GeneralTypedExtractError::new(
            "trusted Index1D return type is not the trusted ThreadIndex",
        ));
    }
    args.first()
        .and_then(|arg| arg.as_type())
        .ok_or_else(|| GeneralTypedExtractError::new("trusted Index1D type argument is missing"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_artifacts::{DigestBytes, derive_generated_host_contract_identity_v1};
    use reserved_fe2o3_symbols::{
        GeneratedHostContractIdV3, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
    };

    fn argument(kind: GeneralTypedArgumentKindV3) -> GeneralTypedArgumentV3 {
        let scalar = kind.scalar();
        let (source_type, abi_class, size, alignment, components) = match kind {
            GeneralTypedArgumentKindV3::Scalar(_) => (
                RustSourceTypeShapeV1::scalar(scalar),
                RustcAbiClassV1::Scalar,
                scalar.size_bytes(),
                scalar.size_bytes() as u32,
                vec![
                    RustPhysicalComponentV1::new(
                        0,
                        scalar.size_bytes(),
                        scalar.size_bytes() as u32,
                        RustPhysicalComponentKindV1::Scalar { scalar },
                    )
                    .unwrap(),
                ],
            ),
            GeneralTypedArgumentKindV3::SharedSlice(_) => (
                RustSourceTypeShapeV1::shared_slice(scalar),
                RustcAbiClassV1::ScalarPair,
                16,
                8,
                slice_components(scalar, RustPointerMutabilityV1::Const),
            ),
            GeneralTypedArgumentKindV3::DisjointSlice(_) => (
                RustSourceTypeShapeV1::disjoint_slice(scalar, RustDisjointIndexSpaceV1::Index1D),
                RustcAbiClassV1::ScalarPair,
                16,
                8,
                slice_components(scalar, RustPointerMutabilityV1::Mut),
            ),
        };
        GeneralTypedArgumentV3 {
            kind,
            layout: RustLayoutEvidenceV1::new(
                RustTypeEvidenceV1::new(source_type),
                abi_class,
                PointerWidth::Bits64,
                size,
                alignment,
                components,
            )
            .unwrap(),
        }
    }

    fn slice_components(
        scalar: RustScalarElementTypeV1,
        mutability: RustPointerMutabilityV1,
    ) -> Vec<RustPhysicalComponentV1> {
        vec![
            RustPhysicalComponentV1::new(
                0,
                8,
                8,
                RustPhysicalComponentKindV1::Pointer {
                    mutability,
                    pointee: scalar,
                },
            )
            .unwrap(),
            RustPhysicalComponentV1::new(8, 8, 8, RustPhysicalComponentKindV1::Usize).unwrap(),
        ]
    }

    fn alpha_arguments() -> Vec<GeneralTypedArgumentV3> {
        vec![
            argument(GeneralTypedArgumentKindV3::Scalar(
                RustScalarElementTypeV1::F32,
            )),
            argument(GeneralTypedArgumentKindV3::SharedSlice(
                RustScalarElementTypeV1::F32,
            )),
            argument(GeneralTypedArgumentKindV3::DisjointSlice(
                RustScalarElementTypeV1::F32,
            )),
        ]
    }

    fn zeta_arguments() -> Vec<GeneralTypedArgumentV3> {
        vec![
            argument(GeneralTypedArgumentKindV3::SharedSlice(
                RustScalarElementTypeV1::F32,
            )),
            argument(GeneralTypedArgumentKindV3::SharedSlice(
                RustScalarElementTypeV1::F32,
            )),
            argument(GeneralTypedArgumentKindV3::Scalar(
                RustScalarElementTypeV1::F32,
            )),
            argument(GeneralTypedArgumentKindV3::DisjointSlice(
                RustScalarElementTypeV1::F32,
            )),
        ]
    }

    fn scalar_gemm_v1_arguments() -> Vec<GeneralTypedArgumentV3> {
        SCALAR_GEMM_V1_ARGUMENT_KINDS
            .into_iter()
            .map(argument)
            .collect()
    }

    fn launch() -> LaunchContract {
        LaunchContract::new(
            1,
            BlockSize::Exact(Dimensions::new(256, 1, 1).unwrap()),
            Dimensions::new(u32::MAX, 1, 1).unwrap(),
            0,
            0,
        )
        .unwrap()
    }

    fn identity(name: &str, arguments: &[GeneralTypedArgumentV3]) -> DigestBytes {
        let abi = build_abi(name, name, arguments).unwrap();
        identity_with_abi(name, &abi)
    }

    fn identity_with_abi(name: &str, abi: &AbiLayout) -> DigestBytes {
        derive_generated_host_contract_identity_v1(
            MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
            [0x42; 32],
            name,
            name,
            abi,
            &launch(),
        )
    }

    #[test]
    fn alpha_and_zeta_reconstruct_exact_offsets_sizes_and_effects() {
        let alpha = build_abi("alpha", "alpha", &alpha_arguments()).unwrap();
        assert_eq!(alpha.size(), 40);
        assert_eq!(alpha.alignment(), 8);
        assert_eq!(
            alpha
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["scale", "input", "output"]
        );
        assert_eq!(
            alpha
                .fields()
                .iter()
                .map(|field| field.offset())
                .collect::<Vec<_>>(),
            [0, 8, 24]
        );
        assert_eq!(alpha.fields()[2].access(), Access::ReadWrite);

        let zeta = build_abi("zeta", "zeta", &zeta_arguments()).unwrap();
        assert_eq!(zeta.size(), 56);
        assert_eq!(
            zeta.fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "bias", "output"]
        );
        assert_eq!(
            zeta.fields()
                .iter()
                .map(|field| field.offset())
                .collect::<Vec<_>>(),
            [0, 16, 32, 40]
        );
        assert_eq!(zeta.fields()[3].access(), Access::ReadWrite);
    }

    #[test]
    fn scalar_gemm_v1_reconstructs_the_normative_explicit_cov6_abi() {
        let abi = build_abi(
            "scalar_gemm_v1",
            "scalar_gemm_v1",
            &scalar_gemm_v1_arguments(),
        )
        .unwrap();
        assert_eq!(abi.size(), 64);
        assert_eq!(abi.alignment(), 8);
        assert_eq!(
            abi.fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["a", "b", "c", "m", "n", "k"]
        );
        assert_eq!(
            abi.fields()
                .iter()
                .map(|field| field.offset())
                .collect::<Vec<_>>(),
            [0, 16, 32, 48, 52, 56]
        );
        assert_eq!(
            abi.fields()
                .iter()
                .map(|field| field.size())
                .collect::<Vec<_>>(),
            [16, 16, 16, 4, 4, 4]
        );
        assert_eq!(abi.fields()[0].access(), Access::ReadOnly);
        assert_eq!(abi.fields()[1].access(), Access::ReadOnly);
        assert_eq!(abi.fields()[2].access(), Access::ReadWrite);
    }

    #[test]
    fn host_contract_identity_is_order_and_scalar_sensitive() {
        let alpha = alpha_arguments();
        let alpha_identity = identity("alpha", &alpha);
        let generic_alpha_abi = build_abi("alpha_lookalike", "alpha_lookalike", &alpha).unwrap();
        assert_ne!(
            alpha_identity,
            identity_with_abi("alpha", &generic_alpha_abi),
            "ABI field names must be bound into the generated host contract identity"
        );
        let zeta = zeta_arguments();
        let zeta_identity = identity("zeta", &zeta);
        let generic_zeta_abi = build_abi("zeta_lookalike", "zeta_lookalike", &zeta).unwrap();
        assert_ne!(
            zeta_identity,
            identity_with_abi("zeta", &generic_zeta_abi),
            "zeta ABI field names must be bound into the generated host contract identity"
        );
        let declared = GeneratedHostContractIdV3::from_bytes(*alpha_identity.as_bytes());
        assert_eq!(declared.as_bytes(), *alpha_identity.as_bytes());
        let mut wrong_identity = declared.as_bytes();
        wrong_identity[31] ^= 1;
        assert_ne!(wrong_identity, *alpha_identity.as_bytes());

        let mut reordered = alpha.clone();
        reordered.swap(0, 1);
        assert_ne!(alpha_identity, identity("alpha", &reordered));

        let mut scalar_mutation = alpha;
        scalar_mutation[0] = argument(GeneralTypedArgumentKindV3::Scalar(
            RustScalarElementTypeV1::F64,
        ));
        assert_ne!(alpha_identity, identity("alpha", &scalar_mutation));

        let mut layout_mutation = alpha_arguments();
        layout_mutation[1].layout = argument(GeneralTypedArgumentKindV3::DisjointSlice(
            RustScalarElementTypeV1::F32,
        ))
        .layout;
        assert_ne!(alpha_identity, identity("alpha", &layout_mutation));
        assert_ne!(alpha_identity, identity("renamed", &alpha_arguments()));
    }

    #[test]
    fn lookalike_general_v3_contracts_keep_positional_names() {
        let alpha_lookalike =
            build_abi("alpha_lookalike", "alpha_lookalike", &alpha_arguments()).unwrap();
        assert_eq!(
            alpha_lookalike
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2"]
        );

        let zeta_lookalike =
            build_abi("zeta_lookalike", "zeta_lookalike", &zeta_arguments()).unwrap();
        assert_eq!(
            zeta_lookalike
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2", "arg3"]
        );

        let mut same_abi_shape = alpha_arguments();
        same_abi_shape[0] = argument(GeneralTypedArgumentKindV3::Scalar(
            RustScalarElementTypeV1::U32,
        ));
        let wrong_alpha_signature = build_abi("alpha", "alpha", &same_abi_shape).unwrap();
        assert_eq!(
            wrong_alpha_signature
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2"]
        );

        let mut reordered_zeta = zeta_arguments();
        reordered_zeta.swap(0, 2);
        let wrong_zeta_signature = build_abi("zeta", "zeta", &reordered_zeta).unwrap();
        assert_eq!(
            wrong_zeta_signature
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2", "arg3"]
        );

        for (logical_name, export_name) in [("alpha", "renamed"), ("renamed", "alpha")] {
            let mismatched = build_abi(logical_name, export_name, &alpha_arguments()).unwrap();
            assert_eq!(
                mismatched
                    .fields()
                    .iter()
                    .map(|field| field.name().as_str())
                    .collect::<Vec<_>>(),
                ["arg0", "arg1", "arg2"]
            );
        }
    }
}
