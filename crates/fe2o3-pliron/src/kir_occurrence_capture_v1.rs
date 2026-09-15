//! Private occurrence custody for the fixed neutral seven-pass executor.
//! These are observations, not semantic-preservation or artifact authority.

use crate::{
    KIR_PLIRON_PRODUCTION_PASSES_V12, KirBridgeCoordinateV1 as Coordinate,
    KirOptimizationEndpointV12 as Endpoint, KirOptimizationMapErrorV12 as E, KirOptimizationMapV12,
    OperationGraphEpochV1, PlironOptimizationPassV1,
    kir_optimization_map_v12::{LiveKeyV12, LiveRosterV12},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKirBlockCoordinateV1 as Block, CanonicalKirBlockSegmentV1 as Segment,
    CanonicalKirBlockTransitionV1 as BlockRow, CanonicalKirDefinitionCoordinateV1 as Definition,
    CanonicalKirDefinitionDescendantKindV1 as DescendantKind,
    CanonicalKirDefinitionDescendantV1 as Descendant,
    CanonicalKirDefinitionTransitionV1 as DefinitionRow,
    CanonicalKirEdgeArgumentCoordinateV1 as EdgeArgument,
    CanonicalKirEdgeArgumentTransitionV1 as EdgeArgumentRow, CanonicalKirEdgeCoordinateV1 as Edge,
    CanonicalKirEdgeTransitionV1 as EdgeRow, CanonicalKirFunctionCoordinateV1 as Function,
    CanonicalKirFunctionTransitionV1 as FunctionRow,
    CanonicalKirOperationCoordinateV1 as OperationCoordinate,
    CanonicalKirOperationOriginV1 as Origin, CanonicalKirOperationTransitionV1 as OperationRow,
    CanonicalKirTransitionCandidateV1, CanonicalKirTransitionRangeV1 as Range,
    CanonicalKirUseCoordinateV1 as UseCoordinate, CanonicalKirUseTransitionV1 as UseRow, Module,
};
use pliron::{
    basic_block::BasicBlock,
    builtin::{attributes::OperandSegmentSizesAttr, op_interfaces::ATTR_KEY_OPERAND_SEGMENT_SIZES},
    context::{Context, Ptr},
    irbuild::observer::{RewriteEvent, RewriteObserver, RewriteOccurrenceEvent},
    linked_list::ContainsLinkedList,
    operation::Operation,
    value::{Use, Value},
};
use std::{
    cell::Cell,
    collections::HashMap,
    hash::Hash,
    mem::size_of,
    sync::{Arc, Mutex},
};

type Result<T> = std::result::Result<T, E>;

/// Separate, closed admission envelope. No target V4 accounting changes.
/// N bounds aggregate registered occurrences, not the largest raw UID.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Limits {
    pub(crate) nodes: usize,
    pub(crate) events: usize,
    pub(crate) targets: usize,
}

/// Counts physical imported objects, including operand/successor occurrences.
/// Names, serialized metadata and the largest value/operation UID are irrelevant.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct StructuralCensus {
    pub(crate) functions: usize,
    pub(crate) blocks: usize,
    pub(crate) operations: usize,
    pub(crate) values: usize,
    pub(crate) results: usize,
    pub(crate) operands: usize,
    pub(crate) successors: usize,
    pub(crate) declaration_parameters: usize,
    pub(crate) conditional_branches: usize,
    pub(crate) conditional_operands: usize,
}

impl Limits {
    pub(crate) fn for_structure(census: StructuralCensus) -> Result<Self> {
        // Initial registration claims F+2B+O+2V+R+2U+2E+D. The one SCCP
        // invocation can materialize at most one constant (three claims) per
        // original value. Each original conditional branch can become one
        // branch, claiming 3+2*payload <= 3+2*original_operands. Later passes
        // erase/forward/move existing objects; tombstones remain in these bounds.
        let mut nodes = 0usize;
        for (count, weight) in [
            (census.functions, 1),
            (census.blocks, 2),
            (census.operations, 1),
            (census.values, 5),
            (census.results, 1),
            (census.operands, 2),
            (census.successors, 2),
            (census.declaration_parameters, 1),
            (census.conditional_branches, 3),
            (census.conditional_operands, 2),
        ] {
            nodes = nodes
                .checked_add(count.checked_mul(weight).ok_or(E::Arithmetic)?)
                .ok_or(E::Arithmetic)?;
        }
        // Even an empty module has a root census and fixed capture headers.
        let nodes = nodes.max(1);
        if nodes > 262_144 {
            return Err(E::Limit);
        }
        Ok(Self {
            nodes,
            events: nodes.checked_mul(8).ok_or(E::Arithmetic)?,
            targets: nodes.checked_mul(8).ok_or(E::Arithmetic)?,
        })
    }

    /// Allocation-free admission census. Every traversed object is precharged;
    /// arities are O(1) lengths, not scans through operand definitions or names.
    pub(crate) fn for_graph(
        ctx: &Context,
        root: Ptr<Operation>,
        source: &Module,
        budget: &mut Budget<'_>,
    ) -> Result<Self> {
        use pliron::builtin::ops::{FuncOp, ModuleOp};
        budget.charge_work(4)?;
        if !Operation::is_op::<ModuleOp>(root, ctx) || root.deref(ctx).num_regions() != 1 {
            return Err(E::Coverage);
        }
        let region = root.deref(ctx).get_region(0);
        let region = region.deref(ctx);
        let mut root_blocks = region.iter(ctx);
        let block = root_blocks.next().ok_or(E::Coverage)?;
        if root_blocks.next().is_some() {
            return Err(E::Coverage);
        }
        let block = block.deref(ctx);
        let mut functions = block.iter(ctx);
        let mut census = StructuralCensus {
            functions: source.functions.len(),
            ..StructuralCensus::default()
        };
        budget.charge_work(source.functions.len())?;
        for source_function in &source.functions {
            if source_function.body.is_none() {
                census.declaration_parameters = census
                    .declaration_parameters
                    .checked_add(source_function.signature.parameters.len())
                    .ok_or(E::Arithmetic)?;
                continue;
            }
            budget.charge_work(1)?;
            let function = functions.next().ok_or(E::Coverage)?;
            if !Operation::is_op::<FuncOp>(function, ctx) || function.deref(ctx).num_regions() != 1
            {
                return Err(E::Coverage);
            }
            let region = function.deref(ctx).get_region(0);
            let region = region.deref(ctx);
            for block in region.iter(ctx) {
                budget.charge_work(1)?;
                census.blocks = census.blocks.checked_add(1).ok_or(E::Arithmetic)?;
                let block = block.deref(ctx);
                census.values = census
                    .values
                    .checked_add(block.get_num_arguments())
                    .ok_or(E::Arithmetic)?;
                for operation in block.iter(ctx) {
                    budget.charge_work(1)?;
                    let op = operation.deref(ctx);
                    if op.num_regions() != 0 {
                        return Err(E::UnsupportedMutation);
                    }
                    census.operations = census.operations.checked_add(1).ok_or(E::Arithmetic)?;
                    census.values = census
                        .values
                        .checked_add(op.get_num_results())
                        .ok_or(E::Arithmetic)?;
                    census.results = census
                        .results
                        .checked_add(op.get_num_results())
                        .ok_or(E::Arithmetic)?;
                    census.operands = census
                        .operands
                        .checked_add(op.get_num_operands())
                        .ok_or(E::Arithmetic)?;
                    census.successors = census
                        .successors
                        .checked_add(op.get_num_successors())
                        .ok_or(E::Arithmetic)?;
                    if Operation::is_op::<dialect_gpu::optimization_v1::CondBranchOp>(
                        operation, ctx,
                    ) {
                        census.conditional_branches = census
                            .conditional_branches
                            .checked_add(1)
                            .ok_or(E::Arithmetic)?;
                        census.conditional_operands = census
                            .conditional_operands
                            .checked_add(op.get_num_operands())
                            .ok_or(E::Arithmetic)?;
                    }
                }
            }
        }
        if functions.next().is_some() {
            return Err(E::Coverage);
        }
        Self::for_structure(census)
    }
    pub(crate) fn work(self) -> Result<usize> {
        self.nodes
            .checked_mul(self.events)
            .and_then(|n| n.checked_mul(32))
            .and_then(|n| n.checked_add(self.nodes.checked_mul(256)?))
            .ok_or(E::Arithmetic)
    }
    pub(crate) fn storage(self) -> Result<usize> {
        self.nodes
            .checked_mul(2048)
            .and_then(|n| n.checked_add(self.events.checked_mul(32)?))
            .and_then(|n| n.checked_add(self.targets.checked_mul(128)?))
            .and_then(|n| n.checked_add(8192))
            .ok_or(E::Arithmetic)
    }
}
fn index(n: usize) -> Result<u32> {
    u32::try_from(n).map_err(|_| E::Arithmetic)
}
fn vector<T>(n: usize) -> Result<Vec<T>> {
    let mut v = Vec::new();
    v.try_reserve_exact(n).map_err(|_| E::Allocation)?;
    Ok(v)
}
fn block_coordinate(c: Coordinate) -> Result<Block> {
    match c {
        Coordinate::Operation {
            function, block, ..
        }
        | Coordinate::Terminator { function, block }
        | Coordinate::Block { function, block } => Ok(Block {
            function: Function(function),
            block,
        }),
        _ => Err(E::Coverage),
    }
}
fn operation_coordinate(c: Coordinate) -> Result<OperationCoordinate> {
    let Coordinate::Operation { operation, .. } = c else {
        return Err(E::Coverage);
    };
    Ok(OperationCoordinate {
        block: block_coordinate(c)?,
        operation,
    })
}
fn definition(e: Endpoint) -> Result<Definition> {
    match e {
        Endpoint::FunctionArgument { function, argument } => Ok(Definition::FunctionArgument {
            function: Function(function),
            argument,
        }),
        Endpoint::BlockArgument {
            function,
            block,
            argument,
        } => Ok(Definition::BlockArgument {
            block: Block {
                function: Function(function),
                block,
            },
            argument,
        }),
        Endpoint::Result { operation, result } => Ok(Definition::Result {
            operation: operation_coordinate(operation)?,
            result,
        }),
        _ => Err(E::Coverage),
    }
}
fn use_coordinate(c: Coordinate, operand: usize) -> Result<UseCoordinate> {
    Ok(match c {
        Coordinate::Operation { .. } => UseCoordinate::OperationOperand {
            operation: operation_coordinate(c)?,
            operand: index(operand)?,
        },
        Coordinate::Terminator { .. } => UseCoordinate::TerminatorOperand {
            block: block_coordinate(c)?,
            operand: index(operand)?,
        },
        _ => return Err(E::Coverage),
    })
}
fn endpoint_function(e: Endpoint) -> Result<usize> {
    Ok(match e {
        Endpoint::Operation(c) | Endpoint::Result { operation: c, .. } => {
            block_coordinate(c)?.function.0 as usize
        }
        Endpoint::FunctionArgument { function, .. } | Endpoint::BlockArgument { function, .. } => {
            function as usize
        }
    })
}

