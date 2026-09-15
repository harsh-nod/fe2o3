//! Only copied primitive fields are read sinks. No arithmetic is summarized.
use fe2o3_mir_model::semantic_mir_v1::*;

pub(super) fn shared_snapshot<E>(
    types: &[SemanticTypeDeclV1],
    reference: SemanticTypeIdV1,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Option<SemanticTypeIdV1>, E> {
    charge(8)?;
    let Some(t) = types.get(reference.index() as usize) else {
        return Ok(None);
    };
    let SemanticTypeShapeV1::Pointer(p) = t.shape() else {
        return Ok(None);
    };
    if p.kind() != SemanticPointerKindV1::Reference
        || p.mutability() != SemanticMutabilityV1::Immutable
        || p.metadata() != SemanticPointerMetadataV1::None
        || p.address_space() != 0
        || p.pointer_width_bits() != 64
        || t.layout().size_bytes() != Some(8)
        || t.layout().alignment_bytes() != 8
        || t.layout().is_uninhabited()
        || !matches!(
            types.get(p.pointee().index() as usize).map(|t| t.shape()),
            Some(SemanticTypeShapeV1::Aggregate(_))
        )
        || !super::field_reader::plain_snapshot(types, p.pointee(), charge)?
    {
        return Ok(None);
    }
    Ok(Some(p.pointee()))
}

pub(super) fn copied_field<E>(
    assignment: &SemanticAssignmentV1,
    receiver: SemanticLocalIdV1,
    owned: SemanticTypeIdV1,
    types: &[SemanticTypeDeclV1],
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    charge(8)?;
    let SemanticRvalueKindV1::Use(SemanticOperandV1::Copy(p)) = assignment.value().kind() else {
        return Ok(false);
    };
    if p.local() != receiver
        || assignment.destination().projections().len() != 0
        || assignment.destination().ty() != p.ty()
        || assignment.value().result_type() != p.ty()
        || p.projections().len() < 2
        || p.projections().len() > 8
    {
        return Ok(false);
    }
    let mut projections = p.projections().iter();
    let first = projections.next().unwrap();
    if first.kind() != SemanticProjectionKindV1::Dereference || first.result_type() != owned {
        return Ok(false);
    }
    let mut current = owned;
    for projection in projections {
        charge(2)?;
        let SemanticProjectionKindV1::Field(index) = projection.kind() else {
            return Ok(false);
        };
        let Some(SemanticTypeShapeV1::Aggregate(a)) =
            types.get(current.index() as usize).map(|t| t.shape())
        else {
            return Ok(false);
        };
        let Some(field) = a.fields().get(index as usize) else {
            return Ok(false);
        };
        if *field != projection.result_type() {
            return Ok(false);
        }
        current = *field;
    }
    let Some(t) = types.get(current.index() as usize) else {
        return Ok(false);
    };
    let SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer {
        signed: false,
        bits: bits @ (32 | 64),
    }) = t.shape()
    else {
        return Ok(false);
    };
    let bytes = u64::from(*bits) / 8;
    Ok(current == p.ty()
        && t.layout().size_bytes() == Some(bytes)
        && t.layout().alignment_bytes() == bytes
        && !t.layout().is_uninhabited())
}
