//! N2b pending route only. No call-access proof, ranked owner, capability
//! transport, tensor-layout emission or normal continuation is produced here.
use super::bf16_nominal_call_projection_v1::{
    CheckedNominalCallProjectionV1, RequiredNominalProjectionV1,
    with_bf16_nominal_call_projection_v1,
};
use super::*;
use fe2o3_kernel_analysis::CanonicalKirInventoryV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as QueryError, ProductionPreRankedKirOwnerV1,
};
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

/// Object-safe nonescaping candidate/budget handoff. A read-only N2b recorder
/// and the future N2c capability consumer use this SAME seam, but are different
/// visits. No detached observation or Boolean can substitute for the N2c visit.
pub(super) type NominalCallVisitorV1<'v> = dyn for<'call, 'work> FnMut(
        &CheckedNominalCallProjectionV1<'call>,
        &mut Budget<'work>,
    ) -> Result<(), QueryError>
    + 'v;

pub(super) fn query_error(error: QueryError) -> ProductionRankedProjectionErrorV1 {
    ProductionRankedProjectionErrorV1::CanonicalAssertions(match error {
        QueryError::Resource(error) => CanonicalAssertionErrorV1::Resource(error),
        other => CanonicalAssertionErrorV1::NominalCall(other),
    })
}

fn summary_storage<R>(count: usize) -> Result<usize, QueryError> {
    count
        .checked_mul(size_of::<DefinedCallableEmptyEffectDecisionV1>())
        .and_then(|n| n.checked_add(size_of::<DefinedCallableEmptyEffectSummariesV1>()))
        .and_then(|n| n.checked_add(size_of::<Vec<DefinedCallableEmptyEffectDecisionV1>>()))
        .and_then(|n| n.checked_add(4096))
        .and_then(|n| size_of::<R>().checked_mul(2).and_then(|r| n.checked_add(r)))
        .ok_or(QueryError::Resource(Resource::Arithmetic))
}

// Only a lexical summary survives into the callback. The whole finite decision
// vector/header/initialization is prepaid, not the unmetered legacy graph builder.
// This internal helper's synthetic tests are NOT evidence of source admission.
fn with_summary_vector<'w, R: Copy + 'static>(
    count: usize,
    helper: usize,
    budget: &mut Budget<'w>,
    inspect: impl for<'s> FnOnce(
        &'s DefinedCallableEmptyEffectSummariesV1,
        &mut Budget<'w>,
    ) -> Result<R, QueryError>,
) -> Result<R, QueryError> {
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    if count == 0 || count > MAX_DEFINED_CALLABLE_SUMMARY_FUNCTIONS_V1 || helper >= count {
        return Err(QueryError::Unavailable(
            "nominal summary source helper/count differs",
        ));
    }
    let ledger = budget.work_ledger_identity_v1();
    let reserved = summary_storage::<R>(count)?;
    budget.reserve_storage(reserved)?;
    let protected = budget.storage();
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        budget.charge_work(count.checked_add(8).ok_or(Resource::Arithmetic)?)?;
        let mut decisions = Vec::new();
        decisions
            .try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        // No retained excess-capacity allocation is admitted by the fixed vector
        // accounting. The reserved scratch also covers transient vector headers.
        if decisions.capacity() != count {
            return Err(QueryError::Resource(Resource::Allocation));
        }
        decisions.resize(count, DefinedCallableEmptyEffectDecisionV1::Rejected);
        decisions[helper] = DefinedCallableEmptyEffectDecisionV1::NominalTensorRequiresCall;
        let summaries = DefinedCallableEmptyEffectSummariesV1 {
            decisions: decisions.into_boxed_slice(),
        };
        let result = inspect(&summaries, budget);
        drop(summaries);
        result
    }));
    let result = match outcome {
        Ok(Ok(_)) if budget.failed_work().is_some() || budget.failed_storage().is_some() => {
            Err(Resource::Accounting.into())
        }
        Ok(result) => result,
        Err(payload) => {
            drop(payload);
            Err(QueryError::CallbackPanicked)
        }
    };
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < protected {
        return Err(Resource::Accounting.into());
    }
    // Refund ONLY this scope's reservation; callback-added storage remains live.
    // Payload/vector already dropped, including every unwind path above.
    budget.release_storage(reserved)?;
    result
}

