use crate::production_analysis::pliron_sparse_index::{
    SparseIndexAnalysisV1, preflight_sparse_index_resource_upper_bound_v1,
};

struct ConcreteIndexContextV1<'a> {
    function: &'a FuncOp,
    analyses: &'a mut PlironAnalysisManagerV1,
    census: ProductionAnalysisInputCensusV1,
    unavailable: bool,
}

impl ConcreteIndexContextV1<'_> {
    fn facts_for(
        &mut self,
        context: &Context,
        schedule: &[EventSiteV1],
        accesses: &[AccessSiteV1],
    ) -> Result<Option<&SparseIndexAnalysisV1>, PlironPipelineProtocolFindingV1> {
        // The pipeline descriptor prepays this bounded scan and the subsequent
        // fact queries. Literal-only and recognized dynamic routes need no cache.
        let literals = schedule.iter().all(|event| {
            index_constant(context, event.epoch).is_some()
                && index_constant(context, event.slot).is_some()
        }) && accesses
            .iter()
            .all(|access| index_constant(context, access.slot).is_some());
        if literals {
            return Ok(None);
        }
        if self.unavailable {
            return Err(concrete_index_unavailable_v1());
        }
        if !self.analyses.sparse_indices_prepared() {
            // Current barrier/workgroup callers prepare this cache (including
            // failures) before entering the protocol. Only direct standalone
            // protocol checks initialize it here. Its peak must coexist with
            // the already-admitted pipeline temporary scratch. A future nested
            // absent-cache caller must also account for its own live scratch.
            let admitted = self
                .analyses
                .remaining_resource_limits(ProductionAnalysisResourcePhaseV1::SparseIndex)
                .and_then(|limits| {
                    preflight_sparse_index_resource_upper_bound_v1(
                        context,
                        self.function,
                        self.census,
                        limits,
                    )
                })
                .and_then(|bound| {
                    let bound = concrete_index_nested_bound_v1(self.census, bound)?;
                    self.analyses.admit_retained_resource_upper_bound(
                        ProductionAnalysisResourcePhaseV1::SparseIndex,
                        bound,
                    )
                });
            if admitted.is_err() {
                self.unavailable = true;
                return Err(concrete_index_unavailable_v1());
            }
        }
        // This also asserts the manager's exact function identity on a hit.
        self.analyses.prepare_sparse_indices(context, self.function);
        match self.analyses.sparse_indices() {
            Ok(facts) => Ok(Some(facts)),
            Err(_) => {
                self.unavailable = true;
                Err(concrete_index_unavailable_v1())
            }
        }
    }
}

fn concrete_index_nested_bound_v1(
    census: ProductionAnalysisInputCensusV1,
    sparse: ProductionAnalysisResourceUpperBoundV1,
) -> Result<ProductionAnalysisResourceUpperBoundV1, ProductionAnalysisResourceLimitV1> {
    let phase = ProductionAnalysisResourcePhaseV1::SparseIndex;
    // The descriptor's retained rows are already in the manager ledger. Only
    // its temporary peak is added here, avoiding a second work/retained charge.
    let pipeline = preflight_pipeline_protocol_resource_upper_bound_v1(
        census,
        ProductionAnalysisResourceLimitsV1::production_hard_ceiling(),
    )?;
    let temporary = pipeline
        .peak_storage_upper_bound()
        .checked_sub(pipeline.retained_storage_upper_bound())
        .ok_or(ProductionAnalysisResourceLimitV1 {
            phase,
            resource: "pipeline temporary storage upper bound",
        })?;
    ProductionAnalysisResourceUpperBoundV1::checked_phase(phase, 0, 0, temporary)?
        .checked_with_nested_sequence_retain(&[sparse], phase)
}

fn concrete_index_unavailable_v1() -> PlironPipelineProtocolFindingV1 {
    PlironPipelineProtocolFindingV1::AnalysisIncomplete {
        detail: "concrete pipeline index facts could not be admitted".to_owned(),
    }
}

fn concrete_index_constant_v1(
    context: &Context,
    facts: Option<&SparseIndexAnalysisV1>,
    value: Value,
) -> Option<u64> {
    index_constant(context, value).or_else(|| facts?.fact_ref(value).constant_value())
}
