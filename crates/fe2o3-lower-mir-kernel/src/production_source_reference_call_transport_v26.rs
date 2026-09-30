#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct ReferenceCallTransportV26 {
    occurrence: ProductionCallOccurrenceV1,
    source_call: usize,
    input: ValueId,
    output: ValueId,
}

// Representation only: callers must hold the original-source signature and
// emission additionally rejoins the original call, projection and actual value.
fn source_reference_call_widening_v26(
    types: &[SemanticTypeDeclV1],
    source: SemanticTypeIdV1,
    actual: &Type,
    expected: &Type,
    budget: &mut dyn SemanticEmissionBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(64)?;
    let headers = std::mem::size_of::<(
        &[SemanticTypeDeclV1],
        [&Type; 4],
        Option<&SemanticTypeDeclV1>,
        [usize; 16],
        Result<bool, ProductionSemanticKirErrorV1>,
    )>();
    budget.reserve_storage(headers)?;
    let result = (|| {
        let Some(declaration) = types.get(source.index() as usize) else {
            return Ok(false);
        };
        let SemanticTypeShapeV1::Pointer(pointer) = declaration.shape() else {
            return Ok(false);
        };
        let (Type::Pointer(actual), Type::Pointer(expected)) = (actual, expected) else {
            return Ok(false);
        };
        let access = match pointer.mutability() {
            SemanticMutabilityV1::Immutable => AccessMode::ReadOnly,
            SemanticMutabilityV1::Mutable => AccessMode::ReadWrite,
        };
        if pointer.kind() != SemanticPointerKindV1::Reference
            || pointer.metadata() != SemanticPointerMetadataV1::None
            || pointer.pointer_width_bits() != 64
            || pointer.address_space() != 0
            || declaration.layout().size_bytes() != Some(8)
            || declaration.layout().alignment_bytes() != 8
            || expected.address_space != AddressSpace::Generic
            || !matches!(
                actual.address_space,
                AddressSpace::Global
                    | AddressSpace::Workgroup
                    | AddressSpace::Constant
                    | AddressSpace::Private
            )
            || actual.access != access
            || expected.access != access
            || (actual.address_space == AddressSpace::Constant && access != AccessMode::ReadOnly)
        {
            return Ok(false);
        }
        invocation_equal_types_v1(&actual.pointee, &expected.pointee, budget)
    })();
    let released = budget.release_storage(headers);
    result.and_then(|value| released.map(|()| value))
}

impl SemanticFunctionLoweringV1<'_, '_> {
    #[allow(clippy::too_many_arguments)]
    fn check_reference_call_argument_v26(
        &mut self,
        block: SemanticBlockIdV1,
        call: &SemanticDirectCallV1,
        origin: &PreparedExecutionCallOriginV29<'_>,
        parameter: usize,
        projection: &HelperCallArgumentV1,
        binding: &SemanticValueBindingV1,
        value: ValueId,
        actual: &Type,
        expected: &Type,
    ) -> Result<(), ProductionSemanticKirErrorV1> {
        self.with_emission_budget_v1(|this, budget| {
            let headers = std::mem::size_of::<(
                [usize; 64],
                Result<(), ProductionSemanticKirErrorV1>,
            )>();
            budget.reserve_storage(headers)?;
            let result = (|| {
            let cursor = this.execution.as_ref().ok_or_else(execution_call_error_v29)?;
            cursor.check_ledger(budget)?;
            let references = cursor.references.ok_or_else(execution_call_error_v29)?;
            references.check(budget)?;
            budget.charge_work(64)?;
            let source = cursor.function.blocks().get(block.index() as usize)
                .ok_or_else(execution_call_error_v29)?;
            let operand = call.arguments().get(projection.source_argument as usize)
                .ok_or_else(execution_call_error_v29)?;
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
                return Err(execution_call_error_v29());
            };
            let original_expected = origin.parameter_types.get(parameter)
                .ok_or_else(execution_call_error_v29)?;
            let SemanticValueBindingV1::Value { id, ty } = binding else {
                return Err(execution_call_error_v29());
            };
            if !std::ptr::eq(cursor.function, this.function)
                || !matches!(source.terminator().kind(), SemanticTerminatorKindV1::Call(original) if std::ptr::eq(original, call))
                || origin.ledger != budget.work_ledger_identity_v1()
                || origin.scope.ledger != origin.ledger
                || origin.source != cursor.source
                || origin.function != cursor.function_id
                || origin.occurrence.caller != cursor.instance
                || origin.occurrence.block != block
                || !invocation_equal_types_v1(original_expected, expected, budget)?
                || projection.tuple_field.is_some()
                || !matches!(projection.component, None | Some(0))
                || origin.projections.get(parameter).is_none_or(|original|
                    original.source_argument != projection.source_argument
                        || original.tuple_field != projection.tuple_field
                        || original.component != projection.component)
                || !place.projections().is_empty()
                || cursor.function.locals().get(place.local().index() as usize)
                    .is_none_or(|local| local.ty() != place.ty())
                || *id != value
                || !invocation_equal_types_v1(ty, actual, budget)?
            {
                return Err(execution_call_error_v29());
            }
            let instances = references.plan.instances;
            let calls = instances.calls(cursor.instance).ok_or_else(execution_call_error_v29)?;
            budget.charge_work(calls.len())?;
            let mut matching = calls.iter().filter(|row| row.occurrence() == origin.occurrence);
            let incoming = matching.next().ok_or_else(execution_call_error_v29)?;
            let child = incoming.child().ok_or_else(execution_call_error_v29)?;
            let callee = instances.instance(child).ok_or_else(execution_call_error_v29)?;
            if matching.next().is_some()
                || !std::ptr::eq(incoming.source(), call)
                || !instances.incoming(child).is_some_and(|row| std::ptr::eq(row, incoming))
                || callee.function() != origin.callee
                || callee.declaration().abi().source_input_types()
                    .get(projection.source_argument as usize) != Some(&place.ty())
                || !source_reference_call_widening_v26(this.types, place.ty(), actual, expected, budget)?
            {
                return Err(execution_call_error_v29());
            }
            Ok(())
            })();
            let released = budget.release_storage(headers);
            result.and(released)
        })
    }
}

