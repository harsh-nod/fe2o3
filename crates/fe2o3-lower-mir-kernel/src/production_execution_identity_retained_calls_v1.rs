// Independent source effects only. No solved identity class or emitted value is
// consulted here; a safe return effect is not proof of the returned producer.
struct ExecutionRetainedCallEffectsV1<'a, 'source> {
    instances: &'a ExecutionInstancesV29<'source>,
    closed: Vec<bool>,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
    floor: usize,
}

impl<'a, 'source> ExecutionRetainedCallEffectsV1<'a, 'source> {
    fn derive(
        index: &ExecutionIdentitySourceIndexV1<'a, 'source>,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let instances = index.instances;
        budget.reserve_storage(argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
        ])?)?;
        let mut closed = emission_vec_v1(instances.instances().len(), budget)?;
        budget.charge_work(instances.instances().len())?;
        closed.resize(instances.instances().len(), false);
        for ordinal in (0..instances.instances().len()).rev() {
            budget.charge_work(3)?;
            let instance = instances
                .id_at(ordinal)
                .ok_or_else(execution_identity_error_v1)?;
            if !instances
                .instance_reachable(instance)
                .ok_or_else(execution_identity_error_v1)?
            {
                continue;
            }
            let row = instances
                .instance(instance)
                .ok_or_else(execution_identity_error_v1)?;
            closed[ordinal] = execution_identity_retained_body_v1(
                index,
                &closed,
                instance,
                row.declaration(),
                row.ssa(),
                budget,
            )?;
        }
        Ok(Self {
            instances,
            closed,
            ledger: budget.work_ledger_identity_v1(),
            slot: budget as *const ArgumentBudgetV1<'_> as usize,
            floor: budget.storage(),
        })
    }

    fn accepts(
        &self,
        instances: &ExecutionInstancesV29<'_>,
        instance: ProductionCallInstanceIdV1,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<bool, ProductionSemanticKirErrorV1> {
        budget.charge_work(7)?;
        if !std::ptr::eq(self.instances, instances)
            || self.ledger != budget.work_ledger_identity_v1()
            || self.slot != budget as *const ArgumentBudgetV1<'_> as usize
            || budget.storage() < self.floor
            || self.closed.len() != instances.instances().len()
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        if !instances
            .instance_reachable(instance)
            .ok_or_else(execution_identity_error_v1)?
        {
            return Ok(false);
        }
        self.closed
            .get(instance.index())
            .copied()
            .ok_or_else(execution_identity_error_v1)
    }

    fn discard(
        self,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        if self.ledger != budget.work_ledger_identity_v1()
            || self.slot != budget as *const ArgumentBudgetV1<'_> as usize
            || budget.storage() < self.floor
        {
            return Err(ArgumentResourceV1::Accounting.into());
        }
        let bytes = argument_sum_v1(&[
            std::mem::size_of::<Self>(),
            std::mem::size_of::<Result<Self, ProductionSemanticKirErrorV1>>(),
            argument_product_v1(self.closed.capacity(), std::mem::size_of::<bool>())?,
        ])?;
        drop(self);
        budget.release_storage(bytes)?;
        Ok(())
    }
}

