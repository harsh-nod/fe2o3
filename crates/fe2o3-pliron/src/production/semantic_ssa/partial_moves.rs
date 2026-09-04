use super::*;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum SemanticMovePathElementV1 {
    Field(u32),
    ConstantIndex { offset: u64, from_end: bool },
    Downcast(u32),
}

type SemanticMovePathV1 = Vec<SemanticMovePathElementV1>;
type SemanticPartialMoveStateV1 = BTreeMap<u32, BTreeSet<SemanticMovePathV1>>;

#[derive(Clone, Copy)]
struct SemanticPartialMoveLocationV1 {
    function: SemanticFunctionIdV1,
    block: u32,
    statement: Option<u32>,
}

struct SemanticPartialMoveBudgetV1 {
    function: SemanticFunctionIdV1,
    base_storage_words: usize,
    base_work_units: usize,
    state_entries: usize,
    work_units: usize,
    limits: SsaPlannerLimitsV1,
}

impl SemanticPartialMoveBudgetV1 {
    fn charge_state_entry(&mut self) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.state_entries = self
            .state_entries
            .checked_add(1)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        let required = self
            .base_storage_words
            .checked_add(self.state_entries)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        if required > self.limits.max_storage_words() {
            return Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
                function: self.function,
                resource: SsaPlannerResourceV1::StorageWords,
                required,
                limit: self.limits.max_storage_words(),
            });
        }
        Ok(())
    }

    fn charge_work(&mut self) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.work_units = self
            .work_units
            .checked_add(1)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        let required = self
            .base_work_units
            .checked_add(self.work_units)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        if required > self.limits.max_work_units() {
            return Err(ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
                function: self.function,
                resource: SsaPlannerResourceV1::WorkUnits,
                required,
                limit: self.limits.max_work_units(),
            });
        }
        Ok(())
    }
}

fn partial_move_error_v1(
    location: SemanticPartialMoveLocationV1,
    local: u32,
    violation: SemanticPartialMoveViolationV1,
) -> ProductionSemanticSsaErrorV1 {
    ProductionSemanticSsaErrorV1::PartialMove {
        function: location.function,
        block: location.block,
        statement: location.statement,
        local,
        violation,
    }
}

pub(super) fn validate_partial_moves_v1(
    function_id: SemanticFunctionIdV1,
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    plan: &SsaConstructionPlanV1,
    auxiliary_resources: SemanticSsaAuxiliaryResourcesV1,
    limits: ProductionSemanticSsaLimitsV1,
) -> Result<ProductionSemanticPartialMoveCertificateV1, ProductionSemanticSsaErrorV1> {
    let (projected_moves, _) = projected_local_move_metrics_v1(function)?;
    if projected_moves == 0 {
        return Ok(ProductionSemanticPartialMoveCertificateV1::default());
    }

    let mut budget = SemanticPartialMoveBudgetV1 {
        function: function_id,
        base_storage_words: plan
            .resources()
            .storage_words()
            .checked_add(auxiliary_resources.storage_words)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
        base_work_units: plan
            .resources()
            .work_units()
            .checked_add(auxiliary_resources.work_units)
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
        state_entries: 0,
        work_units: 0,
        limits: limits.planner(),
    };
    let mut incoming = vec![None::<SemanticPartialMoveStateV1>; function.blocks().len()];
    let entry = function.entry().index() as usize;
    incoming[entry] = Some(BTreeMap::new());
    let mut pending = VecDeque::from([entry]);
    let mut queued = vec![false; function.blocks().len()];
    queued[entry] = true;
    let return_local = function
        .locals()
        .iter()
        .position(|local| matches!(local.role(), SemanticLocalRoleV1::Return))
        .map(|local| local as u32);

    while let Some(block_index) = pending.pop_front() {
        queued[block_index] = false;
        if !plan.is_reachable(SsaBlockIdV1::new(block_index as u32)) {
            continue;
        }
        budget.charge_work()?;
        let mut state = incoming[block_index]
            .as_ref()
            .cloned()
            .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
        let block = &function.blocks()[block_index];
        for (statement_index, statement) in block.statements().iter().enumerate() {
            let location = SemanticPartialMoveLocationV1 {
                function: function_id,
                block: block_index as u32,
                statement: Some(statement_index as u32),
            };
            validate_partial_move_statement_v1(
                function,
                types,
                statement.kind(),
                location,
                &mut state,
                &mut budget,
            )?;
        }
        let location = SemanticPartialMoveLocationV1 {
            function: function_id,
            block: block_index as u32,
            statement: None,
        };
        validate_partial_move_terminator_v1(
            function,
            types,
            block.terminator().kind(),
            return_local,
            location,
            &mut state,
            &mut budget,
        )?;

        block.terminator().kind().try_for_each_edge(|edge| {
            let target = edge.target().index() as usize;
            if !plan.is_reachable(SsaBlockIdV1::new(target as u32)) {
                return Ok(());
            }
            let mut edge_state = state.clone();
            if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
                && let Some(destination) = call.destination()
                && destination.edge() == edge
            {
                validate_partial_move_destination_v1(
                    function,
                    types,
                    destination.place(),
                    location,
                    &mut edge_state,
                    &mut budget,
                )?;
            }
            let first_incoming_edge = incoming[target].is_none();
            let changed = merge_partial_move_state_v1(
                incoming[target].get_or_insert_with(BTreeMap::new),
                &edge_state,
                &mut budget,
            )?;
            if (first_incoming_edge || changed) && !queued[target] {
                queued[target] = true;
                pending.push_back(target);
            }
            Ok(())
        })?;
    }

    Ok(ProductionSemanticPartialMoveCertificateV1 {
        projected_moves,
        state_entries: budget.state_entries,
        work_units: budget.work_units,
    })
}

