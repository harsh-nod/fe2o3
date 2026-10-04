//! Actual same-source assertion decisions retained for a lexical observer.
//! Analysis only; BoundsCheck decisions are not bounds/access certificates.
use super::*;
use crate::production_ranked_projection_v1::assertion_analyzer_resources_v1::{
    PreparedAssertionDecisionsV1, analyze_prepared_root_assertions_with_resources_v1,
};

#[cfg(test)]
#[path = "bf16_nominal_root_assertion_preparation_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
#[path = "bf16_nominal_root_assertion_preparation_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(in crate::production_ranked_projection_v1) use genuine::{
    RootAssertionObservationV1, observe_root_assertion_preparation_for_test_v1,
    root_assertion_preparation_controls_for_test_v1,
};

pub(in crate::production_ranked_projection_v1) struct NominalRootAssertionSourceV1<'a> {
    cfg: &'a NominalRootCfgSourceV1<'a>,
    prepared: &'a PreparedAssertionDecisionsV1<'a>,
}
impl NominalRootAssertionSourceV1<'_> {
    pub(in crate::production_ranked_projection_v1) fn cfg(&self) -> &NominalRootCfgSourceV1<'_> {
        self.cfg
    }
    pub(in crate::production_ranked_projection_v1) fn decisions(&self) -> &[bool] {
        self.prepared.decisions()
    }
    pub(in crate::production_ranked_projection_v1) fn logical_work(&self) -> usize {
        self.prepared.logical_work()
    }
}
fn assertion_header<R>(callback_bytes: usize) -> Result<usize> {
    let mut bytes = 4096usize;
    for amount in [
        size_of::<NominalRootAssertionSourceV1<'static>>(),
        size_of::<PreparedAssertionDecisionsV1<'static>>()
            .checked_mul(2)
            .ok_or(Resource::Arithmetic)?,
        size_of::<
            std::result::Result<
                PreparedAssertionDecisionsV1<'static>,
                ProductionRankedProjectionErrorV1,
            >,
        >()
        .checked_mul(2)
        .ok_or(Resource::Arithmetic)?,
        size_of::<PreparationResourcesV1<'static, 'static>>(),
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
fn with_assertion_scope<'w, R, F>(
    source: &AdmittedInertSemanticMirV1,
    caller: SemanticFunctionIdV1,
    cfg: &NominalRootCfgSourceV1<'_>,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'s> FnOnce(&NominalRootAssertionSourceV1<'s>, &mut Budget<'w>) -> Result<R>,
{
    let header = assertion_header::<R>(size_of::<F>())?;
    let before = Custody::new(budget)?;
    budget.charge_work(64)?;
    let mut owned = 0usize;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let prepared =
            {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                resources.reserve_storage(header).map_err(query_error)?;
                resources.work(256).map_err(query_error)?;
                let function = source.functions().get(caller.index() as usize).ok_or(
                    QueryError::Unavailable("root assertion caller absent from actual source"),
                )?;
                let report = cfg.source_tables().induction_report();
                if !std::ptr::eq(function, cfg.function())
                    || cfg.types().as_ptr() != source.types().as_ptr()
                    || cfg.types().len() != source.types().len()
                    || report.semantic_mir_sha256() != source.semantic_sha256()
                    || report.function() != caller
                    || report.function_identity() != function.identity()
                    || report.uses_reachable_scope_v2()
                    || report.ssa_scope_work_units_v2() != 0
                {
                    return Err(QueryError::Unavailable(
                        "root assertion source binding differs",
                    ));
                }
                let prepared =
                    analyze_prepared_root_assertions_with_resources_v1(cfg, &mut resources)
                        .map_err(query_error)?;
                // Exact metadata joins only; no table walk or equality-by-content.
                resources.work(128).map_err(query_error)?;
                if !std::ptr::eq(prepared.function(), function)
                    || !std::ptr::eq(prepared.graph(), cfg.graph())
                    || prepared.types().as_ptr() != cfg.types().as_ptr()
                    || prepared.types().len() != cfg.types().len()
                    || prepared.decisions().len() != function.blocks().len()
                {
                    return Err(QueryError::Unavailable(
                        "root assertion result binding differs",
                    ));
                }
                prepared
            };
        // B's evaluator and adapter borrow ended. Refusal precedes the loan.
        if budget.failed_work().is_some() || budget.failed_storage().is_some() {
            return Err(Resource::Accounting.into());
        }
        before.check(budget, owned)?;
        let view = NominalRootAssertionSourceV1 {
            cfg,
            prepared: &prepared,
        };
        let result = inspect(&view, budget);
        drop(prepared);
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
    // Full/partial masks, evaluator state, callback captures and panic payload
    // are gone. Never repair floor erosion or refund through a foreign ledger.
    before.check(budget, owned)?;
    budget.release_storage(owned)?;
    result
}

#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_root_assertion_preparation_v1<
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
    F: for<'s> FnOnce(&NominalRootAssertionSourceV1<'s>, &mut Budget<'w>) -> Result<R>,
{
    // Protect TRUE incoming floor and arbitrary F/R before nested CFG reserves.
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, move |budget| {
        let _ = assertion_header::<R>(size_of::<F>())?;
        with_nominal_root_cfg_preparation_v1(
            owner,
            inventory,
            root,
            caller,
            block,
            call,
            budget,
            move |cfg, budget| {
                with_assertion_scope(
                    owner.semantic_ssa().source_semantic(),
                    caller,
                    cfg,
                    budget,
                    inspect,
                )
            },
        )
    })
}
