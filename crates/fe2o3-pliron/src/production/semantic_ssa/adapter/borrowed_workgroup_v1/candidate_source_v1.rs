//! Reuse only classification of the same immutable candidate-definition statement.
use super::*;

pub(super) fn record(
    kind: SemanticBorrowCandidateSourceV1,
    budget: &mut Budget,
) -> Result<SemanticBorrowCandidateSourceV1, ProductionSemanticSsaErrorV1> {
    #[cfg(test)]
    if super::tests::candidate_source_tests::record_is_cold() {
        return Ok(kind);
    }
    // One extra retained classification write, within the existing candidate.
    budget.charge(1)?;
    Ok(kind)
}

pub(super) fn is_carrier(
    candidate: &SemanticBorrowCandidateV1,
    #[cfg(test)] routes: &math_capture_flow_v1::Routes<'_>,
    #[cfg(test)] assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    budget: &mut Budget,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    #[cfg(test)]
    if super::tests::candidate_source_tests::query_is_cold() {
        let before = budget.remaining;
        let result = routes.source(assignment, budget).map(|place| place.is_some());
        super::tests::candidate_source_tests::cold_query_work(before - budget.remaining);
        return result;
    }
    // The caller still requires the exact definition site and unique local map.
    budget.charge(1)?;
    Ok(candidate.source_kind == SemanticBorrowCandidateSourceV1::TypedCarrier)
}
