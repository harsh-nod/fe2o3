// Instance-bound call transport. This does not issue roles, discharge borrows,
// or replace source replay and lifecycle verification before production admission.
#[derive(Clone, Copy)]
struct ExecutionCallScopeV29<'scope> {
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    _lifetime: std::marker::PhantomData<&'scope ()>,
}

#[derive(Clone, Copy)]
enum DefinedCallProjectionV29<'scope> {
    Ordinary,
    Execution(ExecutionCallScopeV29<'scope>),
}

fn with_execution_call_scope_v29<R>(
    budget: &mut ArgumentBudgetV1<'_>,
    consume: impl for<'scope> FnOnce(
        ExecutionCallScopeV29<'scope>,
        &mut ArgumentBudgetV1<'_>,
    ) -> Result<R, ProductionSemanticKirErrorV1>,
) -> Result<R, ProductionSemanticKirErrorV1> {
    consume(
        ExecutionCallScopeV29 {
            ledger: budget.work_ledger_identity_v1(),
            _lifetime: std::marker::PhantomData,
        },
        budget,
    )
}

#[derive(Clone, Copy, Eq, PartialEq)]
struct ExecutionCallSourceV29 {
    semantic: [u8; 32],
    ssa: fe2o3_pliron::ProductionSemanticSsaIdentityV1,
    root: SemanticFunctionIdV1,
}

impl ExecutionCallSourceV29 {
    fn from_instances(
        instances: &ExecutionInstancesV29<'_>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        budget.charge_work(20)?;
        Ok(Self {
            semantic: *instances.owner().source_semantic_sha256(),
            ssa: instances.owner().identity(),
            root: instances
                .instance(instances.root())
                .ok_or_else(execution_call_error_v29)?
                .function(),
        })
    }
}

struct PreparedExecutionCallOriginV29<'scope> {
    scope: ExecutionCallScopeV29<'scope>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    source: ExecutionCallSourceV29,
    function: SemanticFunctionIdV1,
    occurrence: ProductionCallOccurrenceV1,
    callee: SemanticFunctionIdV1,
    projections: Vec<HelperCallArgumentV1>,
    parameter_types: Vec<Type>,
}

struct PreparedExecutionParametersV29<'scope> {
    _scope: ExecutionCallScopeV29<'scope>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    instance: ProductionCallInstanceIdV1,
    source: ExecutionCallSourceV29,
    function: SemanticFunctionIdV1,
    values: Vec<ValueDef>,
    locals: Vec<(usize, SemanticValueBindingV1)>,
    nominal_floor: u32,
}

#[track_caller]
fn execution_call_error_v29() -> ProductionSemanticKirErrorV1 {
    #[cfg(test)]
    eprintln!("execution call error at {}", std::panic::Location::caller());
    unsupported(
        0,
        None,
        None,
        "execution call parameters differ from their source instance",
    )
}

fn execution_call_shape_v29(
    types: &[SemanticTypeDeclV1],
    ty: SemanticTypeIdV1,
    binding: &SemanticValueBindingV1,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<Vec<Option<ExecutionCfgLeafV29>>, ProductionSemanticKirErrorV1> {
    let count = execution_cfg_nominal_count_v29(types, ty, budget)?;
    let mut leaves = emission_vec_v1(count, budget)?;
    budget.charge_work(count)?;
    leaves.resize_with(count, || None);
    let mut slots = leaves.iter_mut();
    merge_execution_cfg_binding_v29(types, ty, binding, binding, &mut slots, &mut 0, budget)?;
    if slots.next().is_some() {
        return Err(execution_call_error_v29());
    }
    budget.charge_work(count)?;
    if leaves.iter().any(|leaf| {
        !matches!(
            leaf,
            Some(ExecutionCfgLeafV29::Owned(_) | ExecutionCfgLeafV29::Borrow(_))
        )
    }) {
        return Err(execution_call_error_v29());
    }
    let expected = execution_cfg_types_v29(types, ty, budget)?;
    let mut values = Vec::new();
    execution_cfg_values_v29(binding, &mut values, &mut 0, budget)?;
    budget.charge_work(values.len())?;
    if values.len() != expected.len()
        || values.iter().zip(expected).any(|(value, ty)| {
            value.ty != ty && !(value.ty == Type::INDEX && ty == Type::Scalar(ScalarType::U64))
        })
    {
        return Err(execution_call_error_v29());
    }
    Ok(leaves)
}

impl<'a> SemanticFunctionLoweringV1<'a> {
    fn prepare_execution_call_origin_v29<'scope>(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        callee: SemanticFunctionIdV1,
        signature: &DefinedCallArgumentSignatureV1<'_, 'scope>,
    ) -> Result<Option<PreparedExecutionCallOriginV29<'scope>>, ProductionSemanticKirErrorV1> {
        let scope = match signature.projection {
            DefinedCallProjectionV29::Ordinary => return Ok(None),
            DefinedCallProjectionV29::Execution(scope) => scope,
        };
        let cursor = self
            .execution
            .as_ref()
            .ok_or_else(execution_call_error_v29)?;
        let budget = self
            .emission_work
            .as_deref_mut()
            .ok_or(ArgumentResourceV1::Accounting)?;
        cursor.check_ledger(budget)?;
        if scope.ledger != budget.work_ledger_identity_v1() {
            return Err(execution_call_error_v29());
        }
        budget.charge_work(8)?;
        let actual = self
            .function
            .blocks()
            .get(block.index() as usize)
            .ok_or_else(execution_call_error_v29)?;
        if !matches!(actual.terminator().kind(), SemanticTerminatorKindV1::Call(found)
            if std::ptr::eq(found, call))
            || !matches!(self.callables.get(call.callee().index() as usize),
                Some(SemanticCallableDeclV1::Defined { function }) if *function == callee)
            || cursor.block != Some(SsaBlockIdV1::new(block.index()))
        {
            return Err(execution_call_error_v29());
        }
        let mut projections = emission_vec_v1(signature.projections.len(), budget)?;
        budget.charge_work(signature.projections.len())?;
        projections.extend_from_slice(signature.projections);
        let mut parameter_types = emission_vec_v1(signature.parameter_types.len(), budget)?;
        for ty in &signature.parameter_types {
            parameter_types.push(execution_cfg_clone_type_v29(ty, budget)?);
        }
        Ok(Some(PreparedExecutionCallOriginV29 {
            scope,
            ledger: budget.work_ledger_identity_v1(),
            source: cursor.source,
            function: cursor.function_id,
            occurrence: ProductionCallOccurrenceV1 {
                caller: cursor.instance,
                block,
            },
            callee,
            projections,
            parameter_types,
        }))
    }
}

