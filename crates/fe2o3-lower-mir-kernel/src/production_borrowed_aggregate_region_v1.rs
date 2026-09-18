// A producer-side exclusion check. Independent replay must also account for
// every affected source use and compose the ordinary checked suffix.
fn select_borrowed_aggregate_reference_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    selected: &mut BorrowedAggregatePreparationV1,
    local: u32,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[selected.locals.len(), 12])?)?;
    let declaration = function
        .locals()
        .get(local as usize)
        .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
    if declaration.role() == SemanticLocalRoleV1::Return
        || !borrowed_aggregate_reference_v1(types, declaration.ty())
        || !matches!(types[declaration.ty().index() as usize].shape(),
            SemanticTypeShapeV1::Pointer(pointer) if pointer.kind() == SemanticPointerKindV1::Reference)
    {
        return Err(borrowed_aggregate_error_v1(
            "selected borrowed aggregate requires a non-return reference local",
        ));
    }
    if !selected.locals.contains(&local) {
        budget.reserve_storage(4 * std::mem::size_of::<usize>())?;
        selected.locals.insert(local);
    }
    Ok(())
}

fn seed_borrowed_aggregate_locals_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    callables: &[SemanticCallableDeclV1],
    plan: &LoweredFunctionPlanV1,
    signatures: &BTreeMap<SemanticFunctionIdV1, LoweredFunctionSignatureV1>,
    selected: &mut BorrowedAggregatePreparationV1,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    for binding in &plan.parameter_local_bindings {
        budget.charge_work(2)?;
        if let PlannedParameterLocalBindingV1::BorrowedAggregate { local, shape, .. } = binding {
            let local = u32::try_from(*local).map_err(|_| ArgumentResourceV1::Arithmetic)?;
            let declaration = function
                .locals()
                .get(local as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            if declaration.ty() != shape.reference_type || !declaration.role().is_entry_argument() {
                return Err(ProductionSemanticKirErrorV1::CorrespondenceMismatch);
            }
            select_borrowed_aggregate_reference_v1(types, function, selected, local, budget)?;
        }
    }
    for block in function.blocks() {
        budget.charge_work(4)?;
        let SemanticTerminatorKindV1::Call(call) = block.terminator().kind() else {
            continue;
        };
        let Some(SemanticCallableDeclV1::Defined { function: callee }) =
            callables.get(call.callee().index() as usize)
        else {
            continue;
        };
        budget.charge_work(argument_sum_v1(&[signatures.len(), 2])?)?;
        let Some(signature) = signatures.get(callee) else {
            continue;
        };
        for parameter in &signature.call_arguments {
            budget.charge_work(6)?;
            if parameter.borrowed.is_none() {
                continue;
            }
            if parameter.tuple_field.is_some() {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate tuple actual is unsupported",
                ));
            }
            let operand = call
                .arguments()
                .get(parameter.source_argument as usize)
                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
            let (SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place)) = operand else {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate actual is not a reference local",
                ));
            };
            if !place.projections().is_empty() {
                return Err(borrowed_aggregate_error_v1(
                    "borrowed aggregate projected actual is unsupported",
                ));
            }
            select_borrowed_aggregate_reference_v1(
                types,
                function,
                selected,
                place.local().index(),
                budget,
            )?;
        }
    }
    Ok(())
}

