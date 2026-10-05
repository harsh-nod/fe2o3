// Private source assembly only. Lifecycle insertion, verified graph admission,
// source replay and physical discharge must precede production activation.
#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source admission remains gated")
)]
struct OwnedPendingScopedRootV29 {
    pending: PendingScopedRootEmissionV29,
    kernel: Kernel,
    private_payload: PrivateArrayPayloadV1,
    source_slots: OwnedScopedSourceSlotsV29,
    terminal_failures: TerminalFailureOriginsV18,
    requires_context_issue: bool,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    // Reservation on the shared emission ledger, not total heap usage. The
    // legacy emitter also has structurally bounded, separately accounted data.
    retained_emission_storage: usize,
}

#[cfg(test)]
type SourceReferencePostflightObserverV29 = fn(
    Option<&mut SourceReferenceEmissionV29<'_, '_>>,
    &ExecutionInstancesV29<'_>,
    &mut ArgumentBudgetV1<'_>,
    bool,
);

#[cfg(test)]
thread_local! {
    static SOURCE_REFERENCE_POSTFLIGHT_OBSERVER_V29: std::cell::Cell<Option<SourceReferencePostflightObserverV29>> = const { std::cell::Cell::new(None) };
    static SOURCE_REFERENCE_ABORT_OBSERVER_V29: std::cell::Cell<Option<fn(&SourceReferenceEmissionV29<'_, '_>, &ArgumentBudgetV1<'_>)>> = const { std::cell::Cell::new(None) };
}

fn scoped_root_instance_error_v29(
    error: production_call_instances_v1::ProductionCallInstanceErrorV1,
) -> ProductionSemanticKirErrorV1 {
    match error {
        production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
            error.into()
        }
        _ => execution_call_error_v29(),
    }
}

impl From<production_call_instances_v1::ProductionCallInstanceErrorV1>
    for ProductionSemanticKirErrorV1
{
    fn from(error: production_call_instances_v1::ProductionCallInstanceErrorV1) -> Self {
        scoped_root_instance_error_v29(error)
    }
}

