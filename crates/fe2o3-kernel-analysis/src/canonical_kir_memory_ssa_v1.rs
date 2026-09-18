//! Conservative memory-version graph of one exact immutable canonical owner.
//! One all-memory partition, without alias, initialization, trap or rewrite authority.

use crate::CanonicalKirInventoryV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirFunctionCoordinateV1 as Function, CanonicalKirOperationCoordinateV1 as Operation,
    KirLocalMemoryEffectRefV1, OperationKind,
};
use std::{error::Error as StdError, fmt, mem::size_of, ops::Range};

/// An inert node locator within its report, not a transferable owner identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirMemorySsaNodeIdV1(usize);
impl CanonicalKirMemorySsaNodeIdV1 {
    pub const fn index(self) -> usize {
        self.0
    }
}
use self::CanonicalKirMemorySsaNodeIdV1 as NodeId;

/// Def is a possible clobber or ordering barrier, not a claim of a physical write.
/// A Use observes its incoming version but does not define a new version.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalKirMemorySsaNodeV1 {
    LiveOnEntry {
        function: Function,
    },
    Phi {
        block: Block,
        incoming: Range<usize>,
    },
    Use {
        operation: Operation,
        incoming: NodeId,
    },
    Def {
        operation: Operation,
        incoming: NodeId,
    },
}
type Node = CanonicalKirMemorySsaNodeV1;

