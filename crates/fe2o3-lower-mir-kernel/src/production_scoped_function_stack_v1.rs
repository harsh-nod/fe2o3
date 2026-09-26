#[allow(clippy::too_many_arguments)]
fn emit_owned_function_frame_v1<'source>(
    cursor: OwnedExecutionAvailabilityV1<'source>,
    semantic: &'source fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    plan: LoweredFunctionPlanV1,
    semantic_ssa: &'source ProductionSemanticSsaFunctionPlanV1,
    defined_function_ids: &'source BTreeMap<SemanticFunctionIdV1, FunctionId>,
    defined_function_signatures: impl Into<ExecutionSignatureSourceV29<'source>>,
    required_workgroup: Option<[u32; 3]>,
    infallible_asserts: impl Into<InfallibleAssertDecisionsV1<'source>>,
    launch_rank: u8,
    authenticated_ranked_control: bool,
    max_operations: usize,
    private_array_work: &mut PrivateArrayLazyBudgetV1,
    private_array_sources: Option<PrivateArraySourcesV1<'source>>,
    budget: &mut ArgumentBudgetV1<'_>,
    placement: SemanticEmissionPlacementV1,
    execution_calls: &mut dyn ExecutionDefinedCallConsumerV29,
    lifecycle: &mut dyn ExecutionLifecycleConsumerV29,
) -> Result<LoweredFunctionResultV1, ProductionSemanticKirErrorV1> {
    #[cfg(not(test))]
    let detach_blocks = false;
    #[cfg(test)]
    let detach_blocks = DETACH_EMISSION_BLOCKS_V1.get();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let mut frame = cursor.prepare_frame(
            semantic,
            FunctionEmissionPlanV1::Owned(plan),
            semantic_ssa,
            defined_function_ids,
            defined_function_signatures,
            required_workgroup,
            infallible_asserts.into(),
            launch_rank,
            authenticated_ranked_control,
            max_operations,
            private_array_work,
            private_array_sources,
            budget,
            placement,
            execution_calls,
            lifecycle,
        )?;
        while frame.step()? {
            if detach_blocks {
                let (suspended, services) = frame.detach()?;
                frame = suspended.attach(services)?;
            }
        }
        frame.finish()?.assemble(budget)
    }));
    match outcome {
        Ok(result) => result,
        Err(payload) => {
            let _ = source_reference_discard_v29([Some(payload)]);
            Err(source_reference_error_v29(
                "source reference availability construction or callback panicked",
            ))
        }
    }
}

// Services are released at each boundary; source state and its paid cursor stay
// in the opaque frame owner until the same original child has actually finished.
enum ScopedFunctionBoundaryV1<'source> {
    Await(SuspendedCallFunctionFrameV1<'source>),
    Complete(CompletedFunctionFrameV1<'source>),
}

struct ScopedActiveFunctionV1 {
    instance: ProductionCallInstanceIdV1,
    progress: EmissionSubtreeProgressV1,
    plan_storage: usize,
}

struct ScopedWaitingFunctionV1<'source> {
    frame: SuspendedCallFunctionFrameV1<'source>,
    active: ScopedActiveFunctionV1,
}

fn scoped_function_stack_headers_v1(count: usize) -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<Vec<Option<ExecutionLifecycleProducerV29<'static>>>>(),
        std::mem::size_of::<Vec<ScopedWaitingFunctionV1<'static>>>(),
        std::mem::size_of::<ScopedActiveFunctionV1>(),
        argument_product_v1(count, argument_sum_v1(&[
            std::mem::size_of::<ScopedFunctionBoundaryV1<'static>>(),
            std::mem::size_of::<Result<ScopedFunctionBoundaryV1<'static>, ProductionSemanticKirErrorV1>>(),
            std::mem::size_of::<std::thread::Result<Result<ScopedFunctionBoundaryV1<'static>, ProductionSemanticKirErrorV1>>>(),
        ])?)?,
    ])
}

fn scoped_function_stack_output_headers_v1() -> Result<usize, ArgumentResourceV1> {
    argument_sum_v1(&[
        std::mem::size_of::<Vec<Option<LoweredFunctionResultV1>>>(),
        std::mem::size_of::<(Vec<Option<LoweredFunctionResultV1>>, PrivateArrayPayloadV1)>(),
        std::mem::size_of::<Result<(Vec<Option<LoweredFunctionResultV1>>, PrivateArrayPayloadV1), ProductionSemanticKirErrorV1>>(),
    ])
}