fn expand_borrowed_aggregate_locals_v1(
    types: &[SemanticTypeDeclV1],
    function: &SemanticFunctionDeclV1,
    selected: &mut BorrowedAggregatePreparationV1,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    // A metered closure over original assignments, not a replacement SSA graph.
    // Follow both origins and aliases so a selected view cannot disappear from
    // the affected-region check by being copied to another reference local.
    loop {
        let previous = selected.locals.len();
        for block in function.blocks() {
            budget.charge_work(2)?;
            for statement in block.statements() {
                budget.charge_work(argument_sum_v1(&[
                    argument_product_v1(selected.locals.len(), 2)?,
                    12,
                ])?)?;
                let SemanticStatementKindV1::Assign(assignment) = statement.kind() else {
                    continue;
                };
                let destination = assignment.destination();
                let target = destination.local().index();
                let target_selected = selected.locals.contains(&target);
                let value = assignment.value();
                if !borrowed_aggregate_reference_v1(types, value.result_type()) {
                    continue;
                }
                let place = match value.kind() {
                    SemanticRvalueKindV1::Borrow { place, .. }
                    | SemanticRvalueKindV1::Use(
                        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place),
                    ) => place,
                    _ if target_selected => {
                        return Err(borrowed_aggregate_error_v1(
                            "selected borrowed aggregate reference definition is unsupported",
                        ));
                    }
                    _ => continue,
                };
                let source = place.local().index();
                if !target_selected && !selected.locals.contains(&source) {
                    continue;
                }
                if !destination.projections().is_empty() {
                    return Err(borrowed_aggregate_error_v1(
                        "borrowed aggregate reference cannot escape into a projected destination",
                    ));
                }
                select_borrowed_aggregate_reference_v1(types, function, selected, target, budget)?;
                match value.kind() {
                    SemanticRvalueKindV1::Borrow { .. } if place.projections().is_empty() => {
                        budget.charge_work(argument_sum_v1(&[
                            selected.owners.len(),
                            selected.locals.len(),
                            6,
                        ])?)?;
                        if !selected.owners.contains_key(&source) {
                            let shape =
                                borrowed_aggregate_shape_v1(types, value.result_type(), budget)?;
                            let declaration = function
                                .locals()
                                .get(source as usize)
                                .ok_or(ProductionSemanticKirErrorV1::CorrespondenceMismatch)?;
                            if shape.aggregate_type != place.ty()
                                || declaration.ty() != place.ty()
                                || declaration.role() == SemanticLocalRoleV1::Return
                            {
                                return Err(borrowed_aggregate_error_v1(
                                    "borrowed aggregate owner is not an exact local aggregate",
                                ));
                            }
                            if declaration.role().is_entry_argument() {
                                check_borrowed_entry_owner_v1(function, place.local(), &shape, budget)?;
                            }
                            budget.reserve_storage(
                                std::mem::size_of::<BorrowedAggregateShapeV1>()
                                    + 8 * std::mem::size_of::<usize>(),
                            )?;
                            selected.owners.insert(source, shape);
                            selected.locals.insert(source);
                        }
                    }
                    SemanticRvalueKindV1::Borrow { .. } if matches!(place.projections(), [projection] if projection.kind() == SemanticProjectionKindV1::Dereference) =>
                    {
                        select_borrowed_aggregate_reference_v1(
                            types, function, selected, source, budget,
                        )?;
                    }
                    SemanticRvalueKindV1::Use(_) if place.projections().is_empty() => {
                        select_borrowed_aggregate_reference_v1(
                            types, function, selected, source, budget,
                        )?;
                    }
                    _ => {
                        return Err(borrowed_aggregate_error_v1(
                            "borrowed aggregate subobject/descriptor aliases are unsupported",
                        ));
                    }
                }
            }
        }
        if selected.locals.len() == previous {
            return Ok(());
        }
    }
}

// This is only storage selection. The independent call/source replay still has
// to join the caller's value to these exact helper parameters.
fn check_borrowed_entry_owner_v1(
    function: &SemanticFunctionDeclV1,
    local: SemanticLocalIdV1,
    shape: &BorrowedAggregateShapeV1,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<(), ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_sum_v1(&[shape.leaves.len(), 8])?)?;
    if function.role() != SemanticFunctionRoleV1::InternalHelper
        || function.locals()[local.index() as usize].role() != SemanticLocalRoleV1::Argument(0)
        || function.abi().source_argument_ownership().first()
            != Some(&SemanticSourceArgumentOwnershipV1::ByValue)
        || shape.leaves.iter().any(|leaf| {
            !matches!(leaf.transport,
                BorrowedAggregateLeafTransportV1::ScalarSlot { .. }
                    | BorrowedAggregateLeafTransportV1::InvariantSlice { .. })
        })
    {
        return Err(borrowed_aggregate_error_v1(
            "borrowed entry owner requires a by-value helper argument with scalar or shared-slice fields",
        ));
    }
    for statement in function.blocks()[function.entry().index() as usize].statements() {
        budget.charge_work(4)?;
        match statement.kind() {
            SemanticStatementKindV1::Nop => continue,
            SemanticStatementKindV1::StorageLive(other)
            | SemanticStatementKindV1::StorageDead(other) if *other != local => continue,
            SemanticStatementKindV1::Assign(assignment)
                if assignment.destination().local() != local
                    && assignment.destination().projections().is_empty()
                    && matches!(assignment.value().kind(),
                        SemanticRvalueKindV1::Borrow { place, .. }
                            if place.local() == local && place.projections().is_empty()
                                && place.ty() == shape.aggregate_type) => return Ok(()),
            _ => break,
        }
    }
    Err(borrowed_aggregate_error_v1(
        "borrowed entry owner must first be borrowed in the helper entry block",
    ))
}

