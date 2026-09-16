//! Exact normalization rules for rustc compiler intrinsics with canonical MIR semantics.

use std::fmt;

use crate::trusted_device_items::TrustedDeviceItem;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAtomicAccessV1, SemanticAtomicOrderingV1, SemanticAtomicRmwOpV1, SemanticAtomicScopeV1,
};
use rustc_abi::ExternAbi;
use rustc_hir::{Mutability, Safety};
use rustc_middle::mir::Operand;
use rustc_middle::ty::{
    self, ConstKind, FloatTy, Instance, InstanceKind, IntTy, Ty, TyCtxt, TyKind, UintTy,
};
use rustc_span::Spanned;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum AtomicRmwIntrinsicShapeV1 {
    OneType,
    ElementAndValueTypes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AtomicRmwIntrinsicRuleV1 {
    operation: SemanticAtomicRmwOpV1,
    shape: AtomicRmwIntrinsicShapeV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionRustcIntrinsicOperationV1 {
    AtomicRmw {
        operation: SemanticAtomicRmwOpV1,
        access: SemanticAtomicAccessV1,
    },
    FabsF32,
    AtomicLoad {
        access: SemanticAtomicAccessV1,
    },
    AtomicStore {
        access: SemanticAtomicAccessV1,
    },
}

impl ProductionRustcIntrinsicOperationV1 {
    pub(crate) const fn operation_tag(self) -> u8 {
        match self {
            Self::AtomicRmw { .. } => 0,
            Self::FabsF32 => 1,
            Self::AtomicLoad { .. } => 2,
            Self::AtomicStore { .. } => 3,
        }
    }

    pub(crate) const fn atomic_rmw(
        self,
    ) -> Option<(SemanticAtomicRmwOpV1, SemanticAtomicAccessV1)> {
        match self {
            Self::AtomicRmw { operation, access } => Some((operation, access)),
            Self::FabsF32 | Self::AtomicLoad { .. } | Self::AtomicStore { .. } => None,
        }
    }

    pub(crate) const fn atomic_access(self) -> Option<SemanticAtomicAccessV1> {
        match self {
            Self::AtomicRmw { access, .. }
            | Self::AtomicLoad { access }
            | Self::AtomicStore { access } => Some(access),
            Self::FabsF32 => None,
        }
    }

    pub(crate) const fn call_arity(self) -> usize {
        match self {
            Self::AtomicLoad { .. } | Self::FabsF32 => 1,
            Self::AtomicRmw { .. } | Self::AtomicStore { .. } => 2,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionRustcIntrinsicClassificationV1<'tcx> {
    pub(crate) operation: ProductionRustcIntrinsicOperationV1,
    pub(crate) element_type: Ty<'tcx>,
    pub(crate) source_call_arity: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionRustcIntrinsicErrorV1 {
    MissingMetadata,
    UnsupportedIntrinsic,
    GenericArity,
    ElementTypeArgument,
    ValueTypeArgument,
    MismatchedValueType,
    UnsupportedIntegerType,
    FabsGenericArity,
    FabsElementTypeArgument,
    FabsUnsupportedWidth,
    FabsCallArity,
    FabsInputType,
    FabsResultType,
    OrderingArgument,
    UnsupportedOrdering,
    WrapperSignature,
    WrapperBody,
}

impl fmt::Display for ProductionRustcIntrinsicErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::MissingMetadata => "compiler intrinsic without rustc metadata",
            Self::UnsupportedIntrinsic => "unsupported compiler intrinsic",
            Self::GenericArity => "atomic intrinsic with unexpected generic arity",
            Self::ElementTypeArgument => "atomic intrinsic without an element type argument",
            Self::ValueTypeArgument => "atomic intrinsic without a value type argument",
            Self::MismatchedValueType => "atomic intrinsic whose element and value types differ",
            Self::UnsupportedIntegerType => {
                "atomic intrinsic outside the supported i32/u32/i64/u64 integer subset"
            }
            Self::FabsGenericArity => "fabs intrinsic with unexpected generic arity",
            Self::FabsElementTypeArgument => "fabs intrinsic without a float type argument",
            Self::FabsUnsupportedWidth => "fabs intrinsic outside the supported f32 width",
            Self::FabsCallArity => "fabs intrinsic with unexpected call arity",
            Self::FabsInputType => "fabs intrinsic whose input is not its f32 type argument",
            Self::FabsResultType => "fabs intrinsic whose result is not its f32 type argument",
            Self::OrderingArgument => "atomic intrinsic without a concrete ordering argument",
            Self::UnsupportedOrdering => "atomic intrinsic with an unsupported ordering value",
            Self::WrapperSignature => "reviewed core atomic wrapper has an unexpected signature",
            Self::WrapperBody => "reviewed core atomic wrapper has an unexpected MIR effect graph",
        })
    }
}

include!("production_rustc_intrinsic_v1/atomic_wrapper_body_v1.rs");
include!("production_rustc_intrinsic_v1/branch_hint_origin_v1.rs");

/// Normalizes reviewed effectful core wrappers only when their ordering is an
/// exact constant. This is not pure-call admission: the resulting recipe emits
/// an atomic effect and commits the wrapper's compiler-owned MIR definition.
pub(crate) fn classify_call<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    arguments: &[Spanned<Operand<'tcx>>],
) -> Result<Option<ProductionRustcIntrinsicClassificationV1<'tcx>>, ProductionRustcIntrinsicErrorV1>
{
    if let Some(intrinsic) = classify(tcx, instance)? {
        if arguments.len() != intrinsic.operation.call_arity() {
            return Err(ProductionRustcIntrinsicErrorV1::GenericArity);
        }
        return Ok(Some(intrinsic));
    }
    if !matches!(instance.def, InstanceKind::Item(_))
        || !is_reviewed_core_function_v1(tcx, instance)
        || !tcx.is_mir_available(instance.def_id())
    {
        return Ok(None);
    }
    let load = match tcx.def_path_str(instance.def_id()).as_str() {
        "core::sync::atomic::atomic_load" => true,
        "core::sync::atomic::atomic_store" => false,
        _ => return Ok(None),
    };
    let [element] = instance.args.as_slice() else {
        return Err(ProductionRustcIntrinsicErrorV1::GenericArity);
    };
    let element_type = element
        .as_type()
        .ok_or(ProductionRustcIntrinsicErrorV1::ElementTypeArgument)?;
    if !supported_atomic_integer_v1(element_type) {
        return Err(ProductionRustcIntrinsicErrorV1::UnsupportedIntegerType);
    }
    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    let arity = if load { 2 } else { 3 };
    if signature.safety != Safety::Unsafe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || signature.inputs().len() != arity
        || arguments.len() != arity
        || signature.output() != if load { element_type } else { tcx.types.unit }
        || !matches!(signature.inputs()[0].kind(), TyKind::RawPtr(pointee, mutability)
            if *pointee == element_type && *mutability == if load { Mutability::Not } else { Mutability::Mut })
        || (!load && signature.inputs()[1] != element_type)
    {
        return Err(ProductionRustcIntrinsicErrorV1::WrapperSignature);
    }
    let ordering_ty = signature.inputs()[arity - 1];
    let TyKind::Adt(ordering_definition, ordering_arguments) = ordering_ty.kind() else {
        return Err(ProductionRustcIntrinsicErrorV1::WrapperSignature);
    };
    if ordering_definition.did().krate != instance.def_id().krate
        || tcx
            .def_path(ordering_definition.did())
            .to_string_no_crate_verbose()
            != "::sync::atomic::Ordering"
        || !ordering_definition.is_enum()
        || !ordering_arguments.is_empty()
    {
        return Err(ProductionRustcIntrinsicErrorV1::WrapperSignature);
    }
    let Operand::Constant(constant) = &arguments[arity - 1].node else {
        return Err(ProductionRustcIntrinsicErrorV1::OrderingArgument);
    };
    if constant.const_.ty() != ordering_ty {
        return Err(ProductionRustcIntrinsicErrorV1::OrderingArgument);
    }
    let value = constant
        .const_
        .try_eval_scalar_int(tcx, ty::TypingEnv::fully_monomorphized())
        .ok_or(ProductionRustcIntrinsicErrorV1::OrderingArgument)?;
    let discriminant = u64::try_from(value.to_bits(value.size()))
        .map_err(|_| ProductionRustcIntrinsicErrorV1::OrderingArgument)?;
    let ordering = atomic_ordering_from_discriminant_v1(discriminant)
        .ok_or(ProductionRustcIntrinsicErrorV1::UnsupportedOrdering)?;
    if !load_store_ordering_supported_v1(load, ordering) {
        return Err(ProductionRustcIntrinsicErrorV1::UnsupportedOrdering);
    }
    if !reviewed_atomic_wrapper_body_v1(tcx, instance, load, element_type) {
        return Err(ProductionRustcIntrinsicErrorV1::WrapperBody);
    }
    let access = SemanticAtomicAccessV1::new(ordering, SemanticAtomicScopeV1::System);
    Ok(Some(ProductionRustcIntrinsicClassificationV1 {
        operation: if load {
            ProductionRustcIntrinsicOperationV1::AtomicLoad { access }
        } else {
            ProductionRustcIntrinsicOperationV1::AtomicStore { access }
        },
        element_type,
        source_call_arity: arity,
    }))
}

/// Classifies only exact rustc compiler-generated intrinsic instances.
///
/// Ordinary items return `Ok(None)`. Once rustc identifies an instance as an
/// intrinsic, an unknown or malformed operation is an error rather than a
/// traversable helper or a semantic terminal.
pub(crate) fn classify<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> Result<Option<ProductionRustcIntrinsicClassificationV1<'tcx>>, ProductionRustcIntrinsicErrorV1>
{
    let InstanceKind::Intrinsic(def_id) = instance.def else {
        return Ok(None);
    };
    let intrinsic = tcx
        .intrinsic(def_id)
        .ok_or(ProductionRustcIntrinsicErrorV1::MissingMetadata)?;
    if is_fabs_intrinsic_name_v1(intrinsic.name.as_str()) {
        let arguments = instance.args.as_slice();
        let [element_argument] = arguments else {
            return Err(ProductionRustcIntrinsicErrorV1::FabsGenericArity);
        };
        let element_type = element_argument
            .as_type()
            .ok_or(ProductionRustcIntrinsicErrorV1::FabsElementTypeArgument)?;
        let signature = tcx.instantiate_bound_regions_with_erased(
            tcx.fn_sig(instance.def_id())
                .instantiate(tcx, instance.args),
        );
        validate_fabs_contract_v1(
            scalar_float_width_v1(element_type),
            signature.inputs().len(),
            signature
                .inputs()
                .first()
                .is_some_and(|input| *input == element_type),
            signature.output() == element_type,
        )?;
        return Ok(Some(ProductionRustcIntrinsicClassificationV1 {
            operation: ProductionRustcIntrinsicOperationV1::FabsF32,
            element_type,
            source_call_arity: 1,
        }));
    }
    if matches!(intrinsic.name.as_str(), "atomic_load" | "atomic_store") {
        let [element, ordering] = instance.args.as_slice() else {
            return Err(ProductionRustcIntrinsicErrorV1::GenericArity);
        };
        let element_type = element
            .as_type()
            .ok_or(ProductionRustcIntrinsicErrorV1::ElementTypeArgument)?;
        if !supported_atomic_integer_v1(element_type) {
            return Err(ProductionRustcIntrinsicErrorV1::UnsupportedIntegerType);
        }
        let ordering = ordering
            .as_const()
            .and_then(|value| fieldless_enum_discriminant_v1(tcx, value))
            .ok_or(ProductionRustcIntrinsicErrorV1::OrderingArgument)?;
        let ordering = atomic_ordering_from_discriminant_v1(ordering)
            .ok_or(ProductionRustcIntrinsicErrorV1::UnsupportedOrdering)?;
        let load = intrinsic.name.as_str() == "atomic_load";
        if !load_store_ordering_supported_v1(load, ordering) {
            return Err(ProductionRustcIntrinsicErrorV1::UnsupportedOrdering);
        }
        let access = SemanticAtomicAccessV1::new(ordering, SemanticAtomicScopeV1::System);
        return Ok(Some(ProductionRustcIntrinsicClassificationV1 {
            operation: if load {
                ProductionRustcIntrinsicOperationV1::AtomicLoad { access }
            } else {
                ProductionRustcIntrinsicOperationV1::AtomicStore { access }
            },
            element_type,
            source_call_arity: if load { 1 } else { 2 },
        }));
    }
    let rule = atomic_rmw_intrinsic_rule_v1(intrinsic.name.as_str())
        .ok_or(ProductionRustcIntrinsicErrorV1::UnsupportedIntrinsic)?;

    let arguments = instance.args.as_slice();
    let (element_argument, value_argument, ordering_argument) = match (rule.shape, arguments) {
        (AtomicRmwIntrinsicShapeV1::OneType, [element, ordering]) => (element, None, ordering),
        (AtomicRmwIntrinsicShapeV1::ElementAndValueTypes, [element, value, ordering]) => {
            (element, Some(value), ordering)
        }
        _ => return Err(ProductionRustcIntrinsicErrorV1::GenericArity),
    };
    let element_type = element_argument
        .as_type()
        .ok_or(ProductionRustcIntrinsicErrorV1::ElementTypeArgument)?;
    if let Some(value_argument) = value_argument {
        let value_type = value_argument
            .as_type()
            .ok_or(ProductionRustcIntrinsicErrorV1::ValueTypeArgument)?;
        if element_type != value_type {
            return Err(ProductionRustcIntrinsicErrorV1::MismatchedValueType);
        }
    }
    if !operation_supports_atomic_integer_v1(rule.operation, element_type) {
        return Err(ProductionRustcIntrinsicErrorV1::UnsupportedIntegerType);
    }
    let ordering_discriminant = ordering_argument
        .as_const()
        .and_then(|value| fieldless_enum_discriminant_v1(tcx, value))
        .ok_or(ProductionRustcIntrinsicErrorV1::OrderingArgument)?;
    let ordering = atomic_ordering_from_discriminant_v1(ordering_discriminant)
        .ok_or(ProductionRustcIntrinsicErrorV1::UnsupportedOrdering)?;

    Ok(Some(ProductionRustcIntrinsicClassificationV1 {
        operation: ProductionRustcIntrinsicOperationV1::AtomicRmw {
            operation: rule.operation,
            access: SemanticAtomicAccessV1::new(ordering, SemanticAtomicScopeV1::System),
        },
        element_type,
        source_call_arity: 2,
    }))
}

fn is_fabs_intrinsic_name_v1(name: &str) -> bool {
    name == "fabs"
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarFloatWidthV1 {
    F32,
    Other,
    NotFloat,
}

fn scalar_float_width_v1(ty: Ty<'_>) -> ScalarFloatWidthV1 {
    match ty.kind() {
        TyKind::Float(FloatTy::F32) => ScalarFloatWidthV1::F32,
        TyKind::Float(_) => ScalarFloatWidthV1::Other,
        _ => ScalarFloatWidthV1::NotFloat,
    }
}

fn validate_fabs_contract_v1(
    width: ScalarFloatWidthV1,
    input_count: usize,
    input_matches_element: bool,
    output_matches_element: bool,
) -> Result<(), ProductionRustcIntrinsicErrorV1> {
    match width {
        ScalarFloatWidthV1::F32 => {}
        ScalarFloatWidthV1::Other => {
            return Err(ProductionRustcIntrinsicErrorV1::FabsUnsupportedWidth);
        }
        ScalarFloatWidthV1::NotFloat => {
            return Err(ProductionRustcIntrinsicErrorV1::FabsElementTypeArgument);
        }
    }
    if input_count != 1 {
        return Err(ProductionRustcIntrinsicErrorV1::FabsCallArity);
    }
    if !input_matches_element {
        return Err(ProductionRustcIntrinsicErrorV1::FabsInputType);
    }
    if !output_matches_element {
        return Err(ProductionRustcIntrinsicErrorV1::FabsResultType);
    }
    Ok(())
}

fn atomic_rmw_intrinsic_rule_v1(name: &str) -> Option<AtomicRmwIntrinsicRuleV1> {
    // `atomic_nand` remains excluded until the versioned Kernel IR can encode
    // it; semantic MIR support alone is not executable authority.
    let (operation, shape) = match name {
        "atomic_xchg" => (
            SemanticAtomicRmwOpV1::Exchange,
            AtomicRmwIntrinsicShapeV1::OneType,
        ),
        "atomic_xadd" => (
            SemanticAtomicRmwOpV1::Add,
            AtomicRmwIntrinsicShapeV1::ElementAndValueTypes,
        ),
        "atomic_xsub" => (
            SemanticAtomicRmwOpV1::Subtract,
            AtomicRmwIntrinsicShapeV1::ElementAndValueTypes,
        ),
        "atomic_and" => (
            SemanticAtomicRmwOpV1::BitAnd,
            AtomicRmwIntrinsicShapeV1::ElementAndValueTypes,
        ),
        "atomic_or" => (
            SemanticAtomicRmwOpV1::BitOr,
            AtomicRmwIntrinsicShapeV1::ElementAndValueTypes,
        ),
        "atomic_xor" => (
            SemanticAtomicRmwOpV1::BitXor,
            AtomicRmwIntrinsicShapeV1::ElementAndValueTypes,
        ),
        "atomic_max" => (
            SemanticAtomicRmwOpV1::SignedMaximum,
            AtomicRmwIntrinsicShapeV1::OneType,
        ),
        "atomic_min" => (
            SemanticAtomicRmwOpV1::SignedMinimum,
            AtomicRmwIntrinsicShapeV1::OneType,
        ),
        "atomic_umax" => (
            SemanticAtomicRmwOpV1::UnsignedMaximum,
            AtomicRmwIntrinsicShapeV1::OneType,
        ),
        "atomic_umin" => (
            SemanticAtomicRmwOpV1::UnsignedMinimum,
            AtomicRmwIntrinsicShapeV1::OneType,
        ),
        _ => return None,
    };
    Some(AtomicRmwIntrinsicRuleV1 { operation, shape })
}

fn reviewed_core_atomic_wrapper_shape_v1(path: &str) -> Option<AtomicRmwIntrinsicShapeV1> {
    match path {
        "core::sync::atomic::atomic_swap"
        | "core::sync::atomic::atomic_max"
        | "core::sync::atomic::atomic_min"
        | "core::sync::atomic::atomic_umax"
        | "core::sync::atomic::atomic_umin"
        | "core::sync::atomic::atomic_load"
        | "core::sync::atomic::atomic_store" => Some(AtomicRmwIntrinsicShapeV1::OneType),
        "core::sync::atomic::atomic_add"
        | "core::sync::atomic::atomic_sub"
        | "core::sync::atomic::atomic_and"
        | "core::sync::atomic::atomic_or"
        | "core::sync::atomic::atomic_xor" => Some(AtomicRmwIntrinsicShapeV1::ElementAndValueTypes),
        _ => None,
    }
}

fn reviewed_atomic_wrapper_arguments_v1(
    shape: AtomicRmwIntrinsicShapeV1,
    arguments: &[ty::GenericArg<'_>],
) -> bool {
    match (shape, arguments) {
        (AtomicRmwIntrinsicShapeV1::OneType, [element]) => {
            element.as_type().is_some_and(supported_atomic_integer_v1)
        }
        (AtomicRmwIntrinsicShapeV1::ElementAndValueTypes, [element, value]) => {
            let (Some(element), Some(value)) = (element.as_type(), value.as_type()) else {
                return false;
            };
            element == value && supported_atomic_integer_v1(element)
        }
        _ => false,
    }
}

fn operation_supports_atomic_integer_v1(operation: SemanticAtomicRmwOpV1, ty: Ty<'_>) -> bool {
    match operation {
        SemanticAtomicRmwOpV1::SignedMaximum | SemanticAtomicRmwOpV1::SignedMinimum => {
            matches!(ty.kind(), TyKind::Int(IntTy::I32 | IntTy::I64))
        }
        SemanticAtomicRmwOpV1::UnsignedMaximum | SemanticAtomicRmwOpV1::UnsignedMinimum => {
            matches!(ty.kind(), TyKind::Uint(UintTy::U32 | UintTy::U64))
        }
        SemanticAtomicRmwOpV1::Exchange
        | SemanticAtomicRmwOpV1::Add
        | SemanticAtomicRmwOpV1::Subtract
        | SemanticAtomicRmwOpV1::BitAnd
        | SemanticAtomicRmwOpV1::BitOr
        | SemanticAtomicRmwOpV1::BitXor => supported_atomic_integer_v1(ty),
        SemanticAtomicRmwOpV1::BitNand => false,
    }
}

pub(crate) fn is_reviewed_core_function_v1(tcx: TyCtxt<'_>, instance: Instance<'_>) -> bool {
    let Some(core_lang_item) = tcx.lang_items().sized_trait() else {
        return false;
    };
    instance.def_id().krate == core_lang_item.krate
        && tcx.crate_name(core_lang_item.krate).as_str() == "core"
}

pub(crate) fn is_reviewed_core_atomic_rmw_wrapper_v1(
    tcx: TyCtxt<'_>,
    instance: Instance<'_>,
) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !is_reviewed_core_function_v1(tcx, instance)
        || !tcx.is_mir_available(instance.def_id())
    {
        return false;
    }
    let path = tcx.def_path_str(instance.def_id());
    let Some(shape) = reviewed_core_atomic_wrapper_shape_v1(&path) else {
        return false;
    };
    reviewed_atomic_wrapper_arguments_v1(shape, instance.args.as_slice())
}

fn is_reviewed_core_safe_atomic_rmw_method_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !is_reviewed_core_function_v1(tcx, instance)
        || !matches!(
            tcx.item_name(instance.def_id()).as_str(),
            "swap"
                | "fetch_add"
                | "fetch_sub"
                | "fetch_and"
                | "fetch_or"
                | "fetch_xor"
                | "fetch_max"
                | "fetch_min"
                | "load"
                | "store"
        )
        || !tcx.is_mir_available(instance.def_id())
    {
        return false;
    }

    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != Safety::Safe || signature.abi != ExternAbi::Rust || signature.c_variadic
    {
        return false;
    }
    let (receiver, value, ordering) = match signature.inputs() {
        [receiver, ordering] if tcx.item_name(instance.def_id()).as_str() == "load" => {
            (receiver, None, ordering)
        }
        [receiver, value, ordering] if tcx.item_name(instance.def_id()).as_str() != "load" => {
            (receiver, Some(value), ordering)
        }
        _ => return false,
    };
    let TyKind::Ref(_, atomic, Mutability::Not) = *receiver.kind() else {
        return false;
    };
    let TyKind::Adt(atomic_definition, atomic_arguments) = *atomic.kind() else {
        return false;
    };
    let [atomic_element] = atomic_arguments.as_slice() else {
        return false;
    };
    let Some(atomic_element) = atomic_element.as_type() else {
        return false;
    };
    let TyKind::Adt(ordering_definition, ordering_arguments) = *ordering.kind() else {
        return false;
    };
    atomic_definition.did().krate == instance.def_id().krate
        && tcx.def_path_str(atomic_definition.did()) == "core::sync::atomic::Atomic"
        && supported_atomic_integer_v1(atomic_element)
        && value.is_none_or(|value| *value == atomic_element)
        && signature.output()
            == if tcx.item_name(instance.def_id()).as_str() == "store" {
                tcx.types.unit
            } else {
                atomic_element
            }
        && ordering_definition.did().krate == instance.def_id().krate
        && tcx.item_name(ordering_definition.did()).as_str() == "Ordering"
        && ordering_definition.is_enum()
        && ordering_arguments.is_empty()
}

fn is_reviewed_core_safe_atomic_as_ptr_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !is_reviewed_core_function_v1(tcx, instance)
        || tcx.item_name(instance.def_id()).as_str() != "as_ptr"
        || !tcx.is_mir_available(instance.def_id())
    {
        return false;
    }

    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != Safety::Safe || signature.abi != ExternAbi::Rust || signature.c_variadic
    {
        return false;
    }
    let [receiver] = signature.inputs() else {
        return false;
    };
    let TyKind::Ref(_, atomic, Mutability::Not) = *receiver.kind() else {
        return false;
    };
    let TyKind::Adt(atomic_definition, atomic_arguments) = *atomic.kind() else {
        return false;
    };
    let [atomic_element] = atomic_arguments.as_slice() else {
        return false;
    };
    let Some(atomic_element) = atomic_element.as_type() else {
        return false;
    };
    let TyKind::RawPtr(output_element, Mutability::Mut) = *signature.output().kind() else {
        return false;
    };
    atomic_definition.did().krate == instance.def_id().krate
        && tcx.def_path_str(atomic_definition.did()) == "core::sync::atomic::Atomic"
        && atomic_element == output_element
        && supported_atomic_integer_v1(atomic_element)
}

