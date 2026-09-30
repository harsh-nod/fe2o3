//! Same-owner initial-graph continuation. No ordinary route or source fallback.
use super::*;
pub(super) fn begin_initial_graph_transition(phase: &mut WholePhase) -> bool {
    let prior = *phase == WholePhase::AfterInitialStridedReads;
    *phase = WholePhase::InitialCapabilityGraphTerminal;
    prior
}
impl<'s> PendingWholeRootBeforeArgumentWritersV1<'s> {
    pub(in crate::production_ranked_projection_v1) fn prepare_initial_capability_graph(
        &mut self,
        owner: &'s ProductionPreRankedKirOwnerV1,
        function_id: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> Result<()> {
        // A failed/foreign/repeated entry revokes all predecessor completion.
        // held is only a custody floor and cannot restore an earlier phase.
        if !begin_initial_graph_transition(&mut self.phase) {
            return Err(self
                .failure
                .as_ref()
                .map(saved_query_error)
                .unwrap_or(QueryError::Resource(Resource::Accounting)));
        }
        let result = self.advance_initial_graph(owner, function_id, facts, owned);
        match result {
            Ok(()) => {
                self.phase = WholePhase::AfterInitialCapabilityGraph;
                Ok(())
            }
            Err(error) => {
                let mapped = saved_query_error(&error);
                self.failure = Some(error);
                Err(mapped)
            }
        }
    }
    fn advance_initial_graph(
        &mut self,
        owner: &'s ProductionPreRankedKirOwnerV1,
        function_id: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> BResult<()> {
        // Same old custody/DATA checks, privately separated from its public
        // phase check. Never temporarily restore AfterInitialStridedReads.
        self.check_initial_strided_read_state(owner, function_id, facts, owned)?;
        let entry = self.entry.ok_or_else(accounting)?;
        let semantic = owner.semantic_ssa().source_semantic();
        let function = semantic
            .functions()
            .get(function_id.index() as usize)
            .ok_or_else(accounting)?;
        let source = Source {
            function,
            callables: semantic.callables(),
            types: semantic.types(),
            ledger: (entry.budget_slot, entry.work_ledger),
        };
        let written = with_nominal_source_preparation_v1(
            facts,
            function,
            source.ledger,
            owned,
            |resources| {
                // Validate the exact complete earlier source chain before the
                // disjoint field split; the temporary view does not escape.
                self.earlier.view(&source, resources)?;
                resources.work(8)?;
                let selected = self.selected.as_ref().ok_or_else(accounting)?;
                let launch = bounded_linear_launch_extent_v1(&selected.input().source_launch);
                let scalar_stage = &mut self.earlier.earlier.earlier;
                let scalar = scalar_stage.scalar.completed_for(function, resources)?;
                let enumeration_stage = &mut scalar_stage.earlier;
                let options = enumeration_stage
                    .options
                    .dominance
                    .completed()
                    .ok_or_else(accounting)?;
                let enumeration = enumeration_stage
                    .enumeration
                    .completed()
                    .ok_or_else(accounting)?;
                let initial = &mut enumeration_stage.options.initial;
                self.initial_graph.prepare_into(
                    &source,
                    &scalar.counts,
                    options,
                    enumeration,
                    launch,
                    initial,
                    &mut self.arguments,
                    &self.prefix,
                    resources,
                )?;
                self.initial_graph.completed_for(
                    &source,
                    &scalar.counts,
                    options,
                    enumeration,
                    launch,
                    initial,
                    &self.arguments,
                    &self.prefix,
                    resources,
                )
            },
        );
        if let Err(error) = written {
            // Preserve a precise accepted component denial when the enclosing
            // source-loan postflight reports the more general Accounting.
            return Err(if self.initial_graph.failure.is_some() {
                self.initial_graph.saved()
            } else {
                error
            });
        }
        self.held = Some(facts.retained_whole_root_snapshot_v1(
            owner,
            function_id,
            owned,
            self.held.or(self.entry),
        )?);
        Ok(())
    }
    /// Borrowed private DATA/custody only, not full intrinsic or recipe readiness.
    pub(in crate::production_ranked_projection_v1) fn completed_after_initial_capability_graph(
        &self,
        owner: &ProductionPreRankedKirOwnerV1,
        function_id: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> BResult<()> {
        if self.phase != WholePhase::AfterInitialCapabilityGraph
            || self.failure.is_some()
            || !self.owner.is_some_and(|bound| std::ptr::eq(bound, owner))
            || self.function != Some(function_id)
        {
            return Err(accounting());
        }
        self.check_initial_strided_read_state(owner, function_id, facts, owned)?;
        let entry = self.entry.ok_or_else(accounting)?;
        let semantic = owner.semantic_ssa().source_semantic();
        let function = semantic
            .functions()
            .get(function_id.index() as usize)
            .ok_or_else(accounting)?;
        let source = Source {
            function,
            callables: semantic.callables(),
            types: semantic.types(),
            ledger: (entry.budget_slot, entry.work_ledger),
        };
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            self.earlier.view(&source, resources)?;
            resources.work(8)?;
            let selected = self.selected.as_ref().ok_or_else(accounting)?;
            let launch = bounded_linear_launch_extent_v1(&selected.input().source_launch);
            let scalar_stage = &self.earlier.earlier.earlier;
            let scalar = scalar_stage.scalar.completed_for(function, resources)?;
            let enumeration_stage = &scalar_stage.earlier;
            let options = enumeration_stage
                .options
                .dominance
                .completed()
                .ok_or_else(accounting)?;
            let enumeration = enumeration_stage
                .enumeration
                .completed()
                .ok_or_else(accounting)?;
            self.initial_graph.completed_for(
                &source,
                &scalar.counts,
                options,
                enumeration,
                launch,
                &enumeration_stage.options.initial,
                &self.arguments,
                &self.prefix,
                resources,
            )
        })
    }
}