// The input relation is retained by the real callee parameter installation.
// Replay consumes its exact original input/output IDs, not equal pointer types.
// Candidate functions cannot edit this retained relation; a missing capture
// means the installation emitted no reference representation conversion.
#[allow(clippy::too_many_arguments)]
fn check_reference_call_replay_v26(
    types: &[SemanticTypeDeclV1],
    caller: &SemanticFunctionDeclV1,
    callee: &SemanticFunctionDeclV1,
    occurrence: ProductionCallOccurrenceV1,
    call: &SemanticDirectCallV1,
    target: &Function,
    inputs: &[InvocationInputRowV1],
    block: &BasicBlock,
    arguments_first: usize,
    call_operation: usize,
    values: &CallFunctionIndexV1<'_>,
    budget: &mut ArgumentBudgetV1<'_>,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(inputs.len())?;
    if !inputs
        .iter()
        .any(|row| row.reference_call_transport.is_some())
    {
        return Ok(());
    }
    let headers = std::mem::size_of::<([usize; 64], Result<(), ProductionSemanticKirErrorV1>)>();
    budget.reserve_storage(headers)?;
    let result = (|| {
        let mismatch = || ProductionSemanticKirErrorV1::CorrespondenceMismatch;
        budget.charge_work(32)?;
        let operations = block
            .operations
            .get(arguments_first..call_operation)
            .ok_or_else(mismatch)?;
        let operation = block.operations.get(call_operation).ok_or_else(mismatch)?;
        let OperationKind::Call {
            callee: id,
            arguments,
        } = &operation.kind
        else {
            return Err(mismatch());
        };
        budget.charge_work(argument_sum_v1(&[
            operations.len(),
            inputs.len(),
            id.as_str().len(),
            target.id.as_str().len(),
        ])?)?;
        if id != &target.id || arguments.len() != target.signature.parameters.len()
            || !caller.blocks().get(occurrence.block.index() as usize)
                .is_some_and(|source| matches!(source.terminator().kind(), SemanticTerminatorKindV1::Call(original) if std::ptr::eq(original, call)))
        {
            return Err(mismatch());
        }
        let mut operations = operations.iter();
        let mut previous_parameter = None;
        for row in inputs {
            let Some(transport) = row.reference_call_transport else {
                continue;
            };
            budget.charge_work(24)?;
            let expected = target
                .signature
                .parameters
                .get(row.first_parameter)
                .ok_or_else(mismatch)?;
            let declaration = callee
                .locals()
                .get(row.local as usize)
                .ok_or_else(mismatch)?;
            let operand = call
                .arguments()
                .get(row.source_argument as usize)
                .ok_or_else(mismatch)?;
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
                return Err(mismatch());
            };
            if transport.occurrence != occurrence
                || transport.source_call != std::ptr::from_ref(call) as usize
                || row.tuple_field.is_some()
                || row.parameter_count != 1
                || previous_parameter.is_some_and(|previous| previous >= row.first_parameter)
                || declaration.role() != SemanticLocalRoleV1::Argument(row.source_argument)
                || declaration.ty() != row.ty
                || callee
                    .abi()
                    .source_input_types()
                    .get(row.source_argument as usize)
                    != Some(&row.ty)
                || place.ty() != row.ty
                || !place.projections().is_empty()
                || caller
                    .locals()
                    .get(place.local().index() as usize)
                    .is_none_or(|local| local.ty() != row.ty)
                || arguments.get(row.first_parameter) != Some(&transport.output)
                || transport.input == transport.output
            {
                return Err(mismatch());
            }
            let cast = operations
                .find(|operation| {
                    operation
                        .results
                        .first()
                        .is_some_and(|result| result.id == transport.output)
                })
                .ok_or_else(mismatch)?;
            let [result] = cast.results.as_slice() else {
                return Err(mismatch());
            };
            let OperationKind::Cast {
                kind: CastKind::PointerToGeneric,
                value,
                to,
            } = &cast.kind
            else {
                return Err(mismatch());
            };
            if *value != transport.input
                || !call_types_equal_v1(to, expected, budget)?
                || !call_types_equal_v1(&result.ty, expected, budget)?
                || !source_reference_call_widening_v26(
                    types,
                    row.ty,
                    values.ty(transport.input, budget)?,
                    expected,
                    budget,
                )?
            {
                return Err(mismatch());
            }
            previous_parameter = Some(row.first_parameter);
        }
        Ok(())
    })();
    let released = budget
        .release_storage(headers)
        .map_err(ProductionSemanticKirErrorV1::from);
    result.and(released)
}