fn scoped_root_preflight_v29(
    instances: &ExecutionInstancesV29<'_>,
    root_infallible: &InfallibleAssertDecisionsV1<'_>,
    limits: ProductionSemanticKirLimitsV1,
    closure: &mut ReachableClosureBudgetV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let count = instances.instances().len();
    enforce_limit(
        ProductionSemanticKirResourceV1::Functions,
        count,
        limits.max_functions,
    )?;
    let mut blocks = argument_product_v1(count.saturating_sub(1), 2)?;
    let mut statements = 0;
    let empty = InfallibleAssertDecisionsV1::Legacy(BTreeSet::new());
    for (index, row) in instances.instances().iter().enumerate() {
        budget.charge_work(argument_sum_v1(&[row.declaration().blocks().len(), 4])?)?;
        let function = row.declaration();
        closure.charge(function.blocks().len())?;
        let logical = logical_argument_rows_v1(function);
        closure.charge_arguments(logical, limits.max_operations)?;
        if index != 0 {
            closure.charge_arguments(logical, limits.max_operations)?;
        }
        let has_trap = semantic_requires_runtime_assert_failure(
            function,
            instances.owner().source_semantic().callables(),
            if index == 0 { root_infallible } else { &empty },
        );
        let reserved = with_invocation_entry_plan_v1(
            function,
            row.ssa(),
            SemanticEmissionPlacementV1::default(),
            has_trap,
            budget,
            |plan, _| Ok(plan.layout.next_block as usize),
        )?;
        blocks = argument_sum_v1(&[blocks, reserved])?;
        for block in function.blocks() {
            statements = argument_sum_v1(&[statements, block.statements().len()])?;
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
    }
    Ok(())
}

struct ScopedRootPlacementV29 {
    next: SemanticEmissionPlacementV1,
    remaining_operations: usize,
    private_payload: PrivateArrayPayloadV1,
}

impl ScopedRootPlacementV29 {
    fn advance(
        &mut self,
        emitted: &LoweredFunctionResultV1,
        limits: ProductionSemanticKirLimitsV1,
        private_work: &mut PrivateArrayLazyBudgetV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let events = emitted
            .lifecycle_events
            .as_ref()
            .ok_or_else(execution_lifecycle_error_v29)?;
        let operations = argument_sum_v1(&[emitted.emitted_operations, events.rows.len()])?;
        self.remaining_operations = self.remaining_operations.checked_sub(operations).ok_or(
            ProductionSemanticKirErrorV1::ResourceLimit {
                resource: ProductionSemanticKirResourceV1::Operations,
                actual: limits.max_operations.saturating_add(1),
                limit: limits.max_operations,
            },
        )?;
        if emitted.next_value < self.next.first_value {
            return Err(execution_call_error_v29());
        }
        self.next.first_value = emitted.next_value;
        let body = emitted
            .function
            .body
            .as_ref()
            .ok_or_else(execution_call_error_v29)?;
        budget.charge_work(argument_product_v1(body.blocks.len(), 2)?)?;
        for block in &body.blocks {
            if block.id.0 < self.next.first_block {
                return Err(execution_call_error_v29());
            }
        }
        for block in &body.blocks {
            self.next.first_block = self.next.first_block.max(
                block
                    .id
                    .0
                    .checked_add(1)
                    .ok_or(ArgumentResourceV1::Arithmetic)?,
            );
        }
        if emitted.private_arrays.active {
            self.private_payload = self
                .private_payload
                .add(emitted.private_arrays.payload, private_work)?;
        }
        Ok(())
    }
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scoped source admission remains gated")
)]
#[allow(clippy::too_many_arguments)]
fn emit_pending_scoped_root_v29<'source>(
    checked: &crate::ProductionCheckedContextRootV29<'_>,
    source: &ExecutionLifecycleSourceV29<'source>,
    demands: &source_storage_demands_v29::SourceStorageDemandsV29<'source>,
    layouts: &mut source_storage_v29::SourceStorageLayoutsV29<'source>,
    limits: ProductionSemanticKirLimitsV1,
    closure: &mut ReachableClosureBudgetV1,
    private_work: &mut PrivateArrayLazyBudgetV1,
    outer_private_payload: PrivateArrayPayloadV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<OwnedPendingScopedRootV29, ProductionSemanticKirErrorV1> {
    if source.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    if !std::ptr::eq(source.owner, checked.semantic_ssa()) {
        return Err(execution_lifecycle_error_v29());
    }
    budget.charge_work(source.launch.roots().len())?;
    let ordinal = source
        .launch
        .roots()
        .iter()
        .position(|launch| std::ptr::eq(launch, checked.launch()))
        .ok_or_else(execution_lifecycle_error_v29)?;
    emit_pending_source_root_v29(
        source,
        ordinal,
        demands,
        layouts,
        limits,
        closure,
        private_work,
        outer_private_payload,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn emit_pending_source_root_v29<'source>(
    source: &ExecutionLifecycleSourceV29<'source>,
    ordinal: usize,
    demands: &source_storage_demands_v29::SourceStorageDemandsV29<'source>,
    layouts: &mut source_storage_v29::SourceStorageLayoutsV29<'source>,
    limits: ProductionSemanticKirLimitsV1,
    closure: &mut ReachableClosureBudgetV1,
    private_work: &mut PrivateArrayLazyBudgetV1,
    outer_private_payload: PrivateArrayPayloadV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<OwnedPendingScopedRootV29, ProductionSemanticKirErrorV1> {
    if source.ledger != budget.work_ledger_identity_v1() {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    let floor = budget.storage();
    let mut credit = None;
    // The attempt encloses the planner scopes too: their explicit refunds do
    // not execute during unwinding, although their Rust-owned buffers drop.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        credit = Some(layouts.capture_emission_credit(source.owner, budget)?);
        budget.charge_work(argument_sum_v1(&[source.input.roots.len(), 1])?)?;
        let launch = source
            .launch
            .roots()
            .get(ordinal)
            .ok_or_else(execution_lifecycle_error_v29)?;
        let requires_context_issue = source
            .input
            .roots
            .iter()
            .any(|root| root.root == launch.selected_root());
        let (pending, kernel, private_payload, source_slots, terminal_failures) =
            build_pending_scoped_root_v29(
                source,
                ordinal,
                demands,
                layouts,
                limits,
                closure,
                private_work,
                outer_private_payload,
                budget,
            )?;
        let total = credit
            .as_ref()
            .ok_or(ArgumentResourceV1::Accounting)?
            .root_credit(layouts, budget)
            .ok_or(ArgumentResourceV1::Accounting)?;
        let headers = source_storage_v29::SourceStorageEmissionCreditV29::headers()?;
        let retained_emission_storage = total
            .checked_sub(headers)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if !layouts.permits_root_emission_refund(source.owner, headers, budget) {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        budget.release_storage(headers)?;
        let settled = credit
            .take()
            .ok_or(ArgumentResourceV1::Accounting)?
            .into_root_credit(layouts, budget)
            .ok_or(ArgumentResourceV1::Accounting)?;
        if settled != retained_emission_storage {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        Ok(OwnedPendingScopedRootV29 {
            pending,
            kernel,
            private_payload,
            source_slots,
            terminal_failures,
            requires_context_issue,
            ledger: source.ledger,
            retained_emission_storage,
        })
    }));
    match result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => {
            let retained = match credit.take() {
                Some(credit) => credit.into_root_credit(layouts, budget),
                None => budget.storage().checked_sub(floor),
            };
            if let Some(retained) = retained
                && layouts.permits_root_emission_refund(source.owner, retained, budget)
            {
                let _ = budget.release_storage(retained);
            }
            Err(error)
        }
        Err(payload) => {
            let retained = match credit.take() {
                Some(credit) => credit.into_root_credit(layouts, budget),
                None => budget.storage().checked_sub(floor),
            };
            if let Some(retained) = retained
                && layouts.permits_root_emission_refund(source.owner, retained, budget)
            {
                let _ = budget.release_storage(retained);
            }
            std::panic::resume_unwind(payload)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_pending_scoped_root_v29<'source>(
    source: &ExecutionLifecycleSourceV29<'source>,
    ordinal: usize,
    demands: &source_storage_demands_v29::SourceStorageDemandsV29<'source>,
    layouts: &mut source_storage_v29::SourceStorageLayoutsV29<'source>,
    limits: ProductionSemanticKirLimitsV1,
    closure: &mut ReachableClosureBudgetV1,
    private_work: &mut PrivateArrayLazyBudgetV1,
    outer_private_payload: PrivateArrayPayloadV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (
        PendingScopedRootEmissionV29,
        Kernel,
        PrivateArrayPayloadV1,
        OwnedScopedSourceSlotsV29,
        TerminalFailureOriginsV18,
    ),
    ProductionSemanticKirErrorV1,
> {
    budget.charge_work(16)?;
    let semantic = source.owner.source_semantic();
    let launch = source
        .launch
        .roots()
        .get(ordinal)
        .ok_or_else(execution_lifecycle_error_v29)?;
    let root_id = launch.selected_root();
    let root = semantic
        .functions()
        .get(root_id.index() as usize)
        .ok_or_else(execution_lifecycle_error_v29)?;
    let ssa_plan = source
        .owner
        .plan_for_function(root_id)
        .ok_or_else(execution_lifecycle_error_v29)?;
    if source.launch.semantic_sha256() != source.owner.source_semantic_sha256()
        || semantic.roots().get(ordinal) != Some(&root_id)
        || launch.semantic_root_identity() != root.identity()
        || ssa_plan.function_identity() != root.identity()
        || root
            .kernel_entry()
            .map(|entry| *entry.kernel_binding_identity().as_bytes())
            != Some(launch.kernel_binding())
    {
        return Err(execution_lifecycle_error_v29());
    }
    if !semantic.allocations().is_empty()
        || !semantic.statics().is_empty()
        || !semantic.vtables().is_empty()
    {
        return Err(unsupported(
            0,
            None,
            None,
            "allocations, statics, and vtables are not lowered yet",
        ));
    }
    enforce_limit(
        ProductionSemanticKirResourceV1::Functions,
        semantic.functions().len(),
        limits.max_functions,
    )?;
    for ty in root.abi().source_input_types() {
        if execution_cfg_nominal_count_v29(semantic.types(), *ty, budget)? != 0 {
            return Err(execution_lifecycle_error_v29());
        }
    }
    budget.charge_work(1)?;
    if !matches!(
        semantic.types()[root.abi().source_output_type().index() as usize].shape(),
        SemanticTypeShapeV1::Unit
    ) {
        return Err(execution_lifecycle_error_v29());
    }
    let entry = root
        .kernel_entry()
        .ok_or_else(execution_lifecycle_error_v29)?;
    let symbol = std::str::from_utf8(entry.export_symbol().as_bytes())
        .map_err(|_| unsupported(0, None, None, "kernel export symbol is not UTF-8"))?;
    let required_workgroup = entry
        .source_contract()
        .launch()
        .and_then(|launch| launch.required())
        .map(|required| required.as_array());
    let infallible: InfallibleAssertDecisionsV1<'_> = match required_workgroup {
        Some(required) => InfallibleBoundsAssertAnalysisV1::analyze(
            semantic.types(),
            semantic.callables(),
            root,
            required,
        )?,
        None => BTreeSet::new(),
    }
    .into();
    let root_demands = demands.root_lens(source.owner, ordinal, budget)?;
    let descriptor_root = source
        .kernel_argument_abi
        .map(|profile| profile.descriptor_root(source.owner, ordinal, budget))
        .transpose()?;
    source_reference_emission_prepay_v29::<(
        PendingScopedRootEmissionV29,
        PrivateArrayPayloadV1,
        OwnedScopedSourceSlotsV29,
        TerminalFailureOriginsV18,
    )>(budget)?;
    let (pending, private_payload, source_slots, terminal_failures) =
        production_call_instances_v1::with_production_call_instances_v1(
            source.owner,
            root_id,
            budget,
            |instances, budget| {
                source_storage_v29::with_source_storage_descriptor_demands_root_v29(
                    layouts,
                    instances,
                    descriptor_root,
                    root_demands,
                    budget,
                    |reference_owner, root_storage, budget| {
                        let reference_plan = Some(reference_owner);
                        let (pending, private_payload, source_slots) =
                            scoped_raw_admission_v29::with_original_source_root_v29(
                                instances,
                                reference_plan,
                                &root_storage,
                                limits,
                                budget,
                                |preparation, budget| {
                                    let references = preparation.references();
                                    with_execution_identity_plan_v1(
                                        instances,
                                        reference_owner,
                                        &root_storage,
                                        budget,
                                        |identities, budget| {
                                            with_execution_call_scope_v29(
                                                budget,
                                                |scope, budget| {
                                                    scoped_root_preflight_v29(
                                                        instances,
                                                        &infallible,
                                                        limits,
                                                        closure,
                                                        budget,
                                                    )?;
                                                    with_execution_instance_layouts_v29(
                                                        reference_owner,
                                                        budget,
                                                        |signatures, budget| {
                                                            let mut completion =
                                                                ExecutionReturnCompletionV1::new(
                                                                    instances,
                                                                    identities,
                                                                    reference_plan,
                                                                    budget,
                                                                )?;
                                                            let mut sink =
                                                                ExecutionDefinedCallSinkV29::new(
                                                                    scope, instances, budget,
                                                                )?;
                                                            let defined_function_ids =
                                                                BTreeMap::new();
                                                            for index in
                                                                1..instances.instances().len()
                                                            {
                                                                budget.charge_work(2)?;
                                                                let instance = instances
                                                                    .id_at(index)
                                                                    .ok_or_else(
                                                                        execution_call_error_v29,
                                                                    )?;
                                                                if instances
                                                                    .instance_reachable(instance)
                                                                    != Some(true)
                                                                {
                                                                    continue;
                                                                }
                                                                let row = instances
                                                                    .instance(instance)
                                                                    .ok_or_else(
                                                                        execution_call_error_v29,
                                                                    )?;
                                                                let signature = &signatures
                                                                    .row(instance, budget)?
                                                                    .signature;
                                                                closure.charge_arguments(
                                                    argument_product_v1(
                                                        signature
                                                            .parameter_types
                                                            .len()
                                                            .saturating_sub(
                                                                logical_argument_rows_v1(
                                                                    row.declaration(),
                                                                ),
                                                            ),
                                                        2,
                                                    )?,
                                                    limits.max_operations,
                                                )?;
                                                            }
                                                            let root_plan = signatures.root_plan(
                                                                FunctionId::new(symbol),
                                                                limits.max_operations,
                                                                closure,
                                                                budget,
                                                            )?;
                                                            let (mut emitted, private_payload) =
                                                                emit_scoped_function_stack_v1(
                                                                    source,
                                                                    instances,
                                                                    root_plan,
                                                                    references,
                                                                    reference_plan,
                                                                    identities,
                                                                    &mut completion,
                                                                    &mut sink,
                                                                    &defined_function_ids,
                                                                    signatures,
                                                                    required_workgroup,
                                                                    infallible,
                                                                    launch.source_rank(),
                                                                    limits,
                                                                    outer_private_payload,
                                                                    private_work,
                                                                    budget,
                                                                )?;
                                                            sink.finish_with_completion_v1(
                                                                emitted
                                                                    .iter()
                                                                    .filter_map(Option::as_ref),
                                                                instances,
                                                                Some(&completion),
                                                                budget,
                                                            )?;
                                                            completion.discard(budget)?;
                                                            let source_slots =
                                                derive_scoped_source_slots_with_identities_v1(
                                                    instances,
                                                    &emitted,
                                                    limits.max_operations,
                                                    reference_plan,
                                                    identities,
                                                    budget,
                                                )?;
                                                            #[cfg(test)]
                                                            if let Some(observe) =
                                                                SCOPED_SLOT_OBSERVER_V29.get()
                                                            {
                                                                observe(
                                                                    source,
                                                                    instances,
                                                                    &mut emitted,
                                                                    &source_slots,
                                                                    budget,
                                                                )?;
                                                            }
                                                            #[cfg(test)]
                                                            let mut source_slots = source_slots;
                                                            #[cfg(test)]
                                                            if let Some(observe) =
                                                                SCOPED_SLOT_CUSTODY_OBSERVER_V29
                                                                    .get()
                                                            {
                                                                observe(
                                                                    instances,
                                                                    &emitted,
                                                                    &mut source_slots,
                                                                    references,
                                                                    identities,
                                                                    limits.max_operations,
                                                                    budget,
                                                                )?;
                                                            }
                                                            let unfinished = preparation.assemble(
                                                                &mut emitted,
                                                                source_slots,
                                                                identities,
                                                                private_payload,
                                                                budget,
                                                            )?;
                                                            let slot_bytes = argument_product_v1(
                                                                emitted.capacity(),
                                                                std::mem::size_of::<
                                                                    Option<LoweredFunctionResultV1>,
                                                                >(
                                                                ),
                                                            )?;
                                                            drop(emitted);
                                                            budget.release_storage(argument_sum_v1(&[
                                                slot_bytes,
                                                scoped_function_stack_output_headers_v1()?,
                                            ])?)?;
                                                            Ok(unfinished)
                                                        },
                                                    )
                                                },
                                            )
                                        },
                                    )
                                },
                            )?;
                        // Physical source census and archive custody complete
                        // before capturing terminal sites under this same owner.
                        let terminal_failures =
                            derive_terminal_failure_origins_v18(instances, &pending, budget)
                                .map_err(|error| root_storage.record_callback_failure(error))?;
                        Ok((pending, private_payload, source_slots, terminal_failures))
                    },
                )
            },
        )?;
    let layout = launch.layout();
    let kernel = semantic_kernel_metadata_v1(
        symbol,
        &pending.function,
        required_workgroup,
        launch.source_rank(),
        Some(RetainedRankedLaunchRootV1 {
            selected_root: root_id,
            launch_rank: launch.source_rank(),
            global_extents: layout.global_extents(),
            workgroup_extents: layout.workgroup_extents(),
            full_physical_workgroups: layout.full_physical_workgroups(),
        }),
    )?;
    Ok((
        pending,
        kernel,
        private_payload,
        source_slots,
        terminal_failures,
    ))
}
