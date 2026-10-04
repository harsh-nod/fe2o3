//! Complete coordinate replay between the actual checked LICM endpoints.
//! The enclosing source owner retains both endpoints and all returned credit.

use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV18 as Inventory, CanonicalKirMemorySsaErrorV1 as MemoryError,
    CanonicalKirMemorySsaInputSourceV1 as MemoryInput, CanonicalKirMemorySsaNodeV1 as MemoryNode,
    CanonicalKirMemorySsaV18 as Memory, CheckedCanonicalKirLicmV18 as Pair,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirOperationCoordinateV1 as Operation, CanonicalKirUseCoordinateV1 as Use,
};
use std::mem::{align_of, size_of};

#[derive(Debug)]
pub(super) enum Error {
    Resource(Resource),
    Memory(MemoryError),
    Mismatch(&'static str),
}
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl From<MemoryError> for Error {
    fn from(error: MemoryError) -> Self {
        Self::Memory(error)
    }
}
type Result<T> = std::result::Result<T, Error>;

/// Descriptive complete definition relocation; no source or native authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProductionMixedLicmDefinitionProjectionV28 {
    /// Definition coordinate in the genuine Policy10 prefix output.
    pub input: Definition,
    /// Corresponding definition coordinate in the checked LICM output.
    pub output: Definition,
}
type Row = ProductionMixedLicmDefinitionProjectionV28;

pub(super) struct Projection {
    rows: Vec<Row>,
}

fn block_index(inventory: &Inventory<'_>, coordinate: Block) -> Result<usize> {
    let function = inventory
        .functions()
        .get(coordinate.function.0 as usize)
        .ok_or(Error::Mismatch("LICM projection function"))?;
    let index = function
        .blocks
        .start
        .checked_add(coordinate.block as usize)
        .filter(|index| *index < function.blocks.end)
        .ok_or(Error::Mismatch("LICM projection block"))?;
    if inventory.blocks()[index].coordinate != coordinate {
        return Err(Error::Mismatch("LICM projection block coordinate"));
    }
    Ok(index)
}

pub(super) fn operation_index(inventory: &Inventory<'_>, coordinate: Operation) -> Result<usize> {
    let block = &inventory.blocks()[block_index(inventory, coordinate.block)?];
    let index = block
        .operations
        .start
        .checked_add(coordinate.operation as usize)
        .filter(|index| *index < block.operations.end)
        .ok_or(Error::Mismatch("LICM projection operation"))?;
    if inventory.operations()[index].coordinate != coordinate {
        return Err(Error::Mismatch("LICM projection operation coordinate"));
    }
    Ok(index)
}

pub(super) fn definition_index(inventory: &Inventory<'_>, coordinate: Definition) -> Result<usize> {
    let (range, ordinal) = match coordinate {
        Definition::FunctionArgument { function, argument } => {
            let function = inventory
                .functions()
                .get(function.0 as usize)
                .ok_or(Error::Mismatch("LICM projection argument function"))?;
            let end = function
                .definitions
                .start
                .checked_add(function.function.signature.parameters.len())
                .ok_or(Resource::Arithmetic)?;
            (function.definitions.start..end, argument)
        }
        Definition::BlockArgument { block, argument } => (
            inventory.blocks()[block_index(inventory, block)?]
                .parameters
                .clone(),
            argument,
        ),
        Definition::Result { operation, result } => (
            inventory.operations()[operation_index(inventory, operation)?]
                .results
                .clone(),
            result,
        ),
    };
    let index = range
        .start
        .checked_add(ordinal as usize)
        .filter(|index| *index < range.end)
        .ok_or(Error::Mismatch("LICM projection definition"))?;
    if inventory.definitions().get(index).map(|row| row.coordinate) != Some(coordinate) {
        return Err(Error::Mismatch("LICM projection definition coordinate"));
    }
    Ok(index)
}

fn use_index(inventory: &Inventory<'_>, coordinate: Use) -> Result<usize> {
    let (range, ordinal) = match coordinate {
        Use::OperationOperand { operation, operand } => (
            inventory.operations()[operation_index(inventory, operation)?]
                .operands
                .clone(),
            operand,
        ),
        Use::TerminatorOperand { block, operand } => (
            inventory.blocks()[block_index(inventory, block)?]
                .terminator_uses
                .clone(),
            operand,
        ),
    };
    let index = range
        .start
        .checked_add(ordinal as usize)
        .filter(|index| *index < range.end)
        .ok_or(Error::Mismatch("LICM projection operand"))?;
    if inventory.uses()[index].coordinate != coordinate {
        return Err(Error::Mismatch("LICM projection operand coordinate"));
    }
    Ok(index)
}

pub(super) fn operation(
    pair: &Pair<'_>,
    input: &Inventory<'_>,
    coordinate: Operation,
    budget: &mut Budget<'_>,
) -> Result<Operation> {
    budget.charge_work(10)?;
    let ordinal = operation_index(input, coordinate)?;
    let row = pair
        .origins()
        .get(ordinal)
        .filter(|row| row.input == coordinate)
        .ok_or(Error::Mismatch("LICM complete operation projection"))?;
    Ok(row.output)
}