pub(super) fn projected_local_move_metrics_v1(
    function: &SemanticFunctionDeclV1,
) -> Result<(usize, usize), ProductionSemanticSsaErrorV1> {
    let mut count = 0_usize;
    let mut maximum_depth = 0_usize;
    let mut visit = |operand: &SemanticOperandV1| {
        if let SemanticOperandV1::Move(place) = operand
            && !place.projections().is_empty()
            && !matches!(
                place
                    .projections()
                    .first()
                    .map(|projection| projection.kind()),
                Some(SemanticProjectionKindV1::Dereference),
            )
        {
            count = count
                .checked_add(1)
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            maximum_depth = maximum_depth.max(place.projections().len());
        }
        Ok(())
    };
    for block in function.blocks() {
        for statement in block.statements() {
            match statement.kind() {
                SemanticStatementKindV1::Assign(assignment) => {
                    assignment.value().kind().try_visit_operands(&mut visit)?;
                }
                SemanticStatementKindV1::Store(store) => visit(store.value())?,
                SemanticStatementKindV1::AtomicRmw(operation) => visit(operation.value())?,
                SemanticStatementKindV1::AtomicCompareExchange(operation) => {
                    visit(operation.expected())?;
                    visit(operation.replacement())?;
                }
                SemanticStatementKindV1::Assume(condition) => visit(condition)?,
                SemanticStatementKindV1::SetDiscriminant { .. }
                | SemanticStatementKindV1::Deinitialize(_)
                | SemanticStatementKindV1::StorageLive(_)
                | SemanticStatementKindV1::StorageDead(_)
                | SemanticStatementKindV1::Nop => {}
            }
        }
        match block.terminator().kind() {
            SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => visit(discriminant)?,
            SemanticTerminatorKindV1::Call(call) => {
                for operand in call.arguments() {
                    visit(operand)?;
                }
            }
            SemanticTerminatorKindV1::TailCall(call) => {
                for operand in call.arguments() {
                    visit(operand)?;
                }
            }
            SemanticTerminatorKindV1::Assert {
                condition, message, ..
            } => {
                visit(condition)?;
                visit_assert_operands_v1(message, &mut visit)?;
            }
            SemanticTerminatorKindV1::Goto(_)
            | SemanticTerminatorKindV1::Drop { .. }
            | SemanticTerminatorKindV1::FalseEdge { .. }
            | SemanticTerminatorKindV1::Return
            | SemanticTerminatorKindV1::UnwindResume
            | SemanticTerminatorKindV1::UnwindTerminate
            | SemanticTerminatorKindV1::Abort
            | SemanticTerminatorKindV1::Unreachable => {}
        }
    }
    Ok((count, maximum_depth))
}

fn visit_assert_operands_v1<E>(
    message: &SemanticAssertMessageV1,
    visitor: &mut impl FnMut(&SemanticOperandV1) -> Result<(), E>,
) -> Result<(), E> {
    match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => {
            visitor(length)?;
            visitor(index)
        }
        SemanticAssertMessageV1::Overflow { left, right, .. } => {
            visitor(left)?;
            visitor(right)
        }
        SemanticAssertMessageV1::DivisionByZero(operand)
        | SemanticAssertMessageV1::RemainderByZero(operand) => visitor(operand),
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => {
            visitor(required_alignment)?;
            visitor(found_alignment)
        }
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => Ok(()),
    }
}

