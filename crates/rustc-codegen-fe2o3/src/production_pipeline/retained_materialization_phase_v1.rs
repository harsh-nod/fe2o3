//! Retains the original materialization account across private consuming steps.
//!
//! This is custody, not a new allocation envelope. The account is established
//! before its first source callback, never reconstructed from a borrowed meter,
//! and lives after its payload. Existing materializer charges/limits are reused.
//! Constructor, Context, printer, descriptor/launch and this fixed control-owner
//! allocation are not newly charged or claimed bounded by this adapter.
use super::*;
use fe2o3_kernel_ir::{
    CanonicalKernelIrOwnedVerificationResourceBudgetV1 as OwnedBudget,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkBudgetV1 as Work,
};

type Error = Box<ProductionPipelineError>;
type Result<T> = std::result::Result<T, Error>;

fn accounting() -> Error {
    Box::new(materialization_resource_error_v29(Resource::Accounting))
}

// The Box is allocated before the first callback. Moving the containing phase
// moves only the Box, not its original work/storage state. No raw view escapes.
struct OriginalMaterializationAccountV1 {
    ledger: Box<OwnedBudget>,
}

impl OriginalMaterializationAccountV1 {
    fn new(work_limit: usize, storage_limit: usize) -> Self {
        Self {
            ledger: Box::new(OwnedBudget::new(Work::new(work_limit), storage_limit)),
        }
    }

    fn run<T>(&mut self, consume: impl FnOnce(&mut Budget<'_>) -> Result<T>) -> Result<T> {
        if self.ledger.failed_work().is_some() || self.ledger.failed_storage().is_some() {
            return Err(accounting());
        }
        self.ledger.with_budget(|budget| {
            let identity = budget.work_ledger_identity_v1();
            let floor = budget.storage();
            let work = budget.work();
            let peak = budget.peak_storage();
            // On unwind the callback's owned arguments/results unwind before
            // this account. No catch/retry or counter reconstruction is added.
            let result = consume(budget);
            if budget.work_ledger_identity_v1() != identity
                || budget.storage() < floor
                || budget.work() < work
                || budget.peak_storage() < peak
            {
                drop(result);
                return Err(accounting());
            }
            // Preserve an actual propagated failure, but do not publish a
            // successful owner after a callback swallowed a meter denial.
            if result.is_ok()
                && (budget.failed_work().is_some() || budget.failed_storage().is_some())
            {
                drop(result);
                return Err(accounting());
            }
            result
        })
    }
}

impl Drop for OriginalMaterializationAccountV1 {
    fn drop(&mut self) {
        // The entire account dies only after phase-owned payloads. There is no
        // refund into another live account, nor a reset of its denial history.
        #[cfg(test)]
        tests::observe_account_drop(&self.ledger);
    }
}

/// Private non-Clone custody wrapper. Field order is part of the contract.
/// It has no unguarded owned-payload or owned-account accessor.
///
/// Transitions below are trusted, compiler-private consuming continuations,
/// not a sandbox for callbacks that leak values into captures or forget them.
/// Each caller must keep every phase-owned result inside the returned wrapper.
pub(super) struct RetainedMaterializationPhaseV1<T> {
    payload: T,
    account: OriginalMaterializationAccountV1,
}

impl<T> RetainedMaterializationPhaseV1<T> {
    fn start(
        work_limit: usize,
        storage_limit: usize,
        consume: impl FnOnce(&mut Budget<'_>) -> Result<T>,
    ) -> Result<Self> {
        let mut account = OriginalMaterializationAccountV1::new(work_limit, storage_limit);
        let payload = account.run(consume)?;
        Ok(Self { payload, account })
    }

    /// Move the payload through one private continuation while retaining the
    /// original account, its live floor and all accepted/denied work. Existing
    /// reservations are conservatively retained until this whole phase drops.
    #[allow(dead_code)] // Private continuation selected by the genuine observer.
    pub(super) fn try_map<U>(
        self,
        consume: impl FnOnce(T, &mut Budget<'_>) -> Result<U>,
    ) -> Result<RetainedMaterializationPhaseV1<U>> {
        let Self {
            payload,
            mut account,
        } = self;
        let payload = account.run(|budget| consume(payload, budget))?;
        Ok(RetainedMaterializationPhaseV1 { payload, account })
    }
}

impl<T: Copy> RetainedMaterializationPhaseV1<T> {
    /// Only a fixed Copy observation can leave after all owning continuations.
    /// This is not an owned compiler-payload extraction path.
    #[allow(dead_code)] // Only terminal Copy observations leave the test continuation.
    pub(super) fn finish_copy(self) -> T {
        let observation = self.payload;
        drop(self);
        observation
    }
}

impl<'tcx> ProductionCompilation<'tcx, SsaSemanticMirStage> {
    // Exact original descriptor-to-roster closure, shared with the unchanged
    // stack-backed default budget entry. No additional policy or authority.
    pub(super) fn prepare_materialization_roster_v1(
        self,
    ) -> std::result::Result<PreparedSsaMaterializationV29, ProductionPipelineError> {
        self.prepare_materialization_inputs_v29(|typed_roots| {
            typed_roots
                .iter()
                .map(|typed_root| {
                    let source_launch = typed_root.source_launch().ok_or(
                        ProductionPipelineError::Geometry(
                            crate::production_geometry_v1::ProductionGeometryErrorV1::NonExactDescriptorWorkgroup,
                        ),
                    )?;
                    Ok(crate::production_ranked_projection_v1::ProductionRankedRootInputV1::new(
                        typed_root.logical_name(),
                        typed_root.kernel_binding_bytes(),
                        source_launch,
                    ))
                })
                .collect::<std::result::Result<Vec<_>, ProductionPipelineError>>()
        })
    }

    /// Opt-in owning counterpart of the existing materialization boundary.
    /// It starts the source account here, before any context/materialization
    /// callback, with the same original limits. It does not adopt a prior
    /// borrowed meter, manufacture source inputs or enable nominal compilation.
    #[allow(dead_code)] // Selected by genuine tests; normal production remains unchanged.
    pub(super) fn with_retained_materialization_budget_v1<T>(
        self,
        run: impl FnOnce(PreparedSsaMaterializationV29, &mut Budget<'_>) -> Result<T>,
    ) -> Result<RetainedMaterializationPhaseV1<T>> {
        let prepared = self.prepare_materialization_roster_v1()?;
        let work_limit = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
            .map_err(|_| materialization_resource_error_v29(Resource::Arithmetic))?;
        RetainedMaterializationPhaseV1::start(
            work_limit,
            crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
            |budget| run(prepared, budget),
        )
    }
}

#[cfg(test)]
#[path = "retained_materialization_phase_v1_tests.rs"]
mod tests;

// Opt-in prepaid continuation; no BF16 normal admission or default selection.
#[allow(dead_code)]
#[path = "retained_ranked_allowance_v1.rs"]
mod ranked_allowance;

#[path = "retained_target_pipeline_v1.rs"]
pub(super) mod retained_target_pipeline_v1;
