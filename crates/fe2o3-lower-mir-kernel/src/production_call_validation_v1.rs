// These checks join live emission anchors to the original owner and KIR. Full
// lowering replay remains responsible for source operand/address provenance.
struct CallFunctionIndexV1<'a> {
    values: Vec<(ValueId, &'a Type)>,
    blocks: Vec<&'a BasicBlock>,
}

impl<'a> CallFunctionIndexV1<'a> {
    fn new(
        target: &'a Function,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<Self, ProductionSemanticKirErrorV1> {
        let body = target
            .body
            .as_ref()
            .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
        if body.parameters.len() != target.signature.parameters.len() {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        budget.charge_work(body.blocks.len())?;
        let mut count = body.parameters.len();
        for block in &body.blocks {
            budget.charge_work(block.operations.len())?;
            count = argument_sum_v1(&[count, block.parameters.len()])?;
            for operation in &block.operations {
                count = argument_sum_v1(&[count, operation.results.len()])?;
            }
        }
        budget.charge_work(argument_product_v1(
            argument_sum_v1(&[count, body.blocks.len()])?,
            100,
        )?)?;
        let mut rows = emission_vec_v1(count, budget)?;
        rows.extend(
            body.parameters
                .iter()
                .copied()
                .zip(&target.signature.parameters),
        );
        budget.charge_work(body.blocks.len())?;
        for block in &body.blocks {
            budget.charge_work(block.operations.len())?;
            rows.extend(block.parameters.iter().map(|value| (value.id, &value.ty)));
            for operation in &block.operations {
                rows.extend(operation.results.iter().map(|value| (value.id, &value.ty)));
            }
        }
        sort_correspondence_keys_v1(&mut rows, 31, &|row| u64::from(row.0.0));
        if rows.windows(2).any(|pair| pair[0].0 == pair[1].0) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        let mut blocks = emission_vec_v1(body.blocks.len(), budget)?;
        blocks.extend(&body.blocks);
        sort_correspondence_keys_v1(&mut blocks, 31, &|block| u64::from(block.id.0));
        if blocks.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(Self {
            values: rows,
            blocks,
        })
    }

    fn ty(
        &self,
        value: ValueId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'a Type, ProductionSemanticKirErrorV1> {
        budget.charge_work(40)?;
        self.values
            .binary_search_by_key(&value, |row| row.0)
            .map(|index| self.values[index].1)
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }

    fn block(
        &self,
        id: BlockId,
        budget: &mut ArgumentBudgetV1<'_>,
    ) -> Result<&'a BasicBlock, ProductionSemanticKirErrorV1> {
        budget.charge_work(40)?;
        self.blocks
            .binary_search_by_key(&id, |block| block.id)
            .map(|index| self.blocks[index])
            .map_err(|_| ProductionSemanticKirErrorV1::CorrespondenceMismatch)
    }
}

