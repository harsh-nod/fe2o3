// Type edges only. These helpers cannot create capability or source authority.
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticMutabilityV1, SemanticPointerKindV1, SemanticPointerMetadataV1,
    SemanticProjectionKindV1, SemanticProjectionV1, SemanticTypeDeclV1, SemanticTypeIdV1,
    SemanticTypeShapeV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum PathError {
    Type,
    Projection,
}

pub(in super::super) fn shared_pointee(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<SemanticTypeIdV1> {
    match types.get(ty.index() as usize)?.shape() {
        SemanticTypeShapeV1::Pointer(p)
            if p.kind() == SemanticPointerKindV1::Reference
                && p.mutability() == SemanticMutabilityV1::Immutable
                && p.address_space() == 0
                && p.pointer_width_bits() == 64
                && p.metadata() == SemanticPointerMetadataV1::None =>
        {
            Some(p.pointee())
        }
        _ => None,
    }
}

pub(in super::super) fn aggregate_fields(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
) -> Option<&[SemanticTypeIdV1]> {
    match types.get(ty.index() as usize)?.shape() {
        SemanticTypeShapeV1::Aggregate(a) | SemanticTypeShapeV1::Tuple(a) => Some(a.fields()),
        _ => None,
    }
}

pub(in super::super) fn check_path(
    types: &[SemanticTypeDeclV1],
    mut ty: SemanticTypeIdV1,
    path: &[SemanticProjectionV1],
    target: SemanticTypeIdV1,
) -> Result<(), PathError> {
    for projection in path {
        ty = match projection.kind() {
            SemanticProjectionKindV1::Dereference => {
                shared_pointee(types, ty).ok_or(PathError::Type)?
            }
            SemanticProjectionKindV1::Field(index) => *aggregate_fields(types, ty)
                .and_then(|fields| fields.get(index as usize))
                .ok_or(PathError::Type)?,
            _ => return Err(PathError::Projection),
        };
        if ty != projection.result_type() {
            return Err(PathError::Type);
        }
    }
    if ty != target {
        return Err(PathError::Type);
    }
    Ok(())
}
