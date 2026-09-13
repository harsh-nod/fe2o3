//! Bounded sparse constant propagation over an exact borrowed canonical graph.
//! This is an analysis, not a rewrite, trap-freedom proof or production authority.

use crate::{
    CanonicalKirInventoryV1,
    canonical_kir_sparse_scalar_v1::{self as scalar, Input},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirUseCoordinateV1 as Use, ScalarType, Terminator,
};
use std::{error::Error, fmt, mem::size_of};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirSparseConstantV1 {
    pub(crate) ty: ScalarType,
    pub(crate) bits: u128,
}
impl CanonicalKirSparseConstantV1 {
    pub const fn ty(self) -> ScalarType {
        self.ty
    }
    /// Fixed-width two's-complement bits, or an un-interpreted Index literal.
    pub const fn bits(self) -> u128 {
        self.bits
    }
}

/// Unreachable is a dormant graph definition, not a semantic dead-code proof.
/// Unknown is executable but unresolved; Dynamic is overdefined/unsupported.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirSparseValueV1 {
    Unreachable,
    Unknown,
    Constant(CanonicalKirSparseConstantV1),
    Dynamic,
}
impl CanonicalKirSparseValueV1 {
    pub(crate) fn join(self, other: Self) -> Self {
        use CanonicalKirSparseValueV1 as V;
        match (self, other) {
            (V::Dynamic, _) | (_, V::Dynamic) => V::Dynamic,
            (V::Unreachable, value) | (value, V::Unreachable) => value,
            (V::Unknown, value) | (value, V::Unknown) => value,
            (V::Constant(a), V::Constant(b)) if a == b => V::Constant(a),
            (V::Constant(_), V::Constant(_)) => V::Dynamic,
        }
    }
}

/// Never reports trap freedom. The second state records an exceptional
/// constant operand combination seen during propagation, not a guaranteed trap.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirSparseExceptionV1 {
    NotAnalyzed,
    ExceptionalOperandsObserved,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirSparseResourceV1 {
    Functions,
    Definitions,
    Uses,
    Blocks,
    Operations,
    Edges,
    Worklist,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirSparseLimitsV1 {
    pub functions: usize,
    pub definitions: usize,
    pub uses: usize,
    pub blocks: usize,
    pub operations: usize,
    pub edges: usize,
    pub worklist: usize,
}
impl Default for CanonicalKirSparseLimitsV1 {
    fn default() -> Self {
        Self {
            functions: 16_384,
            definitions: 65_536,
            uses: 262_144,
            blocks: 65_536,
            operations: 65_536,
            edges: 262_144,
            worklist: 131_072,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirSparseErrorV1 {
    Resource(Resource),
    InputLimit {
        resource: CanonicalKirSparseResourceV1,
        actual: usize,
        limit: usize,
    },
    InconsistentInventory,
    /// A locator does not name an exact use in this report's borrowed graph.
    InvalidUseCoordinate {
        coordinate: Use,
    },
}
impl From<Resource> for CanonicalKirSparseErrorV1 {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for CanonicalKirSparseErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::InputLimit {
                resource,
                actual,
                limit,
            } => write!(
                formatter,
                "canonical sparse {resource:?} count {actual} exceeds {limit}"
            ),
            Self::InconsistentInventory => {
                formatter.write_str("inconsistent canonical sparse inventory")
            }
            Self::InvalidUseCoordinate { coordinate } => {
                write!(
                    formatter,
                    "invalid canonical sparse use coordinate {coordinate:?}"
                )
            }
        }
    }
}
impl Error for CanonicalKirSparseErrorV1 {}
type Result<T> = std::result::Result<T, CanonicalKirSparseErrorV1>;
type Value = CanonicalKirSparseValueV1;

/// Explicit logical payload transfer. Reserve before another ledger-controlled
/// allocation while the report lives; release only after dropping the report.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirSparseStorageV1 {
    retained: usize,
}
impl CanonicalKirSparseStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}