fn is_reviewed_core_safe_atomic_unsafe_cell_get_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !is_reviewed_core_function_v1(tcx, instance)
        || tcx.item_name(instance.def_id()).as_str() != "get"
        || !tcx.is_mir_available(instance.def_id())
    {
        return false;
    }

    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != Safety::Safe || signature.abi != ExternAbi::Rust || signature.c_variadic
    {
        return false;
    }
    let [receiver] = signature.inputs() else {
        return false;
    };
    let TyKind::Ref(_, cell, Mutability::Not) = *receiver.kind() else {
        return false;
    };
    let TyKind::Adt(cell_definition, cell_arguments) = *cell.kind() else {
        return false;
    };
    let [storage] = cell_arguments.as_slice() else {
        return false;
    };
    let Some(storage) = storage.as_type() else {
        return false;
    };
    let TyKind::Adt(storage_definition, storage_arguments) = *storage.kind() else {
        return false;
    };
    let [element] = storage_arguments.as_slice() else {
        return false;
    };
    let Some(element) = element.as_type() else {
        return false;
    };
    let TyKind::RawPtr(output_storage, Mutability::Mut) = *signature.output().kind() else {
        return false;
    };
    let storage_path = tcx.def_path_str(storage_definition.did());
    let storage_matches_element = match element.kind() {
        TyKind::Int(IntTy::I32) | TyKind::Uint(UintTy::U32) => {
            storage_path == "core::sync::atomic::private::Align4"
        }
        TyKind::Int(IntTy::I64) | TyKind::Uint(UintTy::U64) => {
            storage_path == "core::sync::atomic::private::Align8"
        }
        _ => false,
    };
    cell_definition.did().krate == instance.def_id().krate
        && tcx.def_path_str(cell_definition.did()) == "core::cell::UnsafeCell"
        && storage_definition.did().krate == instance.def_id().krate
        && storage_matches_element
        && storage == output_storage
}