/// A bounded registry gives pointers stable private indices. Tombstones stay
/// registered, so erasure followed by UID/pointer reuse never revives an origin.
struct Records<K, T> {
    ids: HashMap<K, usize>,
    rows: Vec<T>,
}
impl<K: Copy + Eq + Hash, T> Records<K, T> {
    fn new() -> Self {
        Self {
            ids: HashMap::new(),
            rows: Vec::new(),
        }
    }
    fn id(&self, key: K) -> Result<usize> {
        self.ids.get(&key).copied().ok_or(E::Coverage)
    }
    fn add(&mut self, key: K, row: T) -> Result<usize> {
        if self.ids.contains_key(&key) {
            return Err(E::Lifecycle);
        }
        self.ids.try_reserve(1).map_err(|_| E::Allocation)?;
        self.rows.try_reserve(1).map_err(|_| E::Allocation)?;
        let id = self.rows.len();
        self.ids.insert(key, id);
        self.rows.push(row);
        Ok(id)
    }
}
struct BlockState {
    input: Block,
    alive: bool,
    seen: u8,
    head: usize,
    tail: usize,
    merged: bool,
    dropped: bool,
}
struct ChainLink {
    segment: Segment,
    next: Option<usize>,
}
struct OpState {
    input: Option<Coordinate>,
    constant_from: Option<Definition>,
    parent: usize,
    move_to: Option<usize>,
    linked: bool,
    alive: bool,
    seen: u8,
    operands: Vec<usize>,
    edges: Vec<usize>,
    results: Vec<usize>,
    selected_replacement: Option<usize>,
    dropped: bool,
    validated_event: usize,
}
struct ValueState {
    input: Option<Definition>,
    anchor: Option<Definition>,
    alive: bool,
    seen: u8,
    producer: Option<usize>,
}
struct UseState {
    op: usize,
    value: usize,
    alive: bool,
    input: Option<UseCoordinate>,
    edge_argument: Option<EdgeArgument>,
}
struct EdgeState {
    op: usize,
    target: usize,
    alive: bool,
    input: Option<Edge>,
    arguments: Vec<usize>,
}
struct FunctionState {
    coordinate: Function,
    live: Option<Ptr<Operation>>,
    parameters: usize,
}
struct State {
    root: Ptr<Operation>,
    functions: Vec<FunctionState>,
    inputs: Vec<Definition>,
    blocks: Records<Ptr<BasicBlock>, BlockState>,
    operations: Records<Ptr<Operation>, OpState>,
    values: Records<Value, ValueState>,
    uses: Records<Use<Value>, UseState>,
    edges: Records<Use<Ptr<BasicBlock>>, EdgeState>,
    chains: Vec<ChainLink>,
    limits: Limits,
    claimed: usize,
    events: usize,
    work: Cell<usize>,
    work_limit: usize,
    current: Option<(usize, u64)>,
    completed: usize,
    epoch: Option<u64>,
    failure: Option<E>,
}
impl State {
    fn step(&self, n: usize) -> Result<()> {
        let work = self.work.get().checked_add(n).ok_or(E::Arithmetic)?;
        if work > self.work_limit {
            return Err(E::Limit);
        }
        self.work.set(work);
        Ok(())
    }
    fn claim(&mut self, n: usize) -> Result<()> {
        self.step(n.checked_add(2).ok_or(E::Arithmetic)?)?;
        self.claimed = self.claimed.checked_add(n).ok_or(E::Arithmetic)?;
        if self.claimed > self.limits.nodes {
            return Err(E::Limit);
        }
        Ok(())
    }
    fn event(&mut self) -> Result<()> {
        if self.current.is_none() {
            return Err(E::Passes);
        }
        self.step(1)?;
        self.events = self.events.checked_add(1).ok_or(E::Arithmetic)?;
        if self.events > self.limits.events {
            return Err(E::Limit);
        }
        Ok(())
    }
    fn block(&self, raw: Ptr<BasicBlock>) -> Result<usize> {
        self.step(1)?;
        let id = self.blocks.id(raw)?;
        if !self.blocks.rows[id].alive {
            return Err(E::Lifecycle);
        }
        Ok(id)
    }
    fn op(&self, raw: Ptr<Operation>) -> Result<usize> {
        self.step(1)?;
        let id = self.operations.id(raw)?;
        if !self.operations.rows[id].alive {
            return Err(E::Lifecycle);
        }
        Ok(id)
    }
    fn value(&self, raw: Value) -> Result<usize> {
        self.step(1)?;
        let id = self.values.id(raw)?;
        if !self.values.rows[id].alive {
            return Err(E::Lifecycle);
        }
        Ok(id)
    }
    fn use_id(&self, raw: Use<Value>) -> Result<usize> {
        self.step(1)?;
        self.uses.id(raw)
    }
    fn edge_id(&self, raw: Use<Ptr<BasicBlock>>) -> Result<usize> {
        self.step(1)?;
        self.edges.id(raw)
    }
    fn register_block(&mut self, raw: Ptr<BasicBlock>, input: Block) -> Result<usize> {
        self.step(1)?;
        if let Some(&id) = self.blocks.ids.get(&raw) {
            return if self.blocks.rows[id].input == input {
                Ok(id)
            } else {
                Err(E::Identity)
            };
        }
        self.claim(2)?;
        self.chains.try_reserve(1).map_err(|_| E::Allocation)?;
        let link = self.chains.len();
        self.chains.push(ChainLink {
            segment: Segment {
                input,
                connector: None,
            },
            next: None,
        });
        self.blocks.add(
            raw,
            BlockState {
                input,
                alive: true,
                seen: 0,
                head: link,
                tail: link,
                merged: false,
                dropped: false,
            },
        )
    }
    fn register_value(&mut self, raw: Value, input: Option<Definition>) -> Result<usize> {
        self.claim(1)?;
        let producer = raw.defining_op().map(|op| self.op(op)).transpose()?;
        self.values.add(
            raw,
            ValueState {
                input,
                anchor: input,
                alive: true,
                seen: 0,
                producer,
            },
        )
    }
    fn register_operation(
        &mut self,
        ctx: &Context,
        raw: Ptr<Operation>,
        input: Option<Coordinate>,
    ) -> Result<usize> {
        let op = raw.deref(ctx);
        if op.num_regions() != 0 {
            return Err(E::UnsupportedMutation);
        }
        let parent = self.block(op.get_parent_block().ok_or(E::Coverage)?)?;
        self.claim(
            1usize
                .checked_add(op.get_num_operands())
                .and_then(|n| n.checked_add(op.get_num_successors()))
                .and_then(|n| n.checked_add(op.get_num_results()))
                .ok_or(E::Arithmetic)?,
        )?;
        let id = self.operations.add(
            raw,
            OpState {
                input,
                constant_from: None,
                parent,
                move_to: None,
                linked: true,
                alive: true,
                seen: 0,
                operands: vector(op.get_num_operands())?,
                edges: vector(op.get_num_successors())?,
                results: vector(op.get_num_results())?,
                selected_replacement: None,
                dropped: false,
                validated_event: usize::MAX,
            },
        )?;
        if input.is_none() {
            if !is_constant(ctx, raw)
                && !Operation::is_op::<dialect_gpu::optimization_v1::BranchOp>(raw, ctx)
            {
                return Err(E::UnsupportedMutation);
            }
            for value in op.results() {
                let result = self.register_value(value, None)?;
                self.operations.rows[id].results.push(result);
            }
            self.register_occurrences(ctx, raw, id)?;
        }
        Ok(id)
    }
    fn register_occurrences(
        &mut self,
        ctx: &Context,
        raw: Ptr<Operation>,
        id: usize,
    ) -> Result<()> {
        let op = raw.deref(ctx);
        self.claim(
            op.get_num_operands()
                .checked_add(op.get_num_successors())
                .ok_or(E::Arithmetic)?,
        )?;
        for (i, (occurrence, actual_value)) in op.operands_as_uses().zip(op.operands()).enumerate()
        {
            let value = self.value(actual_value)?;
            let origin = self.operations.rows[id]
                .input
                .map(|c| use_coordinate(c, i))
                .transpose()?;
            let use_id = self.uses.add(
                occurrence,
                UseState {
                    op: id,
                    value,
                    alive: true,
                    input: origin,
                    edge_argument: None,
                },
            )?;
            self.operations.rows[id].operands.push(use_id);
        }
        for (s, (occurrence, actual_target)) in
            op.successors_as_uses().zip(op.successors()).enumerate()
        {
            let target = self.block(actual_target)?;
            self.step(
                op.get_num_successors()
                    .checked_add(1)
                    .ok_or(E::Arithmetic)?,
            )?;
            let range = successor_range(ctx, raw, s)?;
            self.step(range.len())?;
            let mut arguments = vector(range.len())?;
            let input = self.operations.rows[id]
                .input
                .map(|c| -> Result<Edge> {
                    Ok(Edge {
                        source: block_coordinate(c)?,
                        successor: index(s)?,
                    })
                })
                .transpose()?;
            for (argument, operand) in range.enumerate() {
                let use_id = *self.operations.rows[id]
                    .operands
                    .get(operand)
                    .ok_or(E::Coverage)?;
                if self.uses.rows[use_id].edge_argument.is_some() {
                    return Err(E::Coverage);
                }
                self.uses.rows[use_id].edge_argument = input
                    .map(|edge| -> Result<EdgeArgument> {
                        Ok(EdgeArgument {
                            edge,
                            argument: index(argument)?,
                        })
                    })
                    .transpose()?;
                arguments.push(use_id);
            }
            let edge = self.edges.add(
                occurrence,
                EdgeState {
                    op: id,
                    target,
                    alive: true,
                    input,
                    arguments,
                },
            )?;
            self.operations.rows[id].edges.push(edge);
        }
        Ok(())
    }
}