fn validate_partial_move_statement_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    statement: &SemanticStatementKindV1,
    location: SemanticPartialMoveLocationV1,
    state: &mut SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    match statement {
        SemanticStatementKindV1::Assign(assignment) => {
            validate_partial_move_rvalue_v1(
                function,
                types,
                assignment.value().kind(),
                location,
                state,
                budget,
            )?;
            validate_partial_move_destination_v1(
                function,
                types,
                assignment.destination(),
                location,
                state,
                budget,
            )
        }
        SemanticStatementKindV1::Store(store) => {
            validate_partial_move_place_read_v1(
                function,
                types,
                store.destination(),
                location,
                state,
                budget,
            )?;
            validate_partial_move_operand_v1(
                function,
                types,
                store.value(),
                location,
                state,
                budget,
            )
        }
        SemanticStatementKindV1::AtomicRmw(operation) => {
            validate_partial_move_place_read_v1(
                function,
                types,
                operation.address(),
                location,
                state,
                budget,
            )?;
            validate_partial_move_operand_v1(
                function,
                types,
                operation.value(),
                location,
                state,
                budget,
            )?;
            validate_partial_move_destination_v1(
                function,
                types,
                operation.destination(),
                location,
                state,
                budget,
            )
        }
        SemanticStatementKindV1::AtomicCompareExchange(operation) => {
            validate_partial_move_place_read_v1(
                function,
                types,
                operation.address(),
                location,
                state,
                budget,
            )?;
            validate_partial_move_operand_v1(
                function,
                types,
                operation.expected(),
                location,
                state,
                budget,
            )?;
            validate_partial_move_operand_v1(
                function,
                types,
                operation.replacement(),
                location,
                state,
                budget,
            )?;
            validate_partial_move_destination_v1(
                function,
                types,
                operation.destination(),
                location,
                state,
                budget,
            )
        }
        SemanticStatementKindV1::SetDiscriminant { place, .. } => {
            validate_partial_move_place_read_v1(function, types, place, location, state, budget)
        }
        SemanticStatementKindV1::Deinitialize(place) => {
            validate_partial_move_place_read_v1(function, types, place, location, state, budget)?;
            mark_partial_move_v1(place.local().index(), Vec::new(), state, budget)
        }
        SemanticStatementKindV1::StorageLive(local) => {
            state.remove(&local.index());
            Ok(())
        }
        SemanticStatementKindV1::StorageDead(local) => {
            mark_partial_move_v1(local.index(), Vec::new(), state, budget)
        }
        SemanticStatementKindV1::Assume(condition) => {
            validate_partial_move_operand_v1(function, types, condition, location, state, budget)
        }
        SemanticStatementKindV1::Nop => Ok(()),
    }
}

fn validate_partial_move_rvalue_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    value: &SemanticRvalueKindV1,
    location: SemanticPartialMoveLocationV1,
    state: &mut SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    value.try_visit_operands(|operand| {
        validate_partial_move_operand_v1(function, types, operand, location, state, budget)
    })?;
    match value {
        SemanticRvalueKindV1::Borrow { place, .. }
        | SemanticRvalueKindV1::AddressOf { place, .. }
        | SemanticRvalueKindV1::Length(place)
        | SemanticRvalueKindV1::Discriminant(place) => {
            validate_partial_move_place_read_v1(function, types, place, location, state, budget)
        }
        SemanticRvalueKindV1::Load(load) => validate_partial_move_place_read_v1(
            function,
            types,
            load.source(),
            location,
            state,
            budget,
        ),
        SemanticRvalueKindV1::Use(_)
        | SemanticRvalueKindV1::Unary { .. }
        | SemanticRvalueKindV1::Binary { .. }
        | SemanticRvalueKindV1::CheckedBinary(_)
        | SemanticRvalueKindV1::UncheckedBinary(_)
        | SemanticRvalueKindV1::Cast { .. }
        | SemanticRvalueKindV1::Aggregate(_) => Ok(()),
    }
}