/// Entry is explicit even when the entry block also has backedges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirMemorySsaInputSourceV1 {
    Entry(Function),
    Edge(Edge),
}
type InputSource = CanonicalKirMemorySsaInputSourceV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirMemorySsaInputV1 {
    source: InputSource,
    state: NodeId,
}
impl CanonicalKirMemorySsaInputV1 {
    pub const fn source(self) -> InputSource {
        self.source
    }
    pub const fn state(self) -> NodeId {
        self.state
    }
}
type Input = CanonicalKirMemorySsaInputV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirMemorySsaLimitsV1 {
    pub functions: usize,
    pub blocks: usize,
    pub operations: usize,
    pub effects: usize,
    pub edges: usize,
}
impl Default for CanonicalKirMemorySsaLimitsV1 {
    fn default() -> Self {
        Self {
            functions: 16_384,
            blocks: 65_536,
            operations: 65_536,
            effects: 262_144,
            edges: 262_144,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirMemorySsaResourceV1 {
    Functions,
    Blocks,
    Operations,
    Effects,
    Edges,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CanonicalKirMemorySsaErrorV1 {
    Resource(Resource),
    InputLimit {
        resource: CanonicalKirMemorySsaResourceV1,
        actual: usize,
        limit: usize,
    },
    InconsistentInventory,
    InvalidBlock(Block),
    InvalidOperation(Operation),
    InvalidNode(NodeId),
    NotPhi(NodeId),
}
type Error = CanonicalKirMemorySsaErrorV1;
type Result<T> = std::result::Result<T, Error>;
impl From<Resource> for Error {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Resource(error) => error.fmt(formatter),
            Self::InputLimit {
                resource,
                actual,
                limit,
            } => {
                write!(
                    formatter,
                    "memory SSA {resource:?} count {actual} exceeds {limit}"
                )
            }
            Self::InconsistentInventory => formatter.write_str("inconsistent memory SSA inventory"),
            Self::InvalidBlock(block) => write!(formatter, "invalid memory SSA block {block:?}"),
            Self::InvalidOperation(operation) => {
                write!(formatter, "invalid memory SSA operation {operation:?}")
            }
            Self::InvalidNode(node) => {
                write!(formatter, "invalid memory SSA node {}", node.index())
            }
            Self::NotPhi(node) => {
                write!(formatter, "memory SSA node {} is not a phi", node.index())
            }
        }
    }
}
impl StdError for Error {}

/// Actual vector capacities plus the report header, excluding the borrowed
/// owner/inventory, allocator metadata, stack scratch headers and diagnostics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalKirMemorySsaStorageV1(usize);
impl CanonicalKirMemorySsaStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

#[derive(Clone, Copy, Debug)]
struct BlockState {
    entry: NodeId,
    exit: NodeId,
}

/// Borrowed immutable-owner memory SSA with unpruned phis at every block.
/// All syntactic CFG edges are retained; a zero-input phi or disconnected cycle
/// is not an executable-unreachability proof. Calls, assembly, Execution and
/// compiler ordering conservatively define barriers, even with no local effects.
/// Guarded writes may define unchanged memory. No node establishes must-alias,
/// successful execution, initialization, forwarding or legal instruction motion.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{CanonicalKirInventoryV1, CanonicalKirMemorySsaV1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn cannot_drop_inventory(inventory: CanonicalKirInventoryV1<'_>, budget: &mut Budget<'_>) {
///     let (report, _) = CanonicalKirMemorySsaV1::derive(&inventory, Default::default(), budget).unwrap();
///     drop(inventory);
///     let _ = report.node_count();
/// }
/// ```
#[derive(Debug)]
pub struct CanonicalKirMemorySsaV1<'i, 'g> {
    inventory: &'i CanonicalKirInventoryV1<'g>,
    nodes: Vec<Node>,
    inputs: Vec<Input>,
    blocks: Vec<BlockState>,
    operations: Vec<Option<NodeId>>,
    retained: usize,
}

impl<'i, 'g> CanonicalKirMemorySsaV1<'i, 'g> {
    /// O(functions + blocks + operations + effects + edges). Dense stored-order
    /// coordinates avoid sorting, graph rescans, recursive traversal and a
    /// fixed-point iteration. First compute block exits, then link predecessor
    /// versions, so backedges and irreducible components need no special guess.
    ///
    /// Caller retains owner/inventory storage. Requested allocation is prepaid;
    /// actual capacity is reconciled before initialization. Success transfers
    /// only the report receipt; scratch drops before restoring the incoming
    /// floor on every Result exit. Work/peak/first failure are never rewound.
    pub fn derive(
        inventory: &'i CanonicalKirInventoryV1<'g>,
        limits: CanonicalKirMemorySsaLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<(Self, CanonicalKirMemorySsaStorageV1)> {
        let floor = budget.storage();
        let result = Self::build(inventory, limits, budget);
        let release = budget
            .storage()
            .checked_sub(floor)
            .ok_or(Resource::Accounting)?;
        budget.release_storage(release)?;
        result.map(|report| {
            let storage = CanonicalKirMemorySsaStorageV1(report.retained);
            (report, storage)
        })
    }

    pub const fn inventory(&self) -> &'i CanonicalKirInventoryV1<'g> {
        self.inventory
    }
    pub fn belongs_to(&self, inventory: &CanonicalKirInventoryV1<'_>) -> bool {
        std::ptr::eq(self.inventory, inventory)
    }
    pub fn node_count(&self) -> usize {
        self.nodes.len()
    }

    /// Pays eight before lookup, including invalid locators. Pure operations
    /// return None, not permission to move them across memory or control.
    pub fn operation(
        &self,
        coordinate: Operation,
        budget: &mut Budget<'_>,
    ) -> Result<Option<NodeId>> {
        budget.charge_work(8)?;
        let block = block_index(self.inventory, coordinate.block)
            .ok_or(Error::InvalidOperation(coordinate))?;
        let range = &self.inventory.blocks()[block].operations;
        let ordinal = usize::try_from(coordinate.operation)
            .map_err(|_| Error::InvalidOperation(coordinate))?;
        let index = range
            .start
            .checked_add(ordinal)
            .filter(|i| *i < range.end)
            .ok_or(Error::InvalidOperation(coordinate))?;
        let row = self
            .inventory
            .operations()
            .get(index)
            .filter(|row| row.coordinate == coordinate)
            .ok_or(Error::InvalidOperation(coordinate))?;
        let _ = row;
        self.operations
            .get(index)
            .copied()
            .ok_or(Error::InconsistentInventory)
    }

    pub fn block_entry(&self, coordinate: Block, budget: &mut Budget<'_>) -> Result<NodeId> {
        budget.charge_work(4)?;
        let index =
            block_index(self.inventory, coordinate).ok_or(Error::InvalidBlock(coordinate))?;
        self.blocks
            .get(index)
            .map(|row| row.entry)
            .ok_or(Error::InconsistentInventory)
    }
    pub fn block_exit(&self, coordinate: Block, budget: &mut Budget<'_>) -> Result<NodeId> {
        budget.charge_work(4)?;
        let index =
            block_index(self.inventory, coordinate).ok_or(Error::InvalidBlock(coordinate))?;
        self.blocks
            .get(index)
            .map(|row| row.exit)
            .ok_or(Error::InconsistentInventory)
    }
    pub fn node(&self, node: NodeId, budget: &mut Budget<'_>) -> Result<&Node> {
        budget.charge_work(2)?;
        self.nodes.get(node.0).ok_or(Error::InvalidNode(node))
    }
    /// Inputs are Entry first (only for a function entry), then original edge
    /// roster order. Equal source/target pairs do not collapse edge occurrences.
    pub fn phi_inputs(&self, node: NodeId, budget: &mut Budget<'_>) -> Result<&[Input]> {
        budget.charge_work(3)?;
        let row = self.nodes.get(node.0).ok_or(Error::InvalidNode(node))?;
        let Node::Phi { incoming, .. } = row else {
            return Err(Error::NotPhi(node));
        };
        self.inputs
            .get(incoming.clone())
            .ok_or(Error::InconsistentInventory)
    }

    fn build(
        inventory: &'i CanonicalKirInventoryV1<'g>,
        limits: CanonicalKirMemorySsaLimitsV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        use CanonicalKirMemorySsaResourceV1 as Count;
        budget.charge_work(6)?;
        let f = inventory.functions().len();
        let b = inventory.blocks().len();
        let o = inventory.operations().len();
        let e = inventory.edges().len();
        for (resource, actual, limit) in [
            (Count::Functions, f, limits.functions),
            (Count::Blocks, b, limits.blocks),
            (Count::Operations, o, limits.operations),
            (Count::Effects, inventory.effects().len(), limits.effects),
            (Count::Edges, e, limits.edges),
        ] {
            if actual > limit {
                return Err(Error::InputLimit {
                    resource,
                    actual,
                    limit,
                });
            }
        }
        let mut defined = 0usize;
        for function in inventory.functions() {
            budget.charge_work(3)?;
            if function.function.body.is_some() {
                if function.blocks.is_empty() || function.blocks.end > b {
                    return Err(Error::InconsistentInventory);
                }
                defined = defined.checked_add(1).ok_or(Resource::Arithmetic)?;
            } else if !function.blocks.is_empty() {
                return Err(Error::InconsistentInventory);
            }
        }
        let mut memory_operations = 0usize;
        for index in 0..o {
            let class = classify(inventory, index, budget)?;
            budget.charge_work(1)?;
            if class != Class::None {
                memory_operations = memory_operations
                    .checked_add(1)
                    .ok_or(Resource::Arithmetic)?;
            }
        }
        budget.charge_work(3)?;
        let node_count = defined
            .checked_add(b)
            .and_then(|n| n.checked_add(memory_operations))
            .ok_or(Resource::Arithmetic)?;
        let input_count = defined.checked_add(e).ok_or(Resource::Arithmetic)?;
        budget.charge_work(1)?;
        budget.reserve_storage(size_of::<Self>())?;
        let (mut nodes, nodes_bytes) = vector::<Node>(node_count, budget)?;
        let (mut inputs, input_bytes) = vector::<Input>(input_count, budget)?;
        let (mut blocks, block_bytes) = vector::<BlockState>(b, budget)?;
        let (mut operations, operation_bytes) = vector::<Option<NodeId>>(o, budget)?;
        let (mut entries, entry_bytes) = vector::<Option<NodeId>>(f, budget)?;
        let (mut counts, count_bytes) = vector::<usize>(b, budget)?;
        let (mut cursors, cursor_bytes) = vector::<usize>(b, budget)?;
        fill(
            &mut inputs,
            input_count,
            Input {
                source: InputSource::Entry(Function(0)),
                state: NodeId(0),
            },
            budget,
        )?;
        fill(&mut operations, o, None, budget)?;
        fill(&mut entries, f, None, budget)?;
        fill(&mut counts, b, 0, budget)?;
        fill(&mut cursors, b, 0, budget)?;

        for (index, function) in inventory.functions().iter().enumerate() {
            budget.charge_work(4)?;
            if usize::try_from(function.coordinate.0).ok() != Some(index) {
                return Err(Error::InconsistentInventory);
            }
            if function.function.body.is_some() {
                let entry = NodeId(nodes.len());
                push(
                    &mut nodes,
                    Node::LiveOnEntry {
                        function: function.coordinate,
                    },
                    budget,
                )?;
                entries[index] = Some(entry);
                let count = counts
                    .get_mut(function.blocks.start)
                    .ok_or(Error::InconsistentInventory)?;
                *count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
            }
        }
        for (index, edge) in inventory.edges().iter().enumerate() {
            budget.charge_work(10)?;
            let (_, target) = edge_indices(inventory, index).ok_or(Error::InconsistentInventory)?;
            if edge.coordinate.source.function != edge.target.function {
                return Err(Error::InconsistentInventory);
            }
            counts[target] = counts[target].checked_add(1).ok_or(Resource::Arithmetic)?;
        }
        let mut next_input = 0usize;
        for (index, block) in inventory.blocks().iter().enumerate() {
            budget.charge_work(4)?;
            if block_index(inventory, block.coordinate) != Some(index) {
                return Err(Error::InconsistentInventory);
            }
            let end = next_input
                .checked_add(counts[index])
                .ok_or(Resource::Arithmetic)?;
            if end > input_count {
                return Err(Error::InconsistentInventory);
            }
            let phi = NodeId(nodes.len());
            push(
                &mut nodes,
                Node::Phi {
                    block: block.coordinate,
                    incoming: next_input..end,
                },
                budget,
            )?;
            push(
                &mut blocks,
                BlockState {
                    entry: phi,
                    exit: phi,
                },
                budget,
            )?;
            cursors[index] = next_input;
            next_input = end;
        }
        if next_input != input_count {
            return Err(Error::InconsistentInventory);
        }

        let mut next_operation = 0usize;
        for (index, block) in inventory.blocks().iter().enumerate() {
            budget.charge_work(3)?;
            if block.operations.start != next_operation || block.operations.end > o {
                return Err(Error::InconsistentInventory);
            }
            let mut current = blocks[index].entry;
            for operation_index in block.operations.clone() {
                let class = classify(inventory, operation_index, budget)?;
                budget.charge_work(3)?;
                let coordinate = inventory.operations()[operation_index].coordinate;
                if coordinate.block != block.coordinate
                    || usize::try_from(coordinate.operation).ok()
                        != Some(operation_index - block.operations.start)
                {
                    return Err(Error::InconsistentInventory);
                }
                let node = match class {
                    Class::None => continue,
                    Class::Use => Node::Use {
                        operation: coordinate,
                        incoming: current,
                    },
                    Class::Def => Node::Def {
                        operation: coordinate,
                        incoming: current,
                    },
                };
                let id = NodeId(nodes.len());
                push(&mut nodes, node, budget)?;
                operations[operation_index] = Some(id);
                if class == Class::Def {
                    current = id;
                }
            }
            blocks[index].exit = current;
            next_operation = block.operations.end;
        }
        if next_operation != o || nodes.len() != node_count {
            return Err(Error::InconsistentInventory);
        }
        // All exits exist before any predecessor is linked, including backedges.
        for (index, function) in inventory.functions().iter().enumerate() {
            budget.charge_work(3)?;
            if let Some(state) = entries[index] {
                put_input(
                    &nodes,
                    &blocks,
                    &mut inputs,
                    &mut cursors,
                    function.blocks.start,
                    Input {
                        source: InputSource::Entry(function.coordinate),
                        state,
                    },
                    budget,
                )?;
            }
        }
        for (index, edge) in inventory.edges().iter().enumerate() {
            budget.charge_work(8)?;
            let (source, target) =
                edge_indices(inventory, index).ok_or(Error::InconsistentInventory)?;
            put_input(
                &nodes,
                &blocks,
                &mut inputs,
                &mut cursors,
                target,
                Input {
                    source: InputSource::Edge(edge.coordinate),
                    state: blocks[source].exit,
                },
                budget,
            )?;
        }
        for (index, block) in blocks.iter().enumerate() {
            budget.charge_work(3)?;
            let Node::Phi { incoming, .. } = &nodes[block.entry.0] else {
                return Err(Error::InconsistentInventory);
            };
            if cursors[index] != incoming.end {
                return Err(Error::InconsistentInventory);
            }
        }
        budget.charge_work(9)?;
        let retained = size_of::<Self>()
            .checked_add(nodes_bytes)
            .and_then(|n| n.checked_add(input_bytes))
            .and_then(|n| n.checked_add(block_bytes))
            .and_then(|n| n.checked_add(operation_bytes))
            .ok_or(Resource::Arithmetic)?;
        let scratch = entry_bytes
            .checked_add(count_bytes)
            .and_then(|n| n.checked_add(cursor_bytes))
            .ok_or(Resource::Arithmetic)?;
        drop(entries);
        drop(counts);
        drop(cursors);
        budget.release_storage(scratch)?;
        Ok(Self {
            inventory,
            nodes,
            inputs,
            blocks,
            operations,
            retained,
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Class {
    None,
    Use,
    Def,
}

fn classify(
    inventory: &CanonicalKirInventoryV1<'_>,
    index: usize,
    budget: &mut Budget<'_>,
) -> Result<Class> {
    budget.charge_work(3)?;
    let operation = inventory
        .operations()
        .get(index)
        .ok_or(Error::InconsistentInventory)?;
    let mut class = if !operation.compiler_ordering().is_empty()
        || matches!(
            operation.operation.kind,
            OperationKind::Call { .. }
                | OperationKind::InlineAssembly(_)
                | OperationKind::Execution(_)
        ) {
        Class::Def
    } else {
        Class::None
    };
    let effects = inventory
        .effects()
        .get(operation.effects.clone())
        .ok_or(Error::InconsistentInventory)?;
    for (ordinal, effect) in effects.iter().enumerate() {
        budget.charge_work(2)?;
        if effect.coordinate.operation != operation.coordinate
            || usize::try_from(effect.coordinate.effect).ok() != Some(ordinal)
        {
            return Err(Error::InconsistentInventory);
        }
        match effect.effect {
            KirLocalMemoryEffectRefV1::Read(_) => {
                if class == Class::None {
                    class = Class::Use;
                }
            }
            KirLocalMemoryEffectRefV1::Allocate(_)
            | KirLocalMemoryEffectRefV1::Write(_)
            | KirLocalMemoryEffectRefV1::VolatileRead(_)
            | KirLocalMemoryEffectRefV1::VolatileWrite(_)
            | KirLocalMemoryEffectRefV1::Atomic { .. }
            | KirLocalMemoryEffectRefV1::Synchronize { .. }
            | KirLocalMemoryEffectRefV1::Fence { .. } => class = Class::Def,
        }
    }
    Ok(class)
}

fn block_index(inventory: &CanonicalKirInventoryV1<'_>, coordinate: Block) -> Option<usize> {
    let function = inventory
        .functions()
        .get(usize::try_from(coordinate.function.0).ok()?)?;
    if function.coordinate != coordinate.function {
        return None;
    }
    let index = function
        .blocks
        .start
        .checked_add(usize::try_from(coordinate.block).ok()?)?;
    if index >= function.blocks.end {
        return None;
    }
    (inventory.blocks().get(index)?.coordinate == coordinate).then_some(index)
}

fn edge_indices(inventory: &CanonicalKirInventoryV1<'_>, index: usize) -> Option<(usize, usize)> {
    let edge = inventory.edges().get(index)?;
    let source = block_index(inventory, edge.coordinate.source)?;
    let block = inventory.blocks().get(source)?;
    let ordinal = usize::try_from(edge.coordinate.successor).ok()?;
    if block.edges.start.checked_add(ordinal)? != index
        || index >= block.edges.end
        || edge.coordinate.source.function != edge.target.function
    {
        return None;
    }
    Some((source, block_index(inventory, edge.target)?))
}

fn put_input(
    nodes: &[Node],
    blocks: &[BlockState],
    inputs: &mut [Input],
    cursors: &mut [usize],
    block: usize,
    input: Input,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(3)?;
    let entry = blocks.get(block).ok_or(Error::InconsistentInventory)?.entry;
    let Some(Node::Phi { incoming, .. }) = nodes.get(entry.0) else {
        return Err(Error::InconsistentInventory);
    };
    let cursor = cursors.get_mut(block).ok_or(Error::InconsistentInventory)?;
    if !incoming.contains(cursor) || input.state.0 >= nodes.len() {
        return Err(Error::InconsistentInventory);
    }
    *inputs
        .get_mut(*cursor)
        .ok_or(Error::InconsistentInventory)? = input;
    *cursor = cursor.checked_add(1).ok_or(Resource::Arithmetic)?;
    Ok(())
}

fn vector<T>(count: usize, budget: &mut Budget<'_>) -> Result<(Vec<T>, usize)> {
    budget.charge_work(2)?;
    let requested = count
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(requested)?;
    let mut vector = Vec::new();
    vector
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    budget.charge_work(1)?;
    let actual = vector
        .capacity()
        .checked_mul(size_of::<T>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(actual.checked_sub(requested).ok_or(Resource::Accounting)?)?;
    Ok((vector, actual))
}

fn fill<T: Copy>(
    vector: &mut Vec<T>,
    count: usize,
    value: T,
    budget: &mut Budget<'_>,
) -> Result<()> {
    budget.charge_work(count)?;
    if count > vector.capacity() {
        return Err(Error::InconsistentInventory);
    }
    vector.resize(count, value);
    Ok(())
}

fn push<T>(vector: &mut Vec<T>, value: T, budget: &mut Budget<'_>) -> Result<()> {
    budget.charge_work(1)?;
    if vector.len() >= vector.capacity() {
        return Err(Error::InconsistentInventory);
    }
    vector.push(value);
    Ok(())
}

#[cfg(test)]
#[path = "canonical_kir_memory_ssa_v1_tests.rs"]
mod tests;