fn execution_identity_retained_call_v1(
    index: &ExecutionIdentitySourceIndexV1<'_, '_>,
    closed: &[bool],
    instance: ProductionCallInstanceIdV1,
    block: SsaBlockIdV1,
    source: &fe2o3_mir_model::semantic_mir_v1::SemanticDirectCallV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let instances = index.instances;
    charge_execution_cfg_lookup_v29(index.calls.len(), budget)?;
    let ordinal = *index
        .calls
        .get(&(instance.index(), block.get()))
        .ok_or_else(execution_identity_error_v1)?;
    budget.charge_work(10)?;
    let exact = instances
        .calls(instance)
        .and_then(|calls| calls.get(ordinal))
        .ok_or_else(execution_identity_error_v1)?;
    let occurrence = ProductionCallOccurrenceV1 {
        caller: instance,
        block: SemanticBlockIdV1::from_index(block.get()),
    };
    if exact.occurrence() != occurrence
        || !std::ptr::eq(exact.source(), source)
        || !instances
            .owner()
            .source_semantic()
            .callables()
            .get(source.callee().index() as usize)
            .is_some_and(|callable| std::ptr::eq(callable, exact.callable()))
        || matches!(
            instances.call_control(occurrence),
            None | Some(ProductionCallControlV1::Unreachable)
        )
    {
        return Err(execution_identity_error_v1());
    }
    if !matches!(source.unwind(), SemanticUnwindActionV1::Unreachable)
        || source
            .destination()
            .is_some_and(|destination| !destination.place().projections().is_empty())
        || !source.variadic_argument_abis().is_empty()
        || source.inline_assembly_source_v30().is_some()
        || source.ordered_region_source_v31().is_some()
        || source.ordered_program_source_v32().is_some()
    {
        return Ok(false);
    }
    let types = instances.owner().source_semantic().types();
    for operand in source.arguments() {
        budget.charge_work(1)?;
        if execution_identity_channels_v1(types, operand.ty(), budget)? != 0 {
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
                return Ok(false);
            };
            if !place.projections().is_empty() {
                return Ok(false);
            }
        }
    }
    match exact.callable() {
        fe2o3_mir_model::semantic_mir_v1::SemanticCallableDeclV1::Defined { function } => {
            let child = exact.child().ok_or_else(execution_identity_error_v1)?;
            budget.charge_work(6)?;
            let row = instances
                .instance(child)
                .ok_or_else(execution_identity_error_v1)?;
            if child.index() <= instance.index()
                || row.function() != *function
                || instances.instance_reachable(child) != Some(true)
                || !instances
                    .incoming(child)
                    .is_some_and(|incoming| std::ptr::eq(incoming, exact))
            {
                return Err(execution_identity_error_v1());
            }
            for (ordinal, local) in row.declaration().locals().iter().enumerate() {
                budget.charge_work(1)?;
                if !local.role().is_entry_argument() {
                    continue;
                }
                let selector = instances.parameter_source(
                    child,
                    SemanticLocalIdV1::from_index(
                        u32::try_from(ordinal).map_err(|_| ArgumentResourceV1::Arithmetic)?,
                    ),
                    budget,
                )?;
                if selector.ty != local.ty()
                    || !source
                        .arguments()
                        .get(selector.source_argument as usize)
                        .is_some_and(|operand| std::ptr::eq(operand, selector.operand))
                {
                    return Err(execution_identity_error_v1());
                }
            }
            closed
                .get(child.index())
                .copied()
                .ok_or_else(execution_identity_error_v1)
        }
        fe2o3_mir_model::semantic_mir_v1::SemanticCallableDeclV1::CompilerIntrinsic {
            operation:
                fe2o3_mir_model::semantic_mir_v1::SemanticCompilerIntrinsicOperationV1::Execution(
                    operation,
                ),
            binding,
            ..
        } => {
            if exact.child().is_some() {
                return Err(execution_identity_error_v1());
            }
            budget.charge_work(argument_sum_v1(&[source.arguments().len(), 3])?)?;
            let abi = binding.abi();
            if abi.source_input_types().len() != source.arguments().len()
                || !abi
                    .source_input_types()
                    .iter()
                    .zip(source.arguments())
                    .all(|(ty, operand)| *ty == operand.ty())
                || source
                    .destination()
                    .is_some_and(|destination| destination.place().ty() != abi.source_output_type())
            {
                return Err(execution_identity_error_v1());
            }
            // These admitted descriptors manipulate nominal values without
            // observing/replacing the backing of a shared borrowed carrier.
            // Their identity/lifetime/consumption obligations remain later gates.
            use fe2o3_mir_model::semantic_mir_v1::SemanticExecutionOperationV29 as Op;
            Ok(match operation {
                Op::ContextIssue { .. }
                | Op::WorkgroupDerive { .. }
                | Op::MaskedTileLoadU32 { .. }
                | Op::MaskedTileIntoFragmentU32 { .. }
                | Op::LaneFragmentIntoPartsU32 { .. } => true,
            })
        }
        _ => Ok(false),
    }
}

fn execution_identity_effect_whole_operand_v1(
    function: &SemanticFunctionDeclV1,
    operand: &SemanticOperandV1,
    expected: SemanticTypeIdV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(3)?;
    let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
        return Ok(false);
    };
    Ok(place.projections().is_empty()
        && place.ty() == expected
        && function
            .locals()
            .get(place.local().index() as usize)
            .is_some_and(|local| local.ty() == expected))
}

