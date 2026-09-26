//! N2b observational facts only: never constructs a ranked/source admission.
//! Private child access keeps real facts fields sealed. The original N1 query
//! checks the complete owner/occurrence/inventory floor before sparse allocation.
use super::CanonicalSourceAssertionFactsV1;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV1, CanonicalKirSparseErrorV1, CanonicalKirSparseLimitsV1,
    CanonicalKirSparseV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    Bf16NominalCallQueryErrorV1 as QueryError, ProductionPreRankedKirOwnerV1,
};
use fe2o3_mir_model::semantic_mir_v1::{
    SemanticBlockIdV1, SemanticDirectCallV1, SemanticFunctionIdV1,
};
use std::mem::size_of;
use std::panic::{AssertUnwindSafe, catch_unwind};

type Result<T> = std::result::Result<T, QueryError>;

fn sparse_error(error: CanonicalKirSparseErrorV1) -> QueryError {
    match error {
        CanonicalKirSparseErrorV1::Resource(error) => QueryError::Resource(error),
        CanonicalKirSparseErrorV1::InputLimit { .. } => {
            QueryError::Unavailable("nominal facts sparse input limit")
        }
        CanonicalKirSparseErrorV1::InconsistentInventory => {
            QueryError::Unavailable("nominal facts sparse inventory inconsistent")
        }
        CanonicalKirSparseErrorV1::InvalidUseCoordinate { .. } => {
            QueryError::Unavailable("nominal facts sparse coordinate differs")
        }
    }
}

fn scope_storage<R>() -> Result<usize> {
    size_of::<CanonicalSourceAssertionFactsV1<'static, 'static, 'static, 'static, 'static>>()
        .checked_add(4096)
        .and_then(|n| {
            size_of::<Result<R>>()
                .checked_mul(2)
                .and_then(|r| n.checked_add(r))
        })
        .ok_or(QueryError::Resource(Resource::Arithmetic))
}

// The callback receives no second mutable budget alias; actual facts queries
// lend their original budget to the HRTB nominal visitor. Copy + 'static cannot
// carry a report, candidate or borrowed facts reference out of the lexical scope.
#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_canonical_facts_observation_v1<
    'g,
    'i,
    'w,
    R: Copy + 'static,
>(
    owner: &'g ProductionPreRankedKirOwnerV1,
    inventory: &'i CanonicalKirInventoryV1<'g>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut Budget<'w>,
    inspect: impl for<'r, 'b> FnOnce(
        &mut CanonicalSourceAssertionFactsV1<'r, 'i, 'g, 'b, 'w>,
    ) -> Result<R>,
) -> Result<R> {
    owner.with_checked_bf16_nominal_call_v1(
        inventory,
        root,
        caller,
        block,
        call,
        budget,
        |_, budget| with_sparse_facts(owner, inventory, root, caller, budget, inspect),
    )
}

fn with_sparse_facts<'g, 'i, 'w, R: Copy + 'static>(
    owner: &'g ProductionPreRankedKirOwnerV1,
    inventory: &'i CanonicalKirInventoryV1<'g>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    budget: &mut Budget<'w>,
    inspect: impl for<'r, 'b> FnOnce(
        &mut CanonicalSourceAssertionFactsV1<'r, 'i, 'g, 'b, 'w>,
    ) -> Result<R>,
) -> Result<R> {
    if budget.failed_work().is_some() || budget.failed_storage().is_some() {
        return Err(Resource::Accounting.into());
    }
    // Prepay the fixed construction and postflight checks, including error paths.
    budget.charge_work(12)?;
    let origins = owner.assert_origins();
    if !std::ptr::eq(origins.executable(), owner.executable()) {
        return Err(QueryError::Unavailable(
            "nominal facts origins executable differs",
        ));
    }
    let slot = budget as *const Budget<'w>;
    let ledger = budget.work_ledger_identity_v1();
    let header = scope_storage::<R>()?;
    budget.reserve_storage(header)?;
    let construction_floor = budget.storage();
    let derived = catch_unwind(AssertUnwindSafe(|| {
        CanonicalKirSparseV1::derive(inventory, CanonicalKirSparseLimitsV1::default(), budget)
    }));
    if budget as *const Budget<'w> != slot
        || budget.work_ledger_identity_v1() != ledger
        || budget.storage() < construction_floor
    {
        drop(derived);
        return Err(Resource::Accounting.into());
    }
    let (report, storage) = match derived {
        Ok(Ok(value)) => value,
        Ok(Err(error)) => {
            if budget.storage() != construction_floor {
                return Err(Resource::Accounting.into());
            }
            budget.release_storage(header)?;
            return Err(sparse_error(error));
        }
        Err(payload) => {
            drop(payload);
            // No consumer ran; only construction-owned scratch can remain.
            budget.release_storage(budget.storage() - construction_floor)?;
            budget.release_storage(header)?;
            return Err(QueryError::CallbackPanicked);
        }
    };
    if budget.storage() != construction_floor {
        drop(report);
        return Err(Resource::Accounting.into());
    }
    // Transfer the actual retained receipt immediately, before another
    // ledger-controlled allocation. No estimated row-count receipt is accepted.
    let retained = storage.retained_storage();
    if let Err(error) = budget.reserve_storage(retained) {
        drop(report);
        budget.release_storage(header)?;
        return Err(error.into());
    }
    let protected = budget.storage();
    if !report.belongs_to(inventory) {
        drop(report);
        budget.release_storage(retained)?;
        budget.release_storage(header)?;
        return Err(QueryError::Unavailable(
            "nominal facts sparse belongs to another inventory",
        ));
    }
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let mut facts = CanonicalSourceAssertionFactsV1 {
            owner,
            origins,
            report: &report,
            budget,
            correspondence_owner: root,
            semantic_function: caller,
            masked: None,
        };
        inspect(&mut facts)
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
    drop(report);
    if budget as *const Budget<'w> != slot
        || budget.work_ledger_identity_v1() != ledger
        || budget.storage() < protected
    {
        return Err(Resource::Accounting.into());
    }
    // Preserve callback-added storage, work, peak and first-denial history.
    budget.release_storage(retained)?;
    budget.release_storage(header)?;
    result
}

#[cfg(test)]
#[path = "bf16_nominal_routing_genuine_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(crate) use tests::{
    inspect_foreign_nominal_facts_refusal_for_test_v1, inspect_nominal_routing_genuine_for_test_v1,
};