/// Exact physical payload ranges of the closed carriers. This checks the
/// carrier's segment metadata, not equality of operand Values or destinations.
fn successor_range(
    ctx: &Context,
    raw: Ptr<Operation>,
    successor: usize,
) -> Result<std::ops::Range<usize>> {
    use dialect_gpu::optimization_v1::{BranchOp, CondBranchOp, PreservedTerminatorOp};
    let op = raw.deref(ctx);
    if successor >= op.get_num_successors() {
        return Err(E::Coverage);
    }
    if Operation::is_op::<BranchOp>(raw, ctx) {
        if successor != 0 || op.get_num_successors() != 1 {
            return Err(E::Coverage);
        }
        return Ok(0..op.get_num_operands());
    }
    if Operation::is_op::<CondBranchOp>(raw, ctx)
        || Operation::is_op::<PreservedTerminatorOp>(raw, ctx)
    {
        let segments = op
            .attributes
            .get::<OperandSegmentSizesAttr>(&ATTR_KEY_OPERAND_SEGMENT_SIZES)
            .ok_or(E::Coverage)?;
        if segments.0.len() != op.get_num_successors() + 1 {
            return Err(E::Coverage);
        }
        let mut start = 0usize;
        let mut selected = None;
        for (i, &length) in segments.0.iter().enumerate() {
            let end = start.checked_add(length as usize).ok_or(E::Arithmetic)?;
            if i == successor + 1 {
                selected = Some(start..end);
            }
            start = end;
        }
        if start != op.get_num_operands() {
            return Err(E::Coverage);
        }
        return selected.ok_or(E::Coverage);
    }
    if Operation::is_op::<dialect_gpu::switch_v3::SwitchOpV3>(raw, ctx) {
        use pliron::op::Op;
        let switch = dialect_gpu::switch_v3::SwitchOpV3::from_operation(raw);
        return switch
            .successor_operand_range(ctx, successor)
            .ok_or(E::Coverage);
    }
    Err(E::UnsupportedMutation)
}

fn is_constant(ctx: &Context, raw: Ptr<Operation>) -> bool {
    Operation::is_op::<dialect_gpu::optimization_v1::ConstantOp>(raw, ctx)
        || Operation::is_op::<pliron::builtin::ops::ConstantOp>(raw, ctx)
}