fn execution_identity_effect_aggregate_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    result: SemanticTypeIdV1,
    aggregate: &fe2o3_mir_model::semantic_mir_v1::SemanticAggregateRvalueV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticAggregateKindV1 as Kind;
    budget.charge_work(3)?;
    let shape = types
        .get(result.index() as usize)
        .ok_or_else(execution_identity_error_v1)?
        .shape();
    let fields = match (aggregate.kind(), shape) {
        (Kind::Array, SemanticTypeShapeV1::Array { element, length }) => {
            if u64::try_from(aggregate.operands().len()).ok() != Some(*length) {
                return Ok(false);
            }
            for operand in aggregate.operands() {
                if !execution_identity_effect_whole_operand_v1(function, operand, *element, budget)?
                {
                    return Ok(false);
                }
            }
            return Ok(true);
        }
        (Kind::Tuple, SemanticTypeShapeV1::Tuple(fields))
        | (Kind::Aggregate, SemanticTypeShapeV1::Aggregate(fields)) => fields.fields(),
        (Kind::EnumVariant(variant), SemanticTypeShapeV1::Enum { variants, .. }) => {
            let Some(variant) = variants
                .get(*variant as usize)
                .filter(|variant| !variant.is_uninhabited())
            else {
                return Ok(false);
            };
            variant.fields().fields()
        }
        _ => return Ok(false),
    };
    if fields.len() != aggregate.operands().len() {
        return Ok(false);
    }
    for (operand, &expected) in aggregate.operands().iter().zip(fields) {
        if !execution_identity_effect_whole_operand_v1(function, operand, expected, budget)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn execution_identity_retained_body_v1(
    index: &ExecutionIdentitySourceIndexV1<'_, '_>,
    closed: &[bool],
    instance: ProductionCallInstanceIdV1,
    function: &SemanticFunctionDeclV1,
    ssa: &ProductionSemanticSsaFunctionPlanV1,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    let instances = index.instances;
    let types = instances.owner().source_semantic().types();
    for &block in ssa.plan().reverse_postorder() {
        budget.charge_work(2)?;
        if !execution_identity_block_active_v1(instances, instance, block, budget)? {
            continue;
        }
        let source_block = block;
        let block = function
            .blocks()
            .get(block.get() as usize)
            .ok_or_else(execution_identity_error_v1)?;
        for statement in block.statements() {
            budget.charge_work(2)?;
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    if !assignment.destination().projections().is_empty() {
                        return Ok(false);
                    }
                    let nominal = execution_identity_channels_v1(
                        types,
                        assignment.value().result_type(),
                        budget,
                    )? != 0;
                    match assignment.value().kind() {
                        SemanticRvalueKindV1::Borrow { kind, place } => {
                            if *kind != SemanticBorrowKindV1::Shared {
                                return Ok(false);
                            }
                            if !place.projections().is_empty() {
                                budget.charge_work(4)?;
                                let holder = function
                                    .locals()
                                    .get(place.local().index() as usize)
                                    .ok_or_else(execution_identity_error_v1)?;
                                if !nominal
                                    || place.projections().len() != 1
                                    || place.projections()[0].kind()
                                        != SemanticProjectionKindV1::Dereference
                                    || !matches!(types[holder.ty().index() as usize].shape(), SemanticTypeShapeV1::Pointer(pointer)
                                        if pointer.kind() == fe2o3_mir_model::semantic_mir_v1::SemanticPointerKindV1::Reference
                                        && pointer.mutability() == SemanticMutabilityV1::Immutable
                                        && pointer.pointee() == place.ty())
                                {
                                    return Ok(false);
                                }
                            }
                            if nominal
                                && (!execution_identity_shared_type_v1(
                                    types,
                                    assignment.value().result_type(),
                                    budget,
                                )? || execution_cfg_nominal_kind_v29(types, place.ty())?
                                    != Some(false))
                            {
                                return Ok(false);
                            }
                        }
                        SemanticRvalueKindV1::Use(operand) if nominal => {
                            if !execution_identity_effect_whole_operand_v1(
                                function,
                                operand,
                                assignment.value().result_type(),
                                budget,
                            )? {
                                return Ok(false);
                            }
                        }
                        SemanticRvalueKindV1::Aggregate(aggregate) if nominal => {
                            if !execution_identity_effect_aggregate_v1(
                                types,
                                function,
                                assignment.value().result_type(),
                                aggregate,
                                budget,
                            )? {
                                return Ok(false);
                            }
                        }
                        SemanticRvalueKindV1::AddressOf { .. } | SemanticRvalueKindV1::Load(_) => {
                            return Ok(false);
                        }
                        value => {
                            if nominal {
                                return Ok(false);
                            }
                            let mut ordinary = true;
                            value.try_visit_operands(|operand| {
                                budget.charge_work(1)?;
                                if let SemanticOperandV1::Copy(place)
                                | SemanticOperandV1::Move(place) = operand
                                {
                                    ordinary &=
                                        execution_identity_channels_v1(types, place.ty(), budget)?
                                            == 0;
                                }
                                Ok::<_, ProductionSemanticKirErrorV1>(())
                            })?;
                            if let SemanticRvalueKindV1::Length(place)
                            | SemanticRvalueKindV1::Discriminant(place) = value
                            {
                                ordinary &=
                                    execution_identity_channels_v1(types, place.ty(), budget)? == 0;
                            }
                            if !ordinary {
                                return Ok(false);
                            }
                        }
                    }
                }
                SemanticStatementKindV1::Deinitialize(place) => {
                    if !place.projections().is_empty() {
                        return Ok(false);
                    }
                }
                SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Assume(_)
                | SemanticStatementKindV1::Nop => {}
                SemanticStatementKindV1::Store(_)
                | SemanticStatementKindV1::AtomicRmw(_)
                | SemanticStatementKindV1::AtomicCompareExchange(_)
                | SemanticStatementKindV1::SetDiscriminant { .. } => return Ok(false),
            }
        }
        budget.charge_work(1)?;
        match block.terminator().kind() {
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::SwitchInt { .. }
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Unreachable
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Return => {}
            SemanticTerminatorKindV1::Call(call) => {
                if !execution_identity_retained_call_v1(
                    index,
                    closed,
                    instance,
                    source_block,
                    call,
                    budget,
                )? {
                    return Ok(false);
                }
            }
            SemanticTerminatorKindV1::TailCall(_)
            | SemanticTerminatorKindV1::Drop { .. }
            | SemanticTerminatorKindV1::Assert { .. }
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate => return Ok(false),
        }
    }
    Ok(true)
}