// Ordinary scalar IDs are replaced by callee parameters; nominal identities are
// preserved. The returned object cannot be cloned or installed in another call.
fn prepare_execution_parameters_v29<'scope>(
    instances: &ExecutionInstancesV29<'_>,
    child: ProductionCallInstanceIdV1,
    prepared: PreparedDefinedCallArgumentsV1<'scope>,
    plan: &LoweredFunctionPlanV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(Vec<ValueId>, PreparedExecutionParametersV29<'scope>), ProductionSemanticKirErrorV1> {
    budget.charge_work(12)?;
    let origin = prepared.execution.ok_or_else(execution_call_error_v29)?;
    let incoming = instances
        .incoming(child)
        .ok_or_else(execution_call_error_v29)?;
    let row = instances
        .instance(child)
        .ok_or_else(execution_call_error_v29)?;
    let parent = instances
        .instance(origin.occurrence.caller)
        .ok_or_else(execution_call_error_v29)?;
    let types = instances.owner().source_semantic().types();
    if origin.ledger != budget.work_ledger_identity_v1()
        || origin.occurrence != incoming.occurrence()
        || incoming.child() != Some(child)
        || origin.source != ExecutionCallSourceV29::from_instances(instances, budget)?
        || origin.function != parent.function()
        || origin.callee != row.function()
        || plan.semantic_function != row.function()
        || !plan.parameter_local_bindings.is_empty()
        || prepared.source_bindings.len() != incoming.source().arguments().len()
        || prepared.arguments.len() != plan.parameter_values.len()
        || origin.projections.len() != plan.parameter_values.len()
        || plan.parameter_types.len() != plan.parameter_values.len()
        || origin.parameter_types.len() != plan.parameter_types.len()
    {
        return Err(execution_call_error_v29());
    }
    let mut locals = emission_vec_v1(row.declaration().locals().len(), budget)?;
    let mut values = emission_vec_v1(plan.parameter_values.len(), budget)?;
    let mut nominal_floor = 0u32;
    let mut next = 0usize;
    for (local, declaration) in row.declaration().locals().iter().enumerate() {
        budget.charge_work(1)?;
        if !declaration.role().is_entry_argument() {
            continue;
        }
        let selector = instances
            .parameter_source(child, SemanticLocalIdV1::from_index(local as u32), budget)
            .map_err(|error| match error {
                production_call_instances_v1::ProductionCallInstanceErrorV1::Resource(error) => {
                    error.into()
                }
                _ => execution_call_error_v29(),
            })?;
        let argument = selector.source_argument as usize;
        if !std::ptr::eq(selector.operand, &incoming.source().arguments()[argument]) {
            return Err(execution_call_error_v29());
        }
        let source = prepared
            .source_bindings
            .get(argument)
            .ok_or_else(execution_call_error_v29)?;
        let binding = match (source, selector.tuple_field) {
            (SemanticValueBindingV1::Aggregate(fields), Some(field)) => fields
                .get(field as usize)
                .ok_or_else(execution_call_error_v29)?,
            (_, None) => source,
            _ => return Err(execution_call_error_v29()),
        };
        let leaves = execution_call_shape_v29(types, selector.ty, binding, budget)?;
        for leaf in &leaves {
            budget.charge_work(4)?;
            let held = match leaf {
                Some(ExecutionCfgLeafV29::Owned(held)) => held,
                Some(ExecutionCfgLeafV29::Borrow(held)) => &held.borrowed,
                _ => return Err(execution_call_error_v29()),
            };
            for identity in [Some(held.identity), Some(held.context), held.workgroup]
                .into_iter()
                .flatten()
            {
                nominal_floor = nominal_floor.max(
                    identity
                        .value
                        .0
                        .checked_add(1)
                        .ok_or(ArgumentResourceV1::Arithmetic)?,
                );
            }
        }
        let physical = execution_cfg_types_v29(types, selector.ty, budget)?;
        let start = next;
        for (component, expected) in physical.into_iter().enumerate() {
            budget.charge_work(5)?;
            let projection = origin
                .projections
                .get(next)
                .ok_or_else(execution_call_error_v29)?;
            let id = *plan
                .parameter_values
                .get(next)
                .ok_or_else(execution_call_error_v29)?;
            if plan.parameter_types.get(next) != Some(&expected)
                || origin.parameter_types.get(next) != Some(&expected)
                || projection.source_argument != selector.source_argument
                || projection.tuple_field != selector.tuple_field
                || match projection.component {
                    Some(found) => found != component,
                    None => {
                        component != 0 || !matches!(binding, SemanticValueBindingV1::Value { .. })
                    }
                }
                || values
                    .last()
                    .is_some_and(|previous: &ValueDef| previous.id.0 >= id.0)
            {
                return Err(execution_call_error_v29());
            }
            values.push(ValueDef::new(id, expected));
            next = argument_sum_v1(&[next, 1])?;
        }
        let mut leaves = leaves.iter();
        let mut physical = values[start..next].iter();
        let rebuilt = rebuild_execution_cfg_binding_v29(
            types,
            selector.ty,
            true,
            &mut leaves,
            &mut physical,
            &mut 0,
            budget,
        )?;
        if leaves.next().is_some() || physical.next().is_some() {
            return Err(execution_call_error_v29());
        }
        locals.push((local, rebuilt));
    }
    budget.charge_work(values.len())?;
    if next != prepared.arguments.len() || values.iter().any(|value| value.id.0 < nominal_floor) {
        return Err(execution_call_error_v29());
    }
    Ok((
        prepared.arguments,
        PreparedExecutionParametersV29 {
            _scope: origin.scope,
            ledger: budget.work_ledger_identity_v1(),
            instance: child,
            source: origin.source,
            function: row.function(),
            values,
            locals,
            nominal_floor,
        },
    ))
}

