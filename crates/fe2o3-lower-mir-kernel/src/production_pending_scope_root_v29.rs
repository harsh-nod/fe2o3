// A pending expansion, not a verified graph or a source/lifecycle receipt.
// Original sidecars stay instance-qualified; coordinates describe the expansion.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source materialization remains gated")
)]
struct PendingInstanceSidecarsV29 {
    next_value: u32,
    #[cfg(test)]
    execution_observation: Option<ExecutionTestObservationV29>,
    source_call_instance: Option<ProductionCallInstanceIdV1>,
    instance_assert_origins: Option<InstanceAssertCaptureV1>,
    lifecycle_events: Option<PendingLifecycleEventsV29>,
    private_arrays: PrivateArrayFunctionRowsV1,
    operation_capabilities: BTreeSet<fe2o3_kernel_ir::TargetCapability>,
    diagnostic_declarations: BTreeMap<FunctionId, Function>,
    float_declarations: BTreeMap<FunctionId, Function>,
    blocks: Vec<SemanticKirBlockCorrespondenceV1>,
    statement_operation_spans: Vec<SemanticKirStatementOperationSpanV1>,
    terminator_operation_spans: Vec<SemanticKirTerminatorOperationSpanV1>,
    generated_terminator_values: Vec<SemanticKirGeneratedTerminatorValuesV1>,
    call_returns: CallReturnBufferV1,
    synthetic_operation_spans: Vec<SemanticKirSyntheticOperationSpanV1>,
    parameter_bindings: Vec<SemanticKirParameterBindingV1>,
    parameter_component_bindings: Vec<SemanticKirParameterComponentBindingV1>,
    ignored_parameter_bindings: Vec<SemanticKirIgnoredParameterBindingV1>,
    emitted_operations: usize,
}

impl PendingInstanceSidecarsV29 {
    fn split(emitted: LoweredFunctionResultV1) -> (Function, Self) {
        // Exhaustive moves make newly added emitter metadata a compile error
        // here until its retention contract is explicitly handled.
        let LoweredFunctionResultV1 {
            next_value,
            #[cfg(test)]
            execution_observation,
            source_call_instance,
            instance_assert_origins,
            lifecycle_events,
            private_arrays,
            function,
            operation_capabilities,
            diagnostic_declarations,
            float_declarations,
            blocks,
            statement_operation_spans,
            terminator_operation_spans,
            generated_terminator_values,
            call_returns,
            synthetic_operation_spans,
            parameter_bindings,
            parameter_component_bindings,
            ignored_parameter_bindings,
            emitted_operations,
        } = emitted;
        (
            function,
            Self {
                next_value,
                #[cfg(test)]
                execution_observation,
                source_call_instance,
                instance_assert_origins,
                lifecycle_events,
                private_arrays,
                operation_capabilities,
                diagnostic_declarations,
                float_declarations,
                blocks,
                statement_operation_spans,
                terminator_operation_spans,
                generated_terminator_values,
                call_returns,
                synthetic_operation_spans,
                parameter_bindings,
                parameter_component_bindings,
                ignored_parameter_bindings,
                emitted_operations,
            },
        )
    }
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source materialization remains gated")
)]
struct PendingScopedRootEmissionV29 {
    function: Function,
    sidecars: InstanceRowsV1<PendingInstanceSidecarsV29>,
    coordinates: OwnedInstanceCoordinatesV1,
    additional_storage_bytes: usize,
}

fn pending_scope_correspondence_error_v29(
    error: InstanceCorrespondenceErrorV1,
) -> ProductionSemanticKirErrorV1 {
    match error {
        InstanceCorrespondenceErrorV1::Resource(error)
        | InstanceCorrespondenceErrorV1::Emission(CallInstanceEmissionErrorV1::Resource(error)) => {
            error.into()
        }
        _ => execution_call_error_v29(),
    }
}