impl State {
    fn new(
        ctx: &Context,
        root: Ptr<Operation>,
        source: &Module,
        roster: &LiveRosterV12,
        limits: Limits,
        roster_work: usize,
    ) -> Result<Self> {
        let mut state = Self {
            root,
            functions: Vec::new(),
            inputs: Vec::new(),
            blocks: Records::new(),
            operations: Records::new(),
            values: Records::new(),
            uses: Records::new(),
            edges: Records::new(),
            chains: Vec::new(),
            limits,
            claimed: 0,
            events: 0,
            work: Cell::new(0),
            work_limit: limits.work()?,
            current: None,
            completed: 0,
            epoch: None,
            failure: None,
        };
        state.step(roster_work)?;
        state.claim(source.functions.len())?;
        state.functions = vector(source.functions.len())?;
        for (i, source) in source.functions.iter().enumerate() {
            state.functions.push(FunctionState {
                coordinate: Function(index(i)?),
                live: None,
                parameters: source.signature.parameters.len(),
            });
        }
        state.step(roster.len())?;
        for &(key, endpoint) in roster {
            if let (LiveKeyV12::Operation(raw), Endpoint::Operation(c)) = (key, endpoint) {
                let block = raw.deref(ctx).get_parent_block().ok_or(E::Coverage)?;
                state.register_block(block, block_coordinate(c)?)?;
                let function = block
                    .deref(ctx)
                    .get_parent_region()
                    .ok_or(E::Coverage)?
                    .deref(ctx)
                    .get_parent_op();
                let function_id = block_coordinate(c)?.function.0 as usize;
                let row = state.functions.get_mut(function_id).ok_or(E::Coverage)?;
                if row.live.is_some_and(|expected| expected != function) {
                    return Err(E::Identity);
                }
                row.live = Some(function);
                state.register_operation(ctx, raw, Some(c))?;
            }
        }
        state.step(roster.len())?;
        for &(key, endpoint) in roster {
            if let LiveKeyV12::Value(raw) = key {
                let value = state.register_value(raw, Some(definition(endpoint)?))?;
                if let Some(op) = state.values.rows[value].producer {
                    state.operations.rows[op].results.push(value);
                }
            }
        }
        // Definitions follow the canonical input inventory order, including
        // declarations which have no live Pliron Value.
        let mut cursor = 0;
        for (function_id, function) in source.functions.iter().enumerate() {
            if function.body.is_none() {
                if state.functions[function_id].live.is_some() {
                    return Err(E::Identity);
                }
                state.claim(function.signature.parameters.len())?;
                state
                    .inputs
                    .try_reserve(function.signature.parameters.len())
                    .map_err(|_| E::Allocation)?;
                for argument in 0..function.signature.parameters.len() {
                    state.inputs.push(Definition::FunctionArgument {
                        function: Function(index(function_id)?),
                        argument: index(argument)?,
                    });
                }
            } else if state.functions[function_id].live.is_none() {
                return Err(E::Coverage);
            }
            while cursor < roster.len() && endpoint_function(roster[cursor].1)? == function_id {
                state.step(1)?;
                let (key, endpoint) = roster[cursor];
                cursor += 1;
                if let LiveKeyV12::Value(_) = key {
                    state.claim(1)?;
                    state.inputs.try_reserve(1).map_err(|_| E::Allocation)?;
                    state.inputs.push(definition(endpoint)?);
                }
            }
        }
        if cursor != roster.len() {
            return Err(E::Coverage);
        }
        for &(key, _) in roster {
            if let LiveKeyV12::Operation(raw) = key {
                let id = state.op(raw)?;
                state.register_occurrences(ctx, raw, id)?;
            }
        }
        state.census(ctx, 1)?;
        Ok(state)
    }

    /// No inference from a vanished object: every live registry entry must be
    /// seen, every actual occurrence must be registered with its expected value.
    fn census(&mut self, ctx: &Context, generation: u8) -> Result<()> {
        self.step(self.claimed.checked_mul(4).ok_or(E::Arithmetic)?)?;
        let root = self.root.deref(ctx);
        if root.num_regions() != 1 {
            return Err(E::Coverage);
        }
        let region = root.get_region(0).deref(ctx);
        let mut root_blocks = region.iter(ctx);
        let root_block = root_blocks.next().ok_or(E::Coverage)?;
        if root_blocks.next().is_some() {
            return Err(E::Coverage);
        }
        let raw_root_block = root_block.deref(ctx);
        let mut live_functions = raw_root_block.iter(ctx);
        for fi in 0..self.functions.len() {
            let Some(function) = self.functions[fi].live else {
                continue;
            };
            if live_functions.next() != Some(function) || function.deref(ctx).num_regions() != 1 {
                return Err(E::Identity);
            }
            let function_region = function.deref(ctx).get_region(0).deref(ctx);
            for (bi, raw_block) in function_region.iter(ctx).enumerate() {
                let block = self.block(raw_block)?;
                if self.blocks.rows[block].seen == generation
                    || self.blocks.rows[block].merged
                    || self.blocks.rows[block].dropped
                {
                    return Err(E::Lifecycle);
                }
                self.blocks.rows[block].seen = generation;
                let bb = raw_block.deref(ctx);
                if bi == 0 && bb.get_num_arguments() < self.functions[fi].parameters {
                    return Err(E::Coverage);
                }
                for raw in bb.arguments() {
                    let id = self.value(raw)?;
                    if self.values.rows[id].seen == generation {
                        return Err(E::Coverage);
                    }
                    self.values.rows[id].seen = generation;
                }
                for raw in bb.iter(ctx) {
                    let id = self.op(raw)?;
                    let op = raw.deref(ctx);
                    self.step(
                        op.get_num_operands()
                            .checked_add(op.get_num_successors())
                            .and_then(|n| n.checked_add(op.get_num_results()))
                            .ok_or(E::Arithmetic)?,
                    )?;
                    let record = &mut self.operations.rows[id];
                    if record.seen == generation
                        || !record.linked
                        || record.parent != block
                        || record.move_to.is_some()
                        || record.dropped
                        || op.num_regions() != 0
                        || record.operands.len() != op.get_num_operands()
                        || record.edges.len() != op.get_num_successors()
                        || record.results.len() != op.get_num_results()
                    {
                        return Err(E::Coverage);
                    }
                    record.seen = generation;
                    for (i, actual) in op.results().enumerate() {
                        let value = self.value(actual)?;
                        if self.operations.rows[id].results[i] != value
                            || self.values.rows[value].seen == generation
                        {
                            return Err(E::Coverage);
                        }
                        self.values.rows[value].seen = generation;
                    }
                    for (i, (actual, actual_value)) in
                        op.operands_as_uses().zip(op.operands()).enumerate()
                    {
                        let use_id = self.use_id(actual)?;
                        let expected = &self.uses.rows[use_id];
                        if self.operations.rows[id].operands[i] != use_id
                            || !expected.alive
                            || expected.op != id
                            || self.value(actual_value)? != expected.value
                            || expected.input.is_none()
                        {
                            return Err(E::Coverage);
                        }
                    }
                    for (s, (actual, actual_target)) in
                        op.successors_as_uses().zip(op.successors()).enumerate()
                    {
                        self.step(
                            op.get_num_successors()
                                .checked_add(1)
                                .ok_or(E::Arithmetic)?,
                        )?;
                        let edge = self.edge_id(actual)?;
                        let expected = &self.edges.rows[edge];
                        let range = successor_range(ctx, raw, s)?;
                        if self.operations.rows[id].edges[s] != edge
                            || !expected.alive
                            || expected.op != id
                            || expected.input.is_none()
                            || self.block(actual_target)? != expected.target
                            || range.len() != expected.arguments.len()
                        {
                            return Err(E::Coverage);
                        }
                        for (i, operand) in range.enumerate() {
                            let use_id = self.operations.rows[id].operands[operand];
                            if expected.arguments[i] != use_id
                                || self.uses.rows[use_id].edge_argument.is_none()
                            {
                                return Err(E::Coverage);
                            }
                        }
                    }
                }
            }
        }
        if live_functions.next().is_some()
            || self
                .blocks
                .rows
                .iter()
                .any(|r| r.alive && r.seen != generation)
            || self
                .operations
                .rows
                .iter()
                .any(|r| r.alive && r.seen != generation)
            || self
                .values
                .rows
                .iter()
                .any(|r| r.alive && r.seen != generation)
        {
            return Err(E::Coverage);
        }
        // Owner counts make removal/reinsertion of a Use UID visible even if a
        // malformed operation's current vector accidentally contains duplicates.
        let actual_uses: usize = self
            .operations
            .rows
            .iter()
            .filter(|o| o.alive)
            .map(|o| o.operands.len())
            .sum();
        let actual_edges: usize = self
            .operations
            .rows
            .iter()
            .filter(|o| o.alive)
            .map(|o| o.edges.len())
            .sum();
        if actual_uses != self.uses.rows.iter().filter(|u| u.alive).count()
            || actual_edges != self.edges.rows.iter().filter(|e| e.alive).count()
        {
            return Err(E::Coverage);
        }
        Ok(())
    }

