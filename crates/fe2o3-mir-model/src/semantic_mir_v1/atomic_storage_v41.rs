//! Explicit inert atomic nominal facts; never a pointer or source capability.
use super::*;

pub(super) const TYPE_WORK: usize = 40;

pub(super) const fn tag(kind: SemanticRustTypeKindV1) -> Option<(u8, bool)> {
    match kind {
        SemanticRustTypeKindV1::AtomicI32 => Some((20, true)),
        SemanticRustTypeKindV1::AtomicU32 => Some((21, false)),
        _ => None,
    }
}

pub(super) fn validate_request_schema(
    request: &InertSemanticMirRequestV1,
    version: SemanticMirWireVersionV1,
) -> Result<(), SemanticMirErrorV1> {
    if version != SemanticMirWireVersionV1::V41
        && request
            .types
            .iter()
            .any(|ty| tag(ty.rust_type_kind).is_some())
    {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: version,
            required: SemanticMirWireVersionV1::V41,
        });
    }
    Ok(())
}

pub(super) fn check_type_version(
    ty: &SemanticTypeDeclV1,
    version: SemanticMirWireVersionV1,
) -> Result<(), SemanticMirErrorV1> {
    if tag(ty.rust_type_kind).is_some() && version != SemanticMirWireVersionV1::V41 {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: version,
            required: SemanticMirWireVersionV1::V41,
        });
    }
    Ok(())
}

fn storage_layout(ty: &SemanticTypeDeclV1) -> bool {
    ty.layout.size_bytes == Some(4)
        && ty.layout.rustc_size_bytes == 4
        && ty.layout.alignment_bytes == 4
        && ty.layout.unadjusted_abi_alignment_bytes == 4
        && !ty.layout.uninhabited
        && ty.layout.largest_niche.is_none()
        && !ty.abi_properties.has_unsized_foreign_tail
        && ty.abi_properties.first_pointee.is_none()
        && ty.abi_properties.second_pointee.is_none()
}

/// Re-derives the three exact field-zero edges of an inert AtomicI32/U32 fact.
///
/// The result is only a bounded structural observation. Caller-asserted nominal
/// data does not authenticate rustc, a source owner, an UnsafeCell, a live loan,
/// or permission to form/use any pointer. A production consumer must retain the
/// original compiler owner and its statement/generation/current-holder checks.
pub fn semantic_atomic_storage_chain_v41(
    types: &[SemanticTypeDeclV1],
    root: SemanticTypeIdV1,
) -> Option<[SemanticTypeIdV1; 3]> {
    let (_, signed) = tag(types.get(root.0 as usize)?.rust_type_kind)?;
    let mut current = root;
    let mut chain = [root; 3];
    for index in 0..3 {
        let ty = types.get(current.0 as usize)?;
        if !storage_layout(ty)
            || (index != 0 && ty.rust_type_kind != SemanticRustTypeKindV1::Ordinary)
        {
            return None;
        }
        let SemanticTypeShapeV1::Aggregate(fields) = &ty.shape else {
            return None;
        };
        let [child] = fields.fields.as_ref() else {
            return None;
        };
        let SemanticTypeLayoutDetailsV1::Aggregate(layout) = &ty.layout.details else {
            return None;
        };
        if layout.field_offsets.as_ref() != [0] || !layout.padding.is_empty() {
            return None;
        }
        let SemanticFieldsShapeV1::Arbitrary {
            source_order_offsets_bytes,
            memory_order_source_indices,
        } = &ty.layout.fields
        else {
            return None;
        };
        if source_order_offsets_bytes.as_ref() != [0]
            || memory_order_source_indices.as_ref() != [0]
            || *child == root
            || chain[..index].contains(child)
        {
            return None;
        }
        chain[index] = *child;
        current = *child;
    }
    let scalar = types.get(current.0 as usize)?;
    if scalar.rust_type_kind != SemanticRustTypeKindV1::Ordinary
        || !storage_layout(scalar)
        || scalar.shape
            != SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits: 32 })
    {
        return None;
    }
    Some(chain)
}

pub(super) fn validate_type(
    context: &mut ValidationContextV1<'_>,
    id: SemanticTypeIdV1,
    ty: &SemanticTypeDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    if tag(ty.rust_type_kind).is_none() {
        return Ok(());
    }
    // Exactly three edges and four records; no recursion or retained side table.
    charge_validation_work(context, TYPE_WORK)?;
    semantic_atomic_storage_chain_v41(&context.request.types, id)
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    Ok(())
}
