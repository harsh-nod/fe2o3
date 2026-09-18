// Only the scoped source producer may supply this consumer. Catalog approval
// permits analysis, but does not create a binding or admit executable output.
trait ExecutionLifecycleConsumerV29 {
    fn check_instance(
        &self,
        execution: &ExecutionAvailabilityV29<'_>,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1>;

    fn require_catalog_entry(
        &self,
        callable: fe2o3_mir_model::semantic_mir_v1::SemanticCallableIdV1,
        declaration: &SemanticCallableDeclV1,
        budget: &mut dyn SemanticEmissionBudgetV1,
    ) -> Result<(), ProductionSemanticKirErrorV1>;

    fn produce(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operation: SemanticExecutionOperationV29,
        operations: &mut Vec<Operation>,
    ) -> Result<SemanticValueBindingV1, ProductionSemanticKirErrorV1>;

    fn normal_return(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
        block: SemanticBlockIdV1,
        operations: &[Operation],
    ) -> Result<(), ProductionSemanticKirErrorV1>;

    fn finish(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
    ) -> Result<PendingLifecycleEventsV29, ProductionSemanticKirErrorV1>;
}

impl SemanticFunctionLoweringV1<'_> {
    fn take_execution_lifecycle_events_v29(
        &mut self,
    ) -> Result<Option<PendingLifecycleEventsV29>, ProductionSemanticKirErrorV1> {
        if self.lifecycle.is_none() {
            return Ok(None);
        }
        self.with_execution_lifecycle_v29(|consumer, lowering| consumer.finish(lowering))
            .map(Some)
    }

    fn with_execution_lifecycle_v29<R>(
        &mut self,
        consume: impl FnOnce(
            &mut dyn ExecutionLifecycleConsumerV29,
            &mut Self,
        ) -> Result<R, ProductionSemanticKirErrorV1>,
    ) -> Result<R, ProductionSemanticKirErrorV1> {
        let consumer = self.lifecycle.take().ok_or_else(|| {
            unsupported(
                self.semantic_function.index(),
                None,
                None,
                "execution capabilities require checked canonical KIR materialization",
            )
        })?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let cursor = self
                .execution
                .as_ref()
                .ok_or_else(execution_availability_error_v29)?;
            let budget = self
                .emission_work
                .as_deref_mut()
                .ok_or(ArgumentResourceV1::Accounting)?;
            cursor.check_ledger(budget)?;
            let checked = consumer.check_instance(cursor, budget);
            cursor.check_ledger(budget)?;
            checked?;
            let (source, instance, function, ssa) =
                (cursor.source, cursor.instance, cursor.function, cursor.ssa);
            let result = consume(consumer, self);
            let cursor = self
                .execution
                .as_ref()
                .ok_or_else(execution_availability_error_v29)?;
            let budget = self
                .emission_work
                .as_deref_mut()
                .ok_or(ArgumentResourceV1::Accounting)?;
            cursor.check_ledger(budget)?;
            cursor.check_source(function, ssa)?;
            if cursor.source != source || cursor.instance != instance {
                return Err(execution_availability_error_v29());
            }
            result
        }));
        self.lifecycle = Some(consumer);
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }

    fn lower_execution_lifecycle_call_v29(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        operation: SemanticExecutionOperationV29,
        operations: &mut Vec<Operation>,
    ) -> Result<Terminator, ProductionSemanticKirErrorV1> {
        let destination = call
            .destination()
            .ok_or_else(execution_availability_error_v29)?;
        let binding = self.with_execution_lifecycle_v29(|consumer, lowering| {
            consumer.produce(lowering, block, call, operation, operations)
        })?;
        let prepared = self.prepare_call_destination_v1(block, destination.place(), operations)?;
        self.store_enum_payload_v1(
            block,
            None,
            destination.place().local(),
            &binding,
            operations,
        )?;
        self.finish_call_destination_v1(
            block,
            destination.place(),
            prepared,
            binding,
            None,
            operations,
        )?;
        let target = self.kernel_block_id_v1(destination.edge().target())?;
        let arguments = self.edge_arguments(block, 0, destination.edge().target(), operations)?;
        Ok(Terminator::Branch { target, arguments })
    }

    fn finish_execution_lifecycle_return_v29(
        &mut self,
        block: SemanticBlockIdV1,
        operations: &[Operation],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.lifecycle.is_none() {
            return Ok(());
        }
        self.with_execution_lifecycle_v29(|consumer, lowering| {
            consumer.normal_return(lowering, block, operations)
        })
    }
}