impl<'a> ExecutionAvailabilityV29<'a> {
    fn with_call_parameters_v29<'scope>(
        self,
        parameters: PreparedExecutionParametersV29<'scope>,
    ) -> Result<ExecutionAvailabilityV29<'scope>, ProductionSemanticKirErrorV1>
    where
        'a: 'scope,
    {
        if self.parameters.is_some() {
            return Err(unsupported(
                self.function_id.index(),
                None,
                None,
                "execution call parameters are already attached",
            ));
        }
        let mut cursor: ExecutionAvailabilityV29<'scope> = self;
        cursor.parameters = Some(parameters);
        Ok(cursor)
    }

    fn install_call_parameters_v29<'work>(
        &mut self,
        parameters: &SemanticParameterBindingsV1<'_>,
        locals: &mut [Option<SemanticValueBindingV1>],
        budget: Option<&mut (dyn SemanticEmissionBudgetV1 + 'work)>,
    ) -> Result<u32, ProductionSemanticKirErrorV1> {
        let Some(prepared) = self.parameters.take() else {
            return Ok(0);
        };
        let budget = budget.ok_or(ArgumentResourceV1::Accounting)?;
        self.check_ledger(budget)?;
        budget.charge_work(argument_sum_v1(&[
            prepared.values.len(),
            prepared.locals.len(),
            5,
        ])?)?;
        if prepared.ledger != self.ledger
            || prepared.instance != self.instance
            || prepared.source != self.source
            || prepared.function != self.function_id
            || parameters.values.len() != prepared.values.len()
            || parameters.types.len() != prepared.values.len()
            || prepared
                .values
                .iter()
                .zip(parameters.values.iter().zip(parameters.types))
                .any(|(value, (id, ty))| value.id != *id || value.ty != *ty)
            || prepared
                .locals
                .iter()
                .any(|(local, _)| !matches!(locals.get(*local), Some(None)))
        {
            return Err(execution_call_error_v29());
        }
        // All fallible checks precede installation; failure drops the constructor.
        for (local, binding) in prepared.locals {
            locals[local] = Some(binding);
        }
        Ok(prepared.nominal_floor)
    }
}
