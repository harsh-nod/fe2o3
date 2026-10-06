// Private occurrence transport. Scope issuance, replay and lifecycle admission
// remain the materializer's separate obligations.
trait ExecutionDefinedCallConsumerV29 {
    #[allow(clippy::too_many_arguments)]
    fn request_child_v1(
        &mut self,
        _lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
        _block: SemanticBlockIdV1,
        _call: &SemanticDirectCallV1,
        _callee: SemanticFunctionIdV1,
        _semantic_types: &[SemanticTypeIdV1],
        _projections: &[HelperCallArgumentV1],
        _parameter_types: Vec<Type>,
        _operations: &mut Vec<Operation>,
    ) -> Result<ExecutionCallRequestV1, ProductionSemanticKirErrorV1> {
        Err(execution_call_error_v29())
    }

    #[allow(clippy::too_many_arguments)]
    fn prepare(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        semantic_types: &[SemanticTypeIdV1],
        projections: &[HelperCallArgumentV1],
        parameter_types: Vec<Type>,
        operations: &mut Vec<Operation>,
    ) -> Result<PreparedExecutionDefinedCallV1, ProductionSemanticKirErrorV1>;

    fn observe_return(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
        block: SemanticBlockIdV1,
        values: &[ValueId],
    ) -> Result<(), ProductionSemanticKirErrorV1>;
}

struct ExecutionDefinedCallRouteV29 {
    occurrence: ProductionCallOccurrenceV1,
    callee: SemanticFunctionIdV1,
    child: ProductionCallInstanceIdV1,
    control: ProductionCallControlV1,
    target: Option<FunctionId>,
    returned: Option<ExecutionIdentityReturnWitnessV1>,
    awaiting_completion: bool,
    requested_taken: bool,
    fresh_returns: Vec<bool>,
    return_inputs: Option<ExecutionReturnInputsV1>,
}

struct PendingExecutionDefinedCallV29<'scope> {
    child: ProductionCallInstanceIdV1,
    kernel_ir_function: FunctionId,
    arguments: PreparedDefinedCallArgumentsV1<'scope>,
}

struct ExecutionDefinedCallSinkV29<'scope> {
    scope: ExecutionCallScopeV29<'scope>,
    source: ExecutionCallSourceV29,
    // Equality-only identity of the still-borrowed original instance roster.
    original_instances: *const (),
    routes: Vec<ExecutionDefinedCallRouteV29>,
    pending: Vec<PendingExecutionDefinedCallV29<'scope>>,
}

// A bounded borrowed roster, not graph or source authority. Every original
// active instance has exactly one output; inactive absence is mandatory. Final
// source/lifecycle replay remains independent of this locator.
fn execution_emitted_index_v1<'emitted>(
    instances: &ExecutionInstancesV29<'_>,
    mut emitted: impl Iterator<Item = &'emitted LoweredFunctionResultV1>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<Vec<Option<&'emitted LoweredFunctionResultV1>>, ProductionSemanticKirErrorV1> {
    scoped_slot_attempt_v29(budget, |budget| {
        let count = instances.instances().len();
        budget.reserve_storage(std::mem::size_of::<Vec<Option<&LoweredFunctionResultV1>>>())?;
        let mut rows = emission_vec_v1(count, budget)?;
        budget.charge_work(count)?;
        rows.resize_with(count, || None);
        let bound = argument_sum_v1(&[count, 1])?;
        for ordinal in 0..bound {
            budget.charge_work(4)?;
            let Some(output) = emitted.next() else {
                budget.charge_work(count)?;
                for (index, output) in rows.iter().enumerate() {
                    let id = instances
                        .id_at(index)
                        .ok_or_else(execution_call_error_v29)?;
                    if instances.instance_reachable(id) != Some(output.is_some()) {
                        return Err(execution_call_error_v29());
                    }
                }
                return Ok(rows);
            };
            let instance = output
                .source_call_instance
                .ok_or_else(execution_call_error_v29)?;
            if ordinal == count
                || instances.id_at(instance.index()) != Some(instance)
                || instances.instance_reachable(instance) != Some(true)
            {
                return Err(execution_call_error_v29());
            }
            let slot = rows
                .get_mut(instance.index())
                .ok_or_else(execution_call_error_v29)?;
            if slot.replace(output).is_some() {
                return Err(execution_call_error_v29());
            }
        }
        Err(execution_call_error_v29())
    })
}

