//! Body-derived scalar transports for exact checked-arithmetic refinements.
//! Called only after complete module structural validation. Unsupported facts
//! stay unavailable; no function name or decoded source identity is trusted.

use super::*;
use crate::semantic_mir_v1::{
    SemanticAssignmentV1, SemanticConstantV1, SemanticConstantValueV1, SemanticEdgeRoleV1,
    SemanticExternAbiV1, SemanticLocalRoleV1, SemanticScalarTypeV1, SemanticTypeIdV1,
    SemanticUnwindActionV1,
};

#[cfg(test)]
#[path = "unchecked_arithmetic_transport_v1_tests.rs"]
mod tests;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Location {
    block: usize,
    statement: usize,
}

#[derive(Clone, Copy)]
enum Definition<'a> {
    Entry,
    Assignment(&'a SemanticAssignmentV1, Location),
    Call(&'a SemanticDirectCallV1, Location),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Origin<'a> {
    Local(SemanticLocalIdV1, SemanticTypeIdV1),
    Constant(&'a SemanticConstantV1),
    Overflow(SemanticLocalIdV1),
}

pub(crate) fn semantic_unchecked_arithmetic_in_module_v1(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
) -> Result<Option<SemanticUncheckedArithmeticViolationV1>, SemanticOptionDominanceErrorV1> {
    analyze(
        function,
        types,
        functions,
        callables,
        &mut WorkBudgetV1::default(),
    )
}

fn analyze(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    functions: &[SemanticFunctionDeclV1],
    callables: &[SemanticCallableDeclV1],
    budget: &mut WorkBudgetV1<'_>,
) -> Result<Option<SemanticUncheckedArithmeticViolationV1>, SemanticOptionDominanceErrorV1> {
    let legacy = super::semantic_unchecked_arithmetic_with_budget_v1(function, budget)?;
    if legacy.is_none() {
        return Ok(None);
    }
    // This fallback must prove every site in its conservative transport domain.
    // A legacy proof of one site is not retained after whole-function refusal.
    let dominators = DominatorIntervalsV1::analyze(function, budget)?;
    if !acyclic(function, &dominators, budget)? {
        return Ok(legacy);
    }
    let counts = local_definition_counts(function, budget)?;
    budget.charge(function.locals().len())?;
    let mut definitions = budget.filled(function.locals().len(), None)?;
    let mut exposed = budget.filled(function.locals().len(), false)?;
    for (index, local) in function.locals().iter().enumerate() {
        if local.role().is_entry_argument() && counts[index] == 1 {
            definitions[index] = Some(Definition::Entry);
        }
    }
    for (block_index, block) in function.blocks().iter().enumerate() {
        budget.charge(block.statements().len().saturating_add(1))?;
        for (statement, item) in block.statements().iter().enumerate() {
            if let SemanticStatementKindV1::Assign(assignment) = item.kind() {
                let destination = assignment.destination();
                let index = destination.local().index() as usize;
                if destination.projections().is_empty() && counts.get(index) == Some(&1) {
                    definitions[index] = Some(Definition::Assignment(
                        assignment,
                        Location {
                            block: block_index,
                            statement,
                        },
                    ));
                }
                if let SemanticRvalueKindV1::Borrow { place, .. }
                | SemanticRvalueKindV1::AddressOf { place, .. } = assignment.value().kind()
                {
                    *exposed
                        .get_mut(place.local().index() as usize)
                        .ok_or(invalid())? = true;
                }
            }
        }
        if let SemanticTerminatorKindV1::Call(call) = block.terminator().kind()
            && let Some(destination) = call.destination()
        {
            let index = destination.place().local().index() as usize;
            if destination.place().projections().is_empty() && counts.get(index) == Some(&1) {
                definitions[index] = Some(Definition::Call(
                    call,
                    Location {
                        block: block_index,
                        statement: block.statements().len(),
                    },
                ));
            }
        }
        if let SemanticTerminatorKindV1::Drop { place, .. } = block.terminator().kind() {
            *exposed
                .get_mut(place.local().index() as usize)
                .ok_or(invalid())? = true;
        }
    }
    budget.charge(functions.len())?;
    let summaries = budget.filled(functions.len(), None)?;
    let mut analysis = Analysis {
        function,
        types,
        functions,
        callables,
        definitions,
        exposed,
        dominators,
        summaries,
        budget,
    };
    for (block, data) in function.blocks().iter().enumerate() {
        for (statement, item) in data.statements().iter().enumerate() {
            analysis.budget.charge(1)?;
            let SemanticStatementKindV1::Assign(assignment) = item.kind() else {
                continue;
            };
            let SemanticRvalueKindV1::UncheckedBinary(unchecked) = assignment.value().kind() else {
                continue;
            };
            let location = Location { block, statement };
            if !analysis.proves(unchecked, location)? {
                return Ok(Some(SemanticUncheckedArithmeticViolationV1 {
                    operation: unchecked.operation(),
                    block: SemanticBlockIdV1::from_index(block as u32),
                    statement: statement as u32,
                }));
            }
        }
    }
    Ok(None)
}

fn invalid() -> SemanticOptionDominanceErrorV1 {
    SemanticOptionDominanceErrorV1::InvalidControlFlow("scalar transport references invalid MIR")
}

fn is_bool(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    matches!(
        types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(SemanticScalarTypeV1::Bool))
    )
}

fn is_scalar(types: &[SemanticTypeDeclV1], ty: SemanticTypeIdV1) -> bool {
    matches!(
        types
            .get(ty.index() as usize)
            .map(SemanticTypeDeclV1::shape),
        Some(SemanticTypeShapeV1::Scalar(
            SemanticScalarTypeV1::Bool | SemanticScalarTypeV1::Integer { .. }
        ))
    )
}

fn visit_location_operands(
    function: &SemanticFunctionDeclV1,
    location: Location,
    mut visit: impl FnMut(&SemanticOperandV1) -> Result<(), SemanticOptionDominanceErrorV1>,
) -> Result<(), SemanticOptionDominanceErrorV1> {
    let block = function.blocks().get(location.block).ok_or(invalid())?;
    if let Some(statement) = block.statements().get(location.statement) {
        return match statement.kind() {
            SemanticStatementKindV1::Assign(assignment) => {
                assignment.value().kind().try_visit_operands(visit)
            }
            SemanticStatementKindV1::Store(store) => visit(store.value()),
            SemanticStatementKindV1::AtomicRmw(atomic) => visit(atomic.value()),
            SemanticStatementKindV1::AtomicCompareExchange(atomic) => {
                visit(atomic.expected())?;
                visit(atomic.replacement())
            }
            SemanticStatementKindV1::Assume(operand) => visit(operand),
            SemanticStatementKindV1::SetDiscriminant { .. }
            | SemanticStatementKindV1::Deinitialize(_)
            | SemanticStatementKindV1::StorageLive(_)
            | SemanticStatementKindV1::StorageDead(_)
            | SemanticStatementKindV1::Nop => Ok(()),
        };
    }
    if location.statement != block.statements().len() {
        return Err(invalid());
    }
    match block.terminator().kind() {
        SemanticTerminatorKindV1::Call(call) => {
            for operand in call.arguments() {
                visit(operand)?;
            }
            Ok(())
        }
        SemanticTerminatorKindV1::TailCall(call) => {
            for operand in call.arguments() {
                visit(operand)?;
            }
            Ok(())
        }
        SemanticTerminatorKindV1::SwitchInt { discriminant, .. } => visit(discriminant),
        SemanticTerminatorKindV1::Assert {
            condition, message, ..
        } => {
            use crate::semantic_mir_v1::SemanticAssertMessageV1 as Message;
            visit(condition)?;
            match message {
                Message::BoundsCheck {
                    length: left,
                    index: right,
                }
                | Message::Overflow { left, right, .. }
                | Message::MisalignedPointerDereference {
                    required_alignment: left,
                    found_alignment: right,
                } => {
                    visit(left)?;
                    visit(right)
                }
                Message::DivisionByZero(operand) | Message::RemainderByZero(operand) => {
                    visit(operand)
                }
                Message::NullPointerDereference
                | Message::ResumedAfterReturn
                | Message::ResumedAfterPanic => Ok(()),
            }
        }
        SemanticTerminatorKindV1::Goto(_)
        | SemanticTerminatorKindV1::Drop { .. }
        | SemanticTerminatorKindV1::FalseEdge { .. }
        | SemanticTerminatorKindV1::Return
        | SemanticTerminatorKindV1::UnwindResume
        | SemanticTerminatorKindV1::UnwindTerminate
        | SemanticTerminatorKindV1::Abort
        | SemanticTerminatorKindV1::Unreachable => Ok(()),
    }
}

fn operand_uses(
    function: &SemanticFunctionDeclV1,
    location: Location,
    local: SemanticLocalIdV1,
    budget: &mut WorkBudgetV1<'_>,
) -> Result<(usize, bool), SemanticOptionDominanceErrorV1> {
    let mut reads = 0usize;
    let mut moved = false;
    visit_location_operands(function, location, |operand| {
        budget.charge(1)?;
        if let SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) = operand {
            budget.charge(place.projections().len().saturating_add(1))?;
            if place.local() == local {
                reads += 1;
                // Match semantic SSA: projected moves read the aggregate, but
                // only whole-local moves invalidate that local's scalar origin.
                moved |=
                    matches!(operand, SemanticOperandV1::Move(_)) && place.projections().is_empty();
            }
        }
        Ok(())
    })?;
    Ok((reads, moved))
}

// A conservative first transport domain: no path can revisit a definition or
// storage generation. The legacy checker remains available for other CFGs.
fn acyclic(
    function: &SemanticFunctionDeclV1,
    dominators: &DominatorIntervalsV1,
    budget: &mut WorkBudgetV1<'_>,
) -> Result<bool, SemanticOptionDominanceErrorV1> {
    let count = function.blocks().len();
    budget.charge(count)?;
    let mut incoming = budget.filled(count, 0usize)?;
    let mut ready = Vec::new();
    let mut reachable = 0usize;
    for (block, predecessors) in dominators.predecessors.iter().enumerate() {
        budget.charge(predecessors.len().saturating_add(1))?;
        if dominators.is_reachable(block) {
            reachable += 1;
            incoming[block] = predecessors
                .iter()
                .filter(|source| dominators.is_reachable(**source))
                .count();
            if incoming[block] == 0 {
                budget.push(&mut ready, block)?;
            }
        }
    }
    let mut visited = 0usize;
    while let Some(block) = ready.pop() {
        budget.charge(1)?;
        visited += 1;
        function.blocks()[block]
            .terminator()
            .kind()
            .try_for_each_edge(|edge| {
                budget.charge(1)?;
                let index = edge.target().index() as usize;
                let remaining = incoming.get_mut(index).ok_or(invalid())?;
                *remaining = remaining.checked_sub(1).ok_or(invalid())?;
                if *remaining == 0 {
                    budget.push(&mut ready, index)?;
                }
                Ok::<_, SemanticOptionDominanceErrorV1>(())
            })?;
    }
    Ok(visited == reachable)
}

struct Analysis<'a, 'budget, 'meter> {
    function: &'a SemanticFunctionDeclV1,
    types: &'a [SemanticTypeDeclV1],
    functions: &'a [SemanticFunctionDeclV1],
    callables: &'a [SemanticCallableDeclV1],
    definitions: Vec<Option<Definition<'a>>>,
    exposed: Vec<bool>,
    dominators: DominatorIntervalsV1,
    summaries: Vec<Option<bool>>,
    budget: &'budget mut WorkBudgetV1<'meter>,
}