fn is_reviewed_core_safe_atomic_pointer_cast_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !is_reviewed_core_function_v1(tcx, instance)
        || tcx.item_name(instance.def_id()).as_str() != "cast"
        || tcx.def_path_str(instance.def_id()) != "core::ptr::mut_ptr::<impl *mut T>::cast"
        || !tcx.is_mir_available(instance.def_id())
    {
        return false;
    }

    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != Safety::Safe || signature.abi != ExternAbi::Rust || signature.c_variadic
    {
        return false;
    }
    let [source] = signature.inputs() else {
        return false;
    };
    let TyKind::RawPtr(source_element, Mutability::Mut) = *source.kind() else {
        return false;
    };
    let TyKind::RawPtr(output_element, Mutability::Mut) = *signature.output().kind() else {
        return false;
    };

    match (source_element.kind(), output_element.kind()) {
        (TyKind::Adt(storage_definition, storage_arguments), _) => {
            let [element] = storage_arguments.as_slice() else {
                return false;
            };
            let Some(element) = element.as_type() else {
                return false;
            };
            let storage_path = tcx.def_path_str(storage_definition.did());
            storage_definition.did().krate == instance.def_id().krate
                && element == output_element
                && match element.kind() {
                    TyKind::Int(IntTy::I32) | TyKind::Uint(UintTy::U32) => {
                        storage_path == "core::sync::atomic::private::Align4"
                    }
                    TyKind::Int(IntTy::I64) | TyKind::Uint(UintTy::U64) => {
                        storage_path == "core::sync::atomic::private::Align8"
                    }
                    _ => false,
                }
        }
        (_, TyKind::Adt(atomic_definition, atomic_arguments)) => {
            let [element] = atomic_arguments.as_slice() else {
                return false;
            };
            let Some(element) = element.as_type() else {
                return false;
            };
            atomic_definition.did().krate == instance.def_id().krate
                && tcx.def_path_str(atomic_definition.did()) == "core::sync::atomic::Atomic"
                && source_element == element
                && supported_atomic_integer_v1(element)
        }
        _ => false,
    }
}