fn execution_call_target_v29(
    source: ExecutionCallSourceV29,
    child: usize,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<FunctionId, ProductionSemanticKirErrorV1> {
    use std::fmt::Write;
    const PREFIX: &str = "__fe2o3_execution_";
    const BYTES: usize = PREFIX.len() + 64 + 1 + 8 + 1 + 16;
    let child = u64::try_from(child).map_err(|_| ArgumentResourceV1::Arithmetic)?;
    budget.charge_work(BYTES)?;
    let mut name = String::from_utf8(emission_vec_v1::<u8>(BYTES, budget)?)
        .map_err(|_| ArgumentResourceV1::Accounting)?;
    name.push_str(PREFIX);
    for byte in source.semantic {
        write!(&mut name, "{byte:02x}").map_err(|_| ArgumentResourceV1::Accounting)?;
    }
    write!(&mut name, "_{:08x}_{child:016x}", source.root.index())
        .map_err(|_| ArgumentResourceV1::Accounting)?;
    if name.len() != BYTES {
        return Err(ArgumentResourceV1::Accounting.into());
    }
    Ok(FunctionId::new(name))
}

fn execution_copy_call_target_v29(
    target: &FunctionId,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<FunctionId, ProductionSemanticKirErrorV1> {
    let name = target.as_str();
    budget.charge_work(name.len())?;
    let mut bytes = emission_vec_v1(name.len(), budget)?;
    bytes.extend_from_slice(name.as_bytes());
    Ok(FunctionId::new(
        String::from_utf8(bytes).map_err(|_| ArgumentResourceV1::Accounting)?,
    ))
}

#[cfg_attr(
    not(test),
    allow(dead_code, reason = "Scope materializer integration remains gated")
)]
impl<'scope> ExecutionDefinedCallSinkV29<'scope> {
    fn new(
        scope: ExecutionCallScopeV29<'scope>,
        instances: &ExecutionInstancesV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let floor = budget.storage();
        let result = Self::build(scope, instances, budget);
        if result.is_err() {
            budget.release_storage(budget.storage() - floor)?;
        }
        result
    }

    fn build(
        scope: ExecutionCallScopeV29<'scope>,
        instances: &ExecutionInstancesV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        if scope.ledger != budget.work_ledger_identity_v1() {
            return Err(execution_call_error_v29());
        }
        let source = ExecutionCallSourceV29::from_instances(instances, budget)?;
        let count = instances
            .instances()
            .len()
            .checked_sub(1)
            .ok_or_else(execution_call_error_v29)?;
        let mut routes = emission_vec_v1(count, budget)?;
        let pending = emission_vec_v1(count, budget)?;
        for index in 1..instances.instances().len() {
            budget.charge_work(8)?;
            let child = instances
                .id_at(index)
                .ok_or_else(execution_call_error_v29)?;
            let row = instances
                .instance(child)
                .ok_or_else(execution_call_error_v29)?;
            let incoming = instances
                .incoming(child)
                .ok_or_else(execution_call_error_v29)?;
            if incoming.child() != Some(child)
                || instances.instance(incoming.occurrence().caller).is_none()
                || !matches!(incoming.callable(),
                    SemanticCallableDeclV1::Defined { function } if *function == row.function())
            {
                return Err(execution_call_error_v29());
            }
            let control = instances
                .call_control(incoming.occurrence())
                .ok_or_else(execution_call_error_v29)?;
            if instances.instance_reachable(child)
                != Some(control != ProductionCallControlV1::Unreachable)
            {
                return Err(execution_call_error_v29());
            }
            routes.push(ExecutionDefinedCallRouteV29 {
                occurrence: incoming.occurrence(),
                callee: row.function(),
                child,
                control,
                target: if control != ProductionCallControlV1::Unreachable {
                    Some(execution_call_target_v29(source, index, budget)?)
                } else {
                    None
                },
                returned: None,
                awaiting_completion: false,
                requested_taken: false,
                fresh_returns: Vec::new(),
                return_inputs: None,
            });
        }
        Ok(Self {
            scope,
            source,
            original_instances: std::ptr::from_ref(instances).cast(),
            routes,
            pending,
        })
    }

