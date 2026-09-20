// Owned coordinates for later scoped replay. This is not payload equivalence,
// assertion provenance, scope verification, or executable admission authority.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct OwnedInstanceSourceV1 {
    instance: ProductionCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    identity: fe2o3_mir_model::semantic_mir_v1::SemanticFunctionIdentityV1,
    incoming: Option<ProductionCallOccurrenceV1>,
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Owning scoped materializer remains gated")
)]
struct OwnedInstanceCoordinatesV1 {
    semantic_sha256: [u8; 32],
    ssa: fe2o3_pliron::ProductionSemanticSsaIdentityV1,
    root: SemanticFunctionIdV1,
    sources: InstanceRowsV1<OwnedInstanceSourceV1>,
    seeds: InstanceRowsV1<InstanceSeedV1>,
    spans: InstanceRowsV1<InstanceMappedSpanV1>,
    controls: InstanceRowsV1<InstanceControlV1>,
    anchors: InstanceRowsV1<InstanceCallAnchorV1>,
    returns: InstanceRowsV1<InstanceReturnAnchorV1>,
    components: InstanceRowsV1<CallResultComponentV1>,
    values: InstanceRowsV1<ValueId>,
    storage: usize,
}

impl ProductionInstanceCorrespondenceV1<'_, '_> {
    fn check_live_ledger_v1(&self, budget: &ArgumentBudgetV1<'_>) -> InstanceMapResultV1<()> {
        if self.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if self.failed || self.transferred {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        Ok(())
    }

    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "Owning scoped materializer remains gated")
    )]
    fn check_complete_expansion_v1(
        &self,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<()> {
        self.check_live_ledger_v1(budget)?;
        budget.charge_work(8)?;
        let plan = self.plan;
        let root = plan.root();
        if self.seeds.rows.len() != plan.instances().len()
            || self.owner != plan.instance(root).map(|row| row.function())
        {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        let mut calls = 0_usize;
        let mut returns = 0_usize;
        for index in 0..plan.instances().len() {
            budget.charge_work(4 + self.seeds.rows.len())?;
            let instance = plan
                .id_at(index)
                .ok_or(InstanceCorrespondenceErrorV1::Source)?;
            let mut seeds = self
                .seeds
                .rows
                .iter()
                .filter(|row| row.instance == instance);
            let seed = seeds.next().ok_or(InstanceCorrespondenceErrorV1::Source)?;
            if seeds.next().is_some() || seed.container != root {
                return Err(InstanceCorrespondenceErrorV1::Source);
            }
            for call in plan
                .calls(instance)
                .ok_or(InstanceCorrespondenceErrorV1::Source)?
            {
                budget.charge_work(1)?;
                let Some(child) = call.child() else { continue };
                calls = calls.checked_add(1).ok_or(ArgumentResourceV1::Arithmetic)?;
                budget.charge_work(self.anchors.rows.len())?;
                let mut anchors = self.anchors.rows.iter().filter(|row| {
                    row.instance == instance && row.source.semantic_block == call.occurrence().block
                });
                let anchor = anchors
                    .next()
                    .ok_or(InstanceCorrespondenceErrorV1::CallAnchor)?;
                if anchors.next().is_some() || !anchor.removed {
                    return Err(InstanceCorrespondenceErrorV1::CallAnchor);
                }
                self.check_final_call_v1(call.occurrence(), child, anchor, function, budget)?;
            }
            budget.charge_work(
                plan.exits(instance)
                    .ok_or(InstanceCorrespondenceErrorV1::Source)?
                    .len(),
            )?;
            for exit in plan
                .returns(instance)
                .ok_or(InstanceCorrespondenceErrorV1::Source)?
            {
                returns = returns
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?;
                budget.charge_work(1 + self.returns.rows.len())?;
                let mut anchors = self.returns.rows.iter().filter(|row| {
                    row.instance == instance && row.source.semantic_block == exit.block
                });
                let anchor = anchors
                    .next()
                    .ok_or(InstanceCorrespondenceErrorV1::CallAnchor)?;
                if anchors.next().is_some() {
                    return Err(InstanceCorrespondenceErrorV1::CallAnchor);
                }
                budget.charge_work(self.controls.rows.len())?;
                let mut controls = self.controls.rows.iter().filter(|row| {
                    row.instance == instance
                        && row.original_block == anchor.original_block
                        && row.semantic_block == Some(exit.block)
                        && row.return_values.is_some()
                });
                let control = controls
                    .next()
                    .ok_or(InstanceCorrespondenceErrorV1::Control)?;
                let expected = match plan.incoming(instance) {
                    Some(call) => InstanceControlOriginV1::ExpandedReturn {
                        call: call.occurrence(),
                    },
                    None => InstanceControlOriginV1::Retained,
                };
                if controls.next().is_some() || control.origin != expected {
                    return Err(InstanceCorrespondenceErrorV1::Control);
                }
            }
        }
        if calls != self.anchors.rows.len() || returns != self.returns.rows.len() {
            return Err(InstanceCorrespondenceErrorV1::CallAnchor);
        }
        self.check_coordinates(root, function, budget)?;
        let body = function
            .body
            .as_ref()
            .ok_or(InstanceCorrespondenceErrorV1::Control)?;
        for (index, control) in self.controls.rows.iter().enumerate() {
            budget.charge_work(index)?;
            if self.controls.rows[..index]
                .iter()
                .any(|previous| previous.physical_block == control.physical_block)
            {
                return Err(InstanceCorrespondenceErrorV1::Control);
            }
        }
        let root_seed = &self.seeds.rows[self.seed_index(root, budget)?];
        let parameters = self
            .values
            .rows
            .get(root_seed.parameters.clone())
            .ok_or(InstanceCorrespondenceErrorV1::Control)?;
        budget.charge_work(1 + parameters.len())?;
        if body.parameters != parameters {
            return Err(InstanceCorrespondenceErrorV1::Control);
        }
        Ok(())
    }

    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "Owning scoped materializer remains gated")
    )]
    fn check_final_call_v1(
        &self,
        call: ProductionCallOccurrenceV1,
        child: ProductionCallInstanceIdV1,
        anchor: &InstanceCallAnchorV1,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<()> {
        budget.charge_work(self.spans.rows.len())?;
        if self
            .spans
            .rows
            .iter()
            .filter(|span| span.removed_call == Some(call))
            .count()
            != 1
        {
            return Err(InstanceCorrespondenceErrorV1::SpanCoverage);
        }
        let body = function
            .body
            .as_ref()
            .ok_or(InstanceCorrespondenceErrorV1::Control)?;
        let find_control = |origin,
                            budget: &mut ArgumentBudgetV1<'_>|
         -> InstanceMapResultV1<&InstanceControlV1> {
            budget.charge_work(self.controls.rows.len())?;
            let mut rows = self.controls.rows.iter().filter(|row| row.origin == origin);
            let row = rows.next().ok_or(InstanceCorrespondenceErrorV1::Control)?;
            if rows.next().is_some() {
                return Err(InstanceCorrespondenceErrorV1::Control);
            }
            Ok(row)
        };
        let entry = find_control(InstanceControlOriginV1::CallEntry { call, child }, budget)?;
        let preheader = find_control(
            InstanceControlOriginV1::ParameterPreheader { call, child },
            budget,
        )?;
        if entry.instance != call.caller
            || entry.physical_block != anchor.physical.block
            || preheader.instance != child
        {
            return Err(InstanceCorrespondenceErrorV1::Control);
        }
        budget.charge_work(self.controls.rows.len())?;
        let mut continuations = self.controls.rows.iter().filter(|row| {
            row.instance == call.caller
                && row.semantic_block == Some(call.block)
                && row.origin == InstanceControlOriginV1::Retained
        });
        let continuation = continuations
            .next()
            .ok_or(InstanceCorrespondenceErrorV1::Control)?;
        if continuations.next().is_some() {
            return Err(InstanceCorrespondenceErrorV1::Control);
        }
        let child_seed = &self.seeds.rows[self.seed_index(child, budget)?];
        for (control, values) in [
            (preheader, child_seed.parameters.clone()),
            (continuation, anchor.results.clone()),
        ] {
            budget.charge_work(body.blocks.len())?;
            let block = body
                .blocks
                .iter()
                .find(|block| block.id == control.physical_block)
                .ok_or(InstanceCorrespondenceErrorV1::Control)?;
            let values = self
                .values
                .rows
                .get(values)
                .ok_or(InstanceCorrespondenceErrorV1::Control)?;
            instance_check_parameters_v1(block, values, budget)?;
        }
        Ok(())
    }

    /// Transfer paid rows only after the complete root expansion is checked.
    /// Reservations stay live; the enclosing owner must account for them until
    /// it drops or explicitly transfers the returned storage receipt.
    #[cfg_attr(
        not(test),
        allow(dead_code, reason = "Owning scoped materializer remains gated")
    )]
    fn take_owned_coordinates_v1(
        &mut self,
        function: &Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<OwnedInstanceCoordinatesV1> {
        self.check_complete_expansion_v1(function, budget)?;
        let floor = budget.storage();
        let prepared = (|| {
            let mut sources = InstanceRowsV1::new();
            let mut storage = 0;
            sources.reserve(self.plan.instances().len(), budget, &mut storage)?;
            budget.charge_work(64)?;
            for (index, row) in self.plan.instances().iter().enumerate() {
                budget.charge_work(8)?;
                let instance = self
                    .plan
                    .id_at(index)
                    .ok_or(InstanceCorrespondenceErrorV1::Source)?;
                sources.rows.push(OwnedInstanceSourceV1 {
                    instance,
                    function: row.function(),
                    identity: row.declaration().identity(),
                    incoming: self.plan.incoming(instance).map(|call| call.occurrence()),
                });
            }
            let total = self
                .storage
                .checked_add(storage)
                .ok_or(ArgumentResourceV1::Arithmetic)?;
            let root = self.owner.ok_or(InstanceCorrespondenceErrorV1::Source)?;
            Ok::<_, InstanceCorrespondenceErrorV1>((sources, total, root))
        })();
        let (sources, storage, root) = match prepared {
            Ok(prepared) => prepared,
            Err(error) => {
                budget.release_storage(
                    budget
                        .storage()
                        .checked_sub(floor)
                        .ok_or(ArgumentResourceV1::Accounting)?,
                )?;
                return Err(error);
            }
        };
        // Every remaining operation is an infallible move, including logical
        // capacities and seed-name allocations already paid by the donor.
        let owned = OwnedInstanceCoordinatesV1 {
            semantic_sha256: *self.plan.owner().source_semantic_sha256(),
            ssa: self.plan.owner().identity(),
            root,
            sources,
            seeds: std::mem::replace(&mut self.seeds, InstanceRowsV1::new()),
            spans: std::mem::replace(&mut self.spans, InstanceRowsV1::new()),
            controls: std::mem::replace(&mut self.controls, InstanceRowsV1::new()),
            anchors: std::mem::replace(&mut self.anchors, InstanceRowsV1::new()),
            returns: std::mem::replace(&mut self.returns, InstanceRowsV1::new()),
            components: std::mem::replace(&mut self.components, InstanceRowsV1::new()),
            values: std::mem::replace(&mut self.values, InstanceRowsV1::new()),
            storage,
        };
        self.storage = 0;
        self.transferred = true;
        Ok(owned)
    }
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Owning scoped materializer remains gated")
)]
impl OwnedInstanceCoordinatesV1 {
    fn retained_storage(&self) -> usize {
        self.storage
    }

    // A bounded source/topology join, not replay of the retained operation data.
    fn check_source_plan(
        &self,
        plan: &ProductionCallInstancePlanV1<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> InstanceMapResultV1<()> {
        budget.charge_work(64)?;
        if self.semantic_sha256 != *plan.owner().source_semantic_sha256()
            || self.ssa != plan.owner().identity()
            || Some(self.root) != plan.instance(plan.root()).map(|row| row.function())
            || self.sources.rows.len() != plan.instances().len()
        {
            return Err(InstanceCorrespondenceErrorV1::Source);
        }
        for (index, (retained, row)) in self.sources.rows.iter().zip(plan.instances()).enumerate() {
            budget.charge_work(8)?;
            let instance = plan
                .id_at(index)
                .ok_or(InstanceCorrespondenceErrorV1::Source)?;
            let expected = OwnedInstanceSourceV1 {
                instance,
                function: row.function(),
                identity: row.declaration().identity(),
                incoming: plan.incoming(instance).map(|call| call.occurrence()),
            };
            if *retained != expected {
                return Err(InstanceCorrespondenceErrorV1::Source);
            }
        }
        Ok(())
    }
}