// KIR's recursive types have one child at each node. Iteration keeps arbitrary
// pointer/slice nesting off the native stack and charges before inspection.
fn call_types_equal_v1(
    mut left: &Type,
    mut right: &Type,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<bool, ArgumentResourceV1> {
    loop {
        budget.charge_work(1)?;
        match (left, right) {
            (Type::Pointer(a), Type::Pointer(b))
                if a.address_space == b.address_space && a.access == b.access =>
            {
                (left, right) = (&a.pointee, &b.pointee);
            }
            (Type::Slice(a), Type::Slice(b))
                if a.address_space == b.address_space && a.access == b.access =>
            {
                (left, right) = (&a.element, &b.element);
            }
            (Type::Unit, Type::Unit) => return Ok(true),
            (Type::Scalar(a), Type::Scalar(b)) => return Ok(a == b),
            (Type::Vector(a), Type::Vector(b)) => return Ok(a == b),
            (Type::StorageObject(a), Type::StorageObject(b)) => return Ok(a == b),
            _ => return Ok(false),
        }
    }
}

#[cfg(test)]
mod call_storage_type_equality_tests_v29 {
    use super::*;

    fn object(schema: u32) -> Type {
        Type::StorageObject(fe2o3_kernel_ir::StorageLayoutIdV1(schema))
    }

    fn pointer(schema: u32, space: AddressSpace, access: AccessMode) -> Type {
        Type::pointer(object(schema), space, access)
    }

    fn compare(left: &Type, right: &Type, limit: usize) -> Result<bool, ArgumentResourceV1> {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(limit);
        let mut budget = ArgumentBudgetV1::new(&mut work, 37);
        budget.reserve_storage(37).unwrap();
        let result = call_types_equal_v1(left, right, &mut budget);
        assert_eq!(budget.storage(), 37);
        assert_eq!(budget.peak_storage(), 37);
        result
    }

    #[test]
    fn call_storage_types_require_exact_schema_pointer_access_and_address_space() {
        let expected = pointer(7, AddressSpace::Private, AccessMode::ReadWrite);
        assert!(compare(&expected, &expected, 2).unwrap());
        for changed in [
            pointer(8, AddressSpace::Private, AccessMode::ReadWrite),
            pointer(7, AddressSpace::Private, AccessMode::ReadOnly),
            pointer(7, AddressSpace::Generic, AccessMode::ReadWrite),
            Type::pointer(Type::Scalar(ScalarType::U32), AddressSpace::Private, AccessMode::ReadWrite),
            Type::slice(object(7), AddressSpace::Private, AccessMode::ReadWrite),
            object(7),
        ] {
            assert!(!compare(&expected, &changed, 2).unwrap(), "{changed:?}");
            assert!(!compare(&changed, &expected, 2).unwrap(), "{changed:?}");
        }
        // Structural equality neither validates a module-local layout ID nor
        // authorizes a first-class object value or an execution capability.
        assert!(compare(&object(u32::MAX), &object(u32::MAX), 1).unwrap());
        assert!(!compare(&object(7), &object(8), 1).unwrap());
        assert!(!object(7).is_storable());
        let execution = Type::Execution(fe2o3_kernel_ir::ExecutionRoleV15::Context);
        assert!(!compare(&execution, &execution, 1).unwrap());
        let execution_pointer = Type::pointer(execution, AddressSpace::Private, AccessMode::ReadWrite);
        assert!(!compare(&execution_pointer, &execution_pointer, 2).unwrap());
    }

    #[test]
    fn call_storage_type_equality_has_exact_iterative_work_without_scratch() {
        for depth in [0, 1, 2, 64, 257] {
            let mut ty = object(19);
            for level in 0..depth {
                ty = if level % 2 == 0 {
                    Type::pointer(ty, AddressSpace::Private, AccessMode::ReadWrite)
                } else {
                    Type::slice(ty, AddressSpace::Generic, AccessMode::ReadOnly)
                };
            }
            let required = depth + 1;
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(required);
            let mut budget = ArgumentBudgetV1::new(&mut work, 37);
            budget.reserve_storage(37).unwrap();
            assert!(call_types_equal_v1(&ty, &ty, &mut budget).unwrap());
            assert_eq!(budget.work(), required);
            assert_eq!(budget.storage(), 37);
            assert_eq!(budget.peak_storage(), 37);
            assert!(matches!(compare(&ty, &ty, required - 1), Err(ArgumentResourceV1::Work(_))));
        }
    }

    #[test]
    fn scoped_object_pointer_returns_preserve_exact_value_and_disallow_invented_casts() {
        let ty = pointer(7, AddressSpace::Private, AccessMode::ReadWrite);
        for fault in 0..8 {
            let input = ValueId(3);
            let other = ValueId(4);
            let mut block = BasicBlock::new(BlockId(0));
            block.terminator = Some(Terminator::Return { values: vec![input] });
            let mut components = vec![CallResultComponentV1::Return { input, conversion: None }];
            let mut result_type = ty.clone();
            match fault {
                0 => {}
                1 => result_type = pointer(8, AddressSpace::Private, AccessMode::ReadWrite),
                2 => result_type = pointer(7, AddressSpace::Generic, AccessMode::ReadWrite),
                3 => result_type = pointer(7, AddressSpace::Private, AccessMode::ReadOnly),
                4 => block.terminator = Some(Terminator::Return { values: vec![other] }),
                5 => components.clear(),
                6 => block.terminator = Some(Terminator::Return { values: vec![input, other] }),
                7 => {
                    block.terminator = Some(Terminator::Return { values: vec![ValueId(5)] });
                    block.operations.push(Operation::new(
                        vec![fe2o3_kernel_ir::ValueDef::new(ValueId(5), ty.clone())],
                        OperationKind::Cast { kind: CastKind::Bitcast, value: input, to: ty.clone() },
                    ));
                    components[0] = CallResultComponentV1::Return { input, conversion: Some(0) };
                }
                _ => unreachable!(),
            }
            let function = Function::internal_helper("typed-return-component",
                fe2o3_kernel_ir::Signature::new(vec![ty.clone(), ty.clone()], vec![result_type]),
                vec![input, other], vec![block]);
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(100_000);
            let mut budget = ArgumentBudgetV1::new(&mut work, 100_000);
            budget.reserve_storage(37).unwrap();
            let mut completed = false;
            with_canonical_call_scratch_v1(&mut budget, |budget| {
                let values = CallFunctionIndexV1::new(&function, budget)?;
                let block = &function.body.as_ref().unwrap().blocks[0];
                let result = check_scoped_call_return_v29(block, 0, &block.operations, &components,
                    &function.signature.results, &values, budget);
                if fault == 0 {
                    result.unwrap();
                } else {
                    assert!(matches!(result, Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch)),
                        "fault {fault}: {result:?}");
                }
                completed = true;
                Ok(())
            }).unwrap();
            assert!(completed, "fault {fault}");
            assert_eq!(budget.storage(), 37);
        }
    }
}

