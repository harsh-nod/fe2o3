//! Source-consistent descriptors, not capability issuance or lifecycle authority.

use fe2o3_mir_model::semantic_mir_v1::{
    SemanticAbiArgumentRoleV1, SemanticAbiPassModeV1, SemanticCanonAbiV1,
    SemanticExecutionOperationV29 as Operation, SemanticExecutionRoleV29 as Role,
    SemanticExternAbiV1, SemanticFunctionAbiV1, SemanticMutabilityV1, SemanticPointerKindV1,
    SemanticPointerMetadataV1, SemanticRustTypeKindV1, SemanticScalarTypeV1,
    SemanticSourceArgumentOwnershipV1 as Ownership, SemanticTypeDeclV1, SemanticTypeIdV1,
    SemanticTypeShapeV1,
};
use rustc_abi::ExternAbi;
use rustc_hir::Mutability;
use rustc_middle::ty::{Instance, Ty, TyCtxt, TyKind, UintTy};

use crate::production_semantic_terminal_v1::ProductionTerminalExpansionV1 as Expansion;
use crate::production_semantic_types_v1::execution_role_v29;
use crate::rustc_semantic_adapter_v1::rustc_type_identity_v1;
use crate::rustc_semantic_plan_v1::source_signature_v1;
use crate::trusted_device_items::{self, TrustedDeviceItem as Item};

pub(super) fn construct<'tcx>(
    tcx: TyCtxt<'tcx>,
    instance: Instance<'tcx>,
    expansion: Expansion,
    abi: &SemanticFunctionAbiV1,
    types: &[SemanticTypeDeclV1],
) -> Option<Operation> {
    let (item, arity) = match expansion {
        Expansion::ContextIssue => (Item::KernelContextIssue, 0),
        Expansion::WorkgroupDerive => (Item::ExecutionWorkgroupCurrent, 1),
        Expansion::MaskedTileLoadU32 => (Item::MaskedTile1DLoadMasked, 3),
        Expansion::MaskedTileIntoFragmentU32 => (Item::MaskedTile1DIntoFragment, 1),
        Expansion::LaneFragmentIntoPartsU32 => (Item::LaneFragment1DIntoParts, 1),
        _ => return None,
    };
    if trusted_device_items::classify(tcx, instance.def_id()) != Some(item) {
        return None;
    }
    let signature = source_signature_v1(tcx, instance).ok()?;
    let inputs = abi.source_input_types();
    let rust_inputs = signature.inputs();
    let output = abi.source_output_type();
    let rust_output = signature.output();
    if signature.abi != ExternAbi::Rust
        || signature.c_variadic
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.c_variadic()
        || abi.fixed_count() as usize != arity
        || inputs.len() != arity
        || rust_inputs.len() != arity
        || abi.arguments().len() != arity
        || abi.source_argument_ownership().len() != arity
        || !abi.hidden_arguments().is_empty()
        || abi.return_value().ty() != output
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
    {
        return None;
    }
    let bindings = DescriptorTypes { tcx, types };
    bindings.exact(output, rust_output)?;
    // The FnAbi constructor retains actual target pass modes. These terminals
    // additionally require an unchanged, non-hidden source signature.
    for (ordinal, (&input, &rust_input)) in inputs.iter().zip(rust_inputs).enumerate() {
        let argument = &abi.arguments()[ordinal];
        let ownership = match rust_input.kind() {
            TyKind::Ref(_, _, Mutability::Mut) => Ownership::UniqueBorrow,
            TyKind::Ref(_, _, Mutability::Not) => Ownership::SharedBorrow,
            _ => Ownership::ByValue,
        };
        if argument.role() != SemanticAbiArgumentRoleV1::Source
            || argument.ty() != input
            || argument.value().adjusted().is_some()
            || argument.value().pointee_override().is_some()
            || abi.source_argument_ownership()[ordinal] != ownership
        {
            return None;
        }
        bindings.exact(input, rust_input)?;
    }
    match expansion {
        Expansion::ContextIssue => {
            bindings.role(output, rust_output, Item::KernelContext)?;
            matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
                .then_some(Operation::ContextIssue { context: output })
        }
        Expansion::WorkgroupDerive => {
            let (context, rust_context) =
                bindings.reference(inputs[0], rust_inputs[0], Mutability::Mut)?;
            bindings.role(context, rust_context, Item::KernelContext)?;
            bindings.role(output, rust_output, Item::ExecutionWorkgroupCapability)?;
            Some(Operation::WorkgroupDerive {
                context,
                workgroup: output,
            })
        }
        Expansion::MaskedTileLoadU32 => {
            let (workgroup, rust_workgroup) =
                bindings.reference(inputs[0], rust_inputs[0], Mutability::Not)?;
            bindings.role(
                workgroup,
                rust_workgroup,
                Item::ExecutionWorkgroupCapability,
            )?;
            bindings.role(output, rust_output, Item::MaskedTile1D)?;
            let (slice, rust_slice) =
                bindings.reference(inputs[1], rust_inputs[1], Mutability::Not)?;
            let TyKind::Slice(rust_element) = *rust_slice.kind() else {
                return None;
            };
            let slice_record = bindings.exact(slice, rust_slice)?;
            let SemanticTypeShapeV1::Slice { element } = slice_record.shape() else {
                return None;
            };
            if slice_record.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
                || !matches!(rust_element.kind(), TyKind::Uint(UintTy::U32))
                || !matches!(rust_inputs[2].kind(), TyKind::Uint(UintTy::Usize))
            {
                return None;
            }
            bindings.scalar(*element, rust_element, U32)?;
            bindings.scalar(
                inputs[2],
                rust_inputs[2],
                SemanticScalarTypeV1::Integer {
                    bits: u16::try_from(tcx.data_layout.pointer_size().bits()).ok()?,
                    signed: false,
                },
            )?;
            Some(Operation::MaskedTileLoadU32 {
                workgroup,
                tile: output,
            })
        }
        Expansion::MaskedTileIntoFragmentU32 => {
            let Role::MaskedTileU32 { lanes, elements } =
                bindings.role(inputs[0], rust_inputs[0], Item::MaskedTile1D)?
            else {
                return None;
            };
            let fragment = bindings.role(output, rust_output, Item::LaneFragment1D)?;
            (fragment == Role::LaneFragmentU32 { lanes, elements }).then_some(
                Operation::MaskedTileIntoFragmentU32 {
                    tile: inputs[0],
                    fragment: output,
                },
            )
        }
        Expansion::LaneFragmentIntoPartsU32 => {
            let Role::LaneFragmentU32 { elements, .. } =
                bindings.role(inputs[0], rust_inputs[0], Item::LaneFragment1D)?
            else {
                return None;
            };
            let TyKind::Tuple(rust_parts) = rust_output.kind() else {
                return None;
            };
            let record = bindings.exact(output, rust_output)?;
            let SemanticTypeShapeV1::Tuple(parts) = record.shape() else {
                return None;
            };
            if record.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
                || rust_parts.len() != 2
                || parts.fields().len() != 2
            {
                return None;
            }
            bindings.array(parts.fields()[0], rust_parts[0], elements, U32)?;
            bindings.array(
                parts.fields()[1],
                rust_parts[1],
                elements,
                SemanticScalarTypeV1::Bool,
            )?;
            Some(Operation::LaneFragmentIntoPartsU32 {
                fragment: inputs[0],
                parts: output,
            })
        }
        _ => None,
    }
}

