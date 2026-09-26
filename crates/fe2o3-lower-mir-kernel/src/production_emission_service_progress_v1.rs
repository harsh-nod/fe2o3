// Checked arithmetic over already emitted child outputs, not a second CFG.
// A subtree contribution can move only to its original immediate caller.
struct EmissionSubtreeProgressV1 {
    source: ExecutionCallSourceV29,
    instance: ProductionCallInstanceIdV1,
    placement: SemanticEmissionPlacementV1,
    next_value: u32,
    descendant_operations: usize,
    descendant_payload: PrivateArrayPayloadV1,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    credit: usize,
}

struct CheckedChildEmissionProgressV1 {
    source: ExecutionCallSourceV29,
    instance: ProductionCallInstanceIdV1,
    caller: Option<ProductionCallOccurrenceV1>,
    placement: SemanticEmissionPlacementV1,
    next_value: u32,
    operations: usize,
    payload: PrivateArrayPayloadV1,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    credit: usize,
}

impl EmissionSubtreeProgressV1 {
    fn new(
        instances: &ExecutionInstancesV29<'_>,
        instance: ProductionCallInstanceIdV1,
        placement: SemanticEmissionPlacementV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.charge_work(3)?;
        if instances.instance(instance).is_none()
            || instances.instance_reachable(instance) != Some(true)
        {
            return Err(execution_call_error_v29());
        }
        let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
        let slot = budget
            .emission_service_slot_v1()
            .ok_or_else(emission_service_error_v1)?;
        let credit = argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<CheckedChildEmissionProgressV1>(),
            std::mem::size_of::<Result<CheckedChildEmissionProgressV1, ProductionSemanticKirErrorV1>>(
            ),
        ])?;
        budget.reserve_storage(credit)?;
        Ok(Self {
            source,
            instance,
            placement,
            next_value: placement.first_value,
            descendant_operations: 0,
            descendant_payload: PrivateArrayPayloadV1::default(),
            ledger: budget.work_ledger_identity_v1(),
            slot,
            credit,
        })
    }

    fn record_child(
        &mut self,
        request: &ExecutionCallRequestV1,
        child: CheckedChildEmissionProgressV1,
        private: &mut PrivateArrayRecorderWorkV1<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        child.check_request(request, budget)?;
        budget.charge_work(5)?;
        if self.source != request.source
            || self.instance != request.occurrence.caller
            || self.ledger != child.ledger
            || self.slot != child.slot
            || child.placement.first_value < self.next_value
        {
            return Err(emission_service_error_v1());
        }
        let operations = argument_sum_v1(&[self.descendant_operations, child.operations])?;
        let payload = private.merge_payload(self.descendant_payload, child.payload)?;
        self.next_value = child.next_value;
        self.descendant_operations = operations;
        self.descendant_payload = payload;
        let credit = child.credit;
        drop(child);
        budget.release_storage(credit)?;
        Ok(())
    }

    fn finish(
        self,
        output: &LoweredFunctionResultV1,
        instances: &ExecutionInstancesV29<'_>,
        private: &mut PrivateArrayLazyBudgetV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<CheckedChildEmissionProgressV1, ProductionSemanticKirErrorV1> {
        budget.charge_work(6)?;
        if self.source != ExecutionCallSourceV29::from_instances(instances, budget)?
            || budget.work_ledger_identity_v1() != self.ledger
            || budget.emission_service_slot_v1() != Some(self.slot)
            || output.source_call_instance != Some(self.instance)
            || output.next_value < self.next_value
        {
            return Err(emission_service_error_v1());
        }
        let lifecycle = output
            .lifecycle_events
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?;
        lifecycle.check_identity(instances, self.instance, budget)?;
        if lifecycle.placement != self.placement
            || output.private_arrays.placement != self.placement
        {
            return Err(emission_service_error_v1());
        }
        let body = output
            .function
            .body
            .as_ref()
            .ok_or_else(emission_service_error_v1)?;
        let mut actual_operations = 0;
        for block in &body.blocks {
            budget.charge_work(1)?;
            actual_operations = argument_sum_v1(&[actual_operations, block.operations.len()])?;
        }
        if actual_operations != output.emitted_operations {
            return Err(emission_service_error_v1());
        }
        let operations = argument_sum_v1(&[
            self.descendant_operations,
            actual_operations,
            lifecycle.rows.len(),
        ])?;
        let payload = if output.private_arrays.active {
            self.descendant_payload
                .add(output.private_arrays.payload, private)?
        } else {
            self.descendant_payload
        };
        Ok(CheckedChildEmissionProgressV1 {
            source: self.source,
            instance: self.instance,
            caller: instances
                .incoming(self.instance)
                .map(|incoming| incoming.occurrence()),
            placement: self.placement,
            next_value: output.next_value,
            operations,
            payload,
            ledger: self.ledger,
            slot: self.slot,
            credit: self.credit,
        })
    }
}

impl CheckedChildEmissionProgressV1 {
    fn check_request(
        &self,
        request: &ExecutionCallRequestV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        budget.charge_work(6)?;
        if self.source != request.source
            || self.instance != request.child
            || self.caller != Some(request.occurrence)
            || self.ledger != budget.work_ledger_identity_v1()
            || Some(self.slot) != budget.emission_service_slot_v1()
            || self.next_value < self.placement.first_value
        {
            return Err(emission_service_error_v1());
        }
        Ok(())
    }
}

impl DetachedEmissionStateV1<'_> {
    fn admit_child_progress_v1(
        &mut self,
        request: &ExecutionCallRequestV1,
        child: &CheckedChildEmissionProgressV1,
        services: &mut EmissionServicesV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let Some(budget) = services.budget.as_deref_mut() else {
            self.deny_original_root_refund();
            return Err(emission_service_error_v1());
        };
        request.check_position_v1(&self.lowering, budget)?;
        child.check_request(request, budget)?;
        budget.charge_work(7)?;
        let source = self
            .lowering
            .execution
            .as_ref()
            .and_then(|cursor| cursor.references)
            .map(|row| row.plan);
        if !budget.permits_prepared_input_refund_v1(
            source,
            self.seal.slots.budget,
            self.seal.ledger,
            self.seal.storage,
            0,
        ) || !self.seal.growth.as_ref().is_none_or(|growth| {
            self.seal
                .storage
                .checked_sub(self.seal.credit)
                .is_some_and(|entry| {
                    growth.permits_refund(entry, self.seal.storage, budget.storage(), 0)
                })
        }) {
            self.deny_original_root_refund();
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if self.lowering.next_value != self.seal.next_value
            || child.placement.first_value != self.seal.next_value
            || self.lowering.emitted_operations != self.seal.emitted_operations
            || self.lowering.max_operations != self.seal.max_operations
            || self
                .lowering
                .execution
                .as_ref()
                .map(emission_service_source_v1)
                != self.seal.source
        {
            return Err(emission_service_error_v1());
        }
        let remaining = self
            .lowering
            .max_operations
            .checked_sub(child.operations)
            .ok_or_else(emission_service_error_v1)?;
        if remaining < self.lowering.emitted_operations {
            return Err(emission_service_error_v1());
        }
        let outer_payload = services.private.carry_inherited_payload_v1(
            self.lowering.private_arrays.outer_payload, child.payload,
        )?;
        self.lowering.next_value = child.next_value;
        self.seal.next_value = child.next_value;
        self.lowering.max_operations = remaining;
        self.seal.max_operations = remaining;
        self.lowering.private_arrays.outer_payload = outer_payload;
        Ok(())
    }
}