pub(crate) fn is_reviewed_core_atomic_function_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    is_reviewed_core_atomic_rmw_wrapper_v1(tcx, instance)
        || is_reviewed_core_safe_atomic_rmw_method_v1(tcx, instance)
        || is_reviewed_core_safe_atomic_as_ptr_v1(tcx, instance)
        || is_reviewed_core_safe_atomic_unsafe_cell_get_v1(tcx, instance)
        || is_reviewed_core_safe_atomic_pointer_cast_v1(tcx, instance)
        || is_reviewed_core_atomic_from_ptr_v1(tcx, instance)
}

pub(crate) fn is_reviewed_device_global_mut_ptr_as_raw_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || tcx.def_path_str(instance.def_id()) != "fe2o3_device::DeviceGlobalMutPtr::<T>::as_raw"
        || !tcx.is_mir_available(instance.def_id())
    {
        return false;
    }
    let Some(wrapper_definition) =
        crate::trusted_device_items::definition(tcx, TrustedDeviceItem::DeviceGlobalMutPtr)
    else {
        return false;
    };
    if instance.def_id().krate != wrapper_definition.krate {
        return false;
    }

    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != Safety::Safe || signature.abi != ExternAbi::Rust || signature.c_variadic
    {
        return false;
    }
    let [input] = signature.inputs() else {
        return false;
    };
    let TyKind::Adt(input_definition, input_arguments) = *input.kind() else {
        return false;
    };
    let [input_element] = input_arguments.as_slice() else {
        return false;
    };
    let Some(input_element) = input_element.as_type() else {
        return false;
    };
    let TyKind::RawPtr(output_element, Mutability::Mut) = *signature.output().kind() else {
        return false;
    };
    input_definition.did() == wrapper_definition
        && input_element == output_element
        && supported_atomic_integer_v1(input_element)
}

