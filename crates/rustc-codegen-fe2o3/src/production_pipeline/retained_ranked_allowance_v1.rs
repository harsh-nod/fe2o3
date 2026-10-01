//! Prepaid analysis/presentation allowances on the original retained account.
//! Constructor, recipe hashing, Context/dialect, Display internals and the
//! complete lowering-owner heap are explicitly outside this component's quote.
use super::*;
use fe2o3_pliron::{
    ProductionConstructionV1, ProductionRankedAnalysisAllowanceV1 as Analysis,
    ProductionRankedCompileErrorV1, ProductionRankedKernelLoweringInputV1,
    ProductionRankedSnapshotAllowanceV1 as Snapshot, ProductionSessionLimitsV1,
    compile_ranked_kernel_for_lowering_with_analysis_and_snapshot_allowances_v1,
};

/// Created only after admission on the phase's original account. No Clone/Copy
/// and no public constructor. Consuming compile permits one session only.
/// Trusted private continuations must keep this permit and any returned lowering
/// input inside the same phase payload; captures/leaks are not a sandboxed API.
pub(in crate::production_pipeline) struct PaidRankedAllowancesV1 {
    analysis: Analysis,
    snapshot: Snapshot,
}

impl PaidRankedAllowancesV1 {
    #[allow(dead_code)] // Opt-in bridge; no BF16 normal route is enabled.
    pub(in crate::production_pipeline) fn compile(
        self,
        construction: ProductionConstructionV1,
        limits: ProductionSessionLimitsV1,
    ) -> std::result::Result<ProductionRankedKernelLoweringInputV1, ProductionRankedCompileErrorV1>
    {
        compile_ranked_kernel_for_lowering_with_analysis_and_snapshot_allowances_v1(
            construction,
            limits,
            self.analysis,
            self.snapshot,
        )
    }
}

// Fixed control-frame terms are the unchanged original window's own quote.
// Snapshot output bytes are a streaming cap, NOT a retained-byte allocation.
pub(super) fn quote_values(
    analysis_work: usize,
    snapshot_work: usize,
    analysis_storage: usize,
) -> std::result::Result<(usize, usize), Resource> {
    let work = analysis_work
        .checked_add(snapshot_work)
        .and_then(|value| value.checked_add(Budget::STORAGE_WINDOW_WORK_V1))
        .ok_or(Resource::Arithmetic)?;
    let storage = analysis_storage
        .checked_add(Budget::STORAGE_WINDOW_SCRATCH_V1)
        .ok_or(Resource::Arithmetic)?;
    Ok((work, storage))
}

impl<T> RetainedMaterializationPhaseV1<T> {
    /// Prepay one existing analysis+snapshot account before any continuation.
    /// Work remains charged and analysis peak storage remains reserved in the
    /// original retained account across success and subsequent owning moves.
    /// The window prevents releases below the new floor while the callback runs.
    ///
    /// Returned payload/session owners must stay inside the phase. Only window
    /// control scratch is released on normal return, after that window ends.
    /// On unwind, payloads unwind before the whole account; no refund/reset or
    /// catch-and-continue policy is introduced.
    #[allow(dead_code)] // Real ordinary Pliron controls; genuine BF16 selection is separate.
    pub(in crate::production_pipeline) fn try_map_with_ranked_allowances<U>(
        self,
        analysis: Analysis,
        snapshot: Snapshot,
        consume: impl FnOnce(T, PaidRankedAllowancesV1, &mut Budget<'_>) -> Result<U>,
    ) -> Result<RetainedMaterializationPhaseV1<U>> {
        let (work, storage) = quote_values(
            analysis.max_work(),
            snapshot.max_work(),
            analysis.max_peak_storage(),
        )
        .map_err(materialization_resource_error_v29)?;
        self.try_map(|payload, budget| {
            // The original window separately charges its fixed eight work units.
            budget
                .charge_work(work - Budget::STORAGE_WINDOW_WORK_V1)
                .map_err(materialization_resource_error_v29)?;
            budget
                .reserve_storage(storage)
                .map_err(materialization_resource_error_v29)?;
            let protected = budget.storage();
            let work_identity = budget.work_ledger_identity_v1();
            let storage_identity = budget
                .storage_account_identity_v1()
                .ok_or_else(accounting)?;
            let remaining = budget
                .storage_limit()
                .checked_sub(protected)
                .ok_or_else(accounting)?;
            let result =
                budget.with_additional_storage_window_v1::<_, Resource>(remaining, |budget| {
                    Ok(consume(
                        payload,
                        PaidRankedAllowancesV1 { analysis, snapshot },
                        budget,
                    ))
                });
            // Never release control scratch against a replaced borrowed account.
            if budget.work_ledger_identity_v1() != work_identity
                || budget.storage_account_identity_v1() != Some(storage_identity)
                || budget.storage() < protected
            {
                drop(result);
                return Err(accounting());
            }
            budget
                .release_storage(Budget::STORAGE_WINDOW_SCRATCH_V1)
                .map_err(materialization_resource_error_v29)?;
            result.map_err(|error| Box::new(materialization_resource_error_v29(error)))?
        })
    }
}
