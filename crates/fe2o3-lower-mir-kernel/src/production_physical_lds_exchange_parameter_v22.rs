//! Narrow ABI-planner recognition for an actual MIR39 whole-body marker.
//! This supplies no source custody: the owner importer has already consumed the
//! current compiler source. The existing root planner still rechecks ownership,
//! adjusted Pair ABI, scalar-pair layout and every pointer/length field.
use super::*;

pub(super) fn physical_lds_exchange_slice_parameter_v22(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1, AccessMode)> {
    if argument != 1 || function.role() != SemanticFunctionRoleV1::KernelRoot {
        return None;
    }
    let Some(SemanticCallableDeclV1::Defined { function: root }) = callables.first() else {
        return None;
    };
    if root.index() != 0 || !(2..=43).contains(&callables.len()) {
        return None;
    }
    let mut markers = callables
        .iter()
        .enumerate()
        .filter_map(|(index, callable)| match callable {
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalLdsExchangeBegin(_),
                binding,
                ..
            } => Some((index, binding)),
            _ => None,
        });
    let (marker, binding) = markers.next()?;
    if markers.next().is_some() {
        return None;
    }
    // Require this exact root's attached occurrence, not merely an otherwise
    // unused declaration or another root's compatible-looking wrapper type.
    let mut occurrence = None;
    for block in function.blocks() {
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && call.callee().index() as usize == marker
        {
            let source = call.physical_lds_exchange_source_v39()?;
            if source.occurrence() != 0
                || occurrence.replace(()).is_some()
                || !source.matches_function(function)
                || source.block_identity() != *block.identity().as_bytes()
                || call.arguments().get(1)?.ty() != ty
            {
                return None;
            }
        }
    }
    occurrence?;
    let (element, length) = physical_lds_exchange_marker_slice_abi_v22(types, binding.abi(), ty)?;
    Some((element, length, AccessMode::ReadWrite))
}

fn physical_lds_exchange_marker_slice_abi_v22(
    types: &[SemanticTypeDeclV1],
    abi: &fe2o3_mir_model::semantic_mir_v1::SemanticFunctionAbiV1,
    ty: SemanticTypeIdV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1)> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticExternAbiV1;
    let inputs = abi.source_input_types();
    if abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.can_unwind()
        || abi.c_variadic()
        || abi.fixed_count() != 2
        || inputs.len() != 2
        || inputs[1] != ty
        || abi.source_argument_ownership()
            != [
                SemanticSourceArgumentOwnershipV1::SharedBorrow,
                SemanticSourceArgumentOwnershipV1::ExclusiveOwner,
            ]
        || abi.adjusted_arguments().len() != 2
        || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
        || !matches!(
            types
                .get(abi.source_output_type().index() as usize)?
                .shape(),
            SemanticTypeShapeV1::Unit
        )
    {
        return None;
    }
    for (input, adjusted) in inputs.iter().zip(abi.adjusted_arguments()) {
        let declaration = types.get(input.index() as usize)?;
        if adjusted.ty() != *input
            || adjusted.value().adjusted().is_some()
            || adjusted.value().pointee_override().is_some()
            || !matches!(adjusted.mode(), SemanticAbiPassModeV1::Pair { .. })
            || declaration.layout().size_bytes() != Some(16)
            || declaration.layout().alignment_bytes() != 8
            || declaration.layout().is_uninhabited()
            || declaration.rust_type_kind()
                != fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1::Ordinary
        {
            return None;
        }
    }
    let element = shared_u32_element(types, inputs[0])?;
    let length = physical_lds_exchange_slice_fields_v22(types, ty, element)?;
    Some((element, length))
}

/// Derive u32 from the actual immutable reference-to-slice pointee, never a
/// missing scalar argument or a guessed wrapper field. Source custody remains
/// with the caller's retained MIR39 owner.
pub(super) fn shared_u32_element(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<SemanticTypeIdV1> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1;
    let declaration = types.get(ty.index() as usize)?;
    if declaration.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
        || declaration.layout().size_bytes() != Some(16)
        || declaration.layout().alignment_bytes() != 8
        || declaration.layout().is_uninhabited()
    {
        return None;
    }
    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
        return None;
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::SliceLength
    {
        return None;
    }
    let SemanticTypeShapeV1::Slice { element } =
        types.get(pointer.pointee().index() as usize)?.shape()
    else {
        return None;
    };
    let word = types.get(element.index() as usize)?;
    if word.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
        || !matches!(
            word.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 32
            })
        )
        || word.layout().size_bytes() != Some(4)
        || word.layout().alignment_bytes() != 4
        || word.layout().is_uninhabited()
    {
        return None;
    }
    Some(*element)
}

/// Exact source field order of the trusted repr(C) DisjointSlice<u32, Index1D>.
/// Only called after the actual marker's exact source second argument has matched.
fn physical_lds_exchange_slice_fields_v22(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    element: SemanticTypeIdV1,
) -> Option<SemanticTypeIdV1> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticRustTypeKindV1;
    let declaration = types.get(ty.index() as usize)?;
    let SemanticTypeShapeV1::Aggregate(aggregate) = declaration.shape() else {
        return None;
    };
    let SemanticTypeLayoutDetailsV1::Aggregate(layout) = declaration.layout().details() else {
        return None;
    };
    let [pointer, length, phantom] = aggregate.fields() else {
        return None;
    };
    if layout.field_offsets() != [0, 8, 16]
        || !layout.padding().is_empty()
        || declaration.layout().size_bytes() != Some(16)
        || declaration.layout().alignment_bytes() != 8
        || declaration.layout().is_uninhabited()
    {
        return None;
    }
    let pointer = types.get(pointer.index() as usize)?;
    let SemanticTypeShapeV1::Pointer(pointer) = pointer.shape() else {
        return None;
    };
    if pointer.pointee() != element
        || pointer.kind() != SemanticPointerKindV1::Raw
        || pointer.mutability() != SemanticMutabilityV1::Mutable
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::None
    {
        return None;
    }
    let length_type = types.get(length.index() as usize)?;
    let phantom = types.get(phantom.index() as usize)?;
    // MIR39 deliberately retains the ordinary integer representation; V35
    // nominal pointer-sized kinds are a separate, excluded schema. The actual
    // trusted marker ABI and exact source wrapper type establish usize lineage;
    // a detached u64-shaped aggregate never reaches this predicate alone.
    if length_type.rust_type_kind() != SemanticRustTypeKindV1::Ordinary
        || !matches!(
            length_type.shape(),
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: 64
            })
        )
        || length_type.layout().size_bytes() != Some(8)
        || length_type.layout().alignment_bytes() != 8
        || phantom.layout().size_bytes() != Some(0)
        || phantom.layout().alignment_bytes() != 1
        || phantom.layout().is_uninhabited()
    {
        return None;
    }
    Some(*length)
}