fn is_reviewed_core_atomic_from_ptr_v1<'tcx>(tcx: TyCtxt<'tcx>, instance: Instance<'tcx>) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !is_reviewed_core_function_v1(tcx, instance)
        || !matches!(
            tcx.def_path_str(instance.def_id()).as_str(),
            "core::sync::atomic::Atomic::<i32>::from_ptr"
                | "core::sync::atomic::Atomic::<u32>::from_ptr"
                | "core::sync::atomic::Atomic::<i64>::from_ptr"
                | "core::sync::atomic::Atomic::<u64>::from_ptr"
        )
        || !tcx.is_mir_available(instance.def_id())
    {
        return false;
    }

    let signature = tcx.instantiate_bound_regions_with_erased(
        tcx.fn_sig(instance.def_id())
            .instantiate(tcx, instance.args),
    );
    if signature.safety != Safety::Unsafe
        || signature.abi != ExternAbi::Rust
        || signature.c_variadic
    {
        return false;
    }
    let [input] = signature.inputs() else {
        return false;
    };
    let TyKind::RawPtr(element, Mutability::Mut) = *input.kind() else {
        return false;
    };
    let TyKind::Ref(_, atomic, Mutability::Not) = *signature.output().kind() else {
        return false;
    };
    let TyKind::Adt(atomic_definition, atomic_arguments) = *atomic.kind() else {
        return false;
    };
    let [atomic_element] = atomic_arguments.as_slice() else {
        return false;
    };
    atomic_definition.did().krate == instance.def_id().krate
        && tcx.def_path_str(atomic_definition.did()) == "core::sync::atomic::Atomic"
        && atomic_element.as_type() == Some(element)
        && supported_atomic_integer_v1(element)
}