    fn pop_pending(&mut self) -> Option<PendingExecutionDefinedCallV29<'scope>> {
        if self.pending.last().is_some_and(|pending| {
            pending
                .child
                .index()
                .checked_sub(1)
                .and_then(|index| self.routes.get(index))
                .is_none_or(|route| route.awaiting_completion)
        }) {
            return None;
        }
        self.pending.pop()
    }

    fn pop_requested_child_v1(
        &mut self,
        request: &ExecutionCallRequestV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<PendingExecutionDefinedCallV29<'scope>, ProductionSemanticKirErrorV1> {
        budget.charge_work(8)?;
        let index = request
            .child
            .index()
            .checked_sub(1)
            .ok_or_else(execution_call_error_v29)?;
        let route = self
            .routes
            .get_mut(index)
            .ok_or_else(execution_call_error_v29)?;
        let pending = self.pending.last().ok_or_else(execution_call_error_v29)?;
        if self.scope.ledger != budget.work_ledger_identity_v1()
            || self.source != request.source
            || route.child != request.child
            || route.callee != request.callee
            || route.occurrence != request.occurrence
            || !route.awaiting_completion
            || route.requested_taken
            || route.target.is_some()
            || pending.child != request.child
            || pending.kernel_ir_function != request.target
        {
            return Err(execution_call_error_v29());
        }
        route.requested_taken = true;
        self.pending.pop().ok_or_else(execution_call_error_v29)
    }

    fn complete_requested_child_v1(
        &mut self,
        request: &ExecutionCallRequestV1,
        completion: &ExecutionReturnCompletionV1<'_, '_>,
        emitted: &dyn ExecutionCompletedLookupV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<CheckedExecutionReturnV1, ProductionSemanticKirErrorV1> {
        completion.check_owner(completion.instances, budget)?;
        budget.charge_work(10)?;
        if self.scope.ledger != budget.work_ledger_identity_v1()
            || self.source != request.source
            || self.source != completion.source
            || self.original_instances != std::ptr::from_ref(completion.instances).cast()
        {
            return Err(execution_call_error_v29());
        }
        let incoming = completion
            .instances
            .incoming(request.child)
            .ok_or_else(execution_call_error_v29)?;
        let index = request
            .child
            .index()
            .checked_sub(1)
            .ok_or_else(execution_call_error_v29)?;
        let route = self
            .routes
            .get_mut(index)
            .ok_or_else(execution_call_error_v29)?;
        if incoming.occurrence() != request.occurrence
            || incoming.child() != Some(request.child)
            || std::ptr::from_ref(incoming.source()) as usize != request.original_call
            || route.child != request.child
            || route.callee != request.callee
            || route.occurrence != request.occurrence
            || !route.awaiting_completion
            || !route.requested_taken
            || route.target.is_some()
            || route.control == ProductionCallControlV1::Unreachable
            || completion.instances.call_control(request.occurrence) != Some(route.control)
            || (route.control == ProductionCallControlV1::MayReturn) != request.normal_return
            || (request.normal_return
                && completion.instances.instance_may_return(request.child) != Some(true))
        {
            return Err(execution_call_error_v29());
        }
        let output = completion.completed_output(request.child, emitted, budget)?;
        if output.function.id != request.target {
            return Err(execution_call_error_v29());
        }
        if let Some(returned) = &route.returned {
            returned.finish(
                route.child,
                route.callee,
                output,
                completion.instances,
                budget,
            )?;
            completion.check_fresh_returns(
                route.child,
                returned,
                &route.fresh_returns,
                route
                    .return_inputs
                    .as_ref()
                    .ok_or_else(execution_identity_error_v1)?,
                emitted,
                budget,
            )?;
        } else if !route.fresh_returns.is_empty() {
            return Err(execution_identity_error_v1());
        }
        let credit = argument_sum_v1(&[
            std::mem::size_of::<CheckedExecutionReturnV1>(),
            std::mem::size_of::<Result<CheckedExecutionReturnV1, ProductionSemanticKirErrorV1>>(),
        ])?;
        budget.reserve_storage(credit)?;
        budget.reserve_storage(std::mem::size_of::<Vec<Option<ExecutionCfgLeafV29>>>())?;
        let count = route
            .returned
            .as_ref()
            .map_or(0, |returned| returned.expected.len());
        let mut nominal = emission_vec_v1(count, budget)?;
        budget.charge_work(count)?;
        if let Some(returned) = &route.returned {
            nominal.extend(returned.expected.iter().cloned());
        }
        route.awaiting_completion = false;
        Ok(CheckedExecutionReturnV1 {
            source: request.source,
            occurrence: request.occurrence,
            child: request.child,
            original_call: request.original_call,
            callee: request.callee,
            normal_return: request.normal_return,
            nominal,
            credit,
        })
    }

    fn finish<'emitted>(
        &mut self,
        emitted: impl Iterator<Item = &'emitted LoweredFunctionResultV1>,
        instances: &ExecutionInstancesV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.finish_with_completion_v1(emitted, instances, None, budget)
    }

    fn finish_with_completion_v1<'emitted>(
        &mut self,
        emitted: impl Iterator<Item = &'emitted LoweredFunctionResultV1>,
        instances: &ExecutionInstancesV29<'_>,
        completion: Option<&ExecutionReturnCompletionV1<'_, '_>>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.scope.ledger != budget.work_ledger_identity_v1()
            || self.original_instances != std::ptr::from_ref(instances).cast()
            || self.source != ExecutionCallSourceV29::from_instances(instances, budget)?
        {
            return Err(execution_call_error_v29());
        }
        budget.charge_work(self.routes.len())?;
        if !self.pending.is_empty()
            || self
                .routes
                .iter()
                .any(|row| row.target.is_some() || row.awaiting_completion)
        {
            return Err(execution_call_error_v29());
        }
        let index = execution_emitted_index_v1(instances, emitted, budget)?;
        let index_storage = argument_sum_v1(&[
            std::mem::size_of::<Vec<Option<&LoweredFunctionResultV1>>>(),
            argument_product_v1(
                index.capacity(),
                std::mem::size_of::<Option<&LoweredFunctionResultV1>>(),
            )?,
        ])?;
        // Names and prepared payloads moved into emitted calls and child plans
        // remain paid. Only these now-empty container backings are reclaimed.
        let checked = (|| {
            let mut returned_bytes = 0;
            for (ordinal, route) in self.routes.iter().enumerate() {
                budget.charge_work(7)?;
                let original = instances
                    .instance(route.child)
                    .ok_or_else(execution_call_error_v29)?;
                let incoming = instances
                    .incoming(route.child)
                    .ok_or_else(execution_call_error_v29)?;
                if instances.id_at(ordinal + 1) != Some(route.child)
                    || original.function() != route.callee
                    || incoming.occurrence() != route.occurrence
                    || incoming.child() != Some(route.child)
                    || instances.call_control(route.occurrence) != Some(route.control)
                {
                    return Err(execution_call_error_v29());
                }
                if route.control == ProductionCallControlV1::Unreachable {
                    if route.returned.is_some()
                        || route.requested_taken
                        || route.awaiting_completion
                        || route.return_inputs.is_some()
                        || !route.fresh_returns.is_empty()
                    {
                        return Err(execution_call_error_v29());
                    }
                    continue;
                }
                let nominal = execution_identity_channels_v1(
                    instances.owner().source_semantic().types(),
                    original.declaration().abi().source_output_type(),
                    budget,
                )? != 0
                    && route.control == ProductionCallControlV1::MayReturn;
                if nominal != route.returned.is_some() {
                    return Err(execution_identity_error_v1());
                }
                if let Some(returned) = &route.returned {
                    if let Some(inputs) = &route.return_inputs {
                        let completion = completion.ok_or_else(execution_identity_error_v1)?;
                        completion.check_owner(instances, budget)?;
                        completion.check_fresh_returns(
                            route.child,
                            returned,
                            &route.fresh_returns,
                            inputs,
                            &index,
                            budget,
                        )?;
                    } else if !route.fresh_returns.is_empty() {
                        return Err(execution_identity_error_v1());
                    }
                    returned_bytes = argument_sum_v1(&[
                        returned_bytes,
                        returned.finish(
                            route.child,
                            route.callee,
                            index
                                .get(route.child.index())
                                .copied()
                                .flatten()
                                .ok_or_else(execution_call_error_v29)?,
                            instances,
                            budget,
                        )?,
                    ])?;
                }
                returned_bytes = argument_sum_v1(&[
                    returned_bytes,
                    route
                        .return_inputs
                        .as_ref()
                        .map_or(0, |inputs| inputs.credit),
                    argument_product_v1(
                        route.fresh_returns.capacity(),
                        std::mem::size_of::<bool>(),
                    )?,
                ])?;
            }
            Ok::<_, ProductionSemanticKirErrorV1>(returned_bytes)
        })();
        drop(index);
        let released = budget.release_storage(index_storage);
        let returned_bytes = match checked {
            Ok(bytes) => {
                released?;
                bytes
            }
            Err(error) => {
                let _ = released;
                return Err(error);
            }
        };
        let bytes = argument_sum_v1(&[
            returned_bytes,
            argument_product_v1(
                self.routes.capacity(),
                std::mem::size_of::<ExecutionDefinedCallRouteV29>(),
            )?,
            argument_product_v1(
                self.pending.capacity(),
                std::mem::size_of::<PendingExecutionDefinedCallV29<'scope>>(),
            )?,
        ])?;
        drop(std::mem::take(&mut self.routes));
        drop(std::mem::take(&mut self.pending));
        budget.release_storage(bytes)?;
        Ok(())
    }
}

