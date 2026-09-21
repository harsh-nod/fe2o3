//! Closed nominal pointer-sized source kinds, not source-origin authority.

use super::*;

pub(super) const NOMINAL_TYPE_WORK_V35: usize = 8;

pub(super) const fn kind(kind: SemanticRustTypeKindV1) -> Option<(u8, bool)> {
    match kind {
        SemanticRustTypeKindV1::Usize => Some((18, false)),
        SemanticRustTypeKindV1::Isize => Some((19, true)),
        _ => None,
    }
}

pub(super) fn contains_nominal(request: &InertSemanticMirRequestV1) -> bool {
    request
        .types
        .iter()
        .any(|ty| kind(ty.rust_type_kind).is_some())
}

pub(super) fn validate_request_schema(
    request: &InertSemanticMirRequestV1,
    wire_version: SemanticMirWireVersionV1,
) -> Result<(), SemanticMirErrorV1> {
    if wire_version == SemanticMirWireVersionV1::V35 {
        let required = minimum_wire_version_without_nominal(request);
        if required > SemanticMirWireVersionV1::V15 {
            return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
                requested: wire_version,
                required,
            });
        }
    } else if contains_nominal(request) {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V35,
        });
    }
    Ok(())
}

pub(super) fn check_type_version(
    ty: &SemanticTypeDeclV1,
    wire_version: SemanticMirWireVersionV1,
) -> Result<(), SemanticMirErrorV1> {
    if kind(ty.rust_type_kind).is_some() && wire_version != SemanticMirWireVersionV1::V35 {
        return Err(SemanticMirErrorV1::WireVersionCannotRepresent {
            requested: wire_version,
            required: SemanticMirWireVersionV1::V35,
        });
    }
    Ok(())
}

pub(super) fn validate_type(
    context: &mut ValidationContextV1<'_>,
    ty: &SemanticTypeDeclV1,
) -> Result<(), SemanticMirErrorV1> {
    let Some((_, signed)) = kind(ty.rust_type_kind) else {
        return Ok(());
    };
    // Eight bounded checks: target, integer shape, width, size, alignment,
    // indirect/unsized flags, definedness and pointee metadata. The existing
    // scalar validator still checks full backend validity and primitive layout.
    charge_validation_work(context, NOMINAL_TYPE_WORK_V35)?;
    let pointer = target_pointer_profile(context.request.target, 0)
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    let bits = pointer
        .size_bytes
        .checked_mul(8)
        .and_then(|bits| u16::try_from(bits).ok())
        .ok_or(SemanticMirErrorV1::InvalidTypeLayout)?;
    if ty.shape != (SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Integer { signed, bits }))
        || ty.layout.size_bytes != Some(pointer.size_bytes)
        || ty.layout.rustc_size_bytes != pointer.size_bytes
        || ty.layout.alignment_bytes != pointer.size_bytes
        || ty.layout.unadjusted_abi_alignment_bytes != pointer.size_bytes
        || ty.abi_properties.pass_indirectly_in_non_rustic_abis
        || ty.abi_properties.has_unsized_foreign_tail
        || !ty.abi_properties.rustc_layout_is_noundef
        || ty.abi_properties.first_pointee.is_some()
        || ty.abi_properties.second_pointee.is_some()
    {
        return Err(SemanticMirErrorV1::InvalidTypeLayout);
    }
    Ok(())
}
