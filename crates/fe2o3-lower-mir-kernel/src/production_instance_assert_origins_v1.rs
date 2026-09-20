// Shared capture of actual emitted assertions. These rows are not source proof.
#[allow(clippy::too_many_arguments)]
fn record_assert_origin_at_v1(
    records: &mut Vec<PendingAssertOriginV1>,
    captured_arguments: &mut Vec<ValueId>,
    span: SemanticKirTerminatorOperationSpanV1,
    emitted_function: &FunctionId,
    source: &SemanticTerminatorKindV1,
    emitted: &Terminator,
    elided_by_existing_rule: bool,
    placement: SemanticEmissionPlacementV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> AssertOriginResultV1<()> {
    let SemanticTerminatorKindV1::Assert {
        expected, target, ..
    } = source
    else {
        return Ok(());
    };
    budget.charge_work(1)?;
    let site = SemanticKirAssertSiteV1::new(
        span.correspondence_owner,
        span.semantic_function,
        span.semantic_block,
    );
    let bad = || assert_origin_invalid_v1(Some(site), "assert emission does not match its rule");
    let (success, arguments, outcome) = match (elided_by_existing_rule, emitted) {
        (true, Terminator::Branch { target, arguments }) => (
            *target,
            arguments.as_slice(),
            PendingAssertOutcomeV1::ElidedByExistingRule,
        ),
        (
            false,
            Terminator::ConditionalBranch {
                condition,
                then_target,
                then_arguments,
                else_target,
                else_arguments,
            },
        ) => {
            let (success, arguments, failure, failure_arguments) = if *expected {
                (*then_target, then_arguments, *else_target, else_arguments)
            } else {
                (*else_target, else_arguments, *then_target, then_arguments)
            };
            if !failure_arguments.is_empty() {
                return Err(bad());
            }
            (
                success,
                arguments.as_slice(),
                PendingAssertOutcomeV1::Emitted {
                    condition: *condition,
                    failure,
                },
            )
        }
        _ => return Err(bad()),
    };
    let physical_success = BlockId(
        placement
            .first_block
            .checked_add(target.target().index())
            .ok_or(AssertOriginResourceV1::Arithmetic)?,
    );
    if success != physical_success {
        return Err(bad());
    }
    let argument_start = captured_arguments.len();
    let required_arguments = argument_start
        .checked_add(arguments.len())
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    let required_records = records
        .len()
        .checked_add(1)
        .ok_or(AssertOriginResourceV1::Arithmetic)?;
    assert_origin_reserve_v1(captured_arguments, required_arguments, budget)?;
    assert_origin_reserve_v1(records, required_records, budget)?;
    budget.charge_work(
        arguments
            .len()
            .checked_add(1)
            .ok_or(AssertOriginResourceV1::Arithmetic)?,
    )?;
    let emitted_function = assert_origin_copy_name_v1(emitted_function.as_str(), budget)?;
    captured_arguments.extend_from_slice(arguments);
    records.push(PendingAssertOriginV1 {
        site,
        emitted_function,
        block: span.kernel_ir_block,
        first_operation: span.first_operation_ordinal,
        operation_count: span.operation_count,
        expected: *expected,
        semantic_success: target.target(),
        physical_success,
        argument_start,
        argument_count: arguments.len(),
        outcome,
    });
    Ok(())
}

struct InstanceAssertCaptureV1 {
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    source: ExecutionCallSourceV29,
    instance: ProductionCallInstanceIdV1,
    function: SemanticFunctionIdV1,
    placement: SemanticEmissionPlacementV1,
    records: Vec<PendingAssertOriginV1>,
    arguments: Vec<ValueId>,
    storage: usize,
    failed: bool,
}

impl InstanceAssertCaptureV1 {
    fn new(cursor: &ExecutionAvailabilityV29<'_>, placement: SemanticEmissionPlacementV1) -> Self {
        Self {
            ledger: cursor.ledger,
            source: cursor.source,
            instance: cursor.instance,
            function: cursor.function_id,
            placement,
            records: Vec::new(),
            arguments: Vec::new(),
            storage: 0,
            failed: false,
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn record(
        &mut self,
        span: SemanticKirTerminatorOperationSpanV1,
        name: &FunctionId,
        source: &SemanticTerminatorKindV1,
        emitted: &Terminator,
        elided: bool,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(4)?;
        if self.failed
            || span.correspondence_owner != self.source.root
            || span.semantic_function != self.function
            || span.kernel_ir_block != self.placement.block(span.semantic_block.index())?
        {
            return Err(execution_call_error_v29());
        }
        let floor = budget.storage();
        let result = record_assert_origin_at_v1(
            &mut self.records,
            &mut self.arguments,
            span,
            name,
            source,
            emitted,
            elided,
            self.placement,
            budget,
        );
        // A failed append can leave grown buffers live. Their charge moves with
        // the failed capture until the enclosing attempt drops all its payloads.
        self.storage = self
            .storage
            .checked_add(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )
            .ok_or(ArgumentResourceV1::Arithmetic)?;
        if result.is_err() {
            self.failed = true;
        }
        result.map_err(Into::into)
    }

    fn check_identity(
        &self,
        instances: &ProductionCallInstancePlanV1<'_>,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.ledger != budget.work_ledger_identity_v1() {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.charge_work(4)?;
        let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
        if self.failed
            || self.source != source
            || self.instance != instance
            || instances.instance(instance).map(|row| row.function()) != Some(self.function)
        {
            return Err(execution_call_error_v29());
        }
        Ok(())
    }
}

include!("production_instance_assert_replay_v1.rs");