impl<'a> Analysis<'a, '_, '_> {
    fn identity(
        &mut self,
        call: &SemanticDirectCallV1,
    ) -> Result<bool, SemanticOptionDominanceErrorV1> {
        if call.arguments().len() != 1 || !call.variadic_argument_abis().is_empty() {
            return Ok(false);
        }
        let Some(SemanticCallableDeclV1::Defined { function }) =
            self.callables.get(call.callee().index() as usize)
        else {
            return Ok(false);
        };
        let index = function.index() as usize;
        self.budget.charge(1)?;
        if let Some(cached) = self.summaries.get(index).copied().flatten() {
            return Ok(cached);
        }
        let function = self.functions.get(index).ok_or(invalid())?;
        let result = bool_identity(function, self.types, self.callables, self.budget)?;
        *self.summaries.get_mut(index).ok_or(invalid())? = Some(result);
        Ok(result)
    }

    fn live(
        &mut self,
        local: SemanticLocalIdV1,
        definition: Definition<'a>,
        usage: Location,
    ) -> Result<bool, SemanticOptionDominanceErrorV1> {
        // A consuming read is valid at this location, but repeated reads with a
        // Move are not ordered by Location. Refuse that ambiguous proof input.
        let (reads, moved) = operand_uses(self.function, usage, local, self.budget)?;
        if moved && reads > 1 {
            return Ok(false);
        }
        let origin = match definition {
            Definition::Entry => Location {
                block: self.function.entry().index() as usize,
                statement: 0,
            },
            Definition::Assignment(_, location) => {
                if usage == location {
                    return Ok(false);
                }
                location
            }
            Definition::Call(call, location) => {
                let Some(destination) = call.destination() else {
                    return Ok(false);
                };
                let continuation = destination.edge().target().index() as usize;
                if call.unwind() != SemanticUnwindActionV1::Unreachable
                    || destination.edge().role() != SemanticEdgeRoleV1::CallReturn
                    || !self
                        .dominators
                        .has_unique_predecessor(continuation, location.block)
                    || !self.dominators.dominates(continuation, usage.block)
                {
                    return Ok(false);
                }
                location
            }
        };
        if !self.dominators.dominates(origin.block, usage.block)
            || (origin.block == usage.block && origin.statement > usage.statement)
        {
            return Ok(false);
        }
        let block_count = self.function.blocks().len();
        self.budget.charge(block_count)?;
        let mut visited = self.budget.filled(block_count, false)?;
        let mut pending = Vec::new();
        self.budget.push(&mut pending, usage)?;
        while let Some(end) = pending.pop() {
            self.budget.charge(1)?;
            if visited[end.block] {
                continue;
            }
            visited[end.block] = true;
            let begin = if end.block == origin.block {
                origin.statement + usize::from(matches!(definition, Definition::Assignment(..)))
            } else {
                0
            };
            let statements = self.function.blocks()[end.block].statements();
            if begin > end.statement || end.statement > statements.len() {
                return Ok(false);
            }
            self.budget.charge(end.statement - begin)?;
            for (offset, item) in statements[begin..end.statement].iter().enumerate() {
                if matches!(item.kind(),
                    SemanticStatementKindV1::StorageLive(candidate)
                        | SemanticStatementKindV1::StorageDead(candidate) if *candidate == local)
                {
                    return Ok(false);
                }
                if operand_uses(
                    self.function,
                    Location {
                        block: end.block,
                        statement: begin + offset,
                    },
                    local,
                    self.budget,
                )?
                .1
                {
                    return Ok(false);
                }
            }
            if end.block != origin.block {
                for predecessor in &self.dominators.predecessors[end.block] {
                    self.budget.charge(1)?;
                    if self.dominators.is_reachable(*predecessor) {
                        let previous = Location {
                            block: *predecessor,
                            statement: self.function.blocks()[*predecessor].statements().len(),
                        };
                        // The call result starts after its arguments are consumed.
                        if !(previous.block == origin.block
                            && matches!(definition, Definition::Call(..)))
                            && operand_uses(self.function, previous, local, self.budget)?.1
                        {
                            return Ok(false);
                        }
                        self.budget.push(&mut pending, previous)?;
                    }
                }
            }
        }
        Ok(true)
    }