impl ExecutionDefinedCallConsumerV29 for ExecutionDefinedCallSinkV29<'_> {
    fn request_child_v1(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        semantic_types: &[SemanticTypeIdV1],
        projections: &[HelperCallArgumentV1],
        parameter_types: Vec<Type>,
        operations: &mut Vec<Operation>,
    ) -> Result<ExecutionCallRequestV1, ProductionSemanticKirErrorV1> {
        let credit = argument_sum_v1(&[
            std::mem::size_of::<ExecutionCallRequestV1>(),
            std::mem::size_of::<Result<ExecutionCallRequestV1, ProductionSemanticKirErrorV1>>(),
        ])?;
        let budget = lowering
            .emission_work
            .as_deref_mut()
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.charge_work(2)?;
        budget.reserve_storage(credit)?;
        let (prepared, child) = self.prepare_route_v1(
            lowering,
            block,
            call,
            callee,
            semantic_types,
            projections,
            parameter_types,
            operations,
            true,
        )?;
        let (target, arguments, nominal) = prepared;
        // Deferred preparation never returns nominal bindings, including
        // input-forwarded bindings. Only completion may expose that result.
        if !nominal.is_empty() {
            return Err(execution_identity_error_v1());
        }
        drop(nominal);
        let budget = lowering
            .emission_work
            .as_deref_mut()
            .ok_or(ArgumentResourceV1::Accounting)?;
        budget.release_storage(std::mem::size_of::<Vec<Option<ExecutionCfgLeafV29>>>())?;
        let cursor = lowering
            .execution
            .as_ref()
            .ok_or_else(execution_call_error_v29)?;
        let control = cursor.source_call_control_v29(block, call, budget)?;
        if control == ProductionCallControlV1::Unreachable {
            return Err(execution_call_error_v29());
        }
        Ok(ExecutionCallRequestV1 {
            source: cursor.source,
            occurrence: ProductionCallOccurrenceV1 {
                caller: cursor.instance,
                block,
            },
            original_call: std::ptr::from_ref(call) as usize,
            child,
            callee,
            normal_return: control == ProductionCallControlV1::MayReturn,
            target,
            arguments,
            credit,
        })
    }

    fn prepare(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        semantic_types: &[SemanticTypeIdV1],
        projections: &[HelperCallArgumentV1],
        parameter_types: Vec<Type>,
        operations: &mut Vec<Operation>,
    ) -> Result<PreparedExecutionDefinedCallV1, ProductionSemanticKirErrorV1> {
        self.prepare_route_v1(
            lowering,
            block,
            call,
            callee,
            semantic_types,
            projections,
            parameter_types,
            operations,
            false,
        )
        .map(|(prepared, _)| prepared)
    }

    fn observe_return(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
        block: SemanticBlockIdV1,
        values: &[ValueId],
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        let cursor = lowering
            .execution
            .as_ref()
            .ok_or_else(execution_call_error_v29)?;
        let budget = lowering
            .emission_work
            .as_deref_mut()
            .ok_or(ArgumentResourceV1::Accounting)?;
        cursor.check_ledger(budget)?;
        budget.charge_work(6)?;
        if self.scope.ledger != budget.work_ledger_identity_v1() || self.source != cursor.source {
            return Err(execution_call_error_v29());
        }
        let Some(index) = cursor.instance.index().checked_sub(1) else {
            return Ok(());
        };
        let route = self
            .routes
            .get_mut(index)
            .ok_or_else(execution_call_error_v29)?;
        if route.child != cursor.instance
            || route.target.is_some()
            || route.callee != cursor.function_id
        {
            return Err(execution_call_error_v29());
        }
        if let Some(returned) = &mut route.returned {
            returned.observe_pending_v1(lowering, block, values, route.awaiting_completion)?;
        }
        Ok(())
    }
}