fn validate_partial_move_terminator_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    terminator: &SemanticTerminatorKindV1,
    return_local: Option<u32>,
    location: SemanticPartialMoveLocationV1,
    state: &mut SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let mut operand = |operand| {
        validate_partial_move_operand_v1(function, types, operand, location, state, budget)
    };
    match terminator {
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => operand(discriminant),
        SemanticTerminatorKindV1::Call(call) => {
            for argument in call.arguments() {
                operand(argument)?;
            }
            drop(operand);
            if let Some(destination) = call.destination()
                && !destination.place().projections().is_empty()
            {
                validate_partial_move_projection_indices_v1(
                    destination.place(),
                    location,
                    state,
                    budget,
                )?;
            }
            Ok(())
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for argument in call.arguments() {
                operand(argument)?;
            }
            Ok(())
        }
        SemanticTerminatorKindV1::Drop { place, .. } => {
            validate_partial_move_place_read_v1(function, types, place, location, state, budget)
        }
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            operand(condition)?;
            drop(operand);
            validate_partial_move_assert_message_v1(
                function, types, message, location, state, budget,
            )
        }
        SemanticTerminatorKindV1::Return => {
            drop(operand);
            if let Some(local) = return_local {
                validate_partial_move_path_read_v1(local, &[], location, state, budget)?;
            }
            Ok(())
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => Ok(()),
    }
}

fn validate_partial_move_assert_message_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    message: &SemanticAssertMessageV1,
    location: SemanticPartialMoveLocationV1,
    state: &mut SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let mut visit = |operand| {
        validate_partial_move_operand_v1(function, types, operand, location, state, budget)
    };
    match message {
        SemanticAssertMessageV1::BoundsCheck { length, index } => {
            visit(length)?;
            visit(index)
        }
        SemanticAssertMessageV1::Overflow { left, right, .. } => {
            visit(left)?;
            visit(right)
        }
        SemanticAssertMessageV1::DivisionByZero(operand)
        | SemanticAssertMessageV1::RemainderByZero(operand) => visit(operand),
        SemanticAssertMessageV1::MisalignedPointerDereference {
            required_alignment,
            found_alignment,
        } => {
            visit(required_alignment)?;
            visit(found_alignment)
        }
        SemanticAssertMessageV1::NullPointerDereference
        | SemanticAssertMessageV1::ResumedAfterReturn
        | SemanticAssertMessageV1::ResumedAfterPanic => Ok(()),
    }
}

fn validate_partial_move_operand_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    operand: &SemanticOperandV1,
    location: SemanticPartialMoveLocationV1,
    state: &mut SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let place = match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
        SemanticOperandV1::Constant(_) => return Ok(()),
    };
    validate_partial_move_place_read_v1(function, types, place, location, state, budget)?;
    if matches!(operand, SemanticOperandV1::Move(_))
        && let Some(path) = canonical_partial_move_path_v1(function, types, place, location)?
    {
        mark_partial_move_v1(place.local().index(), path, state, budget)?;
    }
    Ok(())
}

fn validate_partial_move_destination_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    destination: &SemanticPlaceV1,
    location: SemanticPartialMoveLocationV1,
    state: &mut SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    if destination.projections().is_empty() {
        state.remove(&destination.local().index());
        return Ok(());
    }

    let local = destination.local().index();
    let Some(path) = canonical_partial_move_path_v1(function, types, destination, location)? else {
        return validate_partial_move_projection_indices_v1(destination, location, state, budget);
    };
    if let Some(moved) = state.get_mut(&local) {
        for prefix_length in 0..path.len() {
            budget.charge_work()?;
            if moved.contains(&path[..prefix_length]) {
                return Err(partial_move_error_v1(
                    location,
                    local,
                    SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
                ));
            }
        }
        budget.charge_work()?;
        moved.retain(|candidate| !candidate.starts_with(&path));
        if moved.is_empty() {
            state.remove(&local);
        }
    }
    validate_partial_move_projection_indices_v1(destination, location, state, budget)
}

fn validate_partial_move_projection_indices_v1(
    place: &SemanticPlaceV1,
    location: SemanticPartialMoveLocationV1,
    state: &SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    for projection in place.projections() {
        if let SemanticProjectionKindV1::Index(index) = projection.kind() {
            validate_partial_move_path_read_v1(index.index(), &[], location, state, budget)?;
        }
    }
    Ok(())
}

fn validate_partial_move_place_read_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    place: &SemanticPlaceV1,
    location: SemanticPartialMoveLocationV1,
    state: &SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let local = place.local().index();
    let path = canonical_partial_move_path_v1(function, types, place, location)
        .ok()
        .flatten()
        .unwrap_or_default();
    validate_partial_move_path_read_v1(local, &path, location, state, budget)?;
    validate_partial_move_projection_indices_v1(place, location, state, budget)
}

