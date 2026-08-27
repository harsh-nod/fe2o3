//! Exact normalization rules for rustc compiler intrinsics with canonical MIR semantics.

use std::fmt;

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAtomicAccessV1, SemanticAtomicOrderingV1, SemanticAtomicRmwOpV1, SemanticAtomicScopeV1,
};
use rustc_abi::ExternAbi;
use rustc_hir::{Mutability, Safety};
use rustc_middle::ty::{
    self, ConstKind, Instance, InstanceKind, IntTy, Ty, TyCtxt, TyKind, UintTy,
};
use rustc_span::sym;

use crate::trusted_device_items::TrustedDeviceItem;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProductionRustcIntrinsicOperationV1 {
    AtomicRmw {
        operation: SemanticAtomicRmwOpV1,
        access: SemanticAtomicAccessV1,
    },
}

impl ProductionRustcIntrinsicOperationV1 {
    pub(crate) const fn operation_tag(self) -> u8 {
        match self {
            Self::AtomicRmw {
                operation: SemanticAtomicRmwOpV1::Add,
                ..
            } => 0,
            Self::AtomicRmw { .. } => u8::MAX,
        }
    }

    pub(crate) const fn atomic_rmw(self) -> (SemanticAtomicRmwOpV1, SemanticAtomicAccessV1) {
        match self {
            Self::AtomicRmw { operation, access } => (operation, access),
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ProductionRustcIntrinsicClassificationV1<'tcx> {
    pub(crate) operation: ProductionRustcIntrinsicOperationV1,
    pub(crate) element_type: Ty<'tcx>,
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
    OrderingArgument,
    UnsupportedOrdering,
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
            Self::OrderingArgument => "atomic intrinsic without a concrete ordering argument",
            Self::UnsupportedOrdering => "atomic intrinsic with an unsupported ordering value",
        })
    }
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
    if intrinsic.name != sym::atomic_xadd {
        return Err(ProductionRustcIntrinsicErrorV1::UnsupportedIntrinsic);
    }

    let arguments = instance.args.as_slice();
    if arguments.len() != 3 {
        return Err(ProductionRustcIntrinsicErrorV1::GenericArity);
    }
    let element_type = arguments[0]
        .as_type()
        .ok_or(ProductionRustcIntrinsicErrorV1::ElementTypeArgument)?;
    let value_type = arguments[1]
        .as_type()
        .ok_or(ProductionRustcIntrinsicErrorV1::ValueTypeArgument)?;
    if element_type != value_type {
        return Err(ProductionRustcIntrinsicErrorV1::MismatchedValueType);
    }
    if !supported_atomic_integer_v1(element_type) {
        return Err(ProductionRustcIntrinsicErrorV1::UnsupportedIntegerType);
    }
    let ordering_discriminant = arguments[2]
        .as_const()
        .and_then(|value| fieldless_enum_discriminant_v1(tcx, value))
        .ok_or(ProductionRustcIntrinsicErrorV1::OrderingArgument)?;
    let ordering = atomic_ordering_from_discriminant_v1(ordering_discriminant)
        .ok_or(ProductionRustcIntrinsicErrorV1::UnsupportedOrdering)?;

    Ok(Some(ProductionRustcIntrinsicClassificationV1 {
        operation: ProductionRustcIntrinsicOperationV1::AtomicRmw {
            operation: SemanticAtomicRmwOpV1::Add,
            access: SemanticAtomicAccessV1::new(ordering, SemanticAtomicScopeV1::System),
        },
        element_type,
    }))
}

pub(crate) fn is_reviewed_core_function_v1(tcx: TyCtxt<'_>, instance: Instance<'_>) -> bool {
    let Some(core_lang_item) = tcx.lang_items().sized_trait() else {
        return false;
    };
    instance.def_id().krate == core_lang_item.krate
        && tcx.crate_name(core_lang_item.krate).as_str() == "core"
}

pub(crate) fn is_reviewed_core_atomic_add_wrapper_v1(
    tcx: TyCtxt<'_>,
    instance: Instance<'_>,
) -> bool {
    if !matches!(instance.def, InstanceKind::Item(_))
        || !is_reviewed_core_function_v1(tcx, instance)
        || tcx.def_path_str(instance.def_id()) != "core::sync::atomic::atomic_add"
        || !tcx.is_mir_available(instance.def_id())
    {
        return false;
    }
    let arguments = instance.args.as_slice();
    let [element, value] = arguments else {
        return false;
    };
    let (Some(element), Some(value)) = (element.as_type(), value.as_type()) else {
        return false;
    };
    element == value && supported_atomic_integer_v1(element)
}

pub(crate) fn is_reviewed_core_unsafe_atomic_function_v1<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
) -> bool {
    is_reviewed_core_atomic_add_wrapper_v1(tcx, instance)
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
    fn normalized_atomic_operation_is_system_scoped_add() {
        let operation = ProductionRustcIntrinsicOperationV1::AtomicRmw {
            operation: SemanticAtomicRmwOpV1::Add,
            access: SemanticAtomicAccessV1::new(
                SemanticAtomicOrderingV1::Relaxed,
                SemanticAtomicScopeV1::System,
            ),
        };
        let (rmw, access) = operation.atomic_rmw();
        assert_eq!(rmw, SemanticAtomicRmwOpV1::Add);
        assert_eq!(access.ordering(), SemanticAtomicOrderingV1::Relaxed);
        assert_eq!(access.scope(), SemanticAtomicScopeV1::System);
        assert_eq!(operation.operation_tag(), 0);
    }
}
