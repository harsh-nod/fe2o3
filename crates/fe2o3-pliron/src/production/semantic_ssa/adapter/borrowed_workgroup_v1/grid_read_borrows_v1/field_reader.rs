//! An inert complete-body observation, not a source or borrow certificate.
use fe2o3_mir_model::semantic_mir_v1::*;

#[cfg(test)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct Reader {
    pub reference: SemanticTypeIdV1,
    pub owned: SemanticTypeIdV1,
    pub result: SemanticTypeIdV1,
    pub receiver: SemanticLocalIdV1,
    pub field: u32,
}

#[cfg(test)]
pub(super) fn observe<E>(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Option<Reader>, E> {
    charge(32)?;
    let abi = function.abi();
    if function.role() != SemanticFunctionRoleV1::InternalHelper
        || function.export().is_some()
        || function.blocks().len() != 1
        || function.entry().index() != 0
        || function.locals().len() != 2
        || function.defined_capability_contract().is_some()
        || abi.canon_abi() != SemanticCanonAbiV1::Rust
        || abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.can_unwind()
        || abi.c_variadic()
        || !abi.hidden_arguments().is_empty()
        || abi.fixed_count() != 1
        || abi.source_argument_ownership() != [SemanticSourceArgumentOwnershipV1::SharedBorrow]
    {
        return Ok(None);
    }
    let ([reference], [argument]) = (abi.source_input_types(), abi.arguments()) else {
        return Ok(None);
    };
    let result = abi.source_output_type();
    if argument.ty() != *reference
        || argument.role() != SemanticAbiArgumentRoleV1::Source
        || !matches!(argument.value().mode(), SemanticAbiPassModeV1::Direct(_))
        || argument.value().adjusted().is_some()
        || argument.value().pointee_override().is_some()
        || abi.return_value().ty() != result
        || abi.return_value().adjusted().is_some()
        || abi.return_value().pointee_override().is_some()
    {
        return Ok(None);
    }
    let Some(declaration) = types.get(reference.index() as usize) else {
        return Ok(None);
    };
    let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
        return Ok(None);
    };
    if pointer.kind() != SemanticPointerKindV1::Reference
        || pointer.mutability() != SemanticMutabilityV1::Immutable
        || pointer.address_space() != 0
        || pointer.pointer_width_bits() != 64
        || pointer.metadata() != SemanticPointerMetadataV1::None
        || declaration.layout().size_bytes() != Some(8)
        || declaration.layout().alignment_bytes() != 8
        || declaration.layout().is_uninhabited()
    {
        return Ok(None);
    }
    let owned = pointer.pointee();
    let Some(SemanticTypeShapeV1::Aggregate(aggregate)) =
        types.get(owned.index() as usize).map(|ty| ty.shape())
    else {
        return Ok(None);
    };
    let mut receiver = None;
    let mut returned = None;
    for (index, local) in function.locals().iter().enumerate() {
        let id = SemanticLocalIdV1::from_index(index as u32);
        match local.role() {
            SemanticLocalRoleV1::Argument(0) if local.ty() == *reference => receiver = Some(id),
            SemanticLocalRoleV1::Return if local.ty() == result => returned = Some(id),
            _ => return Ok(None),
        }
    }
    let (Some(receiver), Some(returned)) = (receiver, returned) else {
        return Ok(None);
    };
    let block = &function.blocks()[0];
    let [statement] = block.statements() else {
        return Ok(None);
    };
    if !matches!(block.terminator().kind(), SemanticTerminatorKindV1::Return) {
        return Ok(None);
    }
    let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
        return Ok(None);
    };
    if assignment.destination().local() != returned
        || !assignment.destination().projections().is_empty()
        || assignment.destination().ty() != result
        || assignment.value().result_type() != result
    {
        return Ok(None);
    }
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(place)) = assignment.value().kind()
    else {
        return Ok(None);
    };
    let [deref, field] = place.projections() else {
        return Ok(None);
    };
    let SemanticProjectionKindV1::Field(index) = field.kind() else {
        return Ok(None);
    };
    if place.local() != receiver
        || place.ty() != result
        || deref.kind() != SemanticProjectionKindV1::Dereference
        || deref.result_type() != owned
        || field.result_type() != result
        || aggregate.fields().get(index as usize) != Some(&result)
        || !plain_snapshot(types, owned, charge)?
    {
        return Ok(None);
    }
    Ok(Some(Reader {
        reference: *reference,
        owned,
        result,
        receiver,
        field: index,
    }))
}

// A fixed stack closes cycles, depth and total visits without allocating a
// second type graph. The caller's work counter pays every inspected node/edge.
pub(super) fn plain_snapshot<E>(
    types: &[SemanticTypeDeclV1],
    root: SemanticTypeIdV1,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    charge(64)?;
    let mut stack = [root; 64];
    let mut pending = 1_usize;
    let mut visited = 0_usize;
    while pending != 0 {
        charge(1)?;
        if visited == 64 {
            return Ok(false);
        }
        visited += 1;
        pending -= 1;
        let Some(ty) = types.get(stack[pending].index() as usize) else {
            return Ok(false);
        };
        if ty.layout().is_uninhabited() || ty.layout().size_bytes().is_none() {
            return Ok(false);
        }
        match ty.shape() {
            SemanticTypeShapeV1::Unit => {
                if ty.layout().size_bytes() != Some(0) || ty.layout().alignment_bytes() != 1 {
                    return Ok(false);
                }
            }
            SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
                signed: false,
                bits: bits @ (32 | 64),
            }) => {
                let bytes = u64::from(*bits) / 8;
                if ty.layout().size_bytes() != Some(bytes) || ty.layout().alignment_bytes() != bytes
                {
                    return Ok(false);
                }
            }
            SemanticTypeShapeV1::Aggregate(a) => {
                charge(a.fields().len())?;
                let Some(end) = pending
                    .checked_add(a.fields().len())
                    .filter(|end| *end <= stack.len())
                else {
                    return Ok(false);
                };
                stack[pending..end].copy_from_slice(a.fields());
                pending = end;
            }
            _ => return Ok(false),
        }
    }
    Ok(true)
}

#[cfg(test)]
#[path = "field_reader/tests.rs"]
mod tests;