fn advance_scoped_function_v1<'source>(
    mut frame: OwnedFunctionFrameV1<'source, '_>,
) -> Result<ScopedFunctionBoundaryV1<'source>, ProductionSemanticKirErrorV1> {
    #[cfg(not(test))]
    let detach_blocks = false;
    #[cfg(test)]
    let detach_blocks = DETACH_EMISSION_BLOCKS_V1.get();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| loop {
        match frame.step_until_child()? {
            FunctionFrameStepV1::Done => {
                return frame.finish().map(ScopedFunctionBoundaryV1::Complete);
            }
            FunctionFrameStepV1::Block => {
                if detach_blocks {
                    let (suspended, services) = frame.detach()?;
                    frame = suspended.attach(services)?;
                }
            }
            FunctionFrameStepV1::AwaitChild(pending) => {
                let (suspended, services) = frame.detach_call(pending)?;
                drop(services);
                return Ok(ScopedFunctionBoundaryV1::Await(suspended));
            }
        }
    }));
    match result {
        Ok(result) => result,
        Err(payload) => {
            let _ = source_reference_discard_v29([Some(payload)]);
            Err(source_reference_error_v29(
                "source reference availability construction or callback panicked",
            ))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn emit_scoped_function_stack_v1<'source>(
    source: &'source ExecutionLifecycleSourceV29<'source>,
    instances: &'source ExecutionInstancesV29<'source>,
    root_plan: LoweredFunctionPlanV1,
    references: Option<&'source SourceReferenceEmissionV29<'source, 'source>>,
    reference_plan: Option<&'source SourceReferencePlanV29<'source, 'source>>,
    identities: Option<&'source ExecutionIdentityPlanV1<'source, 'source>>,
    completion: &mut ExecutionReturnCompletionV1<'source, 'source>,
    sink: &mut ExecutionDefinedCallSinkV29<'source>,
    defined_function_ids: &'source BTreeMap<SemanticFunctionIdV1, FunctionId>,
    signatures: &'source ExecutionInstanceLayoutsV29<'source, 'source>,
    required_workgroup: Option<[u32; 3]>,
    infallible: InfallibleAssertDecisionsV1<'source>,
    launch_rank: u8,
    limits: ProductionSemanticKirLimitsV1,
    outer_private: PrivateArrayPayloadV1,
    private_work: &mut PrivateArrayLazyBudgetV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(Vec<Option<LoweredFunctionResultV1>>, PrivateArrayPayloadV1), ProductionSemanticKirErrorV1> {
    let count = instances.instances().len();
    budget.charge_work(argument_product_v1(count, 3)?)?;
    if count == 0 || instances.root().index() != 0 {
        return Err(execution_call_error_v29());
    }
    let headers = scoped_function_stack_headers_v1(count)?;
    budget.reserve_storage(headers)?;
    // This output envelope stays owned by the root until its dense emitted
    // roster is consumed; it is not disposable suspended-stack scratch.
    budget.reserve_storage(scoped_function_stack_output_headers_v1()?)?;
    let mut producers = emission_vec_v1(count, budget)?;
    producers.resize_with(count, || None);
    let mut waiting: Vec<ScopedWaitingFunctionV1<'source>> = emission_vec_v1(count, budget)?;
    let mut emitted = emission_vec_v1(count, budget)?;
    emitted.resize_with(count, || None);
    let semantic = instances.owner().source_semantic();
    let root = instances.root();
    let root_source = instances.instance(root).ok_or_else(execution_call_error_v29)?;
    let placement = SemanticEmissionPlacementV1::default();
    producers[root.index()] = Some(ExecutionLifecycleProducerV29::new(
        source, instances, root, placement, budget,
    )?);
    let mut active = ScopedActiveFunctionV1 {
        instance: root,
        progress: EmissionSubtreeProgressV1::new(instances, root, placement, budget)?,
        plan_storage: 0,
    };
    let cursor = OwnedExecutionAvailabilityV1::new(instances, root, references, identities, budget)?;
    let mut boundary = advance_scoped_function_v1(cursor.prepare_frame(
        semantic, FunctionEmissionPlanV1::Owned(root_plan), root_source.ssa(),
        defined_function_ids, signatures, required_workgroup, infallible, launch_rank,
        true, limits.max_operations, private_work,
        Some(PrivateArraySourcesV1::Pending(outer_private)), budget, placement, sink,
        producers[root.index()].as_mut().ok_or_else(execution_lifecycle_error_v29)?,
    )?)?;
    let mut next_block = 0_u32;
    let final_progress = loop {
        budget.charge_work(6)?;
        match boundary {
            ScopedFunctionBoundaryV1::Await(frame) => {
                if waiting.len() >= count || frame.request().occurrence.caller != active.instance {
                    return Err(execution_call_error_v29());
                }
                let start = frame.start();
                let placement = SemanticEmissionPlacementV1 {
                    first_block: next_block.max(start.next_block),
                    first_value: start.first_value,
                };
                next_block = placement.first_block;
                let outer = start.private_payload;
                let parent = producers.get(active.instance.index()).and_then(Option::as_ref)
                    .ok_or_else(execution_lifecycle_error_v29)?;
                let remaining = start.remaining_operations.checked_sub(parent.pending.rows.len())
                    .ok_or_else(execution_lifecycle_error_v29)?;
                let pending = sink.pop_requested_child_v1(frame.request(), budget)?;
                let child = pending.child;
                if emitted.get(child.index()).is_none_or(Option::is_some)
                    || producers.get(child.index()).is_none_or(Option::is_some)
                {
                    return Err(execution_call_error_v29());
                }
                let row = instances.instance(child).ok_or_else(execution_call_error_v29)?;
                let plan_floor = budget.storage();
                let plan = signatures.instance_plan_v29(
                    instances, child, pending.kernel_ir_function, placement, budget,
                )?;
                let plan_storage = budget.storage().checked_sub(plan_floor)
                    .ok_or(ArgumentResourceV1::Accounting)?;
                let (_, parameters) = prepare_execution_parameters_with_references_v29(
                    instances, child, pending.arguments, &plan, references, budget,
                )?;
                producers[child.index()] = Some(ExecutionLifecycleProducerV29::new(
                    source, instances, child, placement, budget,
                )?);
                let progress = EmissionSubtreeProgressV1::new(instances, child, placement, budget)?;
                waiting.push(ScopedWaitingFunctionV1 { frame, active });
                active = ScopedActiveFunctionV1 { instance: child, progress, plan_storage };
                let cursor = OwnedExecutionAvailabilityV1::new(
                    instances, child, references, identities, budget,
                )?.with_call_parameters(parameters)?;
                boundary = advance_scoped_function_v1(cursor.prepare_frame(
                    semantic, FunctionEmissionPlanV1::Owned(plan), row.ssa(), defined_function_ids,
                    signatures, None, BTreeSet::new().into(), launch_rank, false, remaining,
                    private_work, Some(PrivateArraySourcesV1::Pending(outer)), budget, placement,
                    sink, producers[child.index()].as_mut().ok_or_else(execution_lifecycle_error_v29)?,
                )?)?;
            }
            ScopedFunctionBoundaryV1::Complete(frame) => {
                let output = frame.assemble(budget)?;
                budget.release_storage(active.plan_storage)?;
                completion.record_completed(&output, budget)?;
                let progress = active.progress.finish(&output, instances, private_work, budget)?;
                let body = output.function.body.as_ref().ok_or_else(execution_call_error_v29)?;
                budget.charge_work(body.blocks.len())?;
                for block in &body.blocks {
                    next_block = next_block.max(block.id.0.checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?);
                }
                let slot = emitted.get_mut(active.instance.index()).ok_or_else(execution_call_error_v29)?;
                if slot.replace(output).is_some() { return Err(execution_call_error_v29()); }
                let Some(parent) = waiting.pop() else {
                    if active.instance != root { return Err(execution_call_error_v29()); }
                    break progress;
                };
                let completed = sink.complete_requested_child_v1(
                    parent.frame.request(), completion, &emitted, budget,
                )?;
                active = parent.active;
                let producer = producers.get_mut(active.instance.index()).and_then(Option::as_mut)
                    .ok_or_else(execution_lifecycle_error_v29)?;
                let services = EmissionServicesV1 {
                    budget: Some(budget),
                    private: PrivateArrayRecorderWorkV1::Shared(private_work),
                    calls: Some(sink),
                    lifecycle: Some(producer),
                };
                boundary = advance_scoped_function_v1(parent.frame.resume(
                    services, completed, progress, &mut active.progress,
                )?)?;
            }
        }
    };
    enforce_limit(ProductionSemanticKirResourceV1::Operations, final_progress.operations, limits.max_operations)?;
    let private_payload = private_work.carry_inherited_payload_v1(outer_private, final_progress.payload)?;
    let progress_credit = final_progress.credit;
    drop(final_progress);
    let scratch = argument_sum_v1(&[
        headers, progress_credit,
        argument_product_v1(producers.capacity(), std::mem::size_of::<Option<ExecutionLifecycleProducerV29<'_>>>())?,
        argument_product_v1(waiting.capacity(), std::mem::size_of::<ScopedWaitingFunctionV1<'_>>())?,
    ])?;
    drop(waiting);
    drop(producers);
    budget.release_storage(scratch)?;
    Ok((emitted, private_payload))
}
