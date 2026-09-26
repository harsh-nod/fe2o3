// A pending expansion, not a verified graph or a source/lifecycle receipt.
// Original sidecars stay instance-qualified; coordinates describe the expansion.
include!("production_scoped_defined_calls_v29.rs");

// Only a locator into this exact borrowed roster. Original source/control and
// each sidecar's own typed receipts remain the admission authorities.
struct PendingActiveInstanceIndexV1 {
    rows: Vec<Option<usize>>,
    storage: usize,
}

impl PendingActiveInstanceIndexV1 {
    fn check_source_plan(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        sidecars: &[PendingInstanceSidecarsV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let count = instances.instances().len();
        budget.charge_work(argument_sum_v1(&[count, sidecars.len(), 2])?)?;
        if self.rows.len() != count {
            return Err(execution_call_error_v29());
        }
        let mut previous = None;
        for (ordinal, sidecar) in sidecars.iter().enumerate() {
            let id = sidecar
                .source_call_instance
                .ok_or_else(execution_call_error_v29)?;
            if instances.id_at(id.index()) != Some(id)
                || instances.instance_reachable(id) != Some(true)
                || previous.is_some_and(|value| value >= id.index())
                || self.rows.get(id.index()) != Some(&Some(ordinal))
            {
                return Err(execution_call_error_v29());
            }
            previous = Some(id.index());
        }
        for (original, row) in self.rows.iter().enumerate() {
            let id = instances
                .id_at(original)
                .ok_or_else(execution_call_error_v29)?;
            if instances.instance_reachable(id) != Some(row.is_some()) {
                return Err(execution_call_error_v29());
            }
            if let Some(ordinal) = row {
                if sidecars
                    .get(*ordinal)
                    .and_then(|sidecar| sidecar.source_call_instance)
                    != Some(id)
                {
                    return Err(execution_call_error_v29());
                }
            }
        }
        Ok(())
    }

    fn sidecar_ordinal(
        &self,
        original: usize,
        expected_original_count: usize,
        sidecars: &[PendingInstanceSidecarsV29],
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Option<usize>, ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        if self.rows.len() != expected_original_count || sidecars.len() > self.rows.len() {
            return Err(execution_call_error_v29());
        }
        let selected = *self
            .rows
            .get(original)
            .ok_or_else(execution_call_error_v29)?;
        if let Some(ordinal) = selected {
            let sidecar = sidecars.get(ordinal).ok_or_else(execution_call_error_v29)?;
            if sidecar.source_call_instance.map(|id| id.index()) != Some(original) {
                return Err(execution_call_error_v29());
            }
        }
        Ok(selected)
    }
}

fn pending_active_instance_index_v1(
    instances: &ExecutionInstancesV29<'_>,
    sidecars: &[PendingInstanceSidecarsV29],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingActiveInstanceIndexV1, ProductionSemanticKirErrorV1> {
    let count = instances.instances().len();
    budget.charge_work(argument_sum_v1(&[count, sidecars.len(), 2])?)?;
    let headers = argument_sum_v1(&[
        std::mem::size_of::<PendingActiveInstanceIndexV1>(),
        std::mem::size_of::<Result<PendingActiveInstanceIndexV1, ProductionSemanticKirErrorV1>>(),
    ])?;
    budget.reserve_storage(headers)?;
    let mut rows = emission_vec_v1(count, budget)?;
    rows.resize(count, None);
    for (ordinal, sidecar) in sidecars.iter().enumerate() {
        let id = sidecar
            .source_call_instance
            .ok_or_else(execution_call_error_v29)?;
        if rows
            .get_mut(id.index())
            .ok_or_else(execution_call_error_v29)?
            .replace(ordinal)
            .is_some()
        {
            return Err(execution_call_error_v29());
        }
    }
    let storage = argument_sum_v1(&[
        headers,
        argument_product_v1(rows.capacity(), std::mem::size_of::<Option<usize>>())?,
    ])?;
    let result = PendingActiveInstanceIndexV1 { rows, storage };
    result.check_source_plan(instances, sidecars, budget)?;
    Ok(result)
}
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source materialization remains gated")
)]
struct PendingInstanceSidecarsV29 {
    next_value: u32,
    execution_observation: Option<ExecutionArchiveV29>,
    source_call_instance: Option<ProductionCallInstanceIdV1>,
    invocation_entry: Option<InvocationEntryRelationV1>,
    scoped_slot_origins: Option<Vec<ScopedSlotOriginV29>>,
    scoped_initialization: Option<ScopedRetainedInitializationV29>,
    scoped_memory_anchors: Option<ScopedMemoryAnchorsV29>,
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
            execution_observation,
            source_call_instance,
            invocation_entry,
            scoped_slot_origins,
            scoped_initialization,
            scoped_memory_anchors,
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
                execution_observation,
                source_call_instance,
                invocation_entry,
                scoped_slot_origins,
                scoped_initialization,
                scoped_memory_anchors,
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
    active_instances: PendingActiveInstanceIndexV1,
    coordinates: OwnedInstanceCoordinatesV1,
    // Removed-call anchor ordinals remain pre-relocation tombstones; source
    // spans compose this witness instead of treating them as live operations.
    slot_relocation: Option<scoped_slot_relocation_v29::RelocationV29>,
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
        InstanceCorrespondenceErrorV1::Emission(
            CallInstanceEmissionErrorV1::CalleeFrameAllocation,
        ) => unsupported(
            0,
            None,
            None,
            "scoped helper frame allocation requires checked relocation",
        ),
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
    let mut active = 0_usize;
    let mut blocks = 0_usize;
    let mut operations = 0;
    let mut statements = 0;
    let mut next_block = 0;
    for (index, row) in emitted.iter().enumerate() {
        budget.charge_work(4)?;
        let id = instances
            .id_at(index)
            .ok_or_else(execution_call_error_v29)?;
        if instances.instance_reachable(id) == Some(false) {
            if row.is_some() {
                return Err(execution_call_error_v29());
            }
            continue;
        }
        active = argument_sum_v1(&[active, 1])?;
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
        for (index, block) in source.declaration().blocks().iter().enumerate() {
            budget.charge_work(1)?;
            let block_id = SemanticBlockIdV1::from_index(
                u32::try_from(index).map_err(|_| ArgumentResourceV1::Arithmetic)?,
            );
            if instances.block_reachable(id, block_id) == Some(true) {
                statements = argument_sum_v1(&[statements, block.statements().len()])?;
            }
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
    let generated = argument_product_v1(
        active.checked_sub(1).ok_or_else(execution_call_error_v29)?,
        2,
    )?;
    blocks = argument_sum_v1(&[blocks, generated])?;
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
    let generated = u32::try_from(generated).map_err(|_| ArgumentResourceV1::Arithmetic)?;
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
    assemble_pending_scoped_root_inner_v29(instances, emitted, limits, None, None, budget)
}

fn assemble_pending_scoped_root_inner_v29(
    instances: &ProductionCallInstancePlanV1<'_>,
    emitted: &mut [Option<LoweredFunctionResultV1>],
    limits: ProductionSemanticKirLimitsV1,
    frame: Option<&scoped_slot_relocation_v29::FramePermitV29<'_, '_>>,
    references: Option<&SourceReferencePlanV29<'_, '_>>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<PendingScopedRootEmissionV29, ProductionSemanticKirErrorV1> {
    scoped_raw_admission_v29::assemble_original_zero_raw_v29(
        instances, emitted, limits, frame, references, budget,
    )
}