fn pending_scope_preflight_v29(
    instances: &ProductionCallInstancePlanV1<'_>,
    emitted: &[Option<LoweredFunctionResultV1>],
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<u32, ProductionSemanticKirErrorV1> {
    budget.charge_work(4)?;
    if emitted.is_empty() || emitted.len() != instances.instances().len() {
        return Err(execution_call_error_v29());
    }
    enforce_limit(
        ProductionSemanticKirResourceV1::Functions,
        emitted.len(),
        limits.max_functions,
    )?;
    let mut blocks = argument_product_v1(emitted.len() - 1, 2)?;
    let mut operations = 0;
    let mut statements = 0;
    let mut next_block = 0;
    for (index, row) in emitted.iter().enumerate() {
        budget.charge_work(4)?;
        let row = row.as_ref().ok_or_else(execution_call_error_v29)?;
        if row.source_call_instance != instances.id_at(index) {
            return Err(execution_call_error_v29());
        }
        if let Some(events) = &row.lifecycle_events {
            events.check_identity(
                instances,
                instances
                    .id_at(index)
                    .ok_or_else(execution_call_error_v29)?,
                budget,
            )?;
            operations = argument_sum_v1(&[operations, events.rows.len()])?;
        }
        row.instance_assert_origins
            .as_ref()
            .ok_or_else(execution_call_error_v29)?
            .check_identity(
                instances,
                instances
                    .id_at(index)
                    .ok_or_else(execution_call_error_v29)?,
                budget,
            )?;
        let source = instances
            .instance(
                instances
                    .id_at(index)
                    .ok_or_else(execution_call_error_v29)?,
            )
            .ok_or_else(execution_call_error_v29)?;
        for block in source.declaration().blocks() {
            budget.charge_work(1)?;
            statements = argument_sum_v1(&[statements, block.statements().len()])?;
        }
        let body = row
            .function
            .body
            .as_ref()
            .ok_or_else(execution_call_error_v29)?;
        blocks = argument_sum_v1(&[blocks, body.blocks.len()])?;
        for block in &body.blocks {
            budget.charge_work(2)?;
            operations = argument_sum_v1(&[operations, block.operations.len()])?;
            next_block = next_block.max(
                block
                    .id
                    .0
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
            );
        }
    }
    enforce_limit(
        ProductionSemanticKirResourceV1::Blocks,
        blocks,
        limits.max_blocks,
    )?;
    enforce_limit(
        ProductionSemanticKirResourceV1::Statements,
        statements,
        limits.max_statements,
    )?;
    enforce_limit(
        ProductionSemanticKirResourceV1::Operations,
        operations,
        limits.max_operations,
    )?;
    let generated = u32::try_from(argument_product_v1(emitted.len() - 1, 2)?)
        .map_err(|_| ArgumentResourceV1::Arithmetic)?;
    next_block
        .checked_add(generated)
        .ok_or(ArgumentResourceV1::Arithmetic)?;
    Ok(next_block)
}

fn merge_pending_scope_capabilities_v29(
    caller: &mut BTreeSet<fe2o3_kernel_ir::TargetCapability>,
    callee: &mut BTreeSet<fe2o3_kernel_ir::TargetCapability>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ArgumentResourceV1> {
    budget.charge_work(1)?;
    if callee.is_empty() {
        return Ok(());
    }
    let count = argument_sum_v1(&[caller.len(), callee.len()])?;
    let mut comparison_width = 16;
    for capability in caller.iter().chain(callee.iter()) {
        budget.charge_work(3)?;
        if let fe2o3_kernel_ir::TargetCapability::Extension { namespace, name } = capability {
            comparison_width =
                comparison_width.max(argument_sum_v1(&[16, namespace.len(), name.len()])?);
        }
    }
    budget.charge_work(argument_product_v1(
        argument_product_v1(count, call_splice_search_work_v1(count))?,
        comparison_width,
    )?)?;
    // Keys move; their existing strings remain paid by the input owner.
    // Prepay the logical union while the old set backings can coexist.
    budget.reserve_storage(argument_product_v1(
        count,
        std::mem::size_of::<fe2o3_kernel_ir::TargetCapability>(),
    )?)?;
    caller.append(callee);
    Ok(())
}

/// Keeps the caller's entire preexisting emission reservation and input backing
/// live. The receipt covers only new retained storage. After roster preflight,
/// failure can consume slots: the enclosing emitter must discard that attempt,
/// drop remaining payloads, and release its own original reservation once.
/// Assertion captures and lifecycle events move with their prepaid sidecars.
/// Their inherited reservations remain part of the enclosing emission floor,
/// including failures after a lifecycle producer has transferred its buffer.
/// Coordinate
/// replay checks attachment, not source equivalence or scope/lifecycle admission.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source materialization remains gated")
)]
fn assemble_pending_scoped_root_v29(
    instances: &ProductionCallInstancePlanV1<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    limits: ProductionSemanticKirLimitsV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingScopedRootEmissionV29, ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let mut next_block = pending_scope_preflight_v29(instances, emitted, limits, budget)?;
    let result = with_production_instance_correspondence_v1(instances, budget, |map, budget| {
        // Validate every shared-emitter result before taking any caller slot.
        for (index, row) in emitted.iter().enumerate() {
            map.append_lowered(
                instances
                    .id_at(index)
                    .ok_or(InstanceCorrespondenceErrorV1::Source)?,
                row.as_ref().ok_or(InstanceCorrespondenceErrorV1::Source)?,
                budget,
            )?;
        }
        let mut sidecars = InstanceRowsV1::new();
        let mut functions = InstanceRowsV1::new();
        let mut retained = 0;
        let mut scratch = 0;
        sidecars.reserve(emitted.len(), budget, &mut retained)?;
        functions.reserve(emitted.len(), budget, &mut scratch)?;
        budget.charge_work(emitted.len())?;
        for row in emitted.iter_mut() {
            let (function, metadata) = PendingInstanceSidecarsV29::split(
                row.take().ok_or(InstanceCorrespondenceErrorV1::Source)?,
            );
            functions.rows.push(Some(function));
            sidecars.rows.push(metadata);
        }
        for index in (1..functions.rows.len()).rev() {
            budget.charge_work(4)?;
            let instance = instances
                .id_at(index)
                .ok_or(InstanceCorrespondenceErrorV1::Source)?;
            let call = instances
                .incoming(instance)
                .ok_or(InstanceCorrespondenceErrorV1::Source)?;
            let caller = call.occurrence().caller.index();
            if caller >= index {
                return Err(InstanceCorrespondenceErrorV1::Source);
            }
            let mut expanded = map.splice(
                call,
                functions.rows[caller]
                    .take()
                    .ok_or(InstanceCorrespondenceErrorV1::Source)?,
                functions.rows[index]
                    .take()
                    .ok_or(InstanceCorrespondenceErrorV1::Source)?,
                BlockId(next_block),
                BlockId(next_block + 1),
                budget,
            )?;
            next_block += 2;
            merge_pending_scope_capabilities_v29(
                &mut expanded.caller.required_capabilities,
                &mut expanded.callee_required_capabilities,
                budget,
            )?;
            functions.rows[caller] = Some(expanded.caller);
        }
        let function = functions.rows[0]
            .take()
            .ok_or(InstanceCorrespondenceErrorV1::Source)?;
        drop(functions);
        budget.release_storage(scratch)?;
        let coordinates = map.take_owned_coordinates_v1(&function, budget)?;
        // Calculate all fallible accounting before moving the coordinate owner.
        let additional_storage_bytes = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        Ok::<_, InstanceCorrespondenceErrorV1>(PendingScopedRootEmissionV29 {
            function,
            sidecars,
            coordinates,
            additional_storage_bytes,
        })
    });
    match result {
        Ok(pending) => {
            if let Err(error) = replay_pending_instance_asserts_v1(&pending, instances, budget) {
                drop(pending);
                let extra = budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                budget.release_storage(extra)?;
                return Err(error);
            }
            Ok(pending)
        }
        Err(error) => {
            // Map cleanup and all failed payload drops precede this refund.
            let extra = budget
                .storage()
                .checked_sub(floor)
                .ok_or(ArgumentResourceV1::Accounting)?;
            budget.release_storage(extra)?;
            Err(pending_scope_correspondence_error_v29(error))
        }
    }
}