fn call_components_v1(
    components: &[CallResultComponentV1],
    span: CallComponentSpanV1,
) -> Result<&[CallResultComponentV1], ProductionSemanticKirErrorV1> {
    components
        .get(span.range()?)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)
}

fn validate_call_component_pool_v1(
    sites: &[SemanticKirCallReturnV1],
    components: &[CallResultComponentV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = (|| {
        budget.charge_work(argument_product_v1(
            argument_sum_v1(&[sites.len(), components.len()])?,
            4,
        )?)?;
        budget.reserve_storage(argument_product_v1(
            components.len(),
            std::mem::size_of::<bool>(),
        )?)?;
        let mut seen = argument_vec_v1(components.len())?;
        seen.resize(components.len(), false);
        for site in sites {
            let span = site.components();
            let records = call_components_v1(components, span)?;
            for (index, component) in span.range()?.zip(records) {
                if std::mem::replace(&mut seen[index], true)
                    || !matches!(
                        (site.kind, component),
                        (
                            SemanticKirCallReturnKindV1::Return { .. },
                            CallResultComponentV1::Return { .. }
                        ) | (
                            SemanticKirCallReturnKindV1::Call { .. },
                            CallResultComponentV1::Transport { .. }
                        )
                    )
                {
                    return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
                }
            }
        }
        if seen.iter().any(|present| !present) {
            return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
        }
        Ok(())
    })();
    budget.release_storage(budget.storage() - floor)?;
    result
}

#[allow(clippy::too_many_arguments)]
fn validate_call_correspondence_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    instance: &SemanticKirFunctionCorrespondenceV1,
    target: &Function,
    targets: &CallTargetIndexV1<'_>,
    rows: &[SemanticKirCallReturnV1],
    components: &[CallResultComponentV1],
    spans: &[SemanticKirTerminatorOperationSpanV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let floor = budget.storage();
    let result = check_call_correspondence_v1(
        owner, instance, target, targets, rows, components, spans, budget,
    );
    budget.release_storage(budget.storage() - floor)?;
    result
}

#[allow(clippy::too_many_arguments)]
fn check_call_correspondence_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    instance: &SemanticKirFunctionCorrespondenceV1,
    target: &Function,
    targets: &CallTargetIndexV1<'_>,
    rows: &[SemanticKirCallReturnV1],
    components: &[CallResultComponentV1],
    spans: &[SemanticKirTerminatorOperationSpanV1],
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let semantic = owner.source_semantic();
    budget.charge_work(argument_sum_v1(&[rows.len(), spans.len()])?)?;
    if rows.iter().any(|row| {
        row.correspondence_owner != instance.correspondence_owner
            || row.semantic_function != instance.semantic_function
    }) || rows
        .windows(2)
        .any(|pair| pair[0].semantic_block >= pair[1].semantic_block)
    {
        return Err(mismatch());
    }
    let source = semantic
        .functions()
        .get(instance.semantic_function.index() as usize)
        .ok_or_else(mismatch)?;
    let body = target.body.as_ref().ok_or_else(mismatch)?;
    check_call_signature_v1(
        semantic,
        source,
        instance.semantic_function,
        instance.role,
        target,
        budget,
    )?;
    let values = CallFunctionIndexV1::new(target, budget)?;
    let mut used = 0_usize;
    for (span, block) in spans.iter().zip(&body.blocks) {
        let source_block = source
            .blocks()
            .get(span.semantic_block.index() as usize)
            .ok_or_else(mismatch)?;
        budget.charge_work(40)?;
        let row = rows
            .binary_search_by_key(&span.semantic_block, |row| row.semantic_block)
            .ok()
            .map(|index| &rows[index]);
        let first = span.first_operation_ordinal as usize;
        let end = argument_sum_v1(&[first, span.operation_count as usize])?;
        let operations = block.operations.get(first..end).ok_or_else(mismatch)?;
        budget.charge_work(block.operations.len())?;
        for (ordinal, operation) in block.operations.iter().enumerate() {
            if let OperationKind::Call { callee, .. } = &operation.kind
                && targets.physical(callee, budget)?.role
                    == fe2o3_kernel_ir::FunctionRole::InternalHelper
                && !matches!(row.map(|row| row.kind), Some(SemanticKirCallReturnKindV1::Call { call_operation, .. }) if call_operation as usize == ordinal)
            {
                return Err(mismatch());
            }
        }
        match (source_block.terminator().kind(), row.map(|row| row.kind)) {
            (
                SemanticTerminatorKindV1::Return,
                Some(SemanticKirCallReturnKindV1::Return { components: span }),
            ) => {
                check_call_return_v1(
                    block,
                    first,
                    operations,
                    call_components_v1(components, span)?,
                    &target.signature.results,
                    &values,
                    budget,
                )?;
                used += 1;
            }
            (SemanticTerminatorKindV1::Call(call), kind) => {
                if let Some(SemanticCallableDeclV1::Defined { function }) =
                    semantic.callables().get(call.callee().index() as usize)
                {
                    let Some(SemanticKirCallReturnKindV1::Call {
                        arguments_first,
                        call_operation,
                        destination_end,
                        destination,
                        transport,
                    }) = kind
                    else {
                        return Err(mismatch());
                    };
                    check_defined_call_v1(
                        semantic,
                        instance,
                        targets,
                        block,
                        call,
                        *function,
                        first,
                        end,
                        arguments_first as usize,
                        call_operation as usize,
                        destination_end as usize,
                        destination,
                        &values,
                        budget,
                    )?;
                    check_call_transport_v1(
                        owner,
                        instance,
                        block,
                        call,
                        call_operation as usize,
                        destination_end as usize,
                        end,
                        destination,
                        call_components_v1(components, transport)?,
                        &values,
                        budget,
                    )?;
                    used += 1;
                } else if kind.is_some() {
                    return Err(mismatch());
                }
            }
            (_, None) => {}
            _ => return Err(mismatch()),
        }
    }
    if used != rows.len() {
        return Err(mismatch());
    }
    Ok(())
}