#[derive(Debug)]
pub struct CanonicalKirSparseV1<'i, 'g> {
    inventory: &'i CanonicalKirInventoryV1<'g>,
    values: Vec<Value>,
    blocks: Vec<u8>,
    edges: Vec<u8>,
    exceptions: Vec<CanonicalKirSparseExceptionV1>,
    retained: usize,
}
impl<'i, 'g> CanonicalKirSparseV1<'i, 'g> {
    /// Analyze every defined function once, with dynamic signature inputs.
    /// Calls do not specialize helpers or import return summaries.
    ///
    /// The caller retains all source/owner/inventory floors. Construction
    /// charges before visits, allocations, queue operations and publications.
    /// Errors drop candidate owners before restoring the incoming floor.
    /// Success drops scratch and transfers only the returned report payload.
    /// Accepted work, peak and first-failure history remain in the ledger.
    pub fn derive(
        inventory: &'i CanonicalKirInventoryV1<'g>,
        limits: CanonicalKirSparseLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirSparseStorageV1)> {
        let floor = budget.storage();
        let result = Engine::build(inventory, limits, budget).and_then(|engine| engine.run(budget));
        let release = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?;
        budget.release_storage(release)?;
        result.map(|report| {
            let retained = report.retained;
            (report, CanonicalKirSparseStorageV1 { retained })
        })
    }
    pub const fn inventory(&self) -> &'i CanonicalKirInventoryV1<'g> {
        self.inventory
    }
    pub fn belongs_to(&self, inventory: &CanonicalKirInventoryV1<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }
    pub fn values(&self) -> &[Value] {
        &self.values
    }
    pub fn value(&self, definition: usize) -> Option<Value> {
        self.values.get(definition).copied()
    }
    /// Returns the linked definition's lattice value at one exact graph use.
    ///
    /// Coordinates are inert locators in this report's borrowed inventory, not
    /// owner authentication. Invalid locators are errors, never unknown facts.
    /// A returned constant does not establish that this use or edge executes.
    ///
    /// Charges exactly eight work units before any lookup, including invalid
    /// coordinates and the shorter terminator path. The fixed charge admits the
    /// query, function, block, optional operation, use, definition, value and
    /// consistency checks. Allocates nothing and leaves storage unchanged.
    pub fn value_at_use(&self, coordinate: Use, budget: &mut Budget<'_>) -> Result<Value> {
        budget.charge_work(8)?;
        let invalid = || CanonicalKirSparseErrorV1::InvalidUseCoordinate { coordinate };
        let block_coordinate = match coordinate {
            Use::OperationOperand { operation, .. } => operation.block,
            Use::TerminatorOperand { block, .. } => block,
        };
        let inventory = self.inventory;
        let function = inventory
            .functions()
            .get(usize::try_from(block_coordinate.function.0).map_err(|_| invalid())?)
            .filter(|row| row.coordinate == block_coordinate.function)
            .ok_or_else(invalid)?;
        let block_index = use_query_index(
            &function.blocks,
            block_coordinate.block,
            inventory.blocks().len(),
        )
        .ok_or_else(invalid)?;
        let block = inventory.blocks().get(block_index).ok_or_else(invalid)?;
        if block.coordinate != block_coordinate {
            return Err(invalid());
        }
        let (uses, operand) = match coordinate {
            Use::OperationOperand { operation, operand } => {
                if !use_query_subrange(
                    &function.operations,
                    &block.operations,
                    inventory.operations().len(),
                ) {
                    return Err(invalid());
                }
                let operation_index = use_query_index(
                    &block.operations,
                    operation.operation,
                    inventory.operations().len(),
                )
                .ok_or_else(invalid)?;
                let row = inventory
                    .operations()
                    .get(operation_index)
                    .filter(|row| row.coordinate == operation)
                    .ok_or_else(invalid)?;
                (&row.operands, operand)
            }
            Use::TerminatorOperand { operand, .. } => (&block.terminator_uses, operand),
        };
        if !use_query_subrange(&function.uses, uses, inventory.uses().len()) {
            return Err(invalid());
        }
        let use_index =
            use_query_index(uses, operand, inventory.uses().len()).ok_or_else(invalid)?;
        let used = inventory
            .uses()
            .get(use_index)
            .filter(|row| row.coordinate == coordinate)
            .ok_or_else(invalid)?;
        let definition = inventory
            .definitions()
            .get(used.definition)
            .ok_or(CanonicalKirSparseErrorV1::InconsistentInventory)?;
        let definition_function = match definition.coordinate {
            Definition::FunctionArgument { function, .. } => function,
            Definition::BlockArgument { block, .. } => block.function,
            Definition::Result { operation, .. } => operation.block.function,
        };
        if function.definitions.start > function.definitions.end
            || function.definitions.end > inventory.definitions().len()
            || !function.definitions.contains(&used.definition)
            || definition_function != block_coordinate.function
            || definition.value != Some(used.value)
        {
            return Err(CanonicalKirSparseErrorV1::InconsistentInventory);
        }
        let value = self
            .value(used.definition)
            .ok_or(CanonicalKirSparseErrorV1::InconsistentInventory)?;
        if let Value::Constant(constant) = value
            && definition.ty != &fe2o3_kernel_ir::Type::Scalar(constant.ty())
        {
            return Err(CanonicalKirSparseErrorV1::InconsistentInventory);
        }
        Ok(value)
    }
    pub fn block_executable(&self, block: usize) -> Option<bool> {
        self.blocks.get(block).map(|state| *state != 0)
    }
    pub fn edge_executable(&self, edge: usize) -> Option<bool> {
        self.edges.get(edge).map(|state| *state != 0)
    }
    pub fn exception(&self, operation: usize) -> Option<CanonicalKirSparseExceptionV1> {
        self.exceptions.get(operation).copied()
    }
}