impl ExecutionDefinedCallSinkV29<'_> {
    #[allow(clippy::too_many_arguments)]
    fn prepare_route_v1(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_, '_>,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        semantic_types: &[SemanticTypeIdV1],
        projections: &[HelperCallArgumentV1],
        parameter_types: Vec<Type>,
        operations: &mut Vec<Operation>,
        deferred: bool,
    ) -> Result<
        (PreparedExecutionDefinedCallV1, ProductionCallInstanceIdV1),
        ProductionSemanticKirErrorV1,
    > {
        let cursor = lowering
            .execution
            .as_ref()
            .ok_or_else(execution_call_error_v29)?;
        let budget = lowering
            .emission_work
            .as_deref_mut()
            .ok_or(ArgumentResourceV1::Accounting)?;
        cursor.check_ledger(budget)?;
        budget.charge_work(argument_sum_v1(&[self.routes.len(), 20])?)?;
        if self.scope.ledger != budget.work_ledger_identity_v1()
            || self.source != cursor.source
            || self.pending.len() >= self.routes.len()
        {
            return Err(execution_call_error_v29());
        }
        let occurrence = ProductionCallOccurrenceV1 {
            caller: cursor.instance,
            block,
        };
        let index = self
            .routes
            .iter()
            .position(|route| route.occurrence == occurrence)
            .ok_or_else(execution_call_error_v29)?;
        let route = &self.routes[index];
        let control = cursor.source_call_control_v29(block, call, budget)?;
        if route.callee != callee
            || route.target.is_none()
            || route.control != control
            || control == ProductionCallControlV1::Unreachable
            || (!deferred && control != ProductionCallControlV1::MayReturn)
        {
            return Err(execution_call_error_v29());
        }
        let prepared = lowering.prepare_defined_call_arguments_v1(
            block,
            call,
            callee,
            DefinedCallArgumentSignatureV1 {
                projection: DefinedCallProjectionV29::Execution(self.scope),
                semantic_types,
                projections,
                parameter_types,
            },
            operations,
        )?;
        let budget = lowering
            .emission_work
            .as_deref_mut()
            .ok_or(ArgumentResourceV1::Accounting)?;
        let cursor = lowering
            .execution
            .as_ref()
            .ok_or_else(execution_call_error_v29)?;
        let normal_return = control == ProductionCallControlV1::MayReturn;
        let mut return_inputs = (deferred && normal_return).then(ExecutionReturnInputsV1::empty);
        let returned = match cursor.identities.filter(|_| normal_return) {
            Some((identities, _)) => {
                if self.original_instances != std::ptr::from_ref(identities.index.instances).cast()
                {
                    return Err(execution_identity_error_v1());
                }
                identities.prepare_return_obligation_v1(
                    cursor,
                    self.routes[index].child,
                    &prepared,
                    return_inputs.as_mut(),
                    budget,
                )?
            }
            None => None,
        };
        if normal_return {
            let result_ty = call
                .destination()
                .ok_or_else(execution_call_error_v29)?
                .place()
                .ty();
            if (execution_identity_channels_v1(cursor.cfg.types, result_ty, budget)? != 0)
                != returned.is_some()
            {
                return Err(execution_identity_error_v1());
            }
        }
        budget.reserve_storage(std::mem::size_of::<Vec<Option<ExecutionCfgLeafV29>>>())?;
        let mut nominal = emission_vec_v1(
            if deferred {
                0
            } else {
                returned.as_ref().map_or(0, |row| row.expected.len())
            },
            budget,
        )?;
        if let Some(returned) = &returned
            && !deferred
        {
            budget.charge_work(returned.expected.len())?;
            nominal.extend(returned.expected.iter().cloned());
        }
        let mut arguments = emission_vec_v1(prepared.arguments.len(), budget)?;
        budget.charge_work(prepared.arguments.len())?;
        arguments.extend_from_slice(&prepared.arguments);
        let emitted = execution_copy_call_target_v29(
            self.routes[index]
                .target
                .as_ref()
                .ok_or_else(execution_call_error_v29)?,
            budget,
        )?;
        let mut fresh_returns = if deferred {
            emission_vec_v1(
                returned.as_ref().map_or(0, |row| row.expected.len()),
                budget,
            )?
        } else {
            Vec::new()
        };
        if deferred && let Some(returned) = &returned {
            budget.charge_work(returned.expected.len())?;
            fresh_returns.extend(returned.expected.iter().map(Option::is_none));
        }
        // Everything fallible precedes the one-shot route and queue mutation.
        let route = &mut self.routes[index];
        route.returned = returned;
        route.awaiting_completion = deferred;
        route.fresh_returns = fresh_returns;
        route.return_inputs = return_inputs;
        let target = route
            .target
            .take()
            .expect("validated unconsumed call route");
        self.pending.push(PendingExecutionDefinedCallV29 {
            child: route.child,
            kernel_ir_function: target,
            arguments: prepared,
        });
        Ok(((emitted, arguments, nominal), route.child))
    }
}

impl SemanticFunctionLoweringV1<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn prepare_execution_defined_call_v29(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        semantic_types: &[SemanticTypeIdV1],
        projections: &[HelperCallArgumentV1],
        parameter_types: Vec<Type>,
        operations: &mut Vec<Operation>,
    ) -> Result<PreparedExecutionDefinedCallV1, ProductionSemanticKirErrorV1> {
        let consumer = self
            .execution_calls
            .take()
            .ok_or_else(execution_call_error_v29)?;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            consumer.prepare(
                self,
                block,
                call,
                callee,
                semantic_types,
                projections,
                parameter_types,
                operations,
            )
        }));
        self.execution_calls = Some(consumer);
        match result {
            Ok(result) => result,
            Err(payload) => std::panic::resume_unwind(payload),
        }
    }
}