    fn value_definition_arity(&self, ctx: &Context, value: Value) -> Result<usize> {
        self.step(1)?;
        if let Some(op) = value.defining_op() {
            Ok(op.deref(ctx).get_num_results())
        } else {
            Ok(value
                .defining_block()
                .ok_or(E::Coverage)?
                .deref(ctx)
                .get_num_arguments())
        }
    }
    fn value_use_count(&self, ctx: &Context, value: Value) -> Result<usize> {
        let arity = self.value_definition_arity(ctx, value)?;
        self.step(arity)?;
        Ok(value.num_uses(ctx))
    }
    fn value_uses(&self, ctx: &Context, value: Value) -> Result<Vec<Use<Value>>> {
        let count = self.value_use_count(ctx, value)?;
        if count > self.uses.rows.len() {
            return Err(E::Coverage);
        }
        let arity = self.value_definition_arity(ctx, value)?;
        self.step(arity.checked_add(count).ok_or(E::Arithmetic)?)?;
        Ok(value.uses(ctx))
    }

    fn validate_occurrences(&mut self, ctx: &Context, raw: Ptr<Operation>) -> Result<()> {
        let id = self.op(raw)?;
        if self.operations.rows[id].validated_event == self.events {
            return Ok(());
        }
        let op = raw.deref(ctx);
        self.step(
            op.get_num_operands()
                .checked_add(op.get_num_successors())
                .and_then(|n| n.checked_add(op.get_num_results()))
                .ok_or(E::Arithmetic)?,
        )?;
        if self.operations.rows[id].operands.len() != op.get_num_operands()
            || self.operations.rows[id].edges.len() != op.get_num_successors()
            || self.operations.rows[id].results.len() != op.get_num_results()
            || op.num_regions() != 0
        {
            return Err(E::Coverage);
        }
        for (i, raw_value) in op.results().enumerate() {
            if self.operations.rows[id].results[i] != self.value(raw_value)? {
                return Err(E::Coverage);
            }
        }
        for (i, (raw_use, actual_value)) in op.operands_as_uses().zip(op.operands()).enumerate() {
            let use_id = self.use_id(raw_use)?;
            let expected = &self.uses.rows[use_id];
            if self.operations.rows[id].operands[i] != use_id
                || !expected.alive
                || expected.op != id
                || expected.value != self.value(actual_value)?
            {
                return Err(E::Coverage);
            }
        }
        for (successor, (raw_use, actual_target)) in
            op.successors_as_uses().zip(op.successors()).enumerate()
        {
            self.step(
                op.get_num_successors()
                    .checked_add(1)
                    .ok_or(E::Arithmetic)?,
            )?;
            let edge = self.edge_id(raw_use)?;
            let expected = &self.edges.rows[edge];
            let range = successor_range(ctx, raw, successor)?;
            if self.operations.rows[id].edges[successor] != edge
                || !expected.alive
                || expected.op != id
                || expected.target != self.block(actual_target)?
                || expected.arguments.len() != range.len()
            {
                return Err(E::Coverage);
            }
            for (i, operand) in range.enumerate() {
                if expected.arguments[i] != self.operations.rows[id].operands[operand] {
                    return Err(E::Coverage);
                }
            }
        }
        self.operations.rows[id].validated_event = self.events;
        Ok(())
    }

    fn retire_occurrences(&mut self, op: usize) -> Result<()> {
        self.step(
            self.operations.rows[op]
                .operands
                .len()
                .checked_add(self.operations.rows[op].edges.len())
                .ok_or(E::Arithmetic)?,
        )?;
        for &id in &self.operations.rows[op].operands {
            if !self.uses.rows[id].alive {
                return Err(E::Lifecycle);
            }
            self.uses.rows[id].alive = false;
        }
        for &id in &self.operations.rows[op].edges {
            if !self.edges.rows[id].alive {
                return Err(E::Lifecycle);
            }
            self.edges.rows[id].alive = false;
        }
        self.operations.rows[op].operands.clear();
        self.operations.rows[op].edges.clear();
        Ok(())
    }

    fn observe(&mut self, ctx: &Context, event: RewriteEvent) -> Result<()> {
        self.event()?;
        match event {
            RewriteEvent::OperationInserted(raw) => {
                self.step(1)?;
                if let Some(&id) = self.operations.ids.get(&raw) {
                    let parent =
                        self.block(raw.deref(ctx).get_parent_block().ok_or(E::Coverage)?)?;
                    let record = &mut self.operations.rows[id];
                    if !record.alive || record.linked || record.move_to != Some(parent) {
                        return Err(E::Lifecycle);
                    }
                    record.parent = parent;
                    record.move_to = None;
                    record.linked = true;
                } else {
                    self.register_operation(ctx, raw, None)?;
                }
            }
            RewriteEvent::OperationUnlinked(raw) => {
                self.validate_occurrences(ctx, raw)?;
                let id = self.op(raw)?;
                let record = &mut self.operations.rows[id];
                if !record.linked || record.move_to.is_none() {
                    return Err(E::Lifecycle);
                }
                record.linked = false;
            }
            RewriteEvent::OperationErased(raw) => {
                self.validate_occurrences(ctx, raw)?;
                let id = self.op(raw)?;
                if !self.operations.rows[id].linked {
                    return Err(E::Lifecycle);
                }
                self.retire_occurrences(id)?;
                self.step(self.operations.rows[id].results.len())?;
                for &value in &self.operations.rows[id].results {
                    if !self.values.rows[value].alive {
                        return Err(E::Lifecycle);
                    }
                    self.values.rows[value].alive = false;
                }
                self.operations.rows[id].alive = false;
                self.operations.rows[id].linked = false;
            }
            RewriteEvent::ValueReplaced { old, new } => {
                let old_id = self.value(old)?;
                let new_id = self.value(new)?;
                if old_id == new_id {
                    return Ok(());
                }
                // Both upstream UID-to-definition scans and the exact use
                // snapshot are admitted before traversal/allocation.
                let uses = self.value_uses(ctx, old)?;
                for raw_use in &uses {
                    self.validate_occurrences(ctx, raw_use.user_op())?;
                }
                for raw_use in uses {
                    let id = self.use_id(raw_use)?;
                    let row = &mut self.uses.rows[id];
                    if !row.alive || row.value != old_id {
                        return Err(E::Coverage);
                    }
                    row.value = new_id;
                }
                if let Some(producer) = self.values.rows[new_id].producer
                    && self.operations.rows[producer].input.is_none()
                    && self.operations.rows[producer].constant_from.is_none()
                {
                    let anchor = self.values.rows[old_id].anchor.ok_or(E::Coverage)?;
                    self.operations.rows[producer].constant_from = Some(anchor);
                    self.values.rows[new_id].anchor = Some(anchor);
                }
            }
            RewriteEvent::OperationReplaced { old, new } => {
                let old = self.op(old)?;
                let new = self.op(new)?;
                if self.operations.rows[old].selected_replacement != Some(new) {
                    return Err(E::UnsupportedMutation);
                }
            }
            RewriteEvent::BlockErased(raw) => {
                let id = self.block(raw)?;
                if !self.blocks.rows[id].merged && !self.blocks.rows[id].dropped {
                    return Err(E::UnsupportedMutation);
                }
                self.step(raw.deref(ctx).get_num_arguments())?;
                for value in raw.deref(ctx).arguments() {
                    let value = self.value(value)?;
                    self.values.rows[value].alive = false;
                }
                self.blocks.rows[id].alive = false;
            }
            RewriteEvent::BlockInserted(_)
            | RewriteEvent::BlockUnlinked(_)
            | RewriteEvent::RegionErased(_)
            | RewriteEvent::ValueTypeChanged { .. } => return Err(E::UnsupportedMutation),
        }
        Ok(())
    }

