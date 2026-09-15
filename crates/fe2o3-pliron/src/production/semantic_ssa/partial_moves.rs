use super::*;

mod discriminant_read_v1;
mod incoming_retention_v1;
mod state;
use state::{
    Path as SemanticMovePathV1, PathElement as SemanticMovePathElementV1,
    State as SemanticPartialMoveStateV1,
};

#[derive(Clone, Copy)]
struct SemanticPartialMoveLocationV1 {
    function: SemanticFunctionIdV1,
    block: u32,
    statement: Option<u32>,
}

struct SemanticPartialMoveBudgetV1 {
    function: SemanticFunctionIdV1,
    state: state::Budget,
    auxiliary_storage_words: usize,
    plan_storage_words: usize,
}

impl SemanticPartialMoveBudgetV1 {
    fn error(&self, error: state::Error) -> ProductionSemanticSsaErrorV1 {
        self.error_at(resource_diagnostic_v1::Stage::DynamicState, error)
    }

    fn error_at(
        &self,
        stage: resource_diagnostic_v1::Stage,
        error: state::Error,
    ) -> ProductionSemanticSsaErrorV1 {
        partial_move_budget_error_with_stage_v1(
            self.function,
            error,
            stage,
            self.auxiliary_storage_words,
            self.plan_storage_words,
        )
    }

    fn charge_work(&mut self) -> Result<(), ProductionSemanticSsaErrorV1> {
        self.state.work(1).map_err(|error| self.error(error))
    }

    fn projection_scratch(
        &self,
        place: &SemanticPlaceV1,
    ) -> Result<state::Storage, ProductionSemanticSsaErrorV1> {
        let depth = place.projections().len();
        self.state
            .work(
                depth
                    .checked_add(1)
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
            )
            .map_err(|error| self.error(error))?;
        let words = depth
            .checked_mul(size_of::<SemanticMovePathElementV1>().div_ceil(size_of::<usize>()))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
        self.state.reserve(words).map_err(|error| self.error(error))
    }
}

fn partial_move_budget_error_v1(
    function: SemanticFunctionIdV1,
    error: state::Error,
) -> ProductionSemanticSsaErrorV1 {
    match error {
        state::Error::Overflow => ProductionSemanticSsaErrorV1::ResourceOverflow,
        state::Error::Limit {
            resource,
            required,
            limit,
            ..
        } => ProductionSemanticSsaErrorV1::PartialMoveResourceLimit {
            function,
            resource: match resource {
                state::Resource::Storage => SsaPlannerResourceV1::StorageWords,
                state::Resource::Work => SsaPlannerResourceV1::WorkUnits,
            },
            required,
            limit,
        },
    }
}