fn use_query_index(
    range: &std::ops::Range<usize>,
    ordinal: u32,
    roster_len: usize,
) -> Option<usize> {
    if range.start > range.end || range.end > roster_len {
        return None;
    }
    let index = range.start.checked_add(usize::try_from(ordinal).ok()?)?;
    (index < range.end).then_some(index)
}

fn use_query_subrange(
    parent: &std::ops::Range<usize>,
    child: &std::ops::Range<usize>,
    roster_len: usize,
) -> bool {
    parent.start <= child.start
        && child.start <= child.end
        && child.end <= parent.end
        && parent.end <= roster_len
}

struct Engine<'i, 'g> {
    report: CanonicalKirSparseV1<'i, 'g>,
    heads: Vec<usize>,
    next: Vec<usize>,
    queued: Vec<u8>,
    queue: Vec<usize>,
    queue_head: usize,
    queue_tail: usize,
    queue_len: usize,
    unresolved: Vec<usize>,
    unresolved_len: usize,
    unresolved_cursor: usize,
}
impl<'i, 'g> Engine<'i, 'g> {
    fn build(
        inventory: &'i CanonicalKirInventoryV1<'g>,
        limits: CanonicalKirSparseLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        budget.charge_work(1)?;
        let definitions = inventory.definitions().len();
        let uses = inventory.uses().len();
        let blocks = inventory.blocks().len();
        let operations = inventory.operations().len();
        let edges = inventory.edges().len();
        let tasks = blocks.checked_add(operations).ok_or(Resource::Arithmetic)?;
        use CanonicalKirSparseResourceV1 as R;
        for (resource, actual, limit) in [
            (R::Functions, inventory.functions().len(), limits.functions),
            (R::Definitions, definitions, limits.definitions),
            (R::Uses, uses, limits.uses),
            (R::Blocks, blocks, limits.blocks),
            (R::Operations, operations, limits.operations),
            (R::Edges, edges, limits.edges),
            (R::Worklist, tasks, limits.worklist),
        ] {
            budget.charge_work(1)?;
            if actual > limit {
                return Err(CanonicalKirSparseErrorV1::InputLimit {
                    resource,
                    actual,
                    limit,
                });
            }
        }
        let retained = size_of::<CanonicalKirSparseV1<'_, '_>>()
            .checked_add(bytes::<Value>(definitions)?)
            .and_then(|n| n.checked_add(blocks))
            .and_then(|n| n.checked_add(edges))
            .and_then(|n| {
                n.checked_add(operations.checked_mul(size_of::<CanonicalKirSparseExceptionV1>())?)
            })
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(size_of::<CanonicalKirSparseV1<'_, '_>>())?;
        Ok(Self {
            report: CanonicalKirSparseV1 {
                inventory,
                values: allocate(definitions, Value::Unreachable, budget)?,
                blocks: allocate(blocks, 0, budget)?,
                edges: allocate(edges, 0, budget)?,
                exceptions: allocate(
                    operations,
                    CanonicalKirSparseExceptionV1::NotAnalyzed,
                    budget,
                )?,
                retained,
            },
            heads: allocate(definitions, usize::MAX, budget)?,
            next: allocate(uses, usize::MAX, budget)?,
            queued: allocate(tasks, 0, budget)?,
            queue: allocate(tasks, 0, budget)?,
            queue_head: 0,
            queue_tail: 0,
            queue_len: 0,
            unresolved: allocate(definitions, 0, budget)?,
            unresolved_len: 0,
            unresolved_cursor: 0,
        })
    }