fn validate_partial_move_path_read_v1(
    local: u32,
    path: &[SemanticMovePathElementV1],
    location: SemanticPartialMoveLocationV1,
    state: &SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let Some(moved) = state.get(&local) else {
        return Ok(());
    };
    for prefix_length in 0..=path.len() {
        budget.charge_work()?;
        if moved.contains(&path[..prefix_length]) {
            return Err(partial_move_error_v1(
                location,
                local,
                SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
            ));
        }
    }
    budget.charge_work()?;
    if moved
        .range(path.to_vec()..)
        .next()
        .is_some_and(|candidate| candidate.starts_with(path))
    {
        return Err(partial_move_error_v1(
            location,
            local,
            SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
        ));
    }
    Ok(())
}

fn canonical_partial_move_path_v1(
    function: &SemanticFunctionDeclV1,
    types: Option<&[SemanticTypeDeclV1]>,
    place: &SemanticPlaceV1,
    location: SemanticPartialMoveLocationV1,
) -> Result<Option<SemanticMovePathV1>, ProductionSemanticSsaErrorV1> {
    let local = place.local().index();
    if matches!(
        place
            .projections()
            .first()
            .map(|projection| projection.kind()),
        Some(SemanticProjectionKindV1::Dereference),
    ) {
        return Ok(None);
    }
    let mut current_type = function
        .locals()
        .get(local as usize)
        .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?
        .ty();
    let mut path = Vec::with_capacity(place.projections().len());
    for (projection_index, projection) in place.projections().iter().enumerate() {
        let element = match projection.kind() {
            SemanticProjectionKindV1::Field(field) => {
                let types = types.ok_or_else(|| {
                    partial_move_error_v1(
                        location,
                        local,
                        SemanticPartialMoveViolationV1::MissingTypeContext,
                    )
                })?;
                let declaration = types.get(current_type.index() as usize).ok_or_else(|| {
                    partial_move_error_v1(
                        location,
                        local,
                        SemanticPartialMoveViolationV1::MissingTypeContext,
                    )
                })?;
                if matches!(declaration.shape(), SemanticTypeShapeV1::Union(_)) {
                    return Err(partial_move_error_v1(
                        location,
                        local,
                        SemanticPartialMoveViolationV1::UnionField,
                    ));
                }
                SemanticMovePathElementV1::Field(field)
            }
            SemanticProjectionKindV1::ConstantIndex {
                offset,
                minimum_length: _,
                from_end,
            } => SemanticMovePathElementV1::ConstantIndex { offset, from_end },
            SemanticProjectionKindV1::Downcast(variant) => {
                let has_selected_field = place
                    .projections()
                    .get(projection_index + 1)
                    .is_some_and(|next| matches!(next.kind(), SemanticProjectionKindV1::Field(_)));
                if types.is_none() || !has_selected_field {
                    return Err(partial_move_error_v1(
                        location,
                        local,
                        SemanticPartialMoveViolationV1::UnsupportedProjection,
                    ));
                }
                SemanticMovePathElementV1::Downcast(variant)
            }
            SemanticProjectionKindV1::Dereference
            | SemanticProjectionKindV1::Index(_)
            | SemanticProjectionKindV1::Subslice { .. }
            | SemanticProjectionKindV1::OpaqueCast
            | SemanticProjectionKindV1::Subtype => {
                return Err(partial_move_error_v1(
                    location,
                    local,
                    SemanticPartialMoveViolationV1::UnsupportedProjection,
                ));
            }
        };
        path.push(element);
        current_type = projection.result_type();
    }
    Ok(Some(path))
}

fn mark_partial_move_v1(
    local: u32,
    path: SemanticMovePathV1,
    state: &mut SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<(), ProductionSemanticSsaErrorV1> {
    let paths = state.entry(local).or_default();
    if path.is_empty() {
        paths.clear();
    } else if paths.contains(&Vec::new()) {
        return Ok(());
    }
    if paths.insert(path) {
        budget.charge_state_entry()?;
    }
    Ok(())
}

fn merge_partial_move_state_v1(
    destination: &mut SemanticPartialMoveStateV1,
    source: &SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    let mut changed = false;
    for (local, incoming_paths) in source {
        let paths = destination.entry(*local).or_default();
        if paths.contains(&Vec::new()) {
            continue;
        }
        if incoming_paths.contains(&Vec::new()) {
            if paths.len() != 1 || !paths.contains(&Vec::new()) {
                paths.clear();
                paths.insert(Vec::new());
                budget.charge_state_entry()?;
                changed = true;
            }
            continue;
        }
        for path in incoming_paths {
            budget.charge_work()?;
            if paths.insert(path.clone()) {
                budget.charge_state_entry()?;
                changed = true;
            }
        }
    }
    Ok(changed)
}
