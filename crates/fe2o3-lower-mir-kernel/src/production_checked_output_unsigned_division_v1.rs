use super::*;
use fe2o3_kernel_ir::{
    BinaryOp, CanonicalKirBlockCoordinateV1 as Block,
    CanonicalKirDefinitionCoordinateV1 as Definition, CastKind, ComparePredicate, Constant,
    OperationKind, ScalarType, Terminator, Type, ValueId,
};
use fe2o3_mir_model::semantic_mir_v1::{SemanticTargetArchitectureV1, SemanticTargetDataLayoutV1};

/// Admission facts for this inventory under the consuming source's target.
/// The enclosing admission transaction owns all scratch reservations. These
/// facts neither authorize hoisting a partial operation nor certify its origin.
pub(super) struct UnsignedDivision<'a, 'g> {
    inventory: &'a CanonicalKirInventoryV1<'g>,
    target: SemanticTargetDataLayoutV1,
    operations: Vec<u8>,
}

impl UnsignedDivision<'_, '_> {
    pub(super) fn operation(
        &self,
        inventory: &CanonicalKirInventoryV1<'_>,
        ordinal: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> R<bool> {
        charge(budget, 3)?;
        if !std::ptr::eq(self.inventory, inventory) {
            return Err(refused("unsigned division", "same borrowed inventory"));
        }
        match self.target.architecture() {
            SemanticTargetArchitectureV1::AmdGpuGfx942 => {}
        }
        Ok(self.operations.get(ordinal).copied() == Some(1))
    }
}

#[derive(Clone, Copy)]
enum Fact {
    Unknown,
    Literal(bool),
    Alias(usize),
    Compare {
        predicate: ComparePredicate,
        lhs: usize,
        rhs: usize,
    },
}

fn unsigned(ty: &Type) -> bool {
    matches!(
        ty,
        Type::Scalar(ScalarType::U32 | ScalarType::U64 | ScalarType::Index)
    )
}

fn literal(value: &Constant) -> Option<bool> {
    match value {
        Constant::U32(value) => Some(*value != 0),
        Constant::U64(value) | Constant::Index(value) => Some(*value != 0),
        _ => None,
    }
}

fn block_index(inventory: &CanonicalKirInventoryV1<'_>, block: Block) -> R<usize> {
    let function = inventory
        .functions()
        .get(block.function.0 as usize)
        .ok_or_else(|| refused("unsigned division", "function coordinate"))?;
    let index = function
        .blocks
        .start
        .checked_add(block.block as usize)
        .ok_or_else(arithmetic)?;
    if !function.blocks.contains(&index) {
        return Err(refused("unsigned division", "block coordinate"));
    }
    Ok(index)
}

fn normalized(
    facts: &[Fact],
    mut definition: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<usize> {
    // Verified SSA rules exclude a cycle of operation-only aliases. Keep this
    // walk finite independently of that invariant and of graph recursion.
    for _ in 0..=facts.len() {
        charge(budget, 2)?;
        match facts.get(definition) {
            Some(Fact::Alias(input)) => definition = *input,
            Some(_) => return Ok(definition),
            None => return Err(refused("unsigned division", "definition coordinate")),
        }
    }
    Err(refused(
        "unsigned division",
        "acyclic value-preserving aliases",
    ))
}

fn divisor(inventory: &CanonicalKirInventoryV1<'_>, ordinal: usize) -> Option<usize> {
    let row = &inventory.operations()[ordinal];
    if !matches!(
        row.operation.kind,
        OperationKind::Binary {
            op: BinaryOp::Divide | BinaryOp::Remainder,
            ..
        }
    ) || row.results.len() != 1
        || row.operands.len() != 2
    {
        return None;
    }
    let lhs = inventory.uses()[row.operands.start].definition;
    let rhs = inventory.uses()[row.operands.start + 1].definition;
    let ty = inventory.definitions()[rhs].ty;
    (unsigned(ty)
        && inventory.definitions()[lhs].ty == ty
        && inventory.definitions()[row.results.start].ty == ty)
        .then_some(rhs)
}

fn exact_definition(
    inventory: &CanonicalKirInventoryV1<'_>,
    block: Block,
    value: ValueId,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<usize> {
    inventory
        .definition_index_for_value(block.function, value, budget)
        .map_err(inventory_error)?
        .ok_or_else(|| refused("unsigned division", "exact function-local definition"))
}

fn guarded_edge(
    inventory: &CanonicalKirInventoryV1<'_>,
    facts: &[Fact],
    edge: usize,
    definition: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<bool> {
    charge(budget, 4)?;
    let row = &inventory.edges()[edge];
    let source = row.coordinate.source;
    let successor = row.coordinate.successor as usize;
    let terminator = inventory.blocks()[block_index(inventory, source)?].terminator;
    match terminator {
        Terminator::ConditionalBranch { condition, .. } => {
            let condition = exact_definition(inventory, source, *condition, budget)?;
            charge(budget, 2)?;
            let Fact::Compare {
                predicate,
                lhs,
                rhs,
            } = facts[condition]
            else {
                return Ok(false);
            };
            let nonzero_edge = match predicate {
                ComparePredicate::NotEqual => successor == 0,
                ComparePredicate::Equal => successor == 1,
                _ => false,
            };
            if !nonzero_edge {
                return Ok(false);
            }
            let lhs = normalized(facts, lhs, budget)?;
            let rhs = normalized(facts, rhs, budget)?;
            charge(budget, 4)?;
            Ok(
                (lhs == definition && matches!(facts[rhs], Fact::Literal(false)))
                    || (rhs == definition && matches!(facts[lhs], Fact::Literal(false))),
            )
        }
        Terminator::Switch {
            selector, cases, ..
        } => {
            let selector = exact_definition(inventory, source, *selector, budget)?;
            if normalized(facts, selector, budget)? != definition {
                return Ok(false);
            }
            charge(budget, 1)?;
            if let Some(case) = cases.get(successor) {
                return Ok(case.value != 0);
            }
            if successor != cases.len() {
                return Ok(false);
            }
            for case in cases {
                charge(budget, 1)?;
                if case.value == 0 {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Terminator::IntegerSwitch {
            selector, cases, ..
        } => {
            let selector = exact_definition(inventory, source, *selector, budget)?;
            if normalized(facts, selector, budget)? != definition {
                return Ok(false);
            }
            charge(budget, 1)?;
            if let Some(case) = cases.get(successor) {
                return Ok(literal(&case.value) == Some(true));
            }
            if successor != cases.len() {
                return Ok(false);
            }
            for case in cases {
                charge(budget, 1)?;
                if literal(&case.value) == Some(false) {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        _ => Ok(false),
    }
}

fn immediate(
    inventory: &CanonicalKirInventoryV1<'_>,
    facts: &[Fact],
    block: usize,
    definition: usize,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<Option<bool>> {
    charge(budget, 3)?;
    if let Fact::Literal(nonzero) = facts[definition] {
        return Ok(Some(nonzero));
    }
    let coordinate = inventory.blocks()[block].coordinate;
    match inventory.definitions()[definition].coordinate {
        Definition::Result { operation, .. } if operation.block == coordinate => Ok(Some(false)),
        // This is the explicit unknown-input seed, including an entry block
        // which also has backedges. A cyclic predecessor cannot erase entry.
        Definition::FunctionArgument { .. } | Definition::BlockArgument { .. }
            if coordinate.block == 0 =>
        {
            Ok(Some(false))
        }
        _ => Ok(None),
    }
}

struct Search<'a> {
    facts: &'a [Fact],
    incoming: &'a [usize],
    offsets: &'a [usize],
    reachable: &'a [u8],
    seen: &'a mut [usize],
    queue: &'a mut Vec<usize>,
}

impl Search<'_> {
    fn enqueue(
        &mut self,
        function: &fe2o3_kernel_analysis::CanonicalKirFunctionRefV1<'_>,
        block: usize,
        definition: usize,
        epoch: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> R<()> {
        charge(budget, 5)?;
        if !function.blocks.contains(&block) || !function.definitions.contains(&definition) {
            return Err(refused("unsigned division", "same function state"));
        }
        let state = (block - function.blocks.start)
            .checked_mul(function.definitions.len())
            .and_then(|n| n.checked_add(definition - function.definitions.start))
            .ok_or_else(arithmetic)?;
        let seen = self
            .seen
            .get_mut(state)
            .ok_or_else(|| refused("unsigned division", "finite state bound"))?;
        if *seen != epoch {
            if self.queue.len() == self.queue.capacity() {
                return Err(E::Resource(AssertOriginResourceV1::Accounting));
            }
            charge(budget, 2)?;
            *seen = epoch;
            self.queue.push(state);
        }
        Ok(())
    }

    fn prove(
        &mut self,
        inventory: &CanonicalKirInventoryV1<'_>,
        block: usize,
        definition: usize,
        epoch: usize,
        budget: &mut AssertOriginBudgetV1<'_>,
    ) -> R<bool> {
        charge(budget, 3)?;
        self.queue.clear();
        let function =
            &inventory.functions()[inventory.blocks()[block].coordinate.function.0 as usize];
        self.enqueue(function, block, definition, epoch, budget)?;
        let mut head = 0;
        while head < self.queue.len() {
            charge(budget, 5)?;
            let state = self.queue[head];
            head += 1;
            let block = function.blocks.start + state / function.definitions.len();
            let definition = function.definitions.start + state % function.definitions.len();
            let definition = normalized(self.facts, definition, budget)?;
            match immediate(inventory, self.facts, block, definition, budget)? {
                Some(true) => continue,
                Some(false) => return Ok(false),
                None => {}
            }
            let coordinate = inventory.blocks()[block].coordinate;
            let parameter = match inventory.definitions()[definition].coordinate {
                Definition::BlockArgument {
                    block: owner,
                    argument,
                } if owner == coordinate => Some(argument as usize),
                _ => None,
            };
            let mut predecessor = false;
            for index in self.offsets[block]..self.offsets[block + 1] {
                charge(budget, 5)?;
                let edge = self.incoming[index];
                let row = &inventory.edges()[edge];
                let source = block_index(inventory, row.coordinate.source)?;
                if self.reachable[source] == 0 {
                    continue;
                }
                predecessor = true;
                // Substitute this edge occurrence before inspecting its guard;
                // the old value of a loop parameter is not its incoming value.
                let incoming = if let Some(parameter) = parameter {
                    charge(budget, 3)?;
                    let index = row
                        .bindings
                        .start
                        .checked_add(parameter)
                        .ok_or_else(arithmetic)?;
                    if !row.bindings.contains(&index) {
                        return Err(refused("unsigned division", "exact edge argument"));
                    }
                    let binding = &inventory.edge_arguments()[index];
                    if binding.target_definition != definition {
                        return Err(refused("unsigned division", "exact target parameter"));
                    }
                    binding.incoming_definition
                } else {
                    definition
                };
                let incoming = normalized(self.facts, incoming, budget)?;
                if guarded_edge(inventory, self.facts, edge, incoming, budget)? {
                    continue;
                }
                self.enqueue(function, source, incoming, epoch, budget)?;
            }
            if !predecessor {
                return Ok(false);
            }
        }
        // A revisit only deduplicates work. Success requires exhausting every
        // backward path, including every structurally reachable entry path.
        Ok(true)
    }
}

pub(super) fn check<'a, 'g>(
    inventory: &'a CanonicalKirInventoryV1<'g>,
    target: SemanticTargetDataLayoutV1,
    budget: &mut AssertOriginBudgetV1<'_>,
) -> R<UnsignedDivision<'a, 'g>> {
    charge(budget, 2)?;
    match target.architecture() {
        SemanticTargetArchitectureV1::AmdGpuGfx942 => {}
    }
    budget
        .reserve_storage(
            std::mem::size_of::<&CanonicalKirInventoryV1<'_>>()
                .checked_add(std::mem::size_of::<SemanticTargetDataLayoutV1>())
                .ok_or_else(arithmetic)?,
        )
        .map_err(E::Resource)?;
    let mut operations = scratch::<u8>(inventory.operations().len(), budget)?;
    charge(budget, inventory.operations().len())?;
    operations.resize(inventory.operations().len(), 0);
    let mut candidates = 0usize;
    for (ordinal, flag) in operations.iter_mut().enumerate() {
        charge(budget, 8)?;
        if divisor(inventory, ordinal).is_some() {
            *flag = 2;
            candidates = candidates.checked_add(1).ok_or_else(arithmetic)?;
        }
    }
    if candidates == 0 {
        return Ok(UnsignedDivision {
            inventory,
            target,
            operations,
        });
    }

    let mut facts = scratch::<Fact>(inventory.definitions().len(), budget)?;
    charge(budget, inventory.definitions().len())?;
    facts.resize(inventory.definitions().len(), Fact::Unknown);
    for row in inventory.operations() {
        charge(budget, 5)?;
        if row.results.len() != 1 {
            continue;
        }
        let fact = match &row.operation.kind {
            OperationKind::Constant(value) => literal(value).map(Fact::Literal),
            OperationKind::Cast { kind, to, .. } if row.operands.len() == 1 => {
                let input = inventory.uses()[row.operands.start].definition;
                let from = inventory.definitions()[input].ty;
                let preserving = matches!(
                    (kind, from, to),
                    (
                        CastKind::ZeroExtend,
                        Type::Scalar(ScalarType::U32),
                        Type::Scalar(ScalarType::U64 | ScalarType::Index)
                    ) | (
                        CastKind::Bitcast,
                        Type::Scalar(ScalarType::U64),
                        Type::Scalar(ScalarType::Index)
                    ) | (
                        CastKind::Bitcast,
                        Type::Scalar(ScalarType::Index),
                        Type::Scalar(ScalarType::U64)
                    )
                );
                preserving.then_some(Fact::Alias(input))
            }
            OperationKind::Compare { predicate, .. } if row.operands.len() == 2 => {
                let lhs = inventory.uses()[row.operands.start].definition;
                let rhs = inventory.uses()[row.operands.start + 1].definition;
                (unsigned(inventory.definitions()[lhs].ty)
                    && inventory.definitions()[lhs].ty == inventory.definitions()[rhs].ty)
                    .then_some(Fact::Compare {
                        predicate: *predicate,
                        lhs,
                        rhs,
                    })
            }
            _ => None,
        };
        if let Some(fact) = fact {
            facts[row.results.start] = fact
        }
    }

    let blocks = inventory.blocks().len();
    let mut offsets = scratch::<usize>(blocks.checked_add(1).ok_or_else(arithmetic)?, budget)?;
    let mut cursor = scratch::<usize>(blocks, budget)?;
    let mut incoming = scratch::<usize>(inventory.edges().len(), budget)?;
    let mut reachable = scratch::<u8>(blocks, budget)?;
    let mut reach_queue = scratch::<usize>(blocks, budget)?;
    charge(
        budget,
        blocks
            .checked_mul(3)
            .and_then(|n| n.checked_add(1))
            .and_then(|n| n.checked_add(inventory.edges().len()))
            .ok_or_else(arithmetic)?,
    )?;
    offsets.resize(blocks + 1, 0);
    cursor.resize(blocks, 0);
    incoming.resize(inventory.edges().len(), 0);
    reachable.resize(blocks, 0);
    for edge in inventory.edges() {
        charge(budget, 3)?;
        let target = block_index(inventory, edge.target)?;
        offsets[target + 1] = offsets[target + 1].checked_add(1).ok_or_else(arithmetic)?;
    }
    for block in 0..blocks {
        charge(budget, 3)?;
        offsets[block + 1] = offsets[block + 1]
            .checked_add(offsets[block])
            .ok_or_else(arithmetic)?;
        cursor[block] = offsets[block];
    }
    for (ordinal, edge) in inventory.edges().iter().enumerate() {
        charge(budget, 3)?;
        let target = block_index(inventory, edge.target)?;
        incoming[cursor[target]] = ordinal;
        cursor[target] = cursor[target].checked_add(1).ok_or_else(arithmetic)?;
    }
    for function in inventory.functions() {
        charge(budget, 2)?;
        if function.blocks.is_empty() {
            continue;
        }
        reachable[function.blocks.start] = 1;
        reach_queue.push(function.blocks.start);
    }
    let mut head = 0;
    while head < reach_queue.len() {
        charge(budget, 2)?;
        let block = reach_queue[head];
        head += 1;
        for edge in inventory.blocks()[block].edges.clone() {
            charge(budget, 4)?;
            let target = block_index(inventory, inventory.edges()[edge].target)?;
            if reachable[target] == 0 {
                reachable[target] = 1;
                reach_queue.push(target);
            }
        }
    }

    // Constant, unreachable, and entry-local cases never allocate a dense
    // search space. With dynamic candidates its worst-case size is the maximum
    // per-function B*D, not a product across unrelated functions.
    let mut max_states = 0;
    for (ordinal, flag) in operations.iter_mut().enumerate() {
        charge(budget, 2)?;
        if *flag != 2 {
            continue;
        }
        let row = &inventory.operations()[ordinal];
        let block = block_index(inventory, row.coordinate.block)?;
        if reachable[block] == 0 {
            *flag = 0;
            continue;
        }
        let definition = normalized(
            &facts,
            divisor(inventory, ordinal)
                .ok_or_else(|| refused("unsigned division", "candidate operation"))?,
            budget,
        )?;
        if let Some(proven) = immediate(inventory, &facts, block, definition, budget)? {
            *flag = u8::from(proven);
        } else {
            let function = &inventory.functions()[row.coordinate.block.function.0 as usize];
            max_states = max_states.max(
                function
                    .blocks
                    .len()
                    .checked_mul(function.definitions.len())
                    .ok_or_else(arithmetic)?,
            );
        }
    }
    if max_states != 0 {
        let mut seen = scratch::<usize>(max_states, budget)?;
        let mut queue = scratch::<usize>(max_states, budget)?;
        charge(budget, max_states)?;
        seen.resize(max_states, 0);
        let mut search = Search {
            facts: &facts,
            incoming: &incoming,
            offsets: &offsets,
            reachable: &reachable,
            seen: &mut seen,
            queue: &mut queue,
        };
        for (ordinal, flag) in operations.iter_mut().enumerate() {
            charge(budget, 2)?;
            if *flag != 2 {
                continue;
            }
            let row = &inventory.operations()[ordinal];
            let block = block_index(inventory, row.coordinate.block)?;
            let definition = divisor(inventory, ordinal)
                .ok_or_else(|| refused("unsigned division", "candidate operation"))?;
            *flag = u8::from(search.prove(
                inventory,
                block,
                definition,
                ordinal.checked_add(1).ok_or_else(arithmetic)?,
                budget,
            )?);
        }
    }
    Ok(UnsignedDivision {
        inventory,
        target,
        operations,
    })
}

#[cfg(test)]
#[path = "production_checked_output_unsigned_division_v1_tests.rs"]
mod tests;