    fn origin(
        &mut self,
        mut operand: &'a SemanticOperandV1,
        mut usage: Location,
    ) -> Result<Option<Origin<'a>>, SemanticOptionDominanceErrorV1> {
        let ty = operand.ty();
        if !is_scalar(self.types, ty) {
            return Ok(None);
        }
        for _ in 0..=self.definitions.len() {
            self.budget.charge(1)?;
            if operand.ty() != ty {
                return Ok(None);
            }
            let place = match operand {
                SemanticOperandV1::Constant(value) => {
                    return Ok(matches!(value.value(), SemanticConstantValueV1::Scalar(_))
                        .then_some(Origin::Constant(value)));
                }
                SemanticOperandV1::Copy(place) | SemanticOperandV1::Move(place) => place,
            };
            let index = place.local().index() as usize;
            let Some(definition) = self.definitions.get(index).copied().flatten() else {
                return Ok(None);
            };
            if *self.exposed.get(index).ok_or(invalid())?
                || !self.live(place.local(), definition, usage)?
            {
                return Ok(None);
            }
            if !place.projections().is_empty() {
                return Ok(match (place.projections(), definition) {
                    ([field], Definition::Assignment(assignment, _))
                        if field.kind() == SemanticProjectionKindV1::Field(1)
                            && is_bool(self.types, ty)
                            && matches!(
                                assignment.value().kind(),
                                SemanticRvalueKindV1::CheckedBinary(_)
                            ) =>
                    {
                        Some(Origin::Overflow(place.local()))
                    }
                    _ => None,
                });
            }
            if self.function.locals().get(index).map(|local| local.ty()) != Some(ty) {
                return Ok(None);
            }
            match definition {
                Definition::Assignment(assignment, location) => {
                    if let SemanticRvalueKindV1::Use(source) = assignment.value().kind() {
                        operand = source;
                        usage = location;
                    } else {
                        return Ok(Some(Origin::Local(place.local(), ty)));
                    }
                }
                Definition::Call(call, location)
                    if is_bool(self.types, ty) && self.identity(call)? =>
                {
                    operand = &call.arguments()[0];
                    usage = location;
                }
                Definition::Entry | Definition::Call(..) => {
                    return Ok(Some(Origin::Local(place.local(), ty)));
                }
            }
        }
        Ok(None)
    }

    fn proves(
        &mut self,
        unchecked: &'a crate::semantic_mir_v1::SemanticUncheckedBinaryRvalueV1,
        usage: Location,
    ) -> Result<bool, SemanticOptionDominanceErrorV1> {
        let Some(left) = self.origin(unchecked.left(), usage)? else {
            return Ok(false);
        };
        let Some(right) = self.origin(unchecked.right(), usage)? else {
            return Ok(false);
        };
        let function = self.function;
        for (block_index, block) in function.blocks().iter().enumerate() {
            self.budget.charge(1)?;
            let SemanticTerminatorKindV1::SwitchInt {
                discriminant,
                targets,
            } = block.terminator().kind()
            else {
                continue;
            };
            let switch = Location {
                block: block_index,
                statement: block.statements().len(),
            };
            let Some(Origin::Overflow(local)) = self.origin(discriminant, switch)? else {
                continue;
            };
            let Some(Definition::Assignment(assignment, location)) = self
                .definitions
                .get(local.index() as usize)
                .copied()
                .flatten()
            else {
                continue;
            };
            let SemanticRvalueKindV1::CheckedBinary(checked) = assignment.value().kind() else {
                continue;
            };
            if checked.operation() != unchecked.operation().checked()
                || self.origin(checked.left(), location)? != Some(left)
                || self.origin(checked.right(), location)? != Some(right)
            {
                continue;
            }
            self.budget
                .charge(targets.values().len().saturating_add(1))?;
            if targets
                .values()
                .iter()
                .any(|target| !matches!(target.value(), 0 | 1))
            {
                continue;
            }
            let zero = targets
                .values()
                .iter()
                .find(|target| target.value() == 0)
                .map(|target| target.edge().target())
                .or_else(|| {
                    targets
                        .values()
                        .iter()
                        .any(|target| target.value() == 1)
                        .then(|| targets.otherwise().target())
                });
            if let Some(zero) = zero {
                let zero = zero.index() as usize;
                if self.dominators.has_unique_predecessor(zero, block_index)
                    && self.dominators.dominates(zero, usage.block)
                {
                    return Ok(true);
                }
            }
        }
        Ok(false)
    }
}

