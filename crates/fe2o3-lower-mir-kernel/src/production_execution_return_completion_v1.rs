// Only completed outputs are indexed here. Original source equations and the
// instance-qualified archive remain mandatory; an index row grants no identity.
struct ExecutionReturnInputsV1 {
    owned: BTreeMap<usize, SemanticExecutionBindingV29>,
    credit: usize,
}

impl ExecutionReturnInputsV1 {
    fn empty() -> Self {
        Self {
            owned: BTreeMap::new(),
            credit: 0,
        }
    }

    fn capture(
        &mut self,
        inputs: &BTreeMap<(usize, Option<usize>), ExecutionCfgLeafV29>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<usize, ProductionSemanticKirErrorV1> {
        if !self.owned.is_empty() || self.credit != 0 {
            return Err(execution_identity_error_v1());
        }
        let floor = budget.storage();
        for ((class, referent), leaf) in inputs {
            budget.charge_work(2)?;
            let (class, binding) = match (referent, leaf) {
                (None, ExecutionCfgLeafV29::Owned(binding)) => (*class, binding),
                (Some(referent), ExecutionCfgLeafV29::Borrow(binding)) => {
                    (*referent, &binding.borrowed)
                }
                _ => return Err(execution_identity_error_v1()),
            };
            charge_execution_cfg_lookup_v29(self.owned.len(), budget)?;
            match self.owned.get(&class) {
                Some(previous) if previous != binding => return Err(execution_identity_error_v1()),
                Some(_) => {}
                None => {
                    reserve_execution_cfg_map_entry_v29::<usize, SemanticExecutionBindingV29>(
                        self.owned.len(),
                        budget,
                    )?;
                    self.owned.insert(class, binding.clone());
                }
            }
        }
        self.credit = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        Ok(self.credit)
    }

    fn contains(
        &self,
        class: usize,
        binding: &SemanticExecutionBindingV29,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        charge_execution_cfg_lookup_v29(self.owned.len(), budget)?;
        match self.owned.get(&class) {
            Some(found) if found != binding => Err(execution_identity_error_v1()),
            Some(_) => Ok(true),
            None => Ok(false),
        }
    }
}

struct ExecutionReturnCompletionV1<'scope, 'source> {
    instances: &'scope ExecutionInstancesV29<'source>,
    identities: Option<&'scope ExecutionIdentityPlanV1<'scope, 'source>>,
    source: ExecutionCallSourceV29,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    producers: Vec<Option<Vec<Option<usize>>>>,
    credit: usize,
    entry: usize,
    slot: usize,
    references: Option<&'scope SourceReferencePlanV29<'scope, 'source>>,
    growth: Option<source_storage_v29::SourceStorageRootGrowthV29<'scope, 'scope, 'source>>,
}

struct CheckedExecutionReturnV1 {
    source: ExecutionCallSourceV29,
    occurrence: ProductionCallOccurrenceV1,
    child: ProductionCallInstanceIdV1,
    original_call: usize,
    callee: SemanticFunctionIdV1,
    normal_return: bool,
    nominal: Vec<Option<ExecutionCfgLeafV29>>,
    credit: usize,
}

trait ExecutionCompletedLookupV1 {
    fn completed(&self, instance: usize) -> Option<&LoweredFunctionResultV1>;
}

impl ExecutionCompletedLookupV1 for Vec<Option<LoweredFunctionResultV1>> {
    fn completed(&self, instance: usize) -> Option<&LoweredFunctionResultV1> {
        self.get(instance).and_then(Option::as_ref)
    }
}

impl ExecutionCompletedLookupV1 for Vec<Option<&LoweredFunctionResultV1>> {
    fn completed(&self, instance: usize) -> Option<&LoweredFunctionResultV1> {
        self.get(instance).copied().flatten()
    }
}

impl<'scope, 'source> ExecutionReturnCompletionV1<'scope, 'source> {
    fn new(
        instances: &'scope ExecutionInstancesV29<'source>,
        identities: Option<&'scope ExecutionIdentityPlanV1<'scope, 'source>>,
        references: Option<&'scope SourceReferencePlanV29<'scope, 'source>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
        budget.charge_work(2)?;
        if identities.is_some_and(|plan| !std::ptr::eq(plan.index.instances, instances)) {
            return Err(execution_identity_error_v1());
        }
        let entry = budget.storage();
        let slot = budget
            .emission_service_slot_v1()
            .ok_or_else(emission_service_error_v1)?;
        let growth = match references.and_then(|plan| plan.storage_root.as_ref()) {
            Some(root) => {
                budget.source_reference_owner_v29(
                    references.ok_or_else(execution_identity_error_v1)?,
                )?;
                budget.charge_work(source_storage_v29::SOURCE_STORAGE_ROOT_GROWTH_WORK_V29)?;
                Some(
                    root.capture_retained_growth()
                        .ok_or(ArgumentResourceV1::Accounting)?,
                )
            }
            None => None,
        };
        let header = argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
        ])?;
        budget.reserve_storage(header)?;
        let mut producers = emission_vec_v1(instances.instances().len(), budget)?;
        budget.charge_work(instances.instances().len())?;
        producers.resize_with(instances.instances().len(), || None);
        let credit = argument_sum_v1(&[
            header,
            argument_product_v1(
                producers.capacity(),
                std::mem::size_of::<Option<Vec<Option<usize>>>>(),
            )?,
        ])?;
        Ok(Self {
            instances,
            identities,
            source,
            ledger: budget.work_ledger_identity_v1(),
            producers,
            credit,
            entry,
            slot,
            references,
            growth,
        })
    }

    fn check_owner(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        let required = argument_sum_v1(&[self.entry, self.credit])?;
        if !budget.permits_prepared_input_refund_v1(
            self.references,
            self.slot,
            self.ledger,
            required,
            0,
        ) || !self
            .growth
            .as_ref()
            .is_none_or(|growth| growth.permits_refund(self.entry, required, budget.storage(), 0))
        {
            if let Some(root) = self.references.and_then(|plan| plan.storage_root.as_ref()) {
                root.deny_active_root_refund();
            }
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if !std::ptr::eq(self.instances, instances)
            || self.ledger != budget.work_ledger_identity_v1()
            || self.source != ExecutionCallSourceV29::from_instances(instances, budget)?
        {
            return Err(execution_identity_error_v1());
        }
        Ok(())
    }

    fn discard(
        self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_owner(self.instances, budget)?;
        let required = argument_sum_v1(&[self.entry, self.credit])?;
        let permitted = budget.permits_prepared_input_refund_v1(
            self.references,
            self.slot,
            self.ledger,
            required,
            self.credit,
        ) && self.growth.as_ref().is_none_or(|growth| {
            growth.permits_refund(self.entry, required, budget.storage(), self.credit)
        });
        let source = self.references;
        let credit = self.credit;
        drop(self);
        if !permitted {
            if let Some(root) = source.and_then(|plan| plan.storage_root.as_ref()) {
                root.deny_active_root_refund();
            }
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.release_storage(credit).map_err(Into::into)
    }

    fn record_completed(
        &mut self,
        output: &LoweredFunctionResultV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_owner(self.instances, budget)?;
        let instance = output
            .source_call_instance
            .ok_or_else(execution_identity_error_v1)?;
        let original = self
            .instances
            .instance(instance)
            .ok_or_else(execution_identity_error_v1)?;
        let pending = output
            .lifecycle_events
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?;
        pending.check_identity(self.instances, instance, budget)?;
        let slot = self
            .producers
            .get_mut(instance.index())
            .ok_or_else(execution_identity_error_v1)?;
        if slot.is_some() || pending.function != original.function() {
            return Err(execution_identity_error_v1());
        }
        let mut rows = emission_vec_v1(original.declaration().blocks().len(), budget)?;
        budget.charge_work(original.declaration().blocks().len())?;
        rows.resize_with(original.declaration().blocks().len(), || None);
        for (ordinal, event) in pending.rows.iter().enumerate() {
            budget.charge_work(4)?;
            if matches!(event.kind, DeferredLifecycleKindV29::End { .. }) {
                continue;
            }
            let row = rows
                .get_mut(event.block.index() as usize)
                .ok_or_else(execution_identity_error_v1)?;
            if row.replace(ordinal).is_some() {
                return Err(execution_identity_error_v1());
            }
        }
        self.credit = argument_sum_v1(&[
            self.credit,
            argument_product_v1(rows.capacity(), std::mem::size_of::<Option<usize>>())?,
        ])?;
        *slot = Some(rows);
        Ok(())
    }

    fn completed_output<'emitted>(
        &self,
        instance: ProductionCallInstanceIdV1,
        emitted: &'emitted dyn ExecutionCompletedLookupV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'emitted LoweredFunctionResultV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(4)?;
        let output = emitted
            .completed(instance.index())
            .ok_or_else(execution_identity_error_v1)?;
        if self.instances.id_at(instance.index()) != Some(instance)
            || output.source_call_instance != Some(instance)
            || self
                .producers
                .get(instance.index())
                .and_then(Option::as_ref)
                .is_none()
        {
            return Err(execution_identity_error_v1());
        }
        Ok(output)
    }

    fn owned_producer(
        &self,
        identities: &ExecutionIdentityPlanV1<'_, '_>,
        binding: &SemanticExecutionBindingV29,
        expected_class: usize,
        emitted: &dyn ExecutionCompletedLookupV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let producer = binding.identity.producer;
        let row = self
            .instances
            .instance(producer.caller)
            .ok_or_else(execution_identity_error_v1)?;
        let types = self.instances.owner().source_semantic().types();
        binding
            .check_type(types, binding.identity.semantic_type)
            .map_err(|_| execution_identity_error_v1())?;
        let block = row
            .declaration()
            .blocks()
            .get(producer.block.index() as usize)
            .ok_or_else(execution_identity_error_v1)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            return Err(execution_identity_error_v1());
        };
        budget.charge_work(7)?;
        let destination = call.destination().ok_or_else(execution_identity_error_v1)?;
        if !destination.place().projections().is_empty()
            || destination.place().ty() != binding.identity.semantic_type
            || !matches!(
                self.instances
                    .owner()
                    .source_semantic()
                    .callables()
                    .get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::CompilerIntrinsic {
                    operation: SemanticCompilerIntrinsicOperationV1::Execution(_),
                    ..
                })
            )
        {
            return Err(execution_identity_error_v1());
        }
        let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(producer.block.index()), 0);
        let definitions = row
            .ssa()
            .plan()
            .edge_definitions(edge)
            .ok_or_else(execution_identity_error_v1)?;
        budget.charge_work(definitions.len())?;
        let [definition] = definitions else {
            return Err(execution_identity_error_v1());
        };
        if definition.variable().get() != destination.place().local().index() {
            return Err(execution_identity_error_v1());
        }
        let selected = identities.index.value(
            producer.caller,
            definition.value(),
            destination.place().local(),
            budget,
        )?;
        if selected.ty != binding.identity.semantic_type
            || selected.count != 1
            || identities.class(selected.first, budget)? != expected_class
        {
            return Err(execution_identity_error_v1());
        }
        let output = self.completed_output(producer.caller, emitted, budget)?;
        let archive = output
            .execution_observation
            .as_ref()
            .ok_or_else(execution_identity_error_v1)?;
        let original = archive.lookup_original_v29(
            self.instances,
            producer.caller,
            definition.value(),
            budget,
        )?;
        if !matches!(original, SemanticValueBindingV1::Execution(found) if found == binding) {
            return Err(execution_identity_error_v1());
        }
        let event_ordinal = self.producers[producer.caller.index()]
            .as_ref()
            .and_then(|rows| rows.get(producer.block.index() as usize))
            .copied()
            .flatten()
            .ok_or_else(execution_identity_error_v1)?;
        let events = output
            .lifecycle_events
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?;
        events.check_identity(self.instances, producer.caller, budget)?;
        budget.charge_work(8)?;
        let event = events
            .rows
            .get(event_ordinal)
            .ok_or_else(execution_identity_error_v1)?;
        if event.block != producer.block
            || !match event.kind {
                DeferredLifecycleKindV29::Issue { result } => {
                    result == binding.identity
                        && binding.context == result
                        && binding.workgroup.is_none()
                }
                DeferredLifecycleKindV29::Derive { context, result } => {
                    result == binding.identity
                        && context == binding.context
                        && binding.workgroup == Some(result)
                }
                DeferredLifecycleKindV29::Tile(tile) => {
                    tile.producer == producer
                        && tile.result_type == binding.identity.semantic_type
                        && tile.first_result == binding.identity.value
                        && match (tile.input, binding.role) {
                            (
                                DeferredTileInputV29::Load { workgroup, .. },
                                SemanticExecutionRoleV29::MaskedTileU32 { lanes, elements },
                            ) => {
                                tile.lanes == lanes
                                    && tile.elements == elements
                                    && binding.workgroup == Some(workgroup)
                            }
                            (
                                DeferredTileInputV29::Fragment { .. },
                                SemanticExecutionRoleV29::LaneFragmentU32 { lanes, elements },
                            ) => {
                                tile.lanes == lanes
                                    && tile.elements == elements
                                    && binding.workgroup.is_some()
                            }
                            _ => false,
                        }
                }
                DeferredLifecycleKindV29::End { .. } => false,
            }
        {
            return Err(execution_identity_error_v1());
        }
        Ok(())
    }

    fn check_fresh_returns(
        &self,
        child: ProductionCallInstanceIdV1,
        returned: &ExecutionIdentityReturnWitnessV1,
        fresh: &[bool],
        inputs: &ExecutionReturnInputsV1,
        emitted: &dyn ExecutionCompletedLookupV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.check_owner(self.instances, budget)?;
        let identities = self.identities.ok_or_else(execution_identity_error_v1)?;
        let source = identities
            .returns
            .get(child.index())
            .and_then(Option::as_ref)
            .ok_or_else(execution_identity_error_v1)?;
        if returned.expected.len() != fresh.len() || fresh.len() != source.leaves.len() {
            return Err(execution_identity_error_v1());
        }
        for ((leaf, fresh), layout) in returned.expected.iter().zip(fresh).zip(&source.leaves) {
            budget.charge_work(3)?;
            if !*fresh {
                continue;
            }
            match leaf {
                Some(ExecutionCfgLeafV29::Owned(binding)) if !layout.borrowed => {
                    if layout.ty != binding.identity.semantic_type {
                        return Err(execution_identity_error_v1());
                    }
                    let class = identities.class(layout.first, budget)?;
                    self.owned_producer(identities, binding, class, emitted, budget)?;
                }
                Some(ExecutionCfgLeafV29::Borrow(binding)) if layout.borrowed => {
                    self.borrow_producer(identities, binding, layout, inputs, emitted, budget)?;
                }
                _ => return Err(execution_identity_error_v1()),
            }
        }
        Ok(())
    }

    fn original_event_value(
        &self,
        identities: &ExecutionIdentityPlanV1<'_, '_>,
        instance: ProductionCallInstanceIdV1,
        site: ExecutionSiteV29,
        operand: ExecutionOperandV29,
        role: ExecutionEventV29,
        local: SemanticLocalIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<SsaValueV1, ProductionSemanticKirErrorV1> {
        let block = match site {
            ExecutionSiteV29::Statement { block, .. } | ExecutionSiteV29::Terminator { block } => {
                block
            }
        };
        if !execution_identity_block_active_v1(self.instances, instance, block, budget)? {
            return Err(execution_identity_error_v1());
        }
        let rows = identities
            .index
            .events
            .get(instance.index())
            .ok_or_else(execution_identity_error_v1)?;
        let key = unit_local_source_key_v1(site, operand, Some(role));
        let (mut left, mut right) = (0, rows.len());
        while left < right {
            budget.charge_work(8)?;
            let middle = left + (right - left) / 2;
            match rows[middle].key.cmp(&key) {
                std::cmp::Ordering::Less => left = middle + 1,
                std::cmp::Ordering::Greater => right = middle,
                std::cmp::Ordering::Equal => {
                    let occurrences = self
                        .instances
                        .occurrences(instance)
                        .ok_or_else(execution_identity_error_v1)?;
                    let event = occurrences
                        .events()
                        .get(rows[middle].index)
                        .ok_or_else(execution_identity_error_v1)?;
                    if !event.is_reachable()
                        || !event.is_promoted()
                        || event.site() != site
                        || event.operand() != operand
                        || event.role() != role
                    {
                        return Err(execution_identity_error_v1());
                    }
                    return match (role, event.resolved()) {
                        (
                            ExecutionEventV29::DestinationDefine,
                            Some(SsaResolvedEventV1::Define { variable, value }),
                        )
                        | (
                            ExecutionEventV29::BaseUse,
                            Some(SsaResolvedEventV1::Use { variable, value }),
                        ) if variable.get() == local.index()
                            && event.event().variable() == variable =>
                        {
                            Ok(value)
                        }
                        _ => Err(execution_identity_error_v1()),
                    };
                }
            }
        }
        Err(execution_identity_error_v1())
    }

    fn borrow_producer(
        &self,
        identities: &ExecutionIdentityPlanV1<'_, '_>,
        binding: &SemanticExecutionBorrowBindingV29,
        layout: &ExecutionIdentityReturnLeafV1,
        inputs: &ExecutionReturnInputsV1,
        emitted: &dyn ExecutionCompletedLookupV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let occurrence = binding.occurrence;
        let row = self
            .instances
            .instance(occurrence.instance)
            .ok_or_else(execution_identity_error_v1)?;
        let types = self.instances.owner().source_semantic().types();
        binding
            .check_type(types, layout.ty)
            .map_err(|_| execution_identity_error_v1())?;
        let statement = row
            .declaration()
            .blocks()
            .get(occurrence.block.index() as usize)
            .and_then(|block| block.statements().get(occurrence.statement))
            .ok_or_else(execution_identity_error_v1)?;
        let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
            return Err(execution_identity_error_v1());
        };
        let SemanticRvalueKindV1::Borrow { place, kind } = assignment.value().kind() else {
            return Err(execution_identity_error_v1());
        };
        budget.charge_work(7)?;
        if !assignment.destination().projections().is_empty()
            || assignment.destination().local() != binding.destination_local
            || assignment.destination().ty() != binding.reference_type
            || place.local() != binding.source_local
            || *kind != binding.kind
        {
            return Err(execution_identity_error_v1());
        }
        let statement =
            u32::try_from(occurrence.statement).map_err(|_| ArgumentResourceV1::Arithmetic)?;
        let site = execution_site_v29(occurrence.block, Some(statement));
        let value = self.original_event_value(
            identities,
            occurrence.instance,
            site,
            ExecutionOperandV29::Destination,
            ExecutionEventV29::DestinationDefine,
            binding.destination_local,
            budget,
        )?;
        let selected = identities.index.value(
            occurrence.instance,
            value,
            binding.destination_local,
            budget,
        )?;
        if selected.ty != layout.ty
            || selected.count != 2
            || identities.class(selected.first, budget)?
                != identities.class(layout.first, budget)?
            || identities.class(argument_sum_v1(&[selected.first, 1])?, budget)?
                != identities.class(argument_sum_v1(&[layout.first, 1])?, budget)?
        {
            return Err(execution_identity_error_v1());
        }
        let source = identities.index.place(
            occurrence.instance,
            site,
            ExecutionOperandV29::RvaluePlace,
            place,
            budget,
        )?;
        let referent_class = identities.class(source.first, budget)?;
        if source.count != 1
            || source.ty != binding.borrowed.semantic_type()
            || referent_class != identities.class(argument_sum_v1(&[layout.first, 1])?, budget)?
        {
            return Err(execution_identity_error_v1());
        }
        let output = self.completed_output(occurrence.instance, emitted, budget)?;
        let archive = output
            .execution_observation
            .as_ref()
            .ok_or_else(execution_identity_error_v1)?;
        let original =
            archive.lookup_original_v29(self.instances, occurrence.instance, value, budget)?;
        if !matches!(original, SemanticValueBindingV1::ExecutionBorrow(found) if found == binding) {
            return Err(execution_identity_error_v1());
        }
        if !inputs.contains(referent_class, &binding.borrowed, budget)? {
            self.owned_producer(
                identities,
                &binding.borrowed,
                referent_class,
                emitted,
                budget,
            )?;
        }
        let has_deref = place
            .projections()
            .iter()
            .any(|projection| matches!(projection.kind(), SemanticProjectionKindV1::Dereference));
        budget.charge_work(place.projections().len())?;
        if !has_deref {
            if binding.parent.is_some() {
                return Err(execution_identity_error_v1());
            }
            return Ok(());
        }
        // Select a reborrow's actual parent from the same original SSA archive
        // using the existing typed projection and nominal shape walks.
        let base = identities.index.source_use(
            occurrence.instance,
            site,
            ExecutionOperandV29::RvaluePlace,
            place.local(),
            budget,
        )?;
        let source_value = self.original_event_value(
            identities,
            occurrence.instance,
            site,
            ExecutionOperandV29::RvaluePlace,
            ExecutionEventV29::BaseUse,
            place.local(),
            budget,
        )?;
        let archived = archive.lookup_original_v29(
            self.instances,
            occurrence.instance,
            source_value,
            budget,
        )?;
        let floor = budget.storage();
        let checked = (|| {
            let layouts = execution_identity_return_leaf_layout_v1(types, base, budget)?;
            let mut leaves = emission_vec_v1(layouts.len(), budget)?;
            budget.charge_work(layouts.len())?;
            leaves.resize_with(layouts.len(), || None);
            merge_execution_cfg_binding_v29(
                types,
                base.ty,
                archived,
                archived,
                &mut leaves.iter_mut(),
                &mut 0,
                budget,
            )?;
            let mut parent = None;
            for (layout, leaf) in layouts.iter().zip(&leaves) {
                budget.charge_work(3)?;
                if layout.borrowed && argument_sum_v1(&[layout.first, 1])? == source.first {
                    let Some(ExecutionCfgLeafV29::Borrow(found)) = leaf else {
                        return Err(execution_identity_error_v1());
                    };
                    if parent.replace(found.occurrence).is_some()
                        || found.borrowed != binding.borrowed
                        || (found.kind == SemanticBorrowKindV1::Shared
                            && binding.kind != SemanticBorrowKindV1::Shared)
                    {
                        return Err(execution_identity_error_v1());
                    }
                }
            }
            if parent.is_none() || parent != binding.parent {
                return Err(execution_identity_error_v1());
            }
            Ok(())
        })();
        // This callback-free archive/shape check cannot allocate source arena
        // state; only its now-dropped temporary layout and leaf vectors settle.
        let refunded = budget.release_storage(
            budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?,
        );
        match checked {
            Ok(()) => refunded.map_err(Into::into),
            Err(error) => {
                let _ = refunded;
                Err(error)
            }
        }
    }
}
