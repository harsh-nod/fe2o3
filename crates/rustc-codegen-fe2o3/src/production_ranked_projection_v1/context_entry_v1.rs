//! Consume only the frontend-authenticated entry relation, never a ZST type.
use super::*;
pub(super) use fe2o3_lower_mir_kernel::ProductionKernelContextEntrySsaRelationV1 as Entry;

// This tuple is returned only after the private frontend custody and entry replay
// checks. The relation remains borrowed from the exact current execution owner.
pub(super) fn invocation_anchor(
    function: &SemanticFunctionDeclV1,
    entry: Option<&(Entry<'_>, SemanticKernelCapabilityProvenanceV1)>,
) -> Result<Option<(SemanticTypeIdV1, SemanticLocalIdV1, SemanticKernelCapabilityProvenanceV1)>, ProductionRankedProjectionErrorV1> {
    entry.map(|(entry, provenance)| {
        if !entry.matches_body(function) {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "ranked Context entry SSA owner changed",
            ));
        }
        Ok((entry.context(), entry.issuer_local(), *provenance))
    }).transpose()
}

pub(super) fn transfer(
    entry: Option<&Entry<'_>>,
    function: &SemanticFunctionDeclV1,
    block: usize,
    statement: usize,
    assignment: &fe2o3_mir_model::semantic_mir_v1::SemanticAssignmentV1,
    state: &mut ProjectedCapabilityStateV1,
) -> Result<bool, ProductionRankedProjectionErrorV1> {
    let Some(entry) = entry else { return Ok(false); };
    if !entry.matches_body(function) {
        return Err(ProductionRankedProjectionErrorV1::Incomplete("ranked Context entry SSA owner changed"));
    }
    if entry.block().index() as usize != block || entry.statement() as usize != statement {
        return Ok(false);
    }
    let context = entry.context();
    if assignment.destination().local() != entry.destination()
        || !assignment.destination().projections().is_empty()
        || assignment.destination().ty() != context
        || assignment.value().result_type() != context
        || !matches!(assignment.value().kind(), SemanticRvalueKindV1::Use(SemanticOperandV1::Constant(c))
            if c.ty() == context && matches!(c.value(), fe2o3_mir_model::semantic_mir_v1::SemanticConstantValueV1::ZeroSized))
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete("ranked Context entry parameter statement changed"));
    }
    transfer_origin(state, entry.issuer_local().index() as usize,
        entry.destination().index() as usize, context);
    Ok(true)
}

fn transfer_origin(state: &mut ProjectedCapabilityStateV1, issuer: usize, destination: usize, context: SemanticTypeIdV1) {
    // Missing and Invalid remain the same bottom fact during fixed-point
    // iteration. The original receiver check rejects an unproved final origin.
    let origin = match state.remove(&issuer) {
        Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::KernelContext {
            context: actual, borrow: context_borrow_v1::Borrow::Owned,
        })) if actual == context => Some(ProjectedCapabilityValueV1::Known(
            ProjectedCapabilityOriginV1::KernelContext { context, borrow: context_borrow_v1::Borrow::Owned })),
        _ => None,
    };
    if let Some(origin) = origin { state.insert(destination, origin); }
    else { state.remove(&destination); }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_entry_moves_only_the_exact_owned_origin() {
        let context = SemanticTypeIdV1::from_index(7);
        let owned = ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::KernelContext { context, borrow: context_borrow_v1::Borrow::Owned });
        let mut state = HashMap::from([(2, owned.clone())]);
        transfer_origin(&mut state, 2, 9, context);
        assert_eq!(state.get(&9), Some(&owned));
        assert!(!state.contains_key(&2));
        transfer_origin(&mut state, 2, 9, context);
        assert!(!state.contains_key(&9));
    }

    #[test]
    fn missing_invalid_borrowed_and_other_context_cannot_issue_an_entry() {
        let context = SemanticTypeIdV1::from_index(7);
        for origin in [None, Some(ProjectedCapabilityValueV1::Invalid),
            Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::KernelContext { context, borrow: context_borrow_v1::Borrow::Shared })),
            Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::KernelContext { context, borrow: context_borrow_v1::Borrow::Exclusive })),
            Some(ProjectedCapabilityValueV1::Known(ProjectedCapabilityOriginV1::KernelContext { context: SemanticTypeIdV1::from_index(8), borrow: context_borrow_v1::Borrow::Owned })),
        ] {
            let mut state = HashMap::new();
            if let Some(origin) = origin { state.insert(2, origin); }
            transfer_origin(&mut state, 2, 9, context);
            assert!(!state.contains_key(&9));
        }
    }
}