fn supported_atomic_integer_v1(ty: Ty<'_>) -> bool {
    matches!(
        ty.kind(),
        TyKind::Int(IntTy::I32 | IntTy::I64) | TyKind::Uint(UintTy::U32 | UintTy::U64)
    )
}

fn fieldless_enum_discriminant_v1<'tcx>(tcx: TyCtxt<'tcx>, value: ty::Const<'tcx>) -> Option<u64> {
    let ConstKind::Value(value) = value.kind() else {
        return None;
    };
    let TyKind::Adt(definition, _) = value.ty.kind() else {
        return None;
    };
    if !definition.is_enum() {
        return None;
    }
    let variant = value
        .valtree
        .try_to_branch()?
        .first()?
        .try_to_leaf()?
        .to_u32();
    let variant = rustc_abi::VariantIdx::from_u32(variant);
    if !definition.variant(variant).fields.is_empty() {
        return None;
    }
    u64::try_from(definition.discriminant_for_variant(tcx, variant).val).ok()
}

const fn load_store_ordering_supported_v1(load: bool, ordering: SemanticAtomicOrderingV1) -> bool {
    matches!(
        ordering,
        SemanticAtomicOrderingV1::Relaxed | SemanticAtomicOrderingV1::SequentiallyConsistent
    ) || matches!(
        (load, ordering),
        (true, SemanticAtomicOrderingV1::Acquire) | (false, SemanticAtomicOrderingV1::Release)
    )
}