fn borrowed_aggregate_place_uses_v1(
    place: &SemanticPlaceV1,
    locals: &BTreeSet<u32>,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(argument_product_v1(
        argument_sum_v1(&[locals.len(), 2])?,
        argument_sum_v1(&[place.projections().len(), 1])?,
    )?)?;
    Ok(locals.contains(&place.local().index())
        || place.projections().iter().any(|projection| {
            matches!(projection.kind(),
                SemanticProjectionKindV1::Index(local) if locals.contains(&local.index())
            )
        }))
}

fn borrowed_aggregate_operand_uses_v1(
    operand: &SemanticOperandV1,
    locals: &BTreeSet<u32>,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(1)?;
    match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => {
            borrowed_aggregate_place_uses_v1(place, locals, budget)
        }
        _ => Ok(false),
    }
}

fn borrowed_aggregate_terminator_uses_v1(
    kind: &SemanticTerminatorKindV1,
    locals: &BTreeSet<u32>,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    budget.charge_work(2)?;
    let mut operand = |value| borrowed_aggregate_operand_uses_v1(value, locals, budget);
    Ok(match kind {
        SemanticTerminatorKindV1::Call(call) => {
            for value in call.arguments() {
                if operand(value)? {
                    return Ok(true);
                }
            }
            if let Some(destination) = call.destination() {
                borrowed_aggregate_place_uses_v1(destination.place(), locals, budget)?
            } else {
                false
            }
        }
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => operand(discriminant)?,
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            if operand(condition)? {
                return Ok(true);
            }
            match message {
                SemanticAssertMessageV1::BoundsCheck {
                    length: left,
                    index: right,
                }
                | SemanticAssertMessageV1::Overflow { left, right, .. }
                | SemanticAssertMessageV1::MisalignedPointerDereference {
                    required_alignment: left,
                    found_alignment: right,
                } => operand(left)? || operand(right)?,
                SemanticAssertMessageV1::DivisionByZero(value)
                | SemanticAssertMessageV1::RemainderByZero(value) => operand(value)?,
                _ => false,
            }
        }
        SemanticTerminatorKindV1::Drop { place, .. } => {
            borrowed_aggregate_place_uses_v1(place, locals, budget)?
        }
        SemanticTerminatorKindV1::TailCall(_) => true,
        _ => false,
    })
}

fn borrowed_aggregate_block_uses_v1(
    block: &fe2o3_mir_model::semantic_mir_v1::SemanticBasicBlockV1,
    locals: &BTreeSet<u32>,
    budget: &mut dyn BorrowedAggregateBudgetV1,
) -> Result<bool, ProductionSemanticKirErrorV1> {
    for statement in block.statements() {
        budget.charge_work(2)?;
        let used = match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                if borrowed_aggregate_place_uses_v1(assignment.destination(), locals, budget)? {
                    return Ok(true);
                }
                let value = assignment.value().kind();
                let mut used = false;
                value.try_visit_operands(
                    |operand| -> Result<(), ProductionSemanticKirErrorV1> {
                        used |= borrowed_aggregate_operand_uses_v1(operand, locals, budget)?;
                        Ok(())
                    },
                )?;
                used || match value {
                    SemanticRvalueKindV1::Borrow { place, .. }
                    | SemanticRvalueKindV1::AddressOf { place, .. }
                    | SemanticRvalueKindV1::Length(place)
                    | SemanticRvalueKindV1::Discriminant(place) => {
                        borrowed_aggregate_place_uses_v1(place, locals, budget)?
                    }
                    SemanticRvalueKindV1::Load(load) => {
                        borrowed_aggregate_place_uses_v1(load.source(), locals, budget)?
                    }
                    _ => false,
                }
            }
            SemanticStatementKindV1::Store(store) => {
                borrowed_aggregate_place_uses_v1(store.destination(), locals, budget)?
                    || borrowed_aggregate_operand_uses_v1(store.value(), locals, budget)?
            }
            SemanticStatementKindV1::Deinitialize(place) => {
                borrowed_aggregate_place_uses_v1(place, locals, budget)?
            }
            SemanticStatementKindV1::Assume(value) => {
                borrowed_aggregate_operand_uses_v1(value, locals, budget)?
            }
            SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Nop => false,
            _ => true,
        };
        if used {
            return Ok(true);
        }
    }
    borrowed_aggregate_terminator_uses_v1(block.terminator().kind(), locals, budget)
}
