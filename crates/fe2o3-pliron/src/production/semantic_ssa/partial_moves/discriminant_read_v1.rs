//! A separate direct enum tag is not the moved variant payload.
//! Niche encodings and unmodeled/aliased projections retain the whole-value gate.

use super::*;
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBackendScalarV1, SemanticEnumEncodingV1, SemanticRustcVariantsV1,
};

#[cfg(test)]
mod tests;

fn has_separate_initialized_tag(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    let Some(declaration) = types.get(ty.index() as usize) else {
        return false;
    };
    if declaration.layout().is_uninhabited()
        || !matches!(declaration.shape(), SemanticTypeShapeV1::Enum { .. })
    {
        return false;
    }
    let SemanticRustcVariantsV1::Multiple(layout) = declaration.layout().variants() else {
        return false;
    };
    // Admitted direct layouts reserve these tag bytes and prohibit every variant
    // payload from overlapping them. A niche tag is stored in a payload instead.
    matches!(layout.encoding(), SemanticEnumEncodingV1::Direct(direct)
        if matches!(direct.tag(), SemanticBackendScalarV1::Initialized { .. }))
}

pub(super) fn validate(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    place: &SemanticPlaceV1,
    location: SemanticPartialMoveLocationV1,
    state: &SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    budget.charge_work()?;
    if !types.is_some_and(|types| has_separate_initialized_tag(types, place.ty())) {
        return validate_partial_move_place_read_v1(
            function, types, place, location, state, budget,
        );
    }
    let observed = {
        let _scratch = budget.projection_scratch(place)?;
        match canonical_partial_move_path_v1(function, types, place, location) {
            Ok(Some(path)) => Some(
                state
                    .direct_tag_readable(place.local().index(), &path, &budget.state)
                    .map_err(|error| budget.error(error))?,
            ),
            Ok(None) | Err(_) => None,
        }
    };
    match observed {
        Some(false) => Err(partial_move_error_v1(
            location,
            place.local().index(),
            SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
        )),
        Some(true) => validate_partial_move_projection_indices_v1(place, location, state, budget),
        None => {
            validate_partial_move_place_read_v1(function, types, place, location, state, budget)
        }
    }
}
