//! Same-owner continuation after the sealed pre-writer boundary.
use super::*;
pub(super) fn begin_initial_read_transition(phase: &mut WholePhase) -> bool {
    let prior = *phase == WholePhase::BeforeArgumentWriters;
    *phase = WholePhase::InitialStridedReadsTerminal;
    prior
}
impl<'s> PendingWholeRootBeforeArgumentWritersV1<'s> {
    pub(in crate::production_ranked_projection_v1) fn prepare_initial_strided_reads(
        &mut self,
        owner: &'s ProductionPreRankedKirOwnerV1,
        function_id: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> Result<()> {
        // Immediately revoke the old completion marker, including on foreign
        // input, denial, error and unwind. held remains only a custody floor.
        let prior = begin_initial_read_transition(&mut self.phase);
        if !prior {
            return Err(self
                .failure
                .as_ref()
                .map(saved_query_error)
                .unwrap_or(QueryError::Resource(Resource::Accounting)));
        }
        let result = self.advance_initial_reads(owner, function_id, facts, owned);
        match result {
            Ok(()) => {
                self.phase = WholePhase::AfterInitialStridedReads;
                Ok(())
            }
            Err(error) => {
                let mapped = saved_query_error(&error);
                self.failure = Some(error);
                Err(mapped)
            }
        }
    }
    fn advance_initial_reads(
        &mut self,
        owner: &'s ProductionPreRankedKirOwnerV1,
        function_id: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> BResult<()> {
        self.check_before_writers(owner, function_id, facts, owned)?;
        let entry = self.entry.ok_or_else(accounting)?;
        let semantic = owner.semantic_ssa().source_semantic();
        let function = semantic
            .functions()
            .get(function_id.index() as usize)
            .ok_or_else(accounting)?;
        let source = Source {
            function,
            types: semantic.types(),
            callables: semantic.callables(),
            ledger: (entry.budget_slot, entry.work_ledger),
        };
        let (constants, preparation) = with_nominal_source_preparation_v1(
            facts,
            function,
            source.ledger,
            owned,
            |resources| {
                Ok((
                    self.constants.completed_for(function, resources)?,
                    self.earlier.view(&source, resources)?,
                ))
            },
        )?;
        let inputs = NominalCapabilityInputsV1::from_borrowed_source_v1(
            function,
            preparation.enum_dominance(),
            preparation.allocation_contracts(),
            constants,
        );
        // Only immutable DATA owned by the already retained driver is captured.
        // The Copy + 'static callback result remains (); neither site nor consumer escapes.
        let driver = &self.driver;
        let mut effects = None;
        with_nominal_capability_consumer_v1(facts, |site, consumer| {
            effects = Some(driver.completed_for(site, &inputs, consumer)?.read_views);
            Ok(())
        })
        .map_err(nominal_error)?;
        let effects = effects.ok_or_else(accounting)?;
        let origins = &preparation.provenance().stable_argument_origins;
        let written = with_nominal_source_preparation_v1(
            facts,
            function,
            source.ledger,
            owned,
            |resources| {
                self.initial_reads.prepare_into(
                    semantic.types(),
                    function,
                    effects,
                    origins,
                    &mut self.arguments,
                    &mut self.prefix,
                    resources,
                )?;
                self.initial_reads.completed_for(
                    semantic.types(),
                    function,
                    effects,
                    origins,
                    &self.arguments,
                    &self.prefix,
                    resources,
                )
            },
        );
        if let Err(error) = written {
            // The source borrow's denial postflight can return Accounting.
            // Retain and report the component's earlier precise denial when set.
            return Err(if self.initial_reads.failure.is_some() {
                self.initial_reads.saved()
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
    /// Partial DATA/custody checkpoint only. This is not an intrinsic graph,
    /// CFG recipe, consuming handoff or permission to enter ordinary compilation.
    pub(in crate::production_ranked_projection_v1) fn completed_after_initial_strided_reads(
        &self,
        owner: &ProductionPreRankedKirOwnerV1,
        function_id: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> BResult<()> {
        if self.phase != WholePhase::AfterInitialStridedReads {
            return Err(accounting());
        }
        self.check_initial_strided_read_state(owner, function_id, facts, owned)
    }
    // Private to the retained whole owner and descendants. A successor revokes
    // the public phase first, then checks this unchanged custody/DATA remainder.
    pub(in super::super) fn check_initial_strided_read_state(
        &self,
        owner: &ProductionPreRankedKirOwnerV1,
        function_id: SemanticFunctionIdV1,
        facts: &mut CanonicalSourceAssertionFactsV1<'_, '_, '_, '_, '_>,
        owned: &mut usize,
    ) -> BResult<()> {
        if self.failure.is_some()
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
            types: semantic.types(),
            callables: semantic.callables(),
            ledger: (entry.budget_slot, entry.work_ledger),
        };
        let (constants, preparation) = with_nominal_source_preparation_v1(
            facts,
            function,
            source.ledger,
            owned,
            |resources| {
                Ok((
                    self.constants.completed_for(function, resources)?,
                    self.earlier.view(&source, resources)?,
                ))
            },
        )?;
        let inputs = NominalCapabilityInputsV1::from_borrowed_source_v1(
            function,
            preparation.enum_dominance(),
            preparation.allocation_contracts(),
            constants,
        );
        // Only immutable DATA owned by the already retained driver is captured.
        // The Copy + 'static callback result remains (); neither site nor consumer escapes.
        let driver = &self.driver;
        let mut effects = None;
        with_nominal_capability_consumer_v1(facts, |site, consumer| {
            effects = Some(driver.completed_for(site, &inputs, consumer)?.read_views);
            Ok(())
        })
        .map_err(nominal_error)?;
        let effects = effects.ok_or_else(accounting)?;
        with_nominal_source_preparation_v1(facts, function, source.ledger, owned, |resources| {
            self.initial_reads.completed_for(
                semantic.types(),
                function,
                effects,
                &preparation.provenance().stable_argument_origins,
                &self.arguments,
                &self.prefix,
                resources,
            )
        })
    }
}
