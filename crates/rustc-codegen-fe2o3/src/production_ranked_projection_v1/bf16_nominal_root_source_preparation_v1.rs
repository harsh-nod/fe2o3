//! Joined immutable rich tables and the actual complete-CFG induction report.
//! This is preparation only: no root CFG/recipe, ranked attachment or admission.
use super::*;
use fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1;
use fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1;
use fe2o3_mir_model::{
    SemanticU32InductionAnalysisLimitsV1, SemanticU32InductionBoundSnapshotMeterV1,
    SemanticU32InductionMeteredErrorV1, SemanticU32InductionNoOverflowReportV1,
    analyze_semantic_u32_induction_no_overflow_with_meter_v1,
};

#[path = "bf16_nominal_root_cfg_preparation_v1.rs"]
mod root_cfg_preparation_v1;
#[allow(unused_imports)]
pub(in crate::production_ranked_projection_v1) use root_cfg_preparation_v1::{
    NominalRootAssertionSourceV1, NominalRootCfgSourceV1,
    with_nominal_root_assertion_preparation_v1, with_nominal_root_cfg_preparation_v1,
};
#[cfg(test)]
#[allow(unused_imports)]
pub(in crate::production_ranked_projection_v1) use root_cfg_preparation_v1::{
    RootAssertionObservationV1, observe_root_assertion_preparation_for_test_v1,
    observe_root_cfg_preparation_for_test_v1, root_assertion_preparation_controls_for_test_v1,
    root_cfg_preparation_controls_for_test_v1,
};

#[cfg(test)]
#[path = "bf16_nominal_root_source_preparation_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
#[path = "bf16_nominal_root_source_preparation_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(in crate::production_ranked_projection_v1) use genuine::{
    observe_root_source_preparation_for_test_v1, root_source_preparation_controls_for_test_v1,
};

/// Same-source lexical preparation view. No report/table ownership or borrowed
/// reference can escape the higher-ranked callback. It grants no authority.
pub(in crate::production_ranked_projection_v1) struct NominalRootSourceTablesV1<'a> {
    rich: &'a RichNominalSourceTablesV1<'a>,
    induction: &'a SemanticU32InductionNoOverflowReportV1,
}
impl NominalRootSourceTablesV1<'_> {
    pub(in crate::production_ranked_projection_v1) fn rich(
        &self,
    ) -> &RichNominalSourceTablesV1<'_> {
        self.rich
    }
    pub(in crate::production_ranked_projection_v1) fn induction_report(
        &self,
    ) -> &SemanticU32InductionNoOverflowReportV1 {
        self.induction
    }
}

struct InductionMeter<'r, 'b, 'w>(&'r mut PreparationResourcesV1<'b, 'w>);
impl SemanticU32InductionBoundSnapshotMeterV1 for InductionMeter<'_, '_, '_> {
    type Error = ProductionRankedProjectionErrorV1;
    fn charge_work(&mut self, amount: usize) -> std::result::Result<(), Self::Error> {
        self.0.work(amount)
    }
    fn reserve_storage(&mut self, amount: usize) -> std::result::Result<(), Self::Error> {
        self.0.reserve_storage(amount)
    }
}
fn induction_error(
    error: SemanticU32InductionMeteredErrorV1<ProductionRankedProjectionErrorV1>,
) -> ProductionRankedProjectionErrorV1 {
    match error {
        SemanticU32InductionMeteredErrorV1::Meter(error) => error,
        SemanticU32InductionMeteredErrorV1::Allocation => resource(Resource::Allocation),
        SemanticU32InductionMeteredErrorV1::Arithmetic => resource(Resource::Arithmetic),
        SemanticU32InductionMeteredErrorV1::Analysis(_) => {
            ProductionRankedProjectionErrorV1::Unsupported("actual root induction report refused")
        }
    }
}