    fn run(mut self, budget: &mut Budget<'_>) -> Result<CanonicalKirSparseV1<'i, 'g>> {
        let inventory = self.report.inventory;
        for (index, operand) in inventory.uses().iter().enumerate() {
            budget.charge_work(1)?;
            let head = self
                .heads
                .get_mut(operand.definition)
                .ok_or(CanonicalKirSparseErrorV1::InconsistentInventory)?;
            self.next[index] = *head;
            *head = index;
        }
        for (index, definition) in inventory.definitions().iter().enumerate() {
            budget.charge_work(1)?;
            if matches!(definition.coordinate, Definition::FunctionArgument { .. }) {
                self.report.values[index] = Value::Dynamic;
            }
        }
        for function in inventory.functions() {
            budget.charge_work(1)?;
            if !function.blocks.is_empty() {
                self.activate_block(function.blocks.start, budget)?;
            }
        }
        loop {
            while self.queue_len != 0 {
                let task = self.dequeue(budget)?;
                if task < inventory.operations().len() {
                    self.operation(task, budget)?;
                } else {
                    self.terminator(task - inventory.operations().len(), budget)?;
                }
            }
            // Close unresolved executable cycles conservatively. Each activated
            // definition enters this list once; no repeated whole-graph scan.
            while self.unresolved_cursor < self.unresolved_len {
                budget.charge_work(1)?;
                let definition = self.unresolved[self.unresolved_cursor];
                self.unresolved_cursor += 1;
                if self.report.values[definition] == Value::Unknown {
                    self.publish(definition, Value::Dynamic, budget)?;
                }
            }
            if self.queue_len == 0 {
                break;
            }
        }
        Ok(self.report)
    }

