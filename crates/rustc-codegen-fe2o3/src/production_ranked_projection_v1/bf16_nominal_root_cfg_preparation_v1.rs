//! Existing source-loop CFG retained with exact rich/induction preparation.
//! Analysis only: no asserted proof fallback, ranked recipe or admission.
use super::super::super::source_loop_cfg_resources_v1::projected_loop_cfg_graph_with_resources_v1;
use super::*;

#[cfg(test)]
#[path = "bf16_nominal_root_cfg_preparation_genuine_v1_tests.rs"]
mod genuine;
#[cfg(test)]
#[path = "bf16_nominal_root_cfg_preparation_v1_tests.rs"]
mod tests;
#[cfg(test)]
pub(in crate::production_ranked_projection_v1) use genuine::{
    observe_root_cfg_preparation_for_test_v1, root_cfg_preparation_controls_for_test_v1,
};

pub(in crate::production_ranked_projection_v1) struct NominalRootCfgSourceV1<'a> {
    source: &'a NominalRootSourceTablesV1<'a>,
    graph: &'a ProjectedLoopCfgV1,
}
impl NominalRootCfgSourceV1<'_> {
    pub(in crate::production_ranked_projection_v1) fn source_tables(
        &self,
    ) -> &NominalRootSourceTablesV1<'_> {
        self.source
    }
    pub(in crate::production_ranked_projection_v1) fn function(&self) -> &SemanticFunctionDeclV1 {
        self.source.rich().function()
    }
    pub(in crate::production_ranked_projection_v1) fn graph(&self) -> &ProjectedLoopCfgV1 {
        self.graph
    }
}
fn graph_header<R>(callback_bytes: usize) -> Result<usize> {
    let mut bytes = 4096usize;
    for amount in [
        size_of::<ProjectedLoopCfgV1>(),
        size_of::<NominalRootCfgSourceV1<'static>>(),
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
fn with_graph_scope<'w, R, F>(
    source: &AdmittedInertSemanticMirV1,
    caller: SemanticFunctionIdV1,
    root: &NominalRootSourceTablesV1<'_>,
    budget: &mut Budget<'w>,
    inspect: F,
) -> Result<R>
where
    R: Copy + 'static,
    F: for<'s> FnOnce(&NominalRootCfgSourceV1<'s>, &mut Budget<'w>) -> Result<R>,
{
    let header = graph_header::<R>(size_of::<F>())?;
    let before = Custody::new(budget)?;
    budget.charge_work(64)?;
    let mut owned = 0usize;
    let outcome = catch_unwind(AssertUnwindSafe(|| {
        let graph =
            {
                let mut resources = PreparationResourcesV1::new(budget, &mut owned);
                resources.reserve_storage(header).map_err(query_error)?;
                resources.work(128).map_err(query_error)?;
                let function = source.functions().get(caller.index() as usize).ok_or(
                    QueryError::Unavailable("root CFG caller absent from actual source"),
                )?;
                if !std::ptr::eq(function, root.rich().function())
                    || root.induction_report().semantic_mir_sha256() != source.semantic_sha256()
                    || root.induction_report().function() != caller
                    || root.induction_report().function_identity() != function.identity()
                    || root.induction_report().uses_reachable_scope_v2()
                {
                    return Err(QueryError::Unavailable("root CFG source binding differs"));
                }
                projected_loop_cfg_graph_with_resources_v1(function, &mut resources)
                    .map_err(query_error)?
            };
        let view = NominalRootCfgSourceV1 {
            source: root,
            graph: &graph,
        };
        let result = inspect(&view, budget);
        drop(graph);
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
    before.check(budget, owned)?;
    budget.release_storage(owned)?;
    result
}

#[allow(clippy::too_many_arguments)]
pub(in crate::production_ranked_projection_v1) fn with_nominal_root_cfg_preparation_v1<'w, R, F>(
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
    F: for<'s> FnOnce(&NominalRootCfgSourceV1<'s>, &mut Budget<'w>) -> Result<R>,
{
    // TRUE incoming owner floor and full F/R are protected before either the
    // joined-root or rich inner scope can add their own paid frame.
    owner.with_bf16_nominal_entry_resources_v1(inventory, budget, move |budget| {
        let _ = graph_header::<R>(size_of::<F>())?;
        with_nominal_root_source_preparation_v1(
            owner,
            inventory,
            root,
            caller,
            block,
            call,
            budget,
            move |source, budget| {
                with_graph_scope(
                    owner.semantic_ssa().source_semantic(),
                    caller,
                    source,
                    budget,
                    inspect,
                )
            },
        )
    })
}
