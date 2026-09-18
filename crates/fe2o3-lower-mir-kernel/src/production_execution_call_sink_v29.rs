// Private occurrence transport. Scope issuance, replay and lifecycle admission
// remain the materializer's separate obligations.
trait ExecutionDefinedCallConsumerV29 {
    #[allow(clippy::too_many_arguments)]
    fn prepare(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        semantic_types: &[SemanticTypeIdV1],
        projections: &[HelperCallArgumentV1],
        parameter_types: Vec<Type>,
        operations: &mut Vec<Operation>,
    ) -> Result<(FunctionId, Vec<ValueId>), ProductionSemanticKirErrorV1>;
}

struct ExecutionDefinedCallRouteV29 {
    occurrence: ProductionCallOccurrenceV1,
    callee: SemanticFunctionIdV1,
    child: ProductionCallInstanceIdV1,
    target: Option<FunctionId>,
}

struct PendingExecutionDefinedCallV29<'scope> {
    child: ProductionCallInstanceIdV1,
    kernel_ir_function: FunctionId,
    arguments: PreparedDefinedCallArgumentsV1<'scope>,
}

struct ExecutionDefinedCallSinkV29<'scope> {
    scope: ExecutionCallScopeV29<'scope>,
    source: ExecutionCallSourceV29,
    routes: Vec<ExecutionDefinedCallRouteV29>,
    pending: Vec<PendingExecutionDefinedCallV29<'scope>>,
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
            routes.push(ExecutionDefinedCallRouteV29 {
                occurrence: incoming.occurrence(),
                callee: row.function(),
                child,
                target: Some(execution_call_target_v29(source, index, budget)?),
            });
        }
        Ok(Self {
            scope,
            source,
            routes,
            pending,
        })
    }

    fn pop_pending(&mut self) -> Option<PendingExecutionDefinedCallV29<'scope>> {
        self.pending.pop()
    }

    fn finish(
        &mut self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.scope.ledger != budget.work_ledger_identity_v1() {
            return Err(execution_call_error_v29());
        }
        budget.charge_work(self.routes.len())?;
        if !self.pending.is_empty() || self.routes.iter().any(|row| row.target.is_some()) {
            return Err(execution_call_error_v29());
        }
        // Names and prepared payloads moved into emitted calls and child plans
        // remain paid. Only these now-empty container backings are reclaimed.
        let bytes = argument_sum_v1(&[
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
    fn prepare(
        &mut self,
        lowering: &mut SemanticFunctionLoweringV1<'_>,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        semantic_types: &[SemanticTypeIdV1],
        projections: &[HelperCallArgumentV1],
        parameter_types: Vec<Type>,
        operations: &mut Vec<Operation>,
    ) -> Result<(FunctionId, Vec<ValueId>), ProductionSemanticKirErrorV1> {
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
        if route.callee != callee || route.target.is_none() {
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
        // Everything fallible precedes the one-shot route and queue mutation.
        let route = &mut self.routes[index];
        let target = route
            .target
            .take()
            .expect("validated unconsumed call route");
        self.pending.push(PendingExecutionDefinedCallV29 {
            child: route.child,
            kernel_ir_function: target,
            arguments: prepared,
        });
        Ok((emitted, arguments))
    }
}

impl SemanticFunctionLoweringV1<'_> {
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
    ) -> Result<(FunctionId, Vec<ValueId>), ProductionSemanticKirErrorV1> {
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