    fn block_index(&self, coordinate: Block) -> Result<usize> {
        let function = self
            .report
            .inventory
            .functions()
            .get(coordinate.function.0 as usize)
            .ok_or(CanonicalKirSparseErrorV1::InconsistentInventory)?;
        let index = function
            .blocks
            .start
            .checked_add(coordinate.block as usize)
            .ok_or(Resource::Arithmetic)?;
        if index >= function.blocks.end {
            return Err(CanonicalKirSparseErrorV1::InconsistentInventory);
        }
        Ok(index)
    }
    fn task_for_use(&self, coordinate: Use) -> Result<usize> {
        match coordinate {
            Use::OperationOperand { operation, .. } => {
                let block = &self.report.inventory.blocks()[self.block_index(operation.block)?];
                let index = block
                    .operations
                    .start
                    .checked_add(operation.operation as usize)
                    .ok_or(Resource::Arithmetic)?;
                if index >= block.operations.end {
                    return Err(CanonicalKirSparseErrorV1::InconsistentInventory);
                }
                Ok(index)
            }
            Use::TerminatorOperand { block, .. } => self
                .report
                .inventory
                .operations()
                .len()
                .checked_add(self.block_index(block)?)
                .ok_or(Resource::Arithmetic.into()),
        }
    }
    fn enqueue(&mut self, task: usize, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        let queued = self
            .queued
            .get_mut(task)
            .ok_or(CanonicalKirSparseErrorV1::InconsistentInventory)?;
        if *queued != 0 {
            return Ok(());
        }
        if self.queue_len >= self.queue.len() {
            return Err(CanonicalKirSparseErrorV1::InconsistentInventory);
        }
        self.queue[self.queue_tail] = task;
        self.queue_tail = if self.queue_tail + 1 == self.queue.len() {
            0
        } else {
            self.queue_tail + 1
        };
        self.queue_len += 1;
        *queued = 1;
        Ok(())
    }
    fn dequeue(&mut self, budget: &mut Budget<'_>) -> Result<usize> {
        budget.charge_work(1)?;
        if self.queue_len == 0 {
            return Err(CanonicalKirSparseErrorV1::InconsistentInventory);
        }
        let task = self.queue[self.queue_head];
        self.queue_head = if self.queue_head + 1 == self.queue.len() {
            0
        } else {
            self.queue_head + 1
        };
        self.queue_len -= 1;
        self.queued[task] = 0;
        Ok(task)
    }
    fn activate_definition(&mut self, definition: usize, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        let value = self
            .report
            .values
            .get_mut(definition)
            .ok_or(CanonicalKirSparseErrorV1::InconsistentInventory)?;
        if *value != Value::Unreachable {
            return Err(CanonicalKirSparseErrorV1::InconsistentInventory);
        }
        if self.unresolved_len >= self.unresolved.len() {
            return Err(CanonicalKirSparseErrorV1::InconsistentInventory);
        }
        *value = Value::Unknown;
        self.unresolved[self.unresolved_len] = definition;
        self.unresolved_len += 1;
        Ok(())
    }
    fn activate_block(&mut self, index: usize, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        let state = self
            .report
            .blocks
            .get_mut(index)
            .ok_or(CanonicalKirSparseErrorV1::InconsistentInventory)?;
        if *state != 0 {
            return Ok(());
        }
        *state = 1;
        let inventory = self.report.inventory;
        let block = &inventory.blocks()[index];
        for definition in block.parameters.clone() {
            self.activate_definition(definition, budget)?;
        }
        for operation in block.operations.clone() {
            budget.charge_work(1)?;
            for result in inventory.operations()[operation].results.clone() {
                self.activate_definition(result, budget)?;
            }
            self.enqueue(operation, budget)?;
        }
        self.enqueue(
            inventory
                .operations()
                .len()
                .checked_add(index)
                .ok_or(Resource::Arithmetic)?,
            budget,
        )
    }
    fn publish(&mut self, definition: usize, value: Value, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        let slot = self
            .report
            .values
            .get_mut(definition)
            .ok_or(CanonicalKirSparseErrorV1::InconsistentInventory)?;
        let next = slot.join(value);
        if next == *slot {
            return Ok(());
        }
        *slot = next;
        let mut use_index = self.heads[definition];
        while use_index != usize::MAX {
            budget.charge_work(1)?;
            let operand = &self.report.inventory.uses()[use_index];
            let task = self.task_for_use(operand.coordinate)?;
            self.enqueue(task, budget)?;
            use_index = self.next[use_index];
        }
        Ok(())
    }
    fn operation(&mut self, index: usize, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        let inventory = self.report.inventory;
        let operation = &inventory.operations()[index];
        if self.report.blocks[self.block_index(operation.coordinate.block)?] == 0 {
            return Ok(());
        }
        let mut inputs = [Input {
            value: Value::Unknown,
            ty: None,
        }; 3];
        for (input, operand) in inputs
            .iter_mut()
            .zip(&inventory.uses()[operation.operands.clone()])
        {
            budget.charge_work(1)?;
            input.value = self.report.values[operand.definition];
            input.ty = inventory.definitions()[operand.definition].ty.as_scalar();
        }
        let result_type = operation
            .results
            .clone()
            .next()
            .and_then(|result| inventory.definitions()[result].ty.as_scalar());
        // Transfer is fixed-size scalar work and calls only allocation-free
        // supported paths through the shared exact-width scalar evaluators.
        budget.charge_work(1)?;
        let transfer = scalar::transfer(&operation.operation.kind, result_type, inputs);
        if transfer.exceptional {
            self.report.exceptions[index] =
                CanonicalKirSparseExceptionV1::ExceptionalOperandsObserved;
        }
        for (ordinal, result) in operation.results.clone().enumerate() {
            self.publish(
                result,
                transfer
                    .values
                    .get(ordinal)
                    .copied()
                    .unwrap_or(Value::Dynamic),
                budget,
            )?;
        }
        Ok(())
    }
    fn terminator(&mut self, index: usize, budget: &mut Budget<'_>) -> Result<()> {
        budget.charge_work(1)?;
        if self.report.blocks[index] == 0 {
            return Ok(());
        }
        let inventory = self.report.inventory;
        let block = &inventory.blocks()[index];
        let selector = block
            .terminator_uses
            .clone()
            .next()
            .map(|operand| self.report.values[inventory.uses()[operand].definition]);
        let choice = match block.terminator {
            Terminator::Branch { .. } => Choice::All,
            Terminator::ConditionalBranch { .. } => match selector {
                Some(Value::Unreachable | Value::Unknown) => Choice::Wait,
                Some(Value::Constant(value)) if value.ty == ScalarType::Bool && value.bits <= 1 => {
                    Choice::One(usize::from(value.bits == 0))
                }
                Some(Value::Constant(_) | Value::Dynamic) | None => Choice::All,
            },
            Terminator::IntegerSwitch { cases, .. } => match selector {
                Some(Value::Unreachable | Value::Unknown) => Choice::Wait,
                Some(Value::Constant(value)) if scalar::fixed(value.ty).is_some() => {
                    let mut selected = cases.len();
                    for (ordinal, case) in cases.iter().enumerate() {
                        budget.charge_work(1)?;
                        if scalar::literal(&case.value) == Some(value) {
                            selected = ordinal;
                            break;
                        }
                    }
                    Choice::One(selected)
                }
                Some(Value::Constant(_) | Value::Dynamic) | None => Choice::All,
            },
            // Legacy switch uses the width-unspecified Index domain. No
            // target-width or case representability premise is fabricated.
            Terminator::Switch { .. } => Choice::All,
            Terminator::Return { .. } | Terminator::Unreachable => Choice::Wait,
        };
        for (ordinal, edge_index) in block.edges.clone().enumerate() {
            budget.charge_work(1)?;
            if matches!(choice, Choice::Wait)
                || matches!(choice, Choice::One(selected) if selected != ordinal)
            {
                continue;
            }
            let edge = &inventory.edges()[edge_index];
            if self.report.edges[edge_index] == 0 {
                self.report.edges[edge_index] = 1;
                self.activate_block(self.block_index(edge.target)?, budget)?;
            }
            for binding in &inventory.edge_arguments()[edge.bindings.clone()] {
                budget.charge_work(1)?;
                self.publish(
                    binding.target_definition,
                    self.report.values[binding.incoming_definition],
                    budget,
                )?;
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum Choice {
    Wait,
    All,
    One(usize),
}

fn bytes<T>(count: usize) -> Result<usize> {
    count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic.into())
}
fn allocate<T: Copy>(count: usize, initial: T, budget: &mut Budget<'_>) -> Result<Vec<T>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    budget.charge_work(1)?;
    budget.reserve_storage(bytes::<T>(count)?)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    for _ in 0..count {
        budget.charge_work(1)?;
        if values.len() >= count || values.len() == values.capacity() {
            return Err(CanonicalKirSparseErrorV1::InconsistentInventory);
        }
        values.push(initial);
    }
    Ok(values)
}

#[cfg(test)]
#[path = "canonical_kir_sparse_v1_tests.rs"]
mod tests;