// Evaluate the *actual* restricted body twice. This proves identity for the
// complete bool domain, including the branch-hint shape, without trusting names.
fn bool_identity(
    function: &SemanticFunctionDeclV1,
    types: &[SemanticTypeDeclV1],
    callables: &[SemanticCallableDeclV1],
    budget: &mut WorkBudgetV1<'_>,
) -> Result<bool, SemanticOptionDominanceErrorV1> {
    let abi = function.abi();
    let [argument_ty] = abi.source_input_types() else {
        return Ok(false);
    };
    if abi.extern_abi() != SemanticExternAbiV1::Rust
        || abi.c_variadic()
        || abi.arguments().len() != 1
        || abi.return_value().source_ty() != *argument_ty
        || !is_bool(types, *argument_ty)
    {
        return Ok(false);
    }
    budget.charge(function.locals().len())?;
    let mut argument = None;
    let mut result = None;
    for (index, local) in function.locals().iter().enumerate() {
        match local.role() {
            SemanticLocalRoleV1::Argument(0)
                if local.ty() == *argument_ty && argument.is_none() =>
            {
                argument = Some(index);
            }
            SemanticLocalRoleV1::Return if local.ty() == *argument_ty && result.is_none() => {
                result = Some(index);
            }
            SemanticLocalRoleV1::Temporary => {}
            _ => return Ok(false),
        }
    }
    let (Some(argument), Some(result)) = (argument, result) else {
        return Ok(false);
    };
    for input in [false, true] {
        budget.charge(
            function
                .locals()
                .len()
                .saturating_add(function.blocks().len()),
        )?;
        let mut values = budget.filled(function.locals().len(), None)?;
        let mut visited = budget.filled(function.blocks().len(), false)?;
        values[argument] = Some(input);
        let mut block = function.entry().index() as usize;
        loop {
            budget.charge(1)?;
            let Some(seen) = visited.get_mut(block) else {
                return Err(invalid());
            };
            if *seen {
                return Ok(false);
            }
            *seen = true;
            let data = &function.blocks()[block];
            for item in data.statements() {
                budget.charge(1)?;
                match item.kind() {
                    SemanticStatementKindV1::Assign(assignment) => {
                        let place = assignment.destination();
                        if !place.projections().is_empty() || !is_bool(types, place.ty()) {
                            return Ok(false);
                        }
                        let SemanticRvalueKindV1::Use(operand) = assignment.value().kind() else {
                            return Ok(false);
                        };
                        let Some(value) = bool_value(operand, types, &mut values) else {
                            return Ok(false);
                        };
                        *values
                            .get_mut(place.local().index() as usize)
                            .ok_or(invalid())? = Some(value);
                    }
                    SemanticStatementKindV1::StorageLive(local)
                    | SemanticStatementKindV1::StorageDead(local) => {
                        *values.get_mut(local.index() as usize).ok_or(invalid())? = None;
                    }
                    SemanticStatementKindV1::Nop => {}
                    _ => return Ok(false),
                }
            }
            match data.terminator().kind() {
                SemanticTerminatorKindV1::Goto(edge) => block = edge.target().index() as usize,
                SemanticTerminatorKindV1::SwitchInt {
                    discriminant,
                    targets,
                } => {
                    budget.charge(targets.values().len().saturating_add(1))?;
                    if targets
                        .values()
                        .iter()
                        .any(|target| !matches!(target.value(), 0 | 1))
                    {
                        return Ok(false);
                    }
                    let Some(value) = bool_value(discriminant, types, &mut values) else {
                        return Ok(false);
                    };
                    block = targets
                        .values()
                        .iter()
                        .find(|target| target.value() == u128::from(value))
                        .map(|target| target.edge())
                        .unwrap_or(targets.otherwise())
                        .target()
                        .index() as usize;
                }
                SemanticTerminatorKindV1::Call(call) => {
                    let Some(SemanticCallableDeclV1::CompilerIntrinsic {
                        operation: SemanticCompilerIntrinsicOperationV1::ColdPath,
                        ..
                    }) = callables.get(call.callee().index() as usize)
                    else {
                        return Ok(false);
                    };
                    let Some(destination) = call.destination() else {
                        return Ok(false);
                    };
                    if !call.arguments().is_empty()
                        || !call.variadic_argument_abis().is_empty()
                        || call.unwind() != SemanticUnwindActionV1::Unreachable
                        || !destination.place().projections().is_empty()
                        || !matches!(
                            types
                                .get(destination.place().ty().index() as usize)
                                .map(SemanticTypeDeclV1::shape),
                            Some(SemanticTypeShapeV1::Unit)
                        )
                    {
                        return Ok(false);
                    }
                    block = destination.edge().target().index() as usize;
                }
                SemanticTerminatorKindV1::Return => {
                    if values[result] != Some(input) {
                        return Ok(false);
                    }
                    break;
                }
                _ => return Ok(false),
            }
        }
    }
    Ok(true)
}

fn bool_value(
    operand: &SemanticOperandV1,
    types: &[SemanticTypeDeclV1],
    values: &mut [Option<bool>],
) -> Option<bool> {
    if !is_bool(types, operand.ty()) {
        return None;
    }
    match operand {
        SemanticOperandV1::Copy(place) if place.projections().is_empty() => values
            .get(place.local().index() as usize)
            .copied()
            .flatten(),
        SemanticOperandV1::Move(place) if place.projections().is_empty() => {
            values.get_mut(place.local().index() as usize)?.take()
        }
        SemanticOperandV1::Constant(value) => match value.value() {
            SemanticConstantValueV1::Scalar(value) if value.bits() <= 1 => Some(value.bits() != 0),
            _ => None,
        },
        _ => None,
    }
}