const fn atomic_ordering_from_discriminant_v1(value: u64) -> Option<SemanticAtomicOrderingV1> {
    match value {
        0 => Some(SemanticAtomicOrderingV1::Relaxed),
        1 => Some(SemanticAtomicOrderingV1::Release),
        2 => Some(SemanticAtomicOrderingV1::Acquire),
        3 => Some(SemanticAtomicOrderingV1::AcquireRelease),
        4 => Some(SemanticAtomicOrderingV1::SequentiallyConsistent),
        _ => None,
    }
}

pub(crate) const fn atomic_ordering_tag_v1(ordering: SemanticAtomicOrderingV1) -> u8 {
    match ordering {
        SemanticAtomicOrderingV1::Relaxed => 0,
        SemanticAtomicOrderingV1::Release => 1,
        SemanticAtomicOrderingV1::Acquire => 2,
        SemanticAtomicOrderingV1::AcquireRelease => 3,
        SemanticAtomicOrderingV1::SequentiallyConsistent => 4,
    }
}

pub(crate) const fn atomic_scope_tag_v1(scope: SemanticAtomicScopeV1) -> u8 {
    match scope {
        SemanticAtomicScopeV1::SingleThread => 0,
        SemanticAtomicScopeV1::Workgroup => 1,
        SemanticAtomicScopeV1::Agent => 2,
        SemanticAtomicScopeV1::Device => 3,
        SemanticAtomicScopeV1::System => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rustc_atomic_ordering_discriminants_are_frozen() {
        assert_eq!(
            atomic_ordering_from_discriminant_v1(0),
            Some(SemanticAtomicOrderingV1::Relaxed)
        );
        assert_eq!(
            atomic_ordering_from_discriminant_v1(1),
            Some(SemanticAtomicOrderingV1::Release)
        );
        assert_eq!(
            atomic_ordering_from_discriminant_v1(2),
            Some(SemanticAtomicOrderingV1::Acquire)
        );
        assert_eq!(
            atomic_ordering_from_discriminant_v1(3),
            Some(SemanticAtomicOrderingV1::AcquireRelease)
        );
        assert_eq!(
            atomic_ordering_from_discriminant_v1(4),
            Some(SemanticAtomicOrderingV1::SequentiallyConsistent)
        );
        assert_eq!(atomic_ordering_from_discriminant_v1(5), None);
        assert_eq!(atomic_ordering_from_discriminant_v1(u64::MAX), None);
    }

    #[test]
    fn normalized_atomic_rmw_intrinsics_are_exact_and_complete() {
        let expected = [
            (
                "atomic_xchg",
                SemanticAtomicRmwOpV1::Exchange,
                AtomicRmwIntrinsicShapeV1::OneType,
            ),
            (
                "atomic_xadd",
                SemanticAtomicRmwOpV1::Add,
                AtomicRmwIntrinsicShapeV1::ElementAndValueTypes,
            ),
            (
                "atomic_xsub",
                SemanticAtomicRmwOpV1::Subtract,
                AtomicRmwIntrinsicShapeV1::ElementAndValueTypes,
            ),
            (
                "atomic_and",
                SemanticAtomicRmwOpV1::BitAnd,
                AtomicRmwIntrinsicShapeV1::ElementAndValueTypes,
            ),
            (
                "atomic_or",
                SemanticAtomicRmwOpV1::BitOr,
                AtomicRmwIntrinsicShapeV1::ElementAndValueTypes,
            ),
            (
                "atomic_xor",
                SemanticAtomicRmwOpV1::BitXor,
                AtomicRmwIntrinsicShapeV1::ElementAndValueTypes,
            ),
            (
                "atomic_max",
                SemanticAtomicRmwOpV1::SignedMaximum,
                AtomicRmwIntrinsicShapeV1::OneType,
            ),
            (
                "atomic_min",
                SemanticAtomicRmwOpV1::SignedMinimum,
                AtomicRmwIntrinsicShapeV1::OneType,
            ),
            (
                "atomic_umax",
                SemanticAtomicRmwOpV1::UnsignedMaximum,
                AtomicRmwIntrinsicShapeV1::OneType,
            ),
            (
                "atomic_umin",
                SemanticAtomicRmwOpV1::UnsignedMinimum,
                AtomicRmwIntrinsicShapeV1::OneType,
            ),
        ];
        for (name, operation, shape) in expected {
            let rule = atomic_rmw_intrinsic_rule_v1(name).expect("reviewed atomic intrinsic");
            assert_eq!(rule.operation, operation);
            assert_eq!(rule.shape, shape);

            let operation = ProductionRustcIntrinsicOperationV1::AtomicRmw {
                operation,
                access: SemanticAtomicAccessV1::new(
                    SemanticAtomicOrderingV1::Relaxed,
                    SemanticAtomicScopeV1::System,
                ),
            };
            let (rmw, access) = operation.atomic_rmw().expect("atomic operation");
            assert_eq!(rmw, rule.operation);
            assert_eq!(access.ordering(), SemanticAtomicOrderingV1::Relaxed);
            assert_eq!(access.scope(), SemanticAtomicScopeV1::System);
            assert_eq!(operation.operation_tag(), 0);
        }
        for unsupported in [
            "",
            "atomic_load",
            "atomic_store",
            "atomic_cxchg",
            "atomic_nand",
            "atomic_xadd_relaxed",
        ] {
            assert_eq!(atomic_rmw_intrinsic_rule_v1(unsupported), None);
        }
    }

    #[test]
    fn reviewed_atomic_wrapper_paths_match_the_intrinsic_shapes() {
        for path in [
            "core::sync::atomic::atomic_swap",
            "core::sync::atomic::atomic_max",
            "core::sync::atomic::atomic_min",
            "core::sync::atomic::atomic_umax",
            "core::sync::atomic::atomic_umin",
        ] {
            assert_eq!(
                reviewed_core_atomic_wrapper_shape_v1(path),
                Some(AtomicRmwIntrinsicShapeV1::OneType)
            );
        }
        for path in [
            "core::sync::atomic::atomic_add",
            "core::sync::atomic::atomic_sub",
            "core::sync::atomic::atomic_and",
            "core::sync::atomic::atomic_or",
            "core::sync::atomic::atomic_xor",
        ] {
            assert_eq!(
                reviewed_core_atomic_wrapper_shape_v1(path),
                Some(AtomicRmwIntrinsicShapeV1::ElementAndValueTypes)
            );
        }
        assert_eq!(
            reviewed_core_atomic_wrapper_shape_v1("core::sync::atomic::atomic_load"),
            Some(AtomicRmwIntrinsicShapeV1::OneType)
        );
        assert_eq!(
            reviewed_core_atomic_wrapper_shape_v1("core::sync::atomic::atomic_nand"),
            None
        );
    }

    #[test]
    fn atomic_load_store_ordering_and_identity_are_exact() {
        use SemanticAtomicOrderingV1::*;
        for ordering in [
            Relaxed,
            Release,
            Acquire,
            AcquireRelease,
            SequentiallyConsistent,
        ] {
            assert_eq!(
                load_store_ordering_supported_v1(true, ordering),
                matches!(ordering, Relaxed | Acquire | SequentiallyConsistent)
            );
            assert_eq!(
                load_store_ordering_supported_v1(false, ordering),
                matches!(ordering, Relaxed | Release | SequentiallyConsistent)
            );
            let access = SemanticAtomicAccessV1::new(ordering, SemanticAtomicScopeV1::System);
            let load = ProductionRustcIntrinsicOperationV1::AtomicLoad { access };
            let store = ProductionRustcIntrinsicOperationV1::AtomicStore { access };
            assert_eq!(load.atomic_access(), Some(access));
            assert_eq!(store.atomic_access(), Some(access));
            assert_eq!(load.call_arity(), 1);
            assert_eq!(store.call_arity(), 2);
            assert_ne!(load.operation_tag(), store.operation_tag());
            assert_eq!(load.atomic_rmw(), None);
            assert_eq!(store.atomic_rmw(), None);
        }
        assert_eq!(
            reviewed_core_atomic_wrapper_shape_v1("core::sync::atomic::atomic_store"),
            Some(AtomicRmwIntrinsicShapeV1::OneType)
        );
        for path in [
            "user::atomic_load",
            "core::sync::atomic::atomic_cxchg",
            "core::sync::atomic::atomic_load_extra",
        ] {
            assert_eq!(reviewed_core_atomic_wrapper_shape_v1(path), None);
        }
    }

    #[test]
    fn fabs_f32_contract_is_exact_and_closed() {
        assert_eq!(
            validate_fabs_contract_v1(ScalarFloatWidthV1::F32, 1, true, true),
            Ok(())
        );
        assert_eq!(
            validate_fabs_contract_v1(ScalarFloatWidthV1::F32, 0, true, true),
            Err(ProductionRustcIntrinsicErrorV1::FabsCallArity)
        );
        assert_eq!(
            validate_fabs_contract_v1(ScalarFloatWidthV1::F32, 2, true, true),
            Err(ProductionRustcIntrinsicErrorV1::FabsCallArity)
        );
        assert_eq!(
            validate_fabs_contract_v1(ScalarFloatWidthV1::Other, 1, true, true),
            Err(ProductionRustcIntrinsicErrorV1::FabsUnsupportedWidth)
        );
        assert_eq!(
            validate_fabs_contract_v1(ScalarFloatWidthV1::NotFloat, 1, true, true),
            Err(ProductionRustcIntrinsicErrorV1::FabsElementTypeArgument)
        );
        assert_eq!(
            validate_fabs_contract_v1(ScalarFloatWidthV1::F32, 1, false, true),
            Err(ProductionRustcIntrinsicErrorV1::FabsInputType)
        );
        assert_eq!(
            validate_fabs_contract_v1(ScalarFloatWidthV1::F32, 1, true, false),
            Err(ProductionRustcIntrinsicErrorV1::FabsResultType)
        );
        assert_eq!(
            ProductionRustcIntrinsicOperationV1::FabsF32.atomic_rmw(),
            None
        );
        assert_eq!(
            ProductionRustcIntrinsicOperationV1::FabsF32.operation_tag(),
            1
        );
        assert_eq!(atomic_rmw_intrinsic_rule_v1("fabs"), None);
        assert_eq!(atomic_rmw_intrinsic_rule_v1("fabsf"), None);
        assert!(is_fabs_intrinsic_name_v1("fabs"));
        for unknown in ["", "fabsf", "fabsf32", "llvm.fabs.f32", "sqrt"] {
            assert!(!is_fabs_intrinsic_name_v1(unknown));
        }
    }
}