fn check_call_signature_v1(
    semantic: &AdmittedInertSemanticMirV1,
    source: &SemanticFunctionDeclV1,
    function: SemanticFunctionIdV1,
    role: SemanticKirFunctionRoleV1,
    target: &Function,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    budget.charge_work(source.locals().len())?;
    match role {
        SemanticKirFunctionRoleV1::KernelEntry
            if target.role == fe2o3_kernel_ir::FunctionRole::KernelEntry
                && target.signature.results.is_empty() =>
        {
            Ok(())
        }
        SemanticKirFunctionRoleV1::KernelEntry => Err(mismatch()),
        SemanticKirFunctionRoleV1::InternalHelper => {
            if target.role != fe2o3_kernel_ir::FunctionRole::InternalHelper {
                return Err(mismatch());
            }
            with_canonical_call_scratch_v1(budget, |budget| {
                prepay_typed_shape_v1(
                    semantic.types(),
                    source.abi().source_output_type(),
                    0,
                    budget,
                )?;
                let shape = helper_result_components_v1(semantic.types(), source, function)?;
                if shape.components.len() != target.signature.results.len() {
                    return Err(mismatch());
                }
                for ((_, _, expected, _, _), actual) in
                    shape.components.iter().zip(&target.signature.results)
                {
                    if !call_types_equal_v1(expected, actual, budget)? {
                        return Err(mismatch());
                    }
                }
                Ok(())
            })
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn check_call_transport_v1(
    owner: &ProductionSemanticSsaOwnerV1,
    instance: &SemanticKirFunctionCorrespondenceV1,
    block: &BasicBlock,
    call: &SemanticDirectCallV1,
    operation: usize,
    first: usize,
    end: usize,
    destination_kind: SemanticKirCallDestinationV1,
    transport: &[CallResultComponentV1],
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let plan = owner
        .plan_for_function(instance.semantic_function)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    check_resolved_call_transport_v1(
        plan,
        SemanticBlockIdV1::from_index(block.id.0),
        block,
        call,
        operation,
        first,
        end,
        destination_kind,
        transport,
        values,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn check_resolved_call_transport_v1(
    plan: &ProductionSemanticSsaFunctionPlanV1,
    semantic_block: SemanticBlockIdV1,
    block: &BasicBlock,
    call: &SemanticDirectCallV1,
    operation: usize,
    first: usize,
    end: usize,
    destination_kind: SemanticKirCallDestinationV1,
    transport: &[CallResultComponentV1],
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_resolved_call_transport_policy_v29(plan, semantic_block, block, call, operation,
        first, end, destination_kind, transport, values, false, budget)
}

#[allow(clippy::too_many_arguments)]
fn check_scoped_call_transport_v29(
    plan: &ProductionSemanticSsaFunctionPlanV1,
    semantic_block: SemanticBlockIdV1,
    block: &BasicBlock,
    call: &SemanticDirectCallV1,
    operation: usize,
    first: usize,
    end: usize,
    destination_kind: SemanticKirCallDestinationV1,
    transport: &[CallResultComponentV1],
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_resolved_call_transport_policy_v29(plan, semantic_block, block, call, operation,
        first, end, destination_kind, transport, values, true, budget)
}

fn scoped_call_conversion_v29(kind: CastKind, from: &Type, to: &Type, descriptors: bool) -> bool {
    match kind {
        CastKind::Bitcast => index_and_u64_are_transport_equivalent(from, to),
        CastKind::SliceToGeneric => descriptors && source_descriptor_widening_v29(from, to),
        _ => false,
    }
}

#[allow(clippy::too_many_arguments)]
fn check_resolved_call_transport_policy_v29(
    plan: &ProductionSemanticSsaFunctionPlanV1,
    semantic_block: SemanticBlockIdV1,
    block: &BasicBlock,
    call: &SemanticDirectCallV1,
    operation: usize,
    first: usize,
    end: usize,
    destination_kind: SemanticKirCallDestinationV1,
    transport: &[CallResultComponentV1],
    values: &CallFunctionIndexV1<'_>,
    descriptors: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    use fe2o3_mir_model::{SsaBlockIdV1, SsaEdgeIdV1};
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    if !(operation < first && first <= end && end <= block.operations.len()) {
        return Err(mismatch());
    }
    budget.charge_work(end - first)?;
    for cast in &block.operations[first..end] {
        let OperationKind::Cast {
            kind,
            value,
            to,
        } = &cast.kind
        else {
            return Err(mismatch());
        };
        let [converted] = cast.results.as_slice() else {
            return Err(mismatch());
        };
        let from = values.ty(*value, budget)?;
        if !scoped_call_conversion_v29(*kind, from, to, descriptors)
            || !call_types_equal_v1(&converted.ty, to, budget)?
        {
            return Err(mismatch());
        }
    }
    let plan = plan.plan();
    let edge = SsaEdgeIdV1::new(SsaBlockIdV1::new(semantic_block.index()), 0);
    let definitions = plan.edge_definitions(edge).ok_or_else(mismatch)?;
    let arguments = plan.edge_arguments(edge).ok_or_else(mismatch)?;
    budget.charge_work(argument_sum_v1(&[definitions.len(), arguments.len()])?)?;
    let destination = call.destination().ok_or_else(mismatch)?.place();
    let definition = if destination.projections().is_empty() {
        definitions
            .iter()
            .find(|row| row.variable().get() == destination.local().index())
    } else {
        None
    };
    let applicable = matches!(destination_kind, SemanticKirCallDestinationV1::Local)
        && definition.is_some_and(|definition| arguments.contains(definition))
        && !block.operations[operation].results.is_empty();
    let results = &block.operations[operation].results;
    if transport.len() != if applicable { results.len() } else { 0 } {
        return Err(mismatch());
    }
    budget.charge_work(transport.len())?;
    let mut previous_slot: Option<u32> = None;
    for (component, result) in transport.iter().zip(results) {
        let CallResultComponentV1::Transport { slot, conversion } = *component else {
            return Err(mismatch());
        };
        if previous_slot.is_some_and(|previous| previous.checked_add(1) != Some(slot)) {
            return Err(mismatch());
        }
        previous_slot = Some(slot);
        let Some(Terminator::Branch { arguments, .. }) = &block.terminator else {
            return Err(mismatch());
        };
        let output = *arguments.get(slot as usize).ok_or_else(mismatch)?;
        match conversion {
            None if output == result.id => {}
            Some(ordinal) if (first..end).contains(&(ordinal as usize)) => {
                // SSA transport also permits the inverse U64 -> INDEX bitcast.
                let cast = &block.operations[ordinal as usize];
                let OperationKind::Cast {
                    kind,
                    value,
                    to,
                } = &cast.kind
                else {
                    return Err(mismatch());
                };
                let [converted] = cast.results.as_slice() else {
                    return Err(mismatch());
                };
                if *value != result.id
                    || converted.id != output
                    || !scoped_call_conversion_v29(*kind, &result.ty, to, descriptors)
                    || !call_types_equal_v1(&converted.ty, to, budget)?
                    || !call_types_equal_v1(values.ty(output, budget)?, to, budget)?
                {
                    return Err(mismatch());
                }
            }
            _ => return Err(mismatch()),
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_call_return_v1(
    block: &BasicBlock,
    first: usize,
    operations: &[Operation],
    components: &[CallResultComponentV1],
    results: &[Type],
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_call_return_policy_v29(block, first, operations, components, results, values, false, budget)
}

fn check_scoped_call_return_v29(
    block: &BasicBlock,
    first: usize,
    operations: &[Operation],
    components: &[CallResultComponentV1],
    results: &[Type],
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_call_return_policy_v29(block, first, operations, components, results, values, true, budget)
}

#[allow(clippy::too_many_arguments)]
fn check_call_return_policy_v29(
    block: &BasicBlock,
    first: usize,
    operations: &[Operation],
    components: &[CallResultComponentV1],
    results: &[Type],
    values: &CallFunctionIndexV1<'_>,
    descriptors: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let Some(Terminator::Return { values: returned }) = &block.terminator else {
        return Err(mismatch());
    };
    if components.len() != results.len() || returned.len() != results.len() {
        return Err(mismatch());
    }
    budget.charge_work(components.len())?;
    let mut used = 0_usize;
    for ((component, expected), output) in components.iter().zip(results).zip(returned) {
        let CallResultComponentV1::Return { input, conversion } = *component else {
            return Err(mismatch());
        };
        match conversion {
            None if input == *output
                && call_types_equal_v1(values.ty(input, budget)?, expected, budget)? => {}
            Some(ordinal) if ordinal as usize == first + used => {
                let from = if descriptors { values.ty(input, budget)? } else { &Type::INDEX };
                check_call_cast_policy_v29(
                    operations.get(used).ok_or_else(mismatch)?,
                    input,
                    *output,
                    from,
                    expected,
                    values,
                    descriptors,
                    budget,
                )?;
                used += 1;
            }
            _ => return Err(mismatch()),
        }
    }
    if used != operations.len() {
        return Err(mismatch());
    }
    Ok(())
}

fn check_call_cast_v1(
    operation: &Operation,
    input: ValueId,
    output: ValueId,
    from: &Type,
    to: &Type,
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_call_cast_policy_v29(operation, input, output, from, to, values, false, budget)
}

#[allow(clippy::too_many_arguments)]
fn check_call_cast_policy_v29(
    operation: &Operation,
    input: ValueId,
    output: ValueId,
    from: &Type,
    to: &Type,
    values: &CallFunctionIndexV1<'_>,
    descriptors: bool,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let OperationKind::Cast {
        kind,
        value,
        to: cast_to,
    } = &operation.kind
    else {
        return Err(mismatch());
    };
    let [result] = operation.results.as_slice() else {
        return Err(mismatch());
    };
    if *value != input
        || result.id != output
        || !((*kind == CastKind::Bitcast && *from == Type::INDEX && *to == Type::Scalar(ScalarType::U64))
            || (*kind == CastKind::SliceToGeneric && descriptors && source_descriptor_widening_v29(from, to)))
        || !call_types_equal_v1(cast_to, to, budget)?
        || !call_types_equal_v1(&result.ty, to, budget)?
        || !call_types_equal_v1(values.ty(input, budget)?, from, budget)?
    {
        return Err(mismatch());
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_defined_call_v1(
    semantic: &AdmittedInertSemanticMirV1,
    instance: &SemanticKirFunctionCorrespondenceV1,
    targets: &CallTargetIndexV1<'_>,
    block: &BasicBlock,
    call: &SemanticDirectCallV1,
    callee: SemanticFunctionIdV1,
    first: usize,
    end: usize,
    arguments_first: usize,
    call_operation: usize,
    destination_end: usize,
    destination: SemanticKirCallDestinationV1,
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    let (callee_instance, target) =
        targets.source(instance.correspondence_owner, callee, budget)?;
    if callee_instance.role != SemanticKirFunctionRoleV1::InternalHelper {
        return Err(mismatch());
    }
    let source = semantic
        .functions()
        .get(callee.index() as usize)
        .ok_or_else(mismatch)?;
    let successor = BlockId(
        call.destination()
            .ok_or_else(mismatch)?
            .edge()
            .target()
            .index(),
    );
    check_resolved_defined_call_v1(
        semantic,
        source,
        &callee_instance.kernel_ir_function,
        target,
        successor,
        block,
        call,
        first,
        end,
        arguments_first,
        call_operation,
        destination_end,
        destination,
        values,
        budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn check_resolved_defined_call_v1(
    semantic: &AdmittedInertSemanticMirV1,
    source: &SemanticFunctionDeclV1,
    expected_callee: &FunctionId,
    target: &Function,
    successor: BlockId,
    block: &BasicBlock,
    call: &SemanticDirectCallV1,
    first: usize,
    end: usize,
    arguments_first: usize,
    call_operation: usize,
    destination_end: usize,
    destination: SemanticKirCallDestinationV1,
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    check_resolved_defined_call_with_scoped_recipe_v29(
        semantic, source, expected_callee, target, successor, block, call,
        first, end, arguments_first, call_operation, destination_end,
        destination, None, values, budget,
    )
}

#[allow(clippy::too_many_arguments)]
fn check_resolved_defined_call_with_scoped_recipe_v29(
    semantic: &AdmittedInertSemanticMirV1,
    source: &SemanticFunctionDeclV1,
    expected_callee: &FunctionId,
    target: &Function,
    successor: BlockId,
    block: &BasicBlock,
    call: &SemanticDirectCallV1,
    first: usize,
    end: usize,
    arguments_first: usize,
    call_operation: usize,
    destination_end: usize,
    destination: SemanticKirCallDestinationV1,
    typed_recipe: Option<ScopedTypedCallResultRecipeV29<'_>>,
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    if !(first <= arguments_first
        && arguments_first <= call_operation
        && call_operation < destination_end
        && destination_end <= end
        && end <= block.operations.len()
        && matches!(call.unwind(), SemanticUnwindActionV1::Unreachable))
        || target.role != fe2o3_kernel_ir::FunctionRole::InternalHelper
    {
        return Err(mismatch());
    }
    check_resolved_call_prefix_v1(
        source, expected_callee, target, block, call, first, end, arguments_first,
        call_operation, values, budget,
    )?;
    let operation = &block.operations[call_operation];
    let source_destination = call.destination().ok_or_else(mismatch)?;
    if source_destination.place().ty() != source.abi().source_output_type() {
        return Err(mismatch());
    }
    if !matches!(destination, SemanticKirCallDestinationV1::Local)
        && !matches!(
            semantic.types()[source.abi().source_output_type().index() as usize].shape(),
            SemanticTypeShapeV1::Scalar(_) | SemanticTypeShapeV1::ValidityScalar(_)
        )
    {
        return Err(mismatch());
    }
    let finishing = &block.operations[call_operation + 1..destination_end];
    if !matches!(destination, SemanticKirCallDestinationV1::Projected { .. })
        && arguments_first != first
    {
        return Err(mismatch());
    }
    budget.charge_work(end - first)?;
    for (ordinal, operation) in block.operations[first..end].iter().enumerate() {
        if matches!(operation.kind, OperationKind::Call { .. }) && first + ordinal != call_operation
        {
            return Err(mismatch());
        }
    }
    match destination {
        SemanticKirCallDestinationV1::Local => {
            if typed_recipe.is_some()
                || !source_destination.place().projections().is_empty() || !finishing.is_empty() {
                return Err(mismatch());
            }
        }
        SemanticKirCallDestinationV1::Retained { pointer, access }
        | SemanticKirCallDestinationV1::Projected { pointer, access } => {
            if matches!(destination, SemanticKirCallDestinationV1::Retained { .. })
                != source_destination.place().projections().is_empty()
            {
                return Err(mismatch());
            }
            let ([result], [store]) = (operation.results.as_slice(), finishing) else {
                return Err(mismatch());
            };
            let Type::Pointer(pointer_type) = values.ty(pointer, budget)? else {
                return Err(mismatch());
            };
            let expected_access = if let Some(recipe) = typed_recipe { recipe.access } else { memory_access_for_type(
                semantic.types(),
                source_destination.place().ty(),
                pointer_type.address_space,
            )? };
            let checked_value = if let Some(recipe) = typed_recipe {
                if !std::ptr::eq(recipe.call, call)
                    || !std::ptr::eq(recipe.operation, store)
                    || *pointer_type.pointee != Type::StorageObject(recipe.schema)
                    || store.kind != OperationKind::Storage(ScopedObjectOperationV29::WriteValue {
                        address: pointer, value: result.id, access,
                    })
                { return Err(mismatch()); }
                call_types_equal_v1(&lower_scalar_type(semantic.types(), source_destination.place().ty())?,
                    &result.ty, budget)?
            } else {
                store.kind == (OperationKind::Store { pointer, value: result.id, access })
                    && call_types_equal_v1(&pointer_type.pointee, &result.ty, budget)?
            };
            if !store.results.is_empty()
                || access != expected_access
                || (matches!(destination, SemanticKirCallDestinationV1::Retained { .. })
                    && access.address_space != AddressSpace::Private)
                || pointer_type.address_space != access.address_space
                || pointer_type.access != AccessMode::ReadWrite
                || !checked_value
            {
                return Err(mismatch());
            }
        }
    }
    let Some(Terminator::Branch { target, arguments }) = &block.terminator else {
        return Err(mismatch());
    };
    if *target != successor {
        return Err(mismatch());
    }
    let successor = values.block(*target, budget)?;
    budget.charge_work(arguments.len())?;
    if arguments.len() != successor.parameters.len() {
        return Err(mismatch());
    }
    for (argument, expected) in arguments.iter().zip(&successor.parameters) {
        if !call_types_equal_v1(values.ty(*argument, budget)?, &expected.ty, budget)? {
            return Err(mismatch());
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_resolved_call_prefix_v1(
    source: &SemanticFunctionDeclV1,
    expected_callee: &FunctionId,
    target: &Function,
    block: &BasicBlock,
    call: &SemanticDirectCallV1,
    first: usize,
    end: usize,
    arguments_first: usize,
    call_operation: usize,
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    if !(first <= arguments_first && arguments_first <= call_operation
        && call_operation < end && end <= block.operations.len())
        || !matches!(call.unwind(), SemanticUnwindActionV1::Unreachable)
        || target.role != fe2o3_kernel_ir::FunctionRole::InternalHelper
    { return Err(mismatch()); }
    let operation = &block.operations[call_operation];
    let OperationKind::Call { callee: actual_callee, arguments } = &operation.kind else {
        return Err(mismatch());
    };
    budget.charge_work(argument_sum_v1(&[
        actual_callee.as_str().len(), expected_callee.as_str().len(), arguments.len(), call.arguments().len(),
    ])?)?;
    if actual_callee != expected_callee || actual_callee != &target.id
        || arguments.len() != target.signature.parameters.len()
        || operation.results.len() != target.signature.results.len()
        || call.arguments().len() != source.abi().source_input_types().len()
    { return Err(mismatch()); }
    for (argument, expected) in call.arguments().iter().zip(source.abi().source_input_types()) {
        if semantic_operand_type(argument) != *expected { return Err(mismatch()); }
    }
    for (argument, expected) in arguments.iter().zip(&target.signature.parameters) {
        if !call_types_equal_v1(values.ty(*argument, budget)?, expected, budget)? { return Err(mismatch()); }
    }
    for (result, expected) in operation.results.iter().zip(&target.signature.results) {
        if !call_types_equal_v1(&result.ty, expected, budget)? { return Err(mismatch()); }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn check_resolved_no_normal_call_v1(
    instances: &ExecutionInstancesV29<'_>,
    original: &production_call_instances_v1::ProductionInstanceCallV1<'_>,
    target: &Function,
    block: &BasicBlock,
    first: usize,
    end: usize,
    arguments_first: usize,
    call_operation: usize,
    destination: Option<SemanticKirCallDestinationV1>,
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
    budget.charge_work(6)?;
    let child = original.child().ok_or_else(mismatch)?;
    let callee = instances.instance(child).ok_or_else(mismatch)?;
    if instances.call_control(original.occurrence()) != Some(ProductionCallControlV1::NoNormalReturn)
        || !instances.incoming(child).is_some_and(|incoming| std::ptr::eq(incoming, original))
        || instances.instance_reachable(child) != Some(true)
        || (original.source().destination().is_some() && instances.instance_may_return(child) != Some(false))
        || !matches!(original.callable(), SemanticCallableDeclV1::Defined { function } if *function == callee.function())
        || !matches!(block.terminator, Some(Terminator::Unreachable))
        || call_operation.checked_add(1) != Some(end)
    { return Err(mismatch()); }
    check_resolved_call_prefix_v1(callee.declaration(), &target.id, target, block,
        original.source(), first, end, arguments_first, call_operation, values, budget)?;
    match (original.source().destination(), destination) {
        (None, None) if first == arguments_first => {}
        (Some(source), Some(SemanticKirCallDestinationV1::Local))
            if source.place().projections().is_empty() && first == arguments_first => {}
        (Some(source), Some(SemanticKirCallDestinationV1::Retained { pointer, access }))
        | (Some(source), Some(SemanticKirCallDestinationV1::Projected { pointer, access })) => {
            let retained = matches!(destination, Some(SemanticKirCallDestinationV1::Retained { .. }));
            if retained != source.place().projections().is_empty()
                || (retained && first != arguments_first)
            { return Err(mismatch()); }
            let Type::Pointer(ty) = values.ty(pointer, budget)? else { return Err(mismatch()); };
            let expected = memory_access_for_type(instances.owner().source_semantic().types(),
                source.place().ty(), ty.address_space)?;
            if expected != access || ty.access != AccessMode::ReadWrite
                || (retained && ty.address_space != AddressSpace::Private)
            { return Err(mismatch()); }
        }
        _ => return Err(mismatch()),
    }
    budget.charge_work(end - first)?;
    for (ordinal, operation) in block.operations[first..end].iter().enumerate() {
        if matches!(operation.kind, OperationKind::Call { .. }) && ordinal + first != call_operation {
            return Err(mismatch());
        }
    }
    Ok(())
}