/// Independent lexical summary entry, not a bypass around the ordinary refusing
/// RankedProjectionSourceV1 constructor. N2a performs N1's full original-floor
/// and exact borrowed owner/inventory/root/caller/block/call checks first.
#[allow(clippy::too_many_arguments)]
pub(super) fn with_nominal_summary_v1<'w, R: Copy + 'static>(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut Budget<'w>,
    inspect: impl for<'s> FnOnce(
        &'s DefinedCallableEmptyEffectSummariesV1,
        &mut Budget<'w>,
    ) -> Result<R, QueryError>,
) -> Result<R, QueryError> {
    with_bf16_nominal_call_projection_v1(
        owner,
        inventory,
        root,
        caller,
        block,
        call,
        budget,
        |candidate, budget| {
            budget.charge_work(2)?;
            let helper = candidate.call().emission().helper().index() as usize;
            let count = owner.semantic_ssa().source_semantic().functions().len();
            with_summary_vector(count, helper, budget, inspect)
        },
    )
}

/// An inert dispatch outcome, not a capability, proof, or retained source token.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum DefinedCallAccessRouteV1 {
    ExistingRoute,
    NominalPending,
}
pub(super) const NOMINAL_PENDING_V1: &str =
    "BF16 nominal call access requires pending N2c caller capability/layout/result join";

pub(super) fn require_defined_call_access_ready_v1(
    route: DefinedCallAccessRouteV1,
) -> Result<(), ProductionRankedProjectionErrorV1> {
    match route {
        DefinedCallAccessRouteV1::ExistingRoute => Ok(()),
        DefinedCallAccessRouteV1::NominalPending => Err(
            ProductionRankedProjectionErrorV1::Incomplete(NOMINAL_PENDING_V1),
        ),
    }
}

/// Shared by the ACTUAL access projector and future genuine-source N2b controls.
/// This helper is never a test-only duplicate of access dispatch.
pub(super) fn resolve_defined_call_access_route_v1(
    callables: &[SemanticCallableDeclV1],
    summaries: &DefinedCallableEmptyEffectSummariesV1,
    function: SemanticFunctionIdV1,
    block: usize,
    call: &SemanticDirectCallV1,
    source: SemanticSourceProvenanceV1,
    views: &mut ProjectedViewsV1<'_>,
) -> Result<DefinedCallAccessRouteV1, ProductionRankedProjectionErrorV1> {
    if !matches!(callables.get(call.callee().index() as usize),
        Some(SemanticCallableDeclV1::Defined { function: actual }) if *actual == function)
    {
        return Err(ProductionRankedProjectionErrorV1::Incomplete(
            "defined access route callee/source summary differs",
        ));
    }
    if summaries.is_nominal_tensor_requires_call(function) {
        let mut observed = false;
        let mut record = |candidate: &CheckedNominalCallProjectionV1<'_>,
                          budget: &mut Budget<'_>| {
            budget.charge_work(2)?;
            if observed {
                return Err(QueryError::Unavailable("nominal facts visitor repeated"));
            }
            match candidate.required_projection() {
                RequiredNominalProjectionV1::CallerCapabilitiesTensorLayoutFullWaveAndExactResults => {
                    observed = true;
                    Ok(())
                }
            }
        };
        views.with_nominal_call_v1(block, call, source, &mut record)?;
        if !observed {
            return Err(ProductionRankedProjectionErrorV1::Incomplete(
                "nominal facts visitor did not observe a borrowed candidate",
            ));
        }
        return Ok(DefinedCallAccessRouteV1::NominalPending);
    }
    if !summaries.is_exact_empty(function) {
        views.require_unit_local_call(block, call, source)?;
    } else {
        require_bounds_neutral_callable(callables, summaries, call.callee(), block, source, false)?;
    }
    Ok(DefinedCallAccessRouteV1::ExistingRoute)
}

#[cfg(test)]
#[path = "bf16_nominal_call_routing_v1_tests.rs"]
mod tests;
