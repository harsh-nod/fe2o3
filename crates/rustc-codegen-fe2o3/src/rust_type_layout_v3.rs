//! Compiler-authoritative Rust type/layout facts for the bounded general typed profile.

use std::fmt;

use fe2o3_artifacts::{
    AbiField, AbiKind, AbiLayout, Access, AddressSpace, AliasClass, ArgumentOwnership, BlockSize,
    LaunchContract, MAX_ABI_FIELDS, Mutability, Name, PointerWidth, RustDisjointIndexSpaceV1,
    RustLayoutEvidenceV1, RustPhysicalComponentKindV1, RustPhysicalComponentV1,
    RustPointerMutabilityV1, RustScalarElementTypeV1, RustSourceTypeShapeV1, RustTypeEvidenceV1,
    RustcAbiClassV1, ScalarType, TypeIdentity,
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
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub(crate) enum GeneralTypedArgumentKindV3 {
    Scalar(RustScalarElementTypeV1),
    SharedSlice(RustScalarElementTypeV1),
    WriteOnlyDisjointSlice(RustScalarElementTypeV1),
    DisjointSlice(RustScalarElementTypeV1),
    GlobalMutPointer(RustScalarElementTypeV1),
    CompilerLaidOutByValue,
}

impl GeneralTypedArgumentKindV3 {
    pub(crate) const fn scalar(self) -> Option<RustScalarElementTypeV1> {
        match self {
            Self::Scalar(scalar)
            | Self::SharedSlice(scalar)
            | Self::WriteOnlyDisjointSlice(scalar)
            | Self::DisjointSlice(scalar)
            | Self::GlobalMutPointer(scalar) => Some(scalar),
            Self::CompilerLaidOutByValue => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct GeneralTypedArgumentV3 {
    kind: GeneralTypedArgumentKindV3,
    layout: Option<RustLayoutEvidenceV1>,
    size: u64,
    alignment: u32,
    abi_class: RustcAbiClassV1,
}

impl GeneralTypedArgumentV3 {
    fn from_layout(kind: GeneralTypedArgumentKindV3, layout: RustLayoutEvidenceV1) -> Self {
        Self {
            kind,
            size: layout.size(),
            alignment: layout.abi_alignment(),
            abi_class: layout.abi_class(),
            layout: Some(layout),
        }
    }

    pub(crate) const fn kind(&self) -> GeneralTypedArgumentKindV3 {
        self.kind
    }

    pub(crate) const fn layout(&self) -> Option<&RustLayoutEvidenceV1> {
        self.layout.as_ref()
    }

    pub(crate) const fn size(&self) -> u64 {
        self.size
    }

    pub(crate) const fn alignment(&self) -> u32 {
        self.alignment
    }

    pub(crate) const fn abi_class(&self) -> RustcAbiClassV1 {
        self.abi_class
    }

    pub(crate) fn type_identity(&self) -> Option<TypeIdentity> {
        self.layout
            .as_ref()
            .map(RustLayoutEvidenceV1::type_identity)
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

    pub(crate) fn layout_deferred(&self) -> bool {
        self.arguments
            .iter()
            .any(|argument| argument.kind == GeneralTypedArgumentKindV3::CompilerLaidOutByValue)
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
    validate_general_typed_launch_v3(launch)?;
    let abi = if arguments
        .iter()
        .any(|argument| argument.kind == GeneralTypedArgumentKindV3::CompilerLaidOutByValue)
    {
        AbiLayout::new(0, 1, PointerWidth::Bits64, Vec::new()).map_err(|error| {
            GeneralTypedExtractError::new(format!(
                "invalid layout-deferred registration ABI: {error}"
            ))
        })?
    } else {
        build_abi(&arguments)?
    };
    Ok(GeneralTypedKernelContractV3 {
        arguments,
        abi,
        launch: launch.clone(),
    })
}

fn validate_general_typed_launch_v3(
    launch: &LaunchContract,
) -> Result<(), GeneralTypedExtractError> {
    let BlockSize::Exact(dimensions) = launch.block_size() else {
        return Err(GeneralTypedExtractError::new(
            "general typed V3 requires an exact workgroup",
        ));
    };
    let dimensions = [dimensions.x(), dimensions.y(), dimensions.z()];
    let flat_workgroup_size = dimensions
        .into_iter()
        .try_fold(1_u32, u32::checked_mul)
        .ok_or_else(|| GeneralTypedExtractError::new("exact workgroup size overflows u32"))?;
    if (launch.rank() < 2 && dimensions[1] != 1)
        || (launch.rank() < 3 && dimensions[2] != 1)
        || flat_workgroup_size > fe2o3_mir_model::semantic_mir_v1::MAX_SEMANTIC_WORKGROUP_THREADS_V1
        || launch.max_dynamic_shared_memory_bytes() != 0
    {
        return Err(GeneralTypedExtractError::new(
            "general typed V3 requires a target-sized exact XYZ workgroup and no dynamic shared memory",
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
        return Ok(GeneralTypedArgumentV3::from_layout(
            GeneralTypedArgumentKindV3::Scalar(scalar),
            layout,
        ));
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
        return Ok(GeneralTypedArgumentV3::from_layout(
            GeneralTypedArgumentKindV3::SharedSlice(scalar),
            layout,
        ));
    }

    if let TyKind::Adt(definition, args) = *ty.kind() {
        let trusted = trusted_device_items::classify(tcx, definition.did());
        if trusted == Some(TrustedDeviceItem::DeviceGlobalMutPtr) {
            let Some(element) = args.first().and_then(|argument| argument.as_type()) else {
                return Err(GeneralTypedExtractError::new(format!(
                    "{} has malformed genuine DeviceGlobalMutPtr arguments",
                    argument()
                )));
            };
            if args.len() != 1 {
                return Err(GeneralTypedExtractError::new(format!(
                    "{} has malformed genuine DeviceGlobalMutPtr arguments",
                    argument()
                )));
            }
            let scalar = scalar_type(element).ok_or_else(|| {
                GeneralTypedExtractError::new(format!(
                    "{} has unsupported DeviceGlobalMutPtr element type `{element}`",
                    argument()
                ))
            })?;
            let layout = global_mut_pointer_layout(layout_cx, ty, scalar, &argument())?;
            return Ok(GeneralTypedArgumentV3::from_layout(
                GeneralTypedArgumentKindV3::GlobalMutPointer(scalar),
                layout,
            ));
        }
        let write_only = trusted == Some(TrustedDeviceItem::WriteOnlyDisjointSlice);
        if !write_only && trusted != Some(TrustedDeviceItem::DisjointSlice) {
            return compiler_laid_out_by_value_argument(tcx, layout_cx, ty, &argument());
        }
        let [Some(element), Some(index_space)] = [
            args.first().and_then(|arg| arg.as_type()),
            args.get(1).and_then(|arg| arg.as_type()),
        ] else {
            return Err(GeneralTypedExtractError::new(format!(
                "{} has malformed genuine disjoint-slice arguments",
                argument()
            )));
        };
        if args.len() != 2 {
            return Err(GeneralTypedExtractError::new(format!(
                "{} has malformed genuine disjoint-slice arguments",
                argument()
            )));
        }
        let index_space = disjoint_index_space_v1(tcx, index_space, trusted_index, &argument())?;
        let scalar = scalar_type(element).ok_or_else(|| {
            GeneralTypedExtractError::new(format!(
                "{} has unsupported disjoint-slice element type `{element}`",
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
        return Ok(GeneralTypedArgumentV3::from_layout(
            if write_only {
                GeneralTypedArgumentKindV3::WriteOnlyDisjointSlice(scalar)
            } else {
                GeneralTypedArgumentKindV3::DisjointSlice(scalar)
            },
            layout,
        ));
    }

    if matches!(ty.kind(), TyKind::Tuple(_) | TyKind::Array(..)) {
        return compiler_laid_out_by_value_argument(tcx, layout_cx, ty, &argument());
    }

    Err(GeneralTypedExtractError::new(format!(
        "{} has unsupported type `{ty}`; only bounded scalars, shared slices, genuine DisjointSlice or WriteOnlyDisjointSlice values, and genuine DeviceGlobalMutPtr values are accepted",
        argument()
    )))
}

fn compiler_laid_out_by_value_argument<'tcx>(
    tcx: TyCtxt<'tcx>,
    layout_cx: &LayoutCx<'tcx>,
    ty: Ty<'tcx>,
    argument: &str,
) -> Result<GeneralTypedArgumentV3, GeneralTypedExtractError> {
    fn validate<'tcx>(
        tcx: TyCtxt<'tcx>,
        ty: Ty<'tcx>,
        nodes: &mut usize,
    ) -> Result<(), GeneralTypedExtractError> {
        *nodes = nodes.checked_add(1).ok_or_else(|| {
            GeneralTypedExtractError::new("by-value aggregate type graph overflows")
        })?;
        if *nodes > MAX_ABI_FIELDS.saturating_mul(4) {
            return Err(GeneralTypedExtractError::new(
                "by-value aggregate type graph exceeds the bounded component domain",
            ));
        }
        if scalar_type(ty).is_some() || matches!(ty.kind(), TyKind::Bool | TyKind::Char) {
            return Ok(());
        }
        match ty.kind() {
            TyKind::Tuple(fields) => {
                for field in fields.iter() {
                    validate(tcx, field, nodes)?;
                }
                Ok(())
            }
            TyKind::Array(element, length) => {
                let length = length.try_to_target_usize(tcx).ok_or_else(|| {
                    GeneralTypedExtractError::new(
                        "by-value array length is not an exact target usize",
                    )
                })?;
                if length > (MAX_ABI_FIELDS * 4) as u64 {
                    return Err(GeneralTypedExtractError::new(
                        "by-value array exceeds the bounded component domain",
                    ));
                }
                for _ in 0..length {
                    validate(tcx, *element, nodes)?;
                }
                Ok(())
            }
            TyKind::Adt(definition, arguments) if definition.is_struct() => {
                let variant = definition.non_enum_variant();
                for field in &variant.fields {
                    validate(tcx, field.ty(tcx, arguments), nodes)?;
                }
                Ok(())
            }
            TyKind::Adt(definition, _) if definition.is_enum() => {
                Err(GeneralTypedExtractError::new(
                    "by-value enum arguments require variant-aware packing evidence",
                ))
            }
            TyKind::Adt(definition, _) if definition.is_union() => Err(
                GeneralTypedExtractError::new("by-value union arguments are unsupported"),
            ),
            TyKind::Ref(..) | TyKind::RawPtr(..) => Err(GeneralTypedExtractError::new(
                "by-value aggregate contains a pointer or reference",
            )),
            _ => Err(GeneralTypedExtractError::new(format!(
                "by-value aggregate contains unsupported field type `{ty}`"
            ))),
        }
    }

    if ty.needs_drop(tcx, TypingEnv::fully_monomorphized()) {
        return Err(GeneralTypedExtractError::new(format!(
            "{argument} requires drop and cannot be scalarized as inert by-value input"
        )));
    }
    let mut nodes = 0;
    validate(tcx, ty, &mut nodes).map_err(|error| {
        GeneralTypedExtractError::new(format!("{argument} is not admitted: {error}"))
    })?;
    let layout = layout_cx.layout_of(ty).map_err(|error| {
        GeneralTypedExtractError::new(format!("failed to lay out {argument}: {error}"))
    })?;
    let abi_class = match layout.backend_repr {
        BackendRepr::Scalar(_) => RustcAbiClassV1::Scalar,
        BackendRepr::ScalarPair(..) => RustcAbiClassV1::ScalarPair,
        BackendRepr::Memory { sized: true } => RustcAbiClassV1::Aggregate,
        BackendRepr::Memory { sized: false } => {
            return Err(GeneralTypedExtractError::new(format!(
                "{argument} has an unsized aggregate ABI"
            )));
        }
        _ => {
            return Err(GeneralTypedExtractError::new(format!(
                "{argument} requires an unsupported vector aggregate ABI"
            )));
        }
    };
    let alignment = u32::try_from(layout.align.abi.bytes())
        .map_err(|_| GeneralTypedExtractError::new(format!("{argument} alignment exceeds u32")))?;
    Ok(GeneralTypedArgumentV3 {
        kind: GeneralTypedArgumentKindV3::CompilerLaidOutByValue,
        layout: None,
        size: layout.size.bytes(),
        alignment,
        abi_class,
    })
}

fn global_mut_pointer_layout<'tcx>(
    layout_cx: &LayoutCx<'tcx>,
    ty: Ty<'tcx>,
    scalar: RustScalarElementTypeV1,
    argument: &str,
) -> Result<RustLayoutEvidenceV1, GeneralTypedExtractError> {
    let layout = layout_cx.layout_of(ty).map_err(|error| {
        GeneralTypedExtractError::new(format!("failed to lay out {argument}: {error}"))
    })?;
    let BackendRepr::Scalar(pointer) = layout.backend_repr else {
        return Err(GeneralTypedExtractError::new(format!(
            "{argument} does not have rustc scalar pointer ABI"
        )));
    };
    let pointer_ok = matches!(pointer.primitive(), Primitive::Pointer(address_space) if address_space.0 == 0)
        && pointer.size(layout_cx).bytes() == POINTER_BYTES
        && pointer.align(layout_cx).abi.bytes() == u64::from(POINTER_ALIGNMENT);
    if !pointer_ok
        || layout.size.bytes() != POINTER_BYTES
        || layout.align.abi.bytes() != u64::from(POINTER_ALIGNMENT)
    {
        return Err(GeneralTypedExtractError::new(format!(
            "{argument} does not have the exact 64-bit mutable-pointer ABI"
        )));
    }
    let component = RustPhysicalComponentV1::new(
        0,
        POINTER_BYTES,
        POINTER_ALIGNMENT,
        RustPhysicalComponentKindV1::Pointer {
            mutability: RustPointerMutabilityV1::Mut,
            pointee: scalar,
        },
    )
    .map_err(|error| {
        GeneralTypedExtractError::new(format!("invalid {argument} evidence: {error}"))
    })?;
    RustLayoutEvidenceV1::new(
        RustTypeEvidenceV1::new(RustSourceTypeShapeV1::global_mut_pointer(scalar)),
        RustcAbiClassV1::Scalar,
        PointerWidth::Bits64,
        POINTER_BYTES,
        POINTER_ALIGNMENT,
        vec![component],
    )
    .map_err(|error| GeneralTypedExtractError::new(format!("invalid {argument} evidence: {error}")))
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
        Some(TrustedDeviceItem::RowStriped2DIndexSpace) => {
            let Some(base) = arguments.first().and_then(|argument| argument.as_type()) else {
                return Err(GeneralTypedExtractError::new(format!(
                    "{argument} has malformed genuine RowStriped2D index-space arguments"
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
                    "{argument} has non-value or out-of-range RowStriped2D dimensions"
                )));
            };
            if arguments.len() != 3 || base != trusted_index1d {
                return Err(GeneralTypedExtractError::new(format!(
                    "{argument} supports only genuine RowStriped2D<Index1D, L, E>, found `{ty}`"
                )));
            }
            RustDisjointIndexSpaceV1::row_striped_2d_index_1d(lanes, elements).ok_or_else(|| {
                GeneralTypedExtractError::new(format!(
                    "{argument} has invalid or overflowing RowStriped2D geometry `{ty}`"
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

fn build_abi(arguments: &[GeneralTypedArgumentV3]) -> Result<AbiLayout, GeneralTypedExtractError> {
    let mut offset = 0_u64;
    let mut layout_alignment = 1_u32;
    let mut fields = Vec::with_capacity(arguments.len());
    for (index, argument) in arguments.iter().enumerate() {
        let (size, alignment) = argument_size_alignment(argument.kind);
        offset = align_up(offset, alignment)
            .ok_or_else(|| GeneralTypedExtractError::new("general typed ABI offset overflow"))?;
        let name = format!("arg{index}");
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

fn build_abi_field(
    name: &str,
    index: usize,
    offset: u64,
    argument: &GeneralTypedArgumentV3,
) -> Result<AbiField, GeneralTypedExtractError> {
    let scalar = argument.kind.scalar().ok_or_else(|| {
        GeneralTypedExtractError::new("compiler-laid-out argument reached macro ABI construction")
    })?;
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
            GeneralTypedArgumentKindV3::WriteOnlyDisjointSlice(_) => (
                SLICE_BYTES,
                POINTER_ALIGNMENT,
                AbiKind::Slice {
                    element_size: scalar.size_bytes(),
                    element_alignment: scalar.size_bytes() as u32,
                },
                Mutability::Mutable,
                Access::WriteOnly,
                AddressSpace::Global,
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
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
            GeneralTypedArgumentKindV3::GlobalMutPointer(_) => (
                POINTER_BYTES,
                POINTER_ALIGNMENT,
                AbiKind::Pointer {
                    pointee_size: scalar.size_bytes(),
                    pointee_alignment: scalar.size_bytes() as u32,
                },
                Mutability::Mutable,
                Access::ReadWrite,
                AddressSpace::Global,
                ArgumentOwnership::UniqueBorrow,
                AliasClass::Exclusive,
            ),
            GeneralTypedArgumentKindV3::CompilerLaidOutByValue => unreachable!(),
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
        argument.type_identity().ok_or_else(|| {
            GeneralTypedExtractError::new("argument has no macro-authored type identity")
        })?,
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
        | GeneralTypedArgumentKindV3::WriteOnlyDisjointSlice(_)
        | GeneralTypedArgumentKindV3::DisjointSlice(_) => (SLICE_BYTES, POINTER_ALIGNMENT),
        GeneralTypedArgumentKindV3::GlobalMutPointer(_) => (POINTER_BYTES, POINTER_ALIGNMENT),
        GeneralTypedArgumentKindV3::CompilerLaidOutByValue => (0, 1),
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
    use fe2o3_artifacts::{DigestBytes, Dimensions, derive_generated_host_contract_identity_v1};
    use reserved_fe2o3_symbols::{
        GeneratedHostContractIdV3, MANIFEST_DERIVED_SCALAR_SLICE_PROFILE_TAG_V1,
    };

    fn argument(kind: GeneralTypedArgumentKindV3) -> GeneralTypedArgumentV3 {
        let scalar = kind
            .scalar()
            .expect("test helper accepts concrete ABI kinds");
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
            GeneralTypedArgumentKindV3::WriteOnlyDisjointSlice(_)
            | GeneralTypedArgumentKindV3::DisjointSlice(_) => (
                RustSourceTypeShapeV1::disjoint_slice(scalar, RustDisjointIndexSpaceV1::Index1D),
                RustcAbiClassV1::ScalarPair,
                16,
                8,
                slice_components(scalar, RustPointerMutabilityV1::Mut),
            ),
            GeneralTypedArgumentKindV3::GlobalMutPointer(_) => (
                RustSourceTypeShapeV1::global_mut_pointer(scalar),
                RustcAbiClassV1::Scalar,
                8,
                8,
                vec![
                    RustPhysicalComponentV1::new(
                        0,
                        8,
                        8,
                        RustPhysicalComponentKindV1::Pointer {
                            mutability: RustPointerMutabilityV1::Mut,
                            pointee: scalar,
                        },
                    )
                    .unwrap(),
                ],
            ),
            GeneralTypedArgumentKindV3::CompilerLaidOutByValue => unreachable!(),
        };
        GeneralTypedArgumentV3::from_layout(
            kind,
            RustLayoutEvidenceV1::new(
                RustTypeEvidenceV1::new(source_type),
                abi_class,
                PointerWidth::Bits64,
                size,
                alignment,
                components,
            )
            .unwrap(),
        )
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

    fn three_arguments() -> Vec<GeneralTypedArgumentV3> {
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

    fn four_arguments() -> Vec<GeneralTypedArgumentV3> {
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
        let abi = build_abi(arguments).unwrap();
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
    fn three_and_four_reconstruct_exact_offsets_sizes_and_effects() {
        let three = build_abi(&three_arguments()).unwrap();
        assert_eq!(three.size(), 40);
        assert_eq!(three.alignment(), 8);
        assert_eq!(
            three
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2"]
        );
        assert_eq!(
            three
                .fields()
                .iter()
                .map(|field| field.offset())
                .collect::<Vec<_>>(),
            [0, 8, 24]
        );
        assert_eq!(three.fields()[2].access(), Access::ReadWrite);

        let four = build_abi(&four_arguments()).unwrap();
        assert_eq!(four.size(), 56);
        assert_eq!(
            four.fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2", "arg3"]
        );
        assert_eq!(
            four.fields()
                .iter()
                .map(|field| field.offset())
                .collect::<Vec<_>>(),
            [0, 16, 32, 40]
        );
        assert_eq!(four.fields()[3].access(), Access::ReadWrite);
    }

    #[test]
    fn global_mut_pointer_retains_exact_abi_ownership_and_identity() {
        let pointer = argument(GeneralTypedArgumentKindV3::GlobalMutPointer(
            RustScalarElementTypeV1::U32,
        ));
        let abi = build_abi(std::slice::from_ref(&pointer)).unwrap();
        let field = &abi.fields()[0];

        assert_eq!(abi.size(), 8);
        assert_eq!(abi.alignment(), 8);
        assert_eq!(field.offset(), 0);
        assert_eq!(field.size(), 8);
        assert_eq!(field.alignment(), 8);
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
        let pointer_layout = pointer.layout.as_ref().unwrap();
        assert_eq!(pointer_layout.abi_class(), RustcAbiClassV1::Scalar);
        assert_eq!(pointer_layout.size(), 8);
        assert_eq!(pointer_layout.abi_alignment(), 8);
        assert_eq!(pointer_layout.components().len(), 1);
        assert_eq!(
            pointer_layout.components()[0].kind(),
            RustPhysicalComponentKindV1::Pointer {
                mutability: RustPointerMutabilityV1::Mut,
                pointee: RustScalarElementTypeV1::U32,
            }
        );

        let scalar = argument(GeneralTypedArgumentKindV3::Scalar(
            RustScalarElementTypeV1::U64,
        ));
        assert_ne!(pointer.type_identity(), scalar.type_identity());
    }

    #[test]
    fn host_contract_identity_is_order_and_scalar_sensitive() {
        let three = three_arguments();
        let three_identity = identity("three", &three);
        assert_eq!(
            three_identity,
            identity("three", &three),
            "structural ABI reconstruction must be deterministic"
        );
        let declared = GeneratedHostContractIdV3::from_bytes(*three_identity.as_bytes());
        assert_eq!(declared.as_bytes(), *three_identity.as_bytes());
        let mut wrong_identity = declared.as_bytes();
        wrong_identity[31] ^= 1;
        assert_ne!(wrong_identity, *three_identity.as_bytes());

        let mut reordered = three.clone();
        reordered.swap(0, 1);
        assert_ne!(three_identity, identity("three", &reordered));

        let mut scalar_mutation = three;
        scalar_mutation[0] = argument(GeneralTypedArgumentKindV3::Scalar(
            RustScalarElementTypeV1::F64,
        ));
        assert_ne!(three_identity, identity("three", &scalar_mutation));

        let mut layout_mutation = three_arguments();
        layout_mutation[1].layout = argument(GeneralTypedArgumentKindV3::DisjointSlice(
            RustScalarElementTypeV1::F32,
        ))
        .layout;
        assert_ne!(three_identity, identity("three", &layout_mutation));
        assert_ne!(three_identity, identity("renamed", &three_arguments()));
    }

    #[test]
    fn lookalike_general_v3_contracts_keep_positional_names() {
        let three_lookalike = build_abi(&three_arguments()).unwrap();
        assert_eq!(
            three_lookalike
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2"]
        );

        let four_lookalike = build_abi(&four_arguments()).unwrap();
        assert_eq!(
            four_lookalike
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2", "arg3"]
        );

        let mut same_abi_shape = three_arguments();
        same_abi_shape[0] = argument(GeneralTypedArgumentKindV3::Scalar(
            RustScalarElementTypeV1::U32,
        ));
        let wrong_three_signature = build_abi(&same_abi_shape).unwrap();
        assert_eq!(
            wrong_three_signature
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2"]
        );

        let mut reordered_four = four_arguments();
        reordered_four.swap(0, 2);
        let wrong_four_signature = build_abi(&reordered_four).unwrap();
        assert_eq!(
            wrong_four_signature
                .fields()
                .iter()
                .map(|field| field.name().as_str())
                .collect::<Vec<_>>(),
            ["arg0", "arg1", "arg2", "arg3"]
        );
    }
}