#[derive(Clone, Copy)]
struct Custody {
    slot: usize,
    ledger: CanonicalKernelIrWorkLedgerIdentityV1,
    floor: usize,
    work: usize,
    peak: usize,
}
impl Custody {
    fn new(budget: &Budget<'_>) -> Result<Self> {
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            return Err(Resource::Accounting.into());
        }
        Ok(Self {
            slot: budget as *const Budget<'_> as usize,
            ledger: budget.work_ledger_identity_v1(),
            floor: budget.storage(),
            work: budget.work(),
            peak: budget.peak_storage(),
        })
    }
    fn check(self, budget: &Budget<'_>, owned: usize) -> Result<()> {
        let protected = self.floor.checked_add(owned).ok_or(Resource::Arithmetic)?;
        if self.slot != budget as *const Budget<'_> as usize
            || self.ledger != budget.work_ledger_identity_v1()
            || budget.storage() < protected
            || budget.work() < self.work
            || budget.peak_storage() < self.peak
        {
            return Err(Resource::Accounting.into());
        }
        Ok(())
    }
}
fn root_header<R>(callback_bytes: usize) -> Result<usize> {
    let mut bytes = 4096usize;
    for amount in [
        size_of::<SemanticU32InductionNoOverflowReportV1>(),
        size_of::<NominalRootSourceTablesV1<'static>>(),
        size_of::<PreparationResourcesV1<'static, 'static>>(),
        size_of::<InductionMeter<'static, 'static, 'static>>(),
        size_of::<Custody>(),
        callback_bytes.checked_mul(2).ok_or(Resource::Arithmetic)?,
        size_of::<Result<R>>()
            .checked_mul(2)
            .ok_or(Resource::Arithmetic)?,
    ] {
        bytes = bytes.checked_add(amount).ok_or(Resource::Arithmetic)?;
    }
    Ok(bytes)
}
fn with_root_report_scope<'w, R, F>(
    source: &AdmittedInertSemanticMirV1,
    caller: SemanticFunctionIdV1,
    rich: &RichNominalSourceTablesV1<'_>,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'s> FnOnce(&NominalRootSourceTablesV1<'s>, &mut Budget<'w>) -> Result<R>,
{
    let frame = root_header::<R>(size_of::<F>())?;
    let before = Custody::new(budget)?;
    budget.charge_work(64)?;
    let mut owned = 0usize;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let report =
            {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                resources.reserve_storage(frame).map_err(query_error)?;
                resources.work(128).map_err(query_error)?;
                let function = source.functions().get(caller.index() as usize).ok_or(
                    QueryError::Unavailable("joined root caller absent from actual source"),
                )?;
                // Exact original source object, not merely equal text/identities.
                if !std::ptr::eq(function, rich.function()) {
                    return Err(QueryError::Unavailable(
                        "joined root rich function differs from actual source",
                    ));
                }
                let report = analyze_semantic_u32_induction_no_overflow_with_meter_v1(
                    source,
                    caller,
                    SemanticU32InductionAnalysisLimitsV1::default(),
                    &mut InductionMeter(&mut resources),
                )
                .map_err(induction_error)
                .map_err(query_error)?;
                resources.work(32).map_err(query_error)?;
                if report.semantic_mir_sha256() != source.semantic_sha256()
                    || report.function() != caller
                    || report.function_identity() != function.identity()
                    || report.uses_reachable_scope_v2()
                    || report.ssa_scope_work_units_v2() != 0
                {
                    return Err(QueryError::Unavailable(
                        "joined root induction source binding differs",
                    ));
                }
                report
            };
        let view = NominalRootSourceTablesV1 {
            rich,
            induction: &report,
        };
        let result = inspect(&view, budget);
        drop(report);
        result
    }));
    // Reports and every partial model value are gone before own-only refund.
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
    before.check(budget, owned)?;
    budget.release_storage(owned)?;
    result
}

#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_root_source_preparation_v1<
    'w,
    R,
    F,
>(
    owner: &ProductionPreRankedKirOwnerV1,
    inventory: &CanonicalKirInventoryV1<'_>,
    root: SemanticFunctionIdV1,
    caller: SemanticFunctionIdV1,
    block: SemanticBlockIdV1,
    call: &SemanticDirectCallV1,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'s> FnOnce(&NominalRootSourceTablesV1<'s>, &mut Budget<'w>) -> Result<R>,
{
    // This distinct outer entry captures the TRUE incoming owner floor before
    // any nested rich/header reservation, and owns arbitrary F/R frame storage.
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, move |budget| {
        let _ = root_header::<R>(size_of::<F>())?;
        with_nominal_rich_source_preparation_v1(
            owner,
            inventory,
            root,
            caller,
            block,
            call,
            budget,
            move |rich, budget| {
                with_root_report_scope(
                    owner.semantic_ssa().source_semantic(),
                    caller,
                    rich,
                    budget,
                    inspect,
                )
            },
        )
    })
}