#[cfg(test)]
#[path = "execution_terminal_descriptors_v29_tests.rs"]
mod tests;

const U32: SemanticScalarTypeV1 = SemanticScalarTypeV1::Integer {
    bits: 32,
    signed: false,
};

struct DescriptorTypes<'a, 'tcx> {
    tcx: TyCtxt<'tcx>,
    types: &'a [SemanticTypeDeclV1],
}

impl<'a, 'tcx> DescriptorTypes<'a, 'tcx> {
    fn exact(&self, id: SemanticTypeIdV1, ty: Ty<'tcx>) -> Option<&'a SemanticTypeDeclV1> {
        self.types
            .get(id.index() as usize)
            .filter(|record| record.identity() == rustc_type_identity_v1(self.tcx, ty))
    }

    fn role(&self, id: SemanticTypeIdV1, ty: Ty<'tcx>, item: Item) -> Option<Role> {
        let TyKind::Adt(definition, _) = ty.kind() else {
            return None;
        };
        if trusted_device_items::classify(self.tcx, definition.did()) != Some(item) {
            return None;
        }
        let role = execution_role_v29(self.tcx, ty, item).ok()?;
        (self.exact(id, ty)?.rust_type_kind() == SemanticRustTypeKindV1::Execution(role))
            .then_some(role)
    }

    fn reference(
        &self,
        id: SemanticTypeIdV1,
        ty: Ty<'tcx>,
        mutability: Mutability,
    ) -> Option<(SemanticTypeIdV1, Ty<'tcx>)> {
        let TyKind::Ref(_, pointee, actual) = *ty.kind() else {
            return None;
        };
        let record = self.exact(id, ty)?;
        let SemanticTypeShapeV1::Pointer(pointer) = record.shape() else {
            return None;
        };
        let expected = match mutability {
            Mutability::Mut => SemanticMutabilityV1::Mutable,
            Mutability::Not => SemanticMutabilityV1::Immutable,
        };
        let metadata = if matches!(pointee.kind(), TyKind::Slice(_)) {
            SemanticPointerMetadataV1::SliceLength
        } else {
            SemanticPointerMetadataV1::None
        };
        if record.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
            || actual != mutability
            || pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.mutability() != expected
            || pointer.metadata() != metadata
        {
            return None;
        }
        self.exact(pointer.pointee(), pointee)?;
        Some((pointer.pointee(), pointee))
    }

    fn scalar(
        &self,
        id: SemanticTypeIdV1,
        ty: Ty<'tcx>,
        scalar: SemanticScalarTypeV1,
    ) -> Option<()> {
        let record = self.exact(id, ty)?;
        (record.rust_type_kind() == SemanticRustTypeKindV1::Ordinary
            && record.shape() == &SemanticTypeShapeV1::Scalar(scalar))
            .then_some(())
    }

    fn array(
        &self,
        id: SemanticTypeIdV1,
        ty: Ty<'tcx>,
        elements: u16,
        scalar: SemanticScalarTypeV1,
    ) -> Option<()> {
        let TyKind::Array(rust_element, count) = *ty.kind() else {
            return None;
        };
        let record = self.exact(id, ty)?;
        let SemanticTypeShapeV1::Array { element, length } = record.shape() else {
            return None;
        };
        if record.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
            || *length != u64::from(elements)
            || count.try_to_target_usize(self.tcx) != Some(*length)
        {
            return None;
        }
        self.scalar(*element, rust_element, scalar)
    }
}