    fn occurrence(&mut self, ctx: &Context, event: RewriteOccurrenceEvent) -> Result<()> {
        self.event()?;
        match event {
            RewriteOccurrenceEvent::BlockArgumentErasing { block, argument } => {
                self.block(block)?;
                let value = self.value(argument)?;
                if !matches!(
                    self.values.rows[value].input,
                    Some(Definition::BlockArgument { .. })
                ) || argument.defining_block() != Some(block)
                    || self.value_use_count(ctx, argument)? != 0
                {
                    return Err(E::Coverage);
                }
                self.step(
                    block
                        .deref(ctx)
                        .get_num_arguments()
                        .checked_add(1)
                        .ok_or(E::Arithmetic)?,
                )?;
                let argument_index = argument.find_index(ctx);
                self.step(block.num_preds(ctx))?;
                if block.num_preds(ctx) > self.edges.rows.len() {
                    return Err(E::Coverage);
                }
                let incoming = block.uses(ctx);
                for occurrence in &incoming {
                    self.validate_occurrences(ctx, occurrence.user_op())?;
                }
                for occurrence in incoming {
                    let edge = self.edge_id(occurrence)?;
                    if !self.edges.rows[edge].alive {
                        return Err(E::Lifecycle);
                    }
                    let use_id = *self.edges.rows[edge]
                        .arguments
                        .get(argument_index)
                        .ok_or(E::Coverage)?;
                    let op = self.edges.rows[edge].op;
                    self.step(self.operations.rows[op].operands.len())?;
                    let position = self.operations.rows[op]
                        .operands
                        .iter()
                        .position(|candidate| *candidate == use_id)
                        .ok_or(E::Coverage)?;
                    if !self.uses.rows[use_id].alive {
                        return Err(E::Lifecycle);
                    }
                    self.uses.rows[use_id].alive = false;
                    self.operations.rows[op].operands.remove(position);
                    self.edges.rows[edge].arguments.remove(argument_index);
                }
                self.values.rows[value].alive = false;
            }
            RewriteOccurrenceEvent::BranchSuccessorSelected {
                old,
                new,
                successor,
            } => {
                self.validate_occurrences(ctx, old)?;
                self.validate_occurrences(ctx, new)?;
                let old_id = self.op(old)?;
                let new_id = self.op(new)?;
                if !Operation::is_op::<dialect_gpu::optimization_v1::CondBranchOp>(old, ctx)
                    || !Operation::is_op::<dialect_gpu::optimization_v1::BranchOp>(new, ctx)
                    || self.operations.rows[old_id].selected_replacement.is_some()
                    || self.operations.rows[new_id].input.is_some()
                    || self.operations.rows[new_id].edges.len() != 1
                {
                    return Err(E::UnsupportedMutation);
                }
                let old_edge = *self.operations.rows[old_id]
                    .edges
                    .get(successor)
                    .ok_or(E::Coverage)?;
                let new_edge = self.operations.rows[new_id].edges[0];
                if self.edges.rows[old_edge].target != self.edges.rows[new_edge].target
                    || self.edges.rows[old_edge].arguments.len()
                        != self.edges.rows[new_edge].arguments.len()
                    || self.operations.rows[old_id].parent != self.operations.rows[new_id].parent
                {
                    return Err(E::Coverage);
                }
                self.edges.rows[new_edge].input = self.edges.rows[old_edge].input;
                self.step(self.edges.rows[old_edge].arguments.len())?;
                for i in 0..self.edges.rows[old_edge].arguments.len() {
                    let old_use = self.edges.rows[old_edge].arguments[i];
                    let new_use = self.edges.rows[new_edge].arguments[i];
                    if self.uses.rows[old_use].value != self.uses.rows[new_use].value
                        || self.uses.rows[new_use].input.is_some()
                    {
                        return Err(E::Coverage);
                    }
                    self.uses.rows[new_use].input = self.uses.rows[old_use].input;
                    self.uses.rows[new_use].edge_argument = self.uses.rows[old_use].edge_argument;
                }
                self.operations.rows[old_id].selected_replacement = Some(new_id);
            }
            RewriteOccurrenceEvent::SuccessorBlockMerging {
                predecessor,
                successor,
                terminator,
            } => {
                self.validate_occurrences(ctx, terminator)?;
                let pred = self.block(predecessor)?;
                let succ = self.block(successor)?;
                let term = self.op(terminator)?;
                if pred == succ
                    || self.blocks.rows[succ].merged
                    || self.blocks.rows[succ].dropped
                    || self.operations.rows[term].parent != pred
                    || self.operations.rows[term].edges.len() != 1
                    || successor.num_preds(ctx) != 1
                {
                    return Err(E::Coverage);
                }
                let edge = self.operations.rows[term].edges[0];
                if self.edges.rows[edge].target != succ
                    || self.edges.rows[edge].arguments.len()
                        != successor.deref(ctx).get_num_arguments()
                {
                    return Err(E::Coverage);
                }
                let tail = self.blocks.rows[pred].tail;
                if self.chains[tail].next.is_some() || self.chains[tail].segment.connector.is_some()
                {
                    return Err(E::Lifecycle);
                }
                self.chains[tail].segment.connector =
                    Some(self.edges.rows[edge].input.ok_or(E::Coverage)?);
                self.chains[tail].next = Some(self.blocks.rows[succ].head);
                self.blocks.rows[pred].tail = self.blocks.rows[succ].tail;
                self.blocks.rows[succ].merged = true;
                for raw in successor.deref(ctx).iter(ctx) {
                    self.step(1)?;
                    self.validate_occurrences(ctx, raw)?;
                    let id = self.op(raw)?;
                    if self.operations.rows[id].move_to.is_some() {
                        return Err(E::Lifecycle);
                    }
                    self.operations.rows[id].move_to = Some(pred);
                }
            }
            RewriteOccurrenceEvent::UnreachableBlockDroppingUses { block } => {
                let id = self.block(block)?;
                if self.blocks.rows[id].dropped || self.blocks.rows[id].merged {
                    return Err(E::Lifecycle);
                }
                for raw in block.deref(ctx).iter(ctx) {
                    self.step(1)?;
                    self.validate_occurrences(ctx, raw)?;
                    let op = self.op(raw)?;
                    if self.operations.rows[op].dropped {
                        return Err(E::Lifecycle);
                    }
                    self.retire_occurrences(op)?;
                    self.operations.rows[op].dropped = true;
                }
                self.blocks.rows[id].dropped = true;
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct Capture(Arc<Shared>);
struct Shared {
    state: Mutex<State>,
    poisoned: std::sync::atomic::AtomicBool,
}
impl Capture {
    pub(crate) fn new(
        ctx: &Context,
        root: Ptr<Operation>,
        source: &Module,
        roster: &LiveRosterV12,
        limits: Limits,
        roster_work: usize,
    ) -> Result<Self> {
        Ok(Self(Arc::new(Shared {
            state: Mutex::new(State::new(ctx, root, source, roster, limits, roster_work)?),
            poisoned: std::sync::atomic::AtomicBool::new(false),
        })))
    }
    fn apply(&self, f: impl FnOnce(&mut State) -> Result<()>) -> bool {
        use std::sync::atomic::Ordering;
        if self.0.poisoned.load(Ordering::Relaxed) {
            return false;
        }
        let Ok(mut state) = self.0.state.try_lock() else {
            // An unexpected nested callback cannot deadlock or publish an
            // incomplete trace. The owner discards this session at pass exit.
            self.0.poisoned.store(true, Ordering::Relaxed);
            return false;
        };
        if state.failure.is_some() {
            return false;
        }
        if let Err(error) = f(&mut state) {
            state.failure = Some(error);
            return false;
        }
        true
    }
    pub(crate) fn failure(&self) -> Option<E> {
        if self.0.poisoned.load(std::sync::atomic::Ordering::Relaxed) {
            return Some(E::Lifecycle);
        }
        match self.0.state.try_lock() {
            Ok(state) => state.failure.clone(),
            Err(_) => Some(E::Lifecycle),
        }
    }
    pub(crate) fn begin_pass(
        &self,
        pass: PlironOptimizationPassV1,
        epoch: OperationGraphEpochV1,
    ) -> bool {
        self.apply(|s| {
            if s.current.is_some()
                || s.completed >= 7
                || KIR_PLIRON_PRODUCTION_PASSES_V12[s.completed] != pass
                || s.epoch.is_some_and(|last| last != epoch.sequence())
            {
                return Err(E::Passes);
            }
            s.current = Some((s.completed, epoch.sequence()));
            Ok(())
        })
    }
    pub(crate) fn end_pass(&self, ctx: &Context, epoch: OperationGraphEpochV1) -> bool {
        self.apply(|s| {
            let (pass, input) = s.current.ok_or(E::Passes)?;
            if pass != s.completed || epoch.sequence() < input || epoch.sequence() - input > 1 {
                return Err(E::Passes);
            }
            s.census(ctx, u8::try_from(pass + 2).map_err(|_| E::Arithmetic)?)?;
            s.current = None;
            s.completed += 1;
            s.epoch = Some(epoch.sequence());
            Ok(())
        })
    }
    pub(crate) fn with_roster_meter<T>(
        &self,
        run: impl FnOnce(&mut dyn FnMut(usize) -> Result<()>) -> Result<T>,
    ) -> Result<T> {
        if let Some(error) = self.failure() {
            return Err(error);
        }
        let mut state = self.0.state.try_lock().map_err(|_| E::Lifecycle)?;
        let result = run(&mut |units| state.step(units));
        if let Err(error) = &result {
            state.failure = Some(error.clone());
        }
        result
    }
    pub(crate) fn observer(&self, legacy: Box<dyn RewriteObserver>) -> Box<dyn RewriteObserver> {
        struct Combined {
            legacy: Box<dyn RewriteObserver>,
            occurrences: Capture,
        }
        impl RewriteObserver for Combined {
            fn observe(&mut self, ctx: &Context, event: RewriteEvent) {
                self.legacy.observe(ctx, event);
                self.occurrences.apply(|s| s.observe(ctx, event));
            }
            fn observes_occurrences(&self) -> bool {
                true
            }
            fn observe_occurrence(&mut self, ctx: &Context, event: RewriteOccurrenceEvent) {
                self.occurrences.apply(|s| s.occurrence(ctx, event));
            }
        }
        Box::new(Combined {
            legacy,
            occurrences: self.clone(),
        })
    }
}

/// Move-only immutable row payload. Borrowed candidates remain untrusted until
/// the independent checker admits the exact input/output inventory pair.
#[derive(Debug)]
pub struct KirNeutralOccurrenceRowsV1 {
    functions: Vec<FunctionRow>,
    blocks: Vec<BlockRow>,
    segments: Vec<Segment>,
    operations: Vec<OperationRow>,
    definitions: Vec<DefinitionRow>,
    definition_outputs: Vec<Descendant>,
    uses: Vec<UseRow>,
    edges: Vec<EdgeRow>,
    edge_arguments: Vec<EdgeArgumentRow>,
}
impl KirNeutralOccurrenceRowsV1 {
    pub fn candidate(&self) -> CanonicalKirTransitionCandidateV1<'_> {
        CanonicalKirTransitionCandidateV1 {
            functions: &self.functions,
            blocks: &self.blocks,
            segments: &self.segments,
            operations: &self.operations,
            definitions: &self.definitions,
            definition_outputs: &self.definition_outputs,
            uses: &self.uses,
            edges: &self.edges,
            edge_arguments: &self.edge_arguments,
        }
    }
    pub(crate) fn retained_storage(&self) -> Result<usize> {
        let mut total = size_of::<Self>();
        for (capacity, row) in [
            (self.functions.capacity(), size_of::<FunctionRow>()),
            (self.blocks.capacity(), size_of::<BlockRow>()),
            (self.segments.capacity(), size_of::<Segment>()),
            (self.operations.capacity(), size_of::<OperationRow>()),
            (self.definitions.capacity(), size_of::<DefinitionRow>()),
            (self.definition_outputs.capacity(), size_of::<Descendant>()),
            (self.uses.capacity(), size_of::<UseRow>()),
            (self.edges.capacity(), size_of::<EdgeRow>()),
            (self.edge_arguments.capacity(), size_of::<EdgeArgumentRow>()),
        ] {
            total = total
                .checked_add(capacity.checked_mul(row).ok_or(E::Arithmetic)?)
                .ok_or(E::Arithmetic)?;
        }
        Ok(total)
    }
}

impl Capture {
    pub(crate) fn finish(
        &self,
        ctx: &Context,
        roster: &LiveRosterV12,
        map: &KirOptimizationMapV12,
        output: &Module,
    ) -> Result<KirNeutralOccurrenceRowsV1> {
        if let Some(error) = self.failure() {
            return Err(error);
        }
        let mut state = self.0.state.try_lock().map_err(|_| E::Lifecycle)?;
        let result = (|| {
            if state.completed != 7
                || state.current.is_some()
                || output.functions.len() != state.functions.len()
            {
                return Err(E::Passes);
            }
            state.census(ctx, 9)?;
            state.step(roster.len())?;
            let mut rows = KirNeutralOccurrenceRowsV1 {
                functions: vector(state.functions.len())?,
                blocks: vector(state.blocks.rows.len())?,
                segments: vector(state.chains.len())?,
                operations: vector(state.operations.rows.len())?,
                definitions: vector(state.inputs.len())?,
                definition_outputs: Vec::new(),
                uses: vector(state.uses.rows.len())?,
                edges: vector(state.edges.rows.len())?,
                edge_arguments: vector(state.uses.rows.len())?,
            };
            for (i, function) in state.functions.iter().enumerate() {
                if output.functions[i].body.is_some() != function.live.is_some()
                    || output.functions[i].signature.parameters.len() != function.parameters
                {
                    return Err(E::Identity);
                }
                rows.functions.push(FunctionRow {
                    input: function.coordinate,
                    output: Function(index(i)?),
                });
            }
            let mut previous_block = None;
            for &(key, endpoint) in roster {
                let (LiveKeyV12::Operation(raw), Endpoint::Operation(coordinate)) = (key, endpoint)
                else {
                    continue;
                };
                let id = state.op(raw)?;
                let block = block_coordinate(coordinate)?;
                if previous_block != Some(block) {
                    let start = rows.segments.len();
                    let mut next = Some(state.blocks.rows[state.operations.rows[id].parent].head);
                    while let Some(link) = next {
                        state.step(1)?;
                        if rows.segments.len() >= state.chains.len() {
                            return Err(E::Lifecycle);
                        }
                        let link = &state.chains[link];
                        rows.segments.push(link.segment);
                        next = link.next;
                    }
                    rows.blocks.push(BlockRow {
                        output: block,
                        segments: Range {
                            start: index(start)?,
                            len: index(rows.segments.len() - start)?,
                        },
                    });
                    previous_block = Some(block);
                }
                if matches!(coordinate, Coordinate::Operation { .. }) {
                    let record = &state.operations.rows[id];
                    let origin = match (record.input, record.constant_from) {
                        (Some(input @ Coordinate::Operation { .. }), None) => {
                            Origin::Retained(operation_coordinate(input)?)
                        }
                        (None, Some(definition)) if is_constant(ctx, raw) => {
                            Origin::ConstantFrom(definition)
                        }
                        _ => return Err(E::Coverage),
                    };
                    rows.operations.push(OperationRow {
                        output: operation_coordinate(coordinate)?,
                        origin,
                    });
                }
                let occurrence_count = state.operations.rows[id]
                    .operands
                    .len()
                    .checked_add(state.operations.rows[id].edges.len())
                    .ok_or(E::Arithmetic)?;
                state.step(occurrence_count)?;
                for (operand, &use_id) in state.operations.rows[id].operands.iter().enumerate() {
                    rows.uses.push(UseRow {
                        output: use_coordinate(coordinate, operand)?,
                        input: state.uses.rows[use_id].input.ok_or(E::Coverage)?,
                    });
                }
                for (successor, &edge_id) in state.operations.rows[id].edges.iter().enumerate() {
                    if !matches!(coordinate, Coordinate::Terminator { .. }) {
                        return Err(E::Coverage);
                    }
                    let edge = &state.edges.rows[edge_id];
                    let output = Edge {
                        source: block,
                        successor: index(successor)?,
                    };
                    rows.edges.push(EdgeRow {
                        output,
                        input: edge.input.ok_or(E::Coverage)?,
                    });
                    for (argument, &use_id) in edge.arguments.iter().enumerate() {
                        rows.edge_arguments.push(EdgeArgumentRow {
                            output: EdgeArgument {
                                edge: output,
                                argument: index(argument)?,
                            },
                            input: state.uses.rows[use_id].edge_argument.ok_or(E::Coverage)?,
                        });
                    }
                }
            }
            derive_definition_rows(&mut state, map, &mut rows)?;
            if rows.retained_storage()? > state.limits.storage()? {
                return Err(E::Limit);
            }
            Ok(rows)
        })();
        if let Err(error) = &result {
            state.failure = Some(error.clone());
        }
        result
    }
}

/// Values use the already retained, independently lifecycle-checked old-map
/// witness. This does not infer operands from that map or alter its wire bytes.
fn derive_definition_rows(
    state: &mut State,
    map: &KirOptimizationMapV12,
    rows: &mut KirNeutralOccurrenceRowsV1,
) -> Result<()> {
    let count = map.neutral_node_count_v1();
    if count > state.limits.nodes {
        return Err(E::Limit);
    }
    state.step(
        count
            .checked_add(map.neutral_event_count_v1())
            .ok_or(E::Arithmetic)?,
    )?;
    let mut terminal = vector(count)?;
    terminal.resize(count, None::<Definition>);
    let mut outgoing = vector(count)?;
    outgoing.resize_with(count, Vec::<usize>::new);
    let mut indegree = vector(count)?;
    indegree.resize(count, 0usize);
    let mut sources = HashMap::<Definition, usize>::new();
    sources
        .try_reserve(state.inputs.len())
        .map_err(|_| E::Allocation)?;
    for (id, input, output) in map.neutral_value_nodes_v1() {
        if let Some(input) = input {
            state.step(1)?;
            if sources.insert(definition(input)?, id).is_some() {
                return Err(E::Coverage);
            }
        }
        terminal[id] = output.map(definition).transpose()?;
    }
    let mut edge_count = 0usize;
    for (source, target) in map.neutral_value_edges_v1() {
        state.step(1)?;
        edge_count = edge_count.checked_add(1).ok_or(E::Arithmetic)?;
        if edge_count > state.limits.events || source >= count || target >= count {
            return Err(E::Limit);
        }
        outgoing[source].try_reserve(1).map_err(|_| E::Allocation)?;
        outgoing[source].push(target);
        indegree[target] = indegree[target].checked_add(1).ok_or(E::Arithmetic)?;
    }
    let mut order = vector(count)?;
    for (id, degree) in indegree.iter().enumerate() {
        if *degree == 0 {
            order.push(id);
        }
    }
    let mut consumed = 0;
    while consumed < order.len() {
        let node = order[consumed];
        consumed += 1;
        state.step(outgoing[node].len())?;
        for &target in &outgoing[node] {
            indegree[target] -= 1;
            if indegree[target] == 0 {
                order.push(target);
            }
        }
    }
    if consumed != count {
        return Err(E::Relation);
    }
    let empty = count;
    let multiple = count.checked_add(1).ok_or(E::Arithmetic)?;
    let mut suffix = indegree;
    suffix.fill(empty);
    for &node in order.iter().rev() {
        state.step(outgoing[node].len().checked_add(1).ok_or(E::Arithmetic)?)?;
        let mut summary = if terminal[node].is_some() {
            node
        } else {
            empty
        };
        for &target in &outgoing[node] {
            let other = suffix[target];
            if other != empty {
                summary = if summary == empty || summary == other {
                    other
                } else {
                    multiple
                };
            }
        }
        suffix[node] = summary;
    }
    drop(order);
    let mut seen = vector(count)?;
    seen.resize(count, 0usize);
    let mut stack = vector(edge_count.checked_add(1).ok_or(E::Arithmetic)?)?;
    for ordinal in 0..state.inputs.len() {
        state.step(1)?;
        let input = state.inputs[ordinal];
        let start = rows.definition_outputs.len();
        if let Some(&source) = sources.get(&input) {
            let generation = ordinal.checked_add(1).ok_or(E::Arithmetic)?;
            stack.push(source);
            while let Some(node) = stack.pop() {
                state.step(1)?;
                if seen[node] == generation {
                    continue;
                }
                if suffix[node] == empty {
                    seen[node] = generation;
                    continue;
                }
                if suffix[node] != multiple {
                    let end = suffix[node];
                    if seen[end] != generation {
                        append_descendant(
                            state,
                            rows,
                            terminal[end].ok_or(E::Relation)?,
                            end == source,
                        )?;
                    }
                    seen[end] = generation;
                    seen[node] = generation;
                    continue;
                }
                seen[node] = generation;
                if let Some(output) = terminal[node] {
                    append_descendant(state, rows, output, node == source)?;
                }
                state.step(outgoing[node].len())?;
                if stack
                    .len()
                    .checked_add(outgoing[node].len())
                    .ok_or(E::Arithmetic)?
                    > edge_count + 1
                {
                    return Err(E::Limit);
                }
                stack.extend(outgoing[node].iter().copied());
            }
        } else {
            let Definition::FunctionArgument { function, argument } = input else {
                return Err(E::Coverage);
            };
            let function = state
                .functions
                .get(function.0 as usize)
                .ok_or(E::Coverage)?;
            if function.live.is_some() || argument as usize >= function.parameters {
                return Err(E::Coverage);
            }
            append_descendant(state, rows, input, true)?;
        }
        let len = rows.definition_outputs.len() - start;
        // Comparisons are admitted before sorting. Duplicate output coordinates
        // are rejected, never merged to hide competing retained anchors.
        let log = if len <= 1 {
            0
        } else {
            usize::BITS as usize - (len - 1).leading_zeros() as usize
        };
        state.step(
            len.checked_mul(log.checked_add(1).ok_or(E::Arithmetic)?)
                .ok_or(E::Arithmetic)?,
        )?;
        rows.definition_outputs[start..].sort_unstable_by_key(|row| row.output);
        if rows.definition_outputs[start..]
            .windows(2)
            .any(|pair| pair[0].output == pair[1].output)
        {
            return Err(E::Relation);
        }
        rows.definitions.push(DefinitionRow {
            input,
            outputs: Range {
                start: index(start)?,
                len: index(len)?,
            },
        });
    }
    Ok(())
}
fn append_descendant(
    state: &mut State,
    rows: &mut KirNeutralOccurrenceRowsV1,
    output: Definition,
    retained: bool,
) -> Result<()> {
    state.step(1)?;
    if rows.definition_outputs.len() == state.limits.targets {
        return Err(E::Limit);
    }
    rows.definition_outputs
        .try_reserve(1)
        .map_err(|_| E::Allocation)?;
    rows.definition_outputs.push(Descendant {
        output,
        kind: if retained {
            DescendantKind::Retained
        } else {
            DescendantKind::Substituted
        },
    });
    Ok(())
}
