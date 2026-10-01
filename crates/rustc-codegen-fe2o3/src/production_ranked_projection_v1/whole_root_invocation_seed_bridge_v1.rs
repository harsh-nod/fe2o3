//! Same-owner seed continuation; stops before borrowed-grid recovery/propagation.
use super::*;
pub(super) fn begin_invocation_seed_transition(phase: &mut WholePhase) -> bool {
    let prior = *phase == WholePhase::AfterInitialCapabilityGraph;
    *phase = WholePhase::InvocationSeedsTerminal;
    prior
}
impl<'s> PendingWholeRootBeforeArgumentWritersV1<'s> {
    pub(in crate::production_ranked_projection_v1) fn prepare_invocation_seeds(
        &mut self,
        owner: &'s ProductionPreRankedKirOwnerV1,
        function_id: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> Result<()> {
        // Invalidate predecessor completion BEFORE source checks or mutation.
        if !begin_invocation_seed_transition(&mut self.phase) {
            return Err(self
                .failure
                .as_ref()
                .map(saved_query_error)
                .unwrap_or(QueryError::Resource(Resource::Accounting)));
        }
        let result = self.advance_invocation_seeds(owner, function_id, facts, owned);
        match result {
            Ok(()) => {
                self.phase = WholePhase::AfterInvocationSeeds;
                Ok(())
            }
            Err(error) => {
                let mapped = saved_query_error(&error);
                self.failure = Some(error);
                Err(mapped)
            }
        }
    }
    fn advance_invocation_seeds(
        &mut self,
        owner: &'s ProductionPreRankedKirOwnerV1,
        function_id: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> BResult<()> {
        // Phase-free predecessor check is used only before seed mutation.
        // We never restore the predecessor's public completion phase.
        self.check_initial_capability_graph_state(owner, function_id, facts, owned)?;
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
                self.invocation_seeds.prepare_into(
                    &source,
                    &scalar.counts,
                    &scalar.address_escaped,
                    options,
                    enumeration,
                    launch,
                    &self.initial_graph,
                    initial,
                    &self.arguments,
                    &mut self.prefix,
                    resources,
                )?;
                self.invocation_seeds.completed_for(
                    &source,
                    &scalar.counts,
                    &scalar.address_escaped,
                    options,
                    enumeration,
                    launch,
                    &self.initial_graph,
                    initial,
                    &self.arguments,
                    &self.prefix,
                    resources,
                )
            },
        );
        if let Err(error) = written {
            return Err(if self.invocation_seeds.failure.is_some() {
                self.invocation_seeds.saved()
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
    /// Private seed DATA/custody checkpoint, not propagated graph or ready recipe.
    pub(in crate::production_ranked_projection_v1) fn completed_after_invocation_seeds(
        &self,
        owner: &ProductionPreRankedKirOwnerV1,
        function_id: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> BResult<()> {
        if self.phase != WholePhase::AfterInvocationSeeds
            || self.failure.is_some()
            || !self.owner.is_some_and(|bound| std::ptr::eq(bound, owner))
            || self.function != Some(function_id)
        {
            return Err(accounting());
        }
        let entry = self.entry.ok_or_else(accounting)?;
        facts.retained_whole_root_snapshot_v1(
            owner,
            function_id,
            owned,
            self.held.or(self.entry),
        )?;
        facts.check_retained_shared_reads_v1(
            &self.shared,
            owner.semantic_ssa(),
            function_id,
            owned,
        )?;
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
            // Existing source/ledger owners remain live. Earlier graph/read
            // completed_for methods have obsolete writer floors after seeding.
            self.constants.completed_for(function, resources)?;
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
            self.invocation_seeds.completed_for(
                &source,
                &scalar.counts,
                &scalar.address_escaped,
                options,
                enumeration,
                launch,
                &self.initial_graph,
                &enumeration_stage.options.initial,
                &self.arguments,
                &self.prefix,
                resources,
            )
        })
    }
}