pub(super) fn expected_definition(
    pair: &Pair<'_>,
    input: &Inventory<'_>,
    coordinate: Definition,
    budget: &mut Budget<'_>,
) -> Result<Definition> {
    budget.charge_work(2)?;
    Ok(match coordinate {
        Definition::Result {
            operation: at,
            result,
        } => Definition::Result {
            operation: operation(pair, input, at, budget)?,
            result,
        },
        Definition::FunctionArgument { .. } | Definition::BlockArgument { .. } => coordinate,
    })
}

impl Projection {
    /// Caller construction scratch owns all growth on refusal. The returned
    /// exact row allocation remains reserved until this projection is dropped.
    pub(super) fn build(
        pair: &Pair<'_>,
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(10)?;
        if !input.belongs_to(pair.input()) || !output.belongs_to(pair.output()) {
            return Err(Error::Mismatch("LICM projection substituted an endpoint"));
        }
        let count = input.definitions().len();
        budget.reserve_storage(size_of::<Self>() + align_of::<Self>())?;
        budget.reserve_storage(
            count
                .checked_mul(size_of::<Row>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut rows = Vec::new();
        rows.try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        let excess = rows
            .capacity()
            .checked_sub(count)
            .ok_or(Resource::Accounting)?;
        budget.reserve_storage(
            excess
                .checked_mul(size_of::<Row>())
                .ok_or(Resource::Arithmetic)?,
        )?;
        for row in input.definitions() {
            budget.charge_work(2)?;
            rows.push(Row {
                input: row.coordinate,
                output: expected_definition(pair, input, row.coordinate, budget)?,
            });
        }
        let projection = Self { rows };
        projection.replay(pair, input, output, budget)?;
        Ok(projection)
    }

    pub(super) fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub(super) fn replay(
        &self,
        pair: &Pair<'_>,
        input: &Inventory<'_>,
        output: &Inventory<'_>,
        budget: &mut Budget<'_>,
    ) -> Result<()> {
        budget.charge_work(16)?;
        if !input.belongs_to(pair.input())
            || !output.belongs_to(pair.output())
            || self.rows.len() != input.definitions().len()
            || input.definitions().len() != output.definitions().len()
            || input.operations().len() != output.operations().len()
            || input.operations().len() != pair.origins().len()
            || input.uses().len() != output.uses().len()
            || input.edges().len() != output.edges().len()
            || input.edge_arguments().len() != output.edge_arguments().len()
        {
            return Err(Error::Mismatch("LICM complete source projection census"));
        }
        for (original, row) in input.definitions().iter().zip(&self.rows) {
            budget.charge_work(10)?;
            if row.input != original.coordinate
                || row.output != expected_definition(pair, input, original.coordinate, budget)?
            {
                return Err(Error::Mismatch("LICM source definition projection changed"));
            }
            let actual = &output.definitions()[definition_index(output, row.output)?];
            if actual.value != original.value {
                return Err(Error::Mismatch("LICM source definition value changed"));
            }
        }
        for row in input.uses() {
            budget.charge_work(14)?;
            let coordinate = match row.coordinate {
                Use::OperationOperand {
                    operation: at,
                    operand,
                } => Use::OperationOperand {
                    operation: operation(pair, input, at, budget)?,
                    operand,
                },
                Use::TerminatorOperand { .. } => row.coordinate,
            };
            let actual = &output.uses()[use_index(output, coordinate)?];
            if actual.value != row.value
                || output.definitions()[actual.definition].coordinate
                    != self.rows[row.definition].output
            {
                return Err(Error::Mismatch("LICM exact source operand binding changed"));
            }
        }
        for (before, after) in input.edges().iter().zip(output.edges()) {
            budget.charge_work(8)?;
            if before.coordinate != after.coordinate
                || before.target != after.target
                || before.target_id != after.target_id
                || before.bindings.len() != after.bindings.len()
            {
                return Err(Error::Mismatch("LICM source control edge changed"));
            }
        }
        for (before, after) in input.edge_arguments().iter().zip(output.edge_arguments()) {
            budget.charge_work(10)?;
            if before.coordinate != after.coordinate
                || before.value != after.value
                || self.rows[before.incoming_definition].output
                    != output.definitions()[after.incoming_definition].coordinate
                || self.rows[before.target_definition].output
                    != output.definitions()[after.target_definition].coordinate
            {
                return Err(Error::Mismatch("LICM source edge argument binding changed"));
            }
        }
        Ok(())
    }
}

/// Independently rebuilt memory versions must agree under the complete motion
/// map. Numeric locators are compared only after exact owner and full node,
/// operation, block-entry/exit and phi-input joins, never as detached authority.
pub(super) fn replay_memory(
    pair: &Pair<'_>,
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    before: &Memory<'_, '_>,
    after: &Memory<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(8)?;
    if !input.belongs_to(pair.input())
        || !output.belongs_to(pair.output())
        || !before.belongs_to(input)
        || !after.belongs_to(output)
        || before.node_count() != after.node_count()
        || input.blocks().len() != output.blocks().len()
    {
        return Err(Error::Mismatch("LICM memory graph endpoint or census"));
    }
    for row in pair.origins() {
        budget.charge_work(4)?;
        let left = before.operation(row.input, budget)?;
        let right = after.operation(row.output, budget)?;
        if left != right {
            return Err(Error::Mismatch("LICM memory occurrence changed"));
        }
        if let (Some(left), Some(right)) = (left, right) {
            let matched = match (before.node(left, budget)?, after.node(right, budget)?) {
                (
                    MemoryNode::Use {
                        operation: a,
                        incoming: x,
                    },
                    MemoryNode::Use {
                        operation: b,
                        incoming: y,
                    },
                )
                | (
                    MemoryNode::Def {
                        operation: a,
                        incoming: x,
                    },
                    MemoryNode::Def {
                        operation: b,
                        incoming: y,
                    },
                ) => *a == row.input && *b == row.output && x == y && row.hoist.is_none(),
                _ => false,
            };
            if !matched {
                return Err(Error::Mismatch(
                    "LICM memory effect or reaching state changed",
                ));
            }
        }
    }
    for (left, right) in input.blocks().iter().zip(output.blocks()) {
        budget.charge_work(6)?;
        if left.coordinate != right.coordinate {
            return Err(Error::Mismatch("LICM memory block changed"));
        }
        let entry = before.block_entry(left.coordinate, budget)?;
        if entry != after.block_entry(right.coordinate, budget)?
            || before.block_exit(left.coordinate, budget)?
                != after.block_exit(right.coordinate, budget)?
        {
            return Err(Error::Mismatch("LICM memory block state changed"));
        }
        match (before.node(entry, budget)?, after.node(entry, budget)?) {
            (MemoryNode::LiveOnEntry { function: a }, MemoryNode::LiveOnEntry { function: b })
                if a == b && *a == left.coordinate.function =>
            {
                ()
            }
            (MemoryNode::Phi { block: a, .. }, MemoryNode::Phi { block: b, .. })
                if a == b && *a == left.coordinate =>
            {
                let a = before.phi_inputs(entry, budget)?;
                let b = after.phi_inputs(entry, budget)?;
                budget.charge_work(1)?;
                if a.len() != b.len() {
                    return Err(Error::Mismatch("LICM memory phi census changed"));
                }
                for (a, b) in a.iter().zip(b) {
                    budget.charge_work(3)?;
                    if a.source() != b.source() || a.state() != b.state() {
                        return Err(Error::Mismatch("LICM memory phi edge changed"));
                    }
                    if let MemoryInput::Entry(function) = a.source() {
                        budget.charge_work(4)?;
                        match (
                            before.node(a.state(), budget)?,
                            after.node(b.state(), budget)?,
                        ) {
                            (
                                MemoryNode::LiveOnEntry { function: a },
                                MemoryNode::LiveOnEntry { function: b },
                            ) if *a == function && a == b => (),
                            _ => return Err(Error::Mismatch("LICM memory live-on-entry changed")),
                        }
                    }
                }
            }
            _ => return Err(Error::Mismatch("LICM memory entry kind changed")),
        }
    }
    Ok(())
}

#[cfg(test)]
pub(super) fn test_refusals(
    pair: &Pair<'_>,
    input: &Inventory<'_>,
    output: &Inventory<'_>,
    before: &Memory<'_, '_>,
    after: &Memory<'_, '_>,
    budget: &mut Budget<'_>,
) -> Result<()> {
    let floor = budget.storage();
    let mut map = Projection::build(pair, input, output, budget)?;
    let credit = budget.storage() - floor;
    assert!(map.rows.len() >= 2);
    let last = map.rows.pop().unwrap();
    assert!(matches!(
        map.replay(pair, input, output, budget),
        Err(Error::Mismatch("LICM complete source projection census"))
    ));
    map.rows.push(last);
    for output_field in [false, true] {
        let saved = map.rows[0];
        if output_field {
            map.rows[0].output = map.rows[1].output;
        } else {
            map.rows[0].input = map.rows[1].input;
        }
        assert!(matches!(
            map.replay(pair, input, output, budget),
            Err(Error::Mismatch("LICM source definition projection changed"))
        ));
        map.rows[0] = saved;
    }
    map.rows.swap(0, 1);
    assert!(matches!(
        map.replay(pair, input, output, budget),
        Err(Error::Mismatch("LICM source definition projection changed"))
    ));
    map.rows.swap(0, 1);
    assert!(matches!(
        map.replay(pair, output, input, budget),
        Err(Error::Mismatch("LICM complete source projection census"))
    ));
    assert!(matches!(
        replay_memory(pair, input, output, after, before, budget),
        Err(Error::Mismatch("LICM memory graph endpoint or census"))
    ));
    map.replay(pair, input, output, budget)?;
    replay_memory(pair, input, output, before, after, budget)?;
    drop(map);
    budget.release_storage(credit)?;
    Ok(())
}
