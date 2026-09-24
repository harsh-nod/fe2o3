//! Narrow ABI-planner recognition for an actual MIR37 whole-body marker.
//! This supplies no source custody: the owner importer has already consumed the
//! current compiler source. The existing root planner still rechecks ownership,
//! adjusted Pair ABI, scalar-pair layout and every pointer/length field.
use super::*;

pub fn physical_entry_slice_parameter_v20(
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    function: &SemanticFunctionDeclV1,
    argument: u32,
    ty: SemanticTypeIdV1,
) -> Option<(SemanticTypeIdV1, SemanticTypeIdV1, AccessMode)> {
    if argument != 0 || function.role() != SemanticFunctionRoleV1::KernelRoot {
        return None;
    }
    let Some(SemanticCallableDeclV1::Defined { function: root }) = callables.first() else {
        return None;
    };
    if root.index() != 0 || !(2..=74).contains(&callables.len()) {
        return None;
    }
    let mut markers = callables
        .iter()
        .enumerate()
        .filter_map(|(index, callable)| match callable {
            SemanticCallableDeclV1::CompilerIntrinsic {
                operation: SemanticCompilerIntrinsicOperationV1::Gfx942PhysicalEntryBegin,
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
            let source = call.physical_entry_source_v37()?;
            if source.occurrence() != 0
                || occurrence.replace(()).is_some()
                || !source.matches_function(function)
                || source.block_identity() != *block.identity().as_bytes()
                || call.arguments().first()?.ty() != ty
            {
                return None;
            }
        }
    }
    occurrence?;
    let (element, length) = physical_entry_marker_slice_abi_v20(types, binding.abi(), ty)?;
    Some((element, length, AccessMode::ReadWrite))
}

fn physical_entry_marker_slice_abi_v20(
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
        || abi.fixed_count() != 5
        || abi.source_argument_ownership().len() != 5
        || inputs.len() != 5
        || inputs[0] != ty
        || abi.source_argument_ownership().first()
            != Some(&SemanticSourceArgumentOwnershipV1::ExclusiveOwner)
        || abi.source_argument_ownership()[1..]
            .iter()
            .any(|kind| *kind != SemanticSourceArgumentOwnershipV1::ByValue)
        || abi.adjusted_arguments().len() != 5
        || !matches!(abi.return_value().mode(), SemanticAbiPassModeV1::Ignore)
        || !matches!(
            types
                .get(abi.source_output_type().index() as usize)?
                .shape(),
            SemanticTypeShapeV1::Unit
        )
    {
        return None;
    }
    for (index, (input, adjusted)) in inputs.iter().zip(abi.adjusted_arguments()).enumerate() {
        if adjusted.ty() != *input || adjusted.value().adjusted().is_some() {
            return None;
        }
        if index == 0 {
            if !matches!(adjusted.mode(), SemanticAbiPassModeV1::Pair { .. }) {
                return None;
            }
        } else {
            let bits = 32;
            if !matches!(types.get(input.index() as usize)?.shape(),
                SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed: false, bits: actual }) if *actual == bits)
                || !matches!(adjusted.mode(), SemanticAbiPassModeV1::Direct(_))
                || *input != inputs[1]
            {
                return None;
            }
        }
    }
    let length = physical_entry_slice_fields_v20(types, ty, inputs[1])?;
    Some((inputs[1], length))
}

/// Exact source field order of the trusted repr(C) DisjointSlice<u32, Index1D>.
/// Only called after the actual marker's exact source first argument has matched.
fn physical_entry_slice_fields_v20(
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
    // MIR37 deliberately retains the ordinary integer representation; V35
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
