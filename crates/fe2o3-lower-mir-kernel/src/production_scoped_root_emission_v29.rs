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
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    // Reservation on the shared emission ledger, not total heap usage. The
    // legacy emitter also has structurally bounded, separately accounted data.
    retained_emission_storage: usize,
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

fn scoped_root_preflight_v29(
    instances: &ExecutionInstancesV29<'_>,
    root_infallible: &BTreeSet<u32>,
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
    let empty = BTreeSet::new();
    for (index, row) in instances.instances().iter().enumerate() {
        budget.charge_work(argument_sum_v1(&[row.declaration().blocks().len(), 4])?)?;
        let function = row.declaration();
        closure.charge(function.blocks().len())?;
        let logical = logical_argument_rows_v1(function);
        closure.charge_arguments(logical, limits.max_operations)?;
        if index != 0 {
            closure.charge_arguments(logical, limits.max_operations)?;
        }
        blocks = argument_sum_v1(&[
            blocks,
            function.blocks().len(),
            usize::from(semantic_requires_runtime_assert_failure(
                function,
                instances.owner().source_semantic().callables(),
                if index == 0 { root_infallible } else { &empty },
            )),
        ])?;
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
fn emit_pending_scoped_root_v29(
    checked: &crate::ProductionCheckedContextRootV29<'_>,
    source: &ExecutionLifecycleSourceV29<'_>,
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
    let floor = budget.storage();
    // The attempt encloses the planner scopes too: their explicit refunds do
    // not execute during unwinding, although their Rust-owned buffers drop.
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let (pending, kernel, private_payload) = build_pending_scoped_root_v29(
            checked,
            source,
            limits,
            closure,
            private_work,
            outer_private_payload,
            budget,
        )?;
        let retained_emission_storage = budget
            .storage()
            .checked_sub(floor)
            .ok_or(ArgumentResourceV1::Accounting)?;
        Ok(OwnedPendingScopedRootV29 {
            pending,
            kernel,
            private_payload,
            ledger: source.ledger,
            retained_emission_storage,
        })
    }));
    match result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(error)) => {
            budget.release_storage(
                budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(ArgumentResourceV1::Accounting)?,
            )?;
            Err(error)
        }
        Err(payload) => {
            if let Some(retained) = budget.storage().checked_sub(floor) {
                let _ = budget.release_storage(retained);
            }
            std::panic::resume_unwind(payload)
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn build_pending_scoped_root_v29(
    checked: &crate::ProductionCheckedContextRootV29<'_>,
    source: &ExecutionLifecycleSourceV29<'_>,
    limits: ProductionSemanticKirLimitsV1,
    closure: &mut ReachableClosureBudgetV1,
    private_work: &mut PrivateArrayLazyBudgetV1,
    outer_private_payload: PrivateArrayPayloadV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<
    (PendingScopedRootEmissionV29, Kernel, PrivateArrayPayloadV1),
    ProductionSemanticKirErrorV1,
> {
    let semantic = checked.semantic_ssa().source_semantic();
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
    for ty in checked.root().abi().source_input_types() {
        if execution_cfg_nominal_count_v29(semantic.types(), *ty, budget)? != 0 {
            return Err(execution_lifecycle_error_v29());
        }
    }
    budget.charge_work(1)?;
    if !matches!(
        semantic.types()[checked.root().abi().source_output_type().index() as usize].shape(),
        SemanticTypeShapeV1::Unit
    ) {
        return Err(execution_lifecycle_error_v29());
    }
    let entry = checked
        .root()
        .kernel_entry()
        .ok_or_else(execution_lifecycle_error_v29)?;
    let symbol = std::str::from_utf8(entry.export_symbol().as_bytes())
        .map_err(|_| unsupported(0, None, None, "kernel export symbol is not UTF-8"))?;
    let required_workgroup = entry
        .source_contract()
        .launch()
        .and_then(|launch| launch.required())
        .map(|required| required.as_array());
    let infallible = match required_workgroup {
        Some(required) => InfallibleBoundsAssertAnalysisV1::analyze(
            semantic.types(),
            semantic.callables(),
            checked.root(),
            required,
        )?,
        None => BTreeSet::new(),
    };
    let (pending, private_payload) =
        production_call_instances_v1::with_production_call_instances_v1(
            checked.semantic_ssa(),
            checked.root_id(),
            budget,
            |instances, budget| {
                Ok::<_, production_call_instances_v1::ProductionCallInstanceErrorV1>(
                    with_execution_call_scope_v29(budget, |scope, budget| {
                        scoped_root_preflight_v29(instances, &infallible, limits, closure, budget)?;
                        let mut sink = ExecutionDefinedCallSinkV29::new(scope, instances, budget)?;
                        let signature_floor = budget.storage();
                        let mut signatures = BTreeMap::new();
                        for index in 1..instances.instances().len() {
                            budget.charge_work(
                                call_splice_search_work_v1(signatures.len()).saturating_add(2),
                            )?;
                            let instance = instances
                                .id_at(index)
                                .ok_or_else(execution_call_error_v29)?;
                            let row = instances
                                .instance(instance)
                                .ok_or_else(execution_call_error_v29)?;
                            let signature = match signatures.entry(row.function()) {
                                std::collections::btree_map::Entry::Vacant(entry) => {
                                    budget.reserve_storage(std::mem::size_of::<(
                                        SemanticFunctionIdV1,
                                        LoweredFunctionSignatureV1,
                                    )>(
                                    ))?;
                                    entry.insert(execution_function_signature_v29(
                                        instances, instance, budget,
                                    )?)
                                }
                                std::collections::btree_map::Entry::Occupied(entry) => {
                                    entry.into_mut()
                                }
                            };
                            closure.charge_arguments(
                                argument_product_v1(
                                    signature.parameter_types.len().saturating_sub(
                                        logical_argument_rows_v1(row.declaration()),
                                    ),
                                    2,
                                )?,
                                limits.max_operations,
                            )?;
                        }
                        let signature_storage = budget
                            .storage()
                            .checked_sub(signature_floor)
                            .ok_or(ArgumentResourceV1::Accounting)?;
                        let mut emitted = emission_vec_v1(instances.instances().len(), budget)?;
                        budget.charge_work(instances.instances().len())?;
                        emitted.resize_with(instances.instances().len(), || None);
                        let mut position = ScopedRootPlacementV29 {
                            next: SemanticEmissionPlacementV1::default(),
                            remaining_operations: limits.max_operations,
                            private_payload: outer_private_payload,
                        };
                        let root_plan = kernel_entry_plan_v1(
                            semantic,
                            checked.root_id(),
                            checked.root_id(),
                            FunctionId::new(symbol),
                            limits.max_operations,
                            closure,
                        )?;
                        let mut producer = ExecutionLifecycleProducerV29::new(
                            source,
                            instances,
                            instances.root(),
                            position.next,
                            budget,
                        )?;
                        let root = with_execution_availability_v29(
                            instances,
                            instances.root(),
                            budget,
                            |cursor, budget| {
                                lower_one_semantic_function_with_calls_v29(
                                    semantic,
                                    &root_plan,
                                    checked.root_plan(),
                                    &BTreeMap::new(),
                                    &signatures,
                                    required_workgroup,
                                    infallible,
                                    checked.launch().source_rank(),
                                    true,
                                    position.remaining_operations,
                                    None,
                                    private_work,
                                    Some(PrivateArraySourcesV1::Pending(position.private_payload)),
                                    budget,
                                    position.next,
                                    Some(cursor),
                                    Some(&mut sink),
                                    Some(&mut producer),
                                )
                            },
                        )?;
                        drop(root_plan);
                        position.advance(&root, limits, private_work, budget)?;
                        emitted[instances.root().index()] = Some(root);
                        while let Some(pending) = sink.pop_pending() {
                            budget.charge_work(3)?;
                            let child = pending.child;
                            let slot = emitted
                                .get_mut(child.index())
                                .ok_or_else(execution_call_error_v29)?;
                            if slot.is_some() {
                                return Err(execution_call_error_v29());
                            }
                            let row = instances
                                .instance(child)
                                .ok_or_else(execution_call_error_v29)?;
                            let plan_floor = budget.storage();
                            let plan = execution_instance_plan_v29(
                                instances,
                                child,
                                pending.kernel_ir_function,
                                position.next,
                                budget,
                            )?;
                            let plan_storage = budget
                                .storage()
                                .checked_sub(plan_floor)
                                .ok_or(ArgumentResourceV1::Accounting)?;
                            let (_, parameters) = prepare_execution_parameters_v29(
                                instances,
                                child,
                                pending.arguments,
                                &plan,
                                budget,
                            )?;
                            let mut producer = ExecutionLifecycleProducerV29::new(
                                source,
                                instances,
                                child,
                                position.next,
                                budget,
                            )?;
                            let lowered = with_execution_availability_v29(
                                instances,
                                child,
                                budget,
                                |cursor, budget| {
                                    lower_one_semantic_function_with_calls_v29(
                                        semantic,
                                        &plan,
                                        row.ssa(),
                                        &BTreeMap::new(),
                                        &signatures,
                                        None,
                                        BTreeSet::new(),
                                        checked.launch().source_rank(),
                                        false,
                                        position.remaining_operations,
                                        None,
                                        private_work,
                                        Some(PrivateArraySourcesV1::Pending(
                                            position.private_payload,
                                        )),
                                        budget,
                                        position.next,
                                        Some(cursor.with_call_parameters_v29(parameters)?),
                                        Some(&mut sink),
                                        Some(&mut producer),
                                    )
                                },
                            )?;
                            drop(plan);
                            budget.release_storage(plan_storage)?;
                            position.advance(&lowered, limits, private_work, budget)?;
                            *slot = Some(lowered);
                        }
                        sink.finish(budget)?;
                        let pending = assemble_pending_scoped_root_v29(
                            instances,
                            &mut emitted,
                            limits,
                            budget,
                        )?;
                        let slot_bytes = argument_product_v1(
                            emitted.capacity(),
                            std::mem::size_of::<Option<LoweredFunctionResultV1>>(),
                        )?;
                        drop(emitted);
                        drop(signatures);
                        budget
                            .release_storage(argument_sum_v1(&[slot_bytes, signature_storage])?)?;
                        Ok((pending, position.private_payload))
                    }),
                )
            },
        )
        .map_err(scoped_root_instance_error_v29)??;
    let layout = checked.launch().layout();
    let kernel = semantic_kernel_metadata_v1(
        symbol,
        &pending.function,
        required_workgroup,
        checked.launch().source_rank(),
        Some(RetainedRankedLaunchRootV1 {
            selected_root: checked.root_id(),
            launch_rank: checked.launch().source_rank(),
            global_extents: layout.global_extents(),
            workgroup_extents: layout.workgroup_extents(),
            full_physical_workgroups: layout.full_physical_workgroups(),
        }),
    )?;
    Ok((pending, kernel, private_payload))
}