fn partial_move_budget_error_with_stage_v1(
    function: SemanticFunctionIdV1,
    error: state::Error,
    stage: resource_diagnostic_v1::Stage,
    auxiliary_storage_words: usize,
    plan_storage_words: usize,
) -> ProductionSemanticSsaErrorV1 {
    let storage = match error {
        state::Error::Limit { storage, .. } => storage,
        state::Error::Overflow => None,
    };
    let error = partial_move_budget_error_v1(function, error);
    match storage {
        Some(storage) => resource_diagnostic_v1::wrap(
            error,
            stage,
            auxiliary_storage_words,
            Some(plan_storage_words),
            (storage.live, storage.peak, storage.requested),
        ),
        None => error,
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

    let base_storage_words = plan
        .resources()
        .storage_words()
        .checked_add(auxiliary_resources.storage_words)
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
    let base_work_units = plan
        .resources()
        .work_units()
        .checked_add(auxiliary_resources.work_units)
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
    let mut budget = SemanticPartialMoveBudgetV1 {
        function: function_id,
        state: state::Budget::new(
            base_storage_words,
            base_work_units,
            limits.planner().max_storage_words(),
            limits.planner().max_work_units(),
        )
        .map_err(|error| {
            partial_move_budget_error_with_stage_v1(
                function_id,
                error,
                resource_diagnostic_v1::Stage::StateBase,
                auxiliary_resources.storage_words,
                plan.resources().storage_words(),
            )
        })?,
        auxiliary_storage_words: auxiliary_resources.storage_words,
        plan_storage_words: plan.resources().storage_words(),
    };
    // Incoming handles, queue/queued entries and bounded traversal scratch are
    // live for this pass. Shared nodes and owned paths are charged on allocation.
    let workspace_words = function
        .blocks()
        .len()
        .checked_mul(4)
        .and_then(|words| words.checked_add(80))
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
    let _workspace = budget
        .state
        .reserve(workspace_words)
        .map_err(|error| budget.error_at(resource_diagnostic_v1::Stage::Workspace, error))?;
    let mut incoming = vec![None::<SemanticPartialMoveStateV1>; function.blocks().len()];
    let entry = function.entry().index() as usize;
    incoming[entry] = Some(SemanticPartialMoveStateV1::default());
    let mut pending = VecDeque::from([entry]);
    let mut queued = vec![false; function.blocks().len()];
    queued[entry] = true;
    let return_local = function
        .locals()
        .iter()
        .position(|local| matches!(local.role(), SemanticLocalRoleV1::Return))
        .map(|local| local as u32);
    let retention = incoming_retention_v1::InputRetention::new(
        function.blocks().len(),
        entry,
        &budget.state,
        |record| {
            for (source, block) in function.blocks().iter().enumerate() {
                block
                    .terminator()
                    .kind()
                    .try_for_each_edge(|edge| record(source, edge.target().index() as usize))?;
            }
            Ok(())
        },
    )
    .map_err(|error| budget.error_at(resource_diagnostic_v1::Stage::Retention, error))?;

    while let Some(block_index) = pending.pop_front() {
        queued[block_index] = false;
        if !plan.is_reachable(SsaBlockIdV1::new(block_index as u32)) {
            continue;
        }
        budget.charge_work()?;
        let mut state = retention
            .begin(block_index, &mut incoming)
            .ok_or(ProductionSemanticSsaErrorV1::ReplayMismatch)?;
        let block = &function.blocks()[block_index];
        for (statement_index, statement) in block.statements().iter().enumerate() {
            budget.charge_work()?;
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
        budget.charge_work()?;
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
            budget.charge_work()?;
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
                incoming[target].get_or_insert_with(SemanticPartialMoveStateV1::default),
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
        // Existing certificate field carries peak logical state storage, not
        // cumulative insertions into states that may already have been freed.
        state_entries: budget.state.peak(),
        work_units: budget.state.work_units(),
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
        SemanticStatementKindV1::StorageLive(local) => state
            .clear(local.index(), &budget.state)
            .map_err(|error| budget.error(error)),
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
        | SemanticRvalueKindV1::Length(place) => {
            validate_partial_move_place_read_v1(function, types, place, location, state, budget)
        }
        SemanticRvalueKindV1::Discriminant(place) => {
            discriminant_read_v1::validate(function, types, place, location, state, budget)
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
    budget.charge_work()?;
    let place = match operand {
        SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
        SemanticOperandV1::Constant(_) => return Ok(()),
    };
    validate_partial_move_place_read_v1(function, types, place, location, state, budget)?;
    if matches!(operand, SemanticOperandV1::Move(_)) {
        let _scratch = budget.projection_scratch(place)?;
        if let Some(path) = canonical_partial_move_path_v1(function, types, place, location)? {
            mark_partial_move_v1(place.local().index(), path, state, budget)?;
        }
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
        return state
            .clear(destination.local().index(), &budget.state)
            .map_err(|error| budget.error(error));
    }

    let local = destination.local().index();
    let _scratch = budget.projection_scratch(destination)?;
    if let [projection] = destination.projections()
        && matches!(projection.kind(), SemanticProjectionKindV1::Index(_))
        && let Some(declaration) = function.locals().get(local as usize)
        && let Some(types) = types
        && matches!(types.get(declaration.ty().index() as usize).map(|ty| ty.shape()),
            Some(SemanticTypeShapeV1::Array { element, .. }) if *element == destination.ty())
    {
        // Replacing one element cannot introduce a move hole in a readable
        // owned array. Do not clear any existing hole or infer initialization;
        // retained-storage initialization and bounds checks remain independent.
        validate_partial_move_path_read_v1(local, &[], location, state, budget)?;
        return validate_partial_move_projection_indices_v1(destination, location, state, budget);
    }
    let Some(path) = canonical_partial_move_path_v1(function, types, destination, location)? else {
        return validate_partial_move_projection_indices_v1(destination, location, state, budget);
    };
    if !state
        .initialize(local, &path, &budget.state)
        .map_err(|error| budget.error(error))?
    {
        return Err(partial_move_error_v1(
            location,
            local,
            SemanticPartialMoveViolationV1::MaybeMovedValueUsed,
        ));
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
        budget.charge_work()?;
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
    let _scratch = budget.projection_scratch(place)?;
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
    if !state
        .readable(local, path, &budget.state)
        .map_err(|error| budget.error(error))?
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
    state
        .mark(local, path, &budget.state)
        .map_err(|error| budget.error(error))
}

fn merge_partial_move_state_v1(
    destination: &mut SemanticPartialMoveStateV1,
    source: &SemanticPartialMoveStateV1,
    budget: &mut SemanticPartialMoveBudgetV1,
) -> Result<bool, ProductionSemanticSsaErrorV1> {
    destination
        .merge(source, &budget.state)
        .map_err(|error| budget.error(error))
}

/// Parent hook: replace the old dense partial-move preflight with this adapter
/// envelope. The solver separately enforces actual peak state storage and work.
pub(super) fn auxiliary_resources_v1(
    function: &SemanticFunctionDeclV1,
    input: &SsaConstructionInputV1,
) -> Result<SemanticSsaAuxiliaryResourcesV1, ProductionSemanticSsaErrorV1> {
    let blocks = input.blocks().len();
    let variables = input.promotable().len();
    let statements = function.blocks().iter().try_fold(0usize, |total, block| {
        total
            .checked_add(block.statements().len())
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)
    })?;
    let (events, edges, definitions) = input.blocks().iter().try_fold(
        (0usize, 0usize, 0usize),
        |(events, edges, definitions), block| {
            let events = events
                .checked_add(block.events().len())
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            let edges = edges
                .checked_add(block.edges().len())
                .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
            let definitions = block.edges().iter().try_fold(definitions, |total, edge| {
                total
                    .checked_add(edge.definitions().len())
                    .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)
            })?;
            Ok((events, edges, definitions))
        },
    )?;
    let adapter_items = variables
        .checked_mul(8)
        .and_then(|value| value.checked_add(blocks.checked_mul(12)?))
        .and_then(|value| value.checked_add(statements.checked_mul(8)?))
        .and_then(|value| value.checked_add(events.checked_mul(4)?))
        .and_then(|value| value.checked_add(edges.checked_mul(6)?))
        .and_then(|value| value.checked_add(definitions.checked_mul(2)?))
        .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?;
    Ok(SemanticSsaAuxiliaryResourcesV1 {
        storage_words: adapter_items,
        work_units: adapter_items
            .checked_add(events)
            .and_then(|value| value.checked_add(definitions))
            .ok_or(ProductionSemanticSsaErrorV1::ResourceOverflow)?,
    })
}
