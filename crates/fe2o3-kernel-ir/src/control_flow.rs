use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::ops::Range;

use crate::{BlockId, Function, MAX_BLOCKS_V1, Terminator, ValueId};

pub const MAX_CFG_EDGES: usize = 1_048_576;
pub const MAX_CFG_EDGE_ARGUMENTS: usize = 1_048_576;
pub const MAX_CFG_PHI_INPUTS: usize = 1_048_576;
pub const MAX_CFG_ANALYSIS_WORK: u64 = 16_777_216;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlFlowLimits {
    pub blocks: usize,
    pub edges: usize,
    pub edge_arguments: usize,
    pub phi_inputs: usize,
    pub analysis_work: u64,
}

impl ControlFlowLimits {
    pub const DEFAULT: Self = Self {
        blocks: MAX_BLOCKS_V1,
        edges: MAX_CFG_EDGES,
        edge_arguments: MAX_CFG_EDGE_ARGUMENTS,
        phi_inputs: MAX_CFG_PHI_INPUTS,
        analysis_work: MAX_CFG_ANALYSIS_WORK,
    };
}

impl Default for ControlFlowLimits {
    fn default() -> Self {
        Self::DEFAULT
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ControlFlowResource {
    Blocks,
    Edges,
    EdgeArguments,
    PhiInputs,
    AnalysisWork,
}

impl fmt::Display for ControlFlowResource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Blocks => "CFG blocks",
            Self::Edges => "CFG edges",
            Self::EdgeArguments => "CFG edge arguments",
            Self::PhiInputs => "CFG phi inputs",
            Self::AnalysisWork => "CFG analysis work units",
        };
        formatter.write_str(name)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ControlFlowError {
    EmptyFunction,
    DuplicateBlock(BlockId),
    MissingTerminator(BlockId),
    UnknownSuccessor {
        source: BlockId,
        target: BlockId,
    },
    ResourceLimit {
        resource: ControlFlowResource,
        limit: u64,
        actual: u64,
    },
    ArithmeticOverflow(ControlFlowResource),
}

impl fmt::Display for ControlFlowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFunction => formatter.write_str("defined function has no entry block"),
            Self::DuplicateBlock(block) => {
                write!(formatter, "block {block} is defined more than once")
            }
            Self::MissingTerminator(block) => write!(formatter, "block {block} has no terminator"),
            Self::UnknownSuccessor { source, target } => {
                write!(formatter, "block {source} has unknown successor {target}")
            }
            Self::ResourceLimit {
                resource,
                limit,
                actual,
            } => write!(
                formatter,
                "{resource} exceed the deterministic limit {limit}: found {actual}"
            ),
            Self::ArithmeticOverflow(resource) => {
                write!(formatter, "{resource} overflow their deterministic counter")
            }
        }
    }
}

impl std::error::Error for ControlFlowError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IndexedControlFlowEdge {
    source: usize,
    target: usize,
    ordinal: usize,
    argument_count: usize,
}

impl IndexedControlFlowEdge {
    pub fn ordinal(self) -> usize {
        self.ordinal
    }

    pub fn argument_count(self) -> usize {
        self.argument_count
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControlFlowWork {
    pub index_units: u64,
    pub reachability_edge_visits: u64,
    pub depth_first_edge_visits: u64,
    pub dominator_predecessor_visits: u64,
    pub dominator_climbs: u64,
    pub interval_node_visits: u64,
    pub reducibility_edge_visits: u64,
    pub reducibility_node_visits: u64,
    pub total: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct IndexedControlFlow {
    block_ids: Vec<BlockId>,
    block_positions: BTreeMap<BlockId, usize>,
    edges: Vec<IndexedControlFlowEdge>,
    outgoing: Vec<Range<usize>>,
    incoming: Vec<Vec<usize>>,
    successors: Vec<Vec<usize>>,
    predecessors: Vec<Vec<usize>>,
    reachable: Vec<bool>,
    dominator_preorder: Vec<u32>,
    dominator_postorder: Vec<u32>,
    irreducible_blocks: Vec<BlockId>,
    edge_arguments: usize,
    phi_inputs: usize,
    work: ControlFlowWork,
}

impl IndexedControlFlow {
    pub fn block_count(&self) -> usize {
        self.block_ids.len()
    }

    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }

    pub fn edge_argument_count(&self) -> usize {
        self.edge_arguments
    }

    pub fn phi_input_count(&self) -> usize {
        self.phi_inputs
    }

    pub fn work(&self) -> ControlFlowWork {
        self.work
    }

    pub fn block_position(&self, block: BlockId) -> Option<usize> {
        self.block_positions.get(&block).copied()
    }

    pub fn block_id(&self, position: usize) -> Option<BlockId> {
        self.block_ids.get(position).copied()
    }

    pub fn edge(&self, edge: usize) -> Option<IndexedControlFlowEdge> {
        self.edges.get(edge).copied()
    }

    pub fn edge_source(&self, edge: usize) -> Option<BlockId> {
        self.edge(edge).map(|edge| self.block_ids[edge.source])
    }

    pub fn edge_target(&self, edge: usize) -> Option<BlockId> {
        self.edge(edge).map(|edge| self.block_ids[edge.target])
    }

    pub fn outgoing_edges(&self, block: BlockId) -> Option<Range<usize>> {
        let position = self.block_position(block)?;
        Some(self.outgoing[position].clone())
    }

    pub fn incoming_edges(&self, block: BlockId) -> Option<&[usize]> {
        let position = self.block_position(block)?;
        Some(&self.incoming[position])
    }

    pub fn successor_blocks(&self, block: BlockId) -> Option<impl Iterator<Item = BlockId> + '_> {
        let position = self.block_position(block)?;
        Some(
            self.successors[position]
                .iter()
                .map(|successor| self.block_ids[*successor]),
        )
    }

    pub fn predecessor_blocks(&self, block: BlockId) -> Option<impl Iterator<Item = BlockId> + '_> {
        let position = self.block_position(block)?;
        Some(
            self.predecessors[position]
                .iter()
                .map(|predecessor| self.block_ids[*predecessor]),
        )
    }

    pub fn is_reachable(&self, block: BlockId) -> bool {
        self.block_position(block)
            .is_some_and(|position| self.reachable[position])
    }

    pub fn dominates(&self, definition: BlockId, use_block: BlockId) -> bool {
        let (Some(definition), Some(use_block)) = (
            self.block_position(definition),
            self.block_position(use_block),
        ) else {
            return false;
        };
        if !self.reachable[use_block] {
            return definition == use_block;
        }
        self.reachable[definition]
            && self.dominator_preorder[definition] <= self.dominator_preorder[use_block]
            && self.dominator_postorder[use_block] <= self.dominator_postorder[definition]
    }

    pub fn irreducible_blocks(&self) -> &[BlockId] {
        &self.irreducible_blocks
    }

    pub fn is_reducible(&self) -> bool {
        self.irreducible_blocks.is_empty()
    }

    pub fn edge_arguments<'a>(&self, function: &'a Function, edge: usize) -> &'a [ValueId] {
        let edge = self.edges[edge];
        let body = function
            .body
            .as_ref()
            .expect("analyzed function has a body");
        let terminator = body.blocks[edge.source]
            .terminator
            .as_ref()
            .expect("analyzed block has a terminator");
        terminator_edge(terminator, edge.ordinal)
            .expect("indexed edge ordinal remains valid")
            .1
    }
}

pub fn analyze_control_flow(function: &Function) -> Result<IndexedControlFlow, ControlFlowError> {
    analyze_control_flow_with_limits(function, ControlFlowLimits::DEFAULT)
}

pub fn analyze_control_flow_with_limits(
    function: &Function,
    limits: ControlFlowLimits,
) -> Result<IndexedControlFlow, ControlFlowError> {
    let body = function
        .body
        .as_ref()
        .ok_or(ControlFlowError::EmptyFunction)?;
    check_limit(
        ControlFlowResource::Blocks,
        body.blocks.len(),
        limits.blocks,
    )?;
    if body.blocks.is_empty() {
        return Err(ControlFlowError::EmptyFunction);
    }

    let mut block_ids = Vec::with_capacity(body.blocks.len());
    let mut block_positions = BTreeMap::new();
    for (position, block) in body.blocks.iter().enumerate() {
        if block_positions.insert(block.id, position).is_some() {
            return Err(ControlFlowError::DuplicateBlock(block.id));
        }
        block_ids.push(block.id);
        if block.terminator.is_none() {
            return Err(ControlFlowError::MissingTerminator(block.id));
        }
    }

    // Count every aggregate before allocating storage proportional to edge or phi volume.
    let mut edge_count = 0usize;
    let mut edge_arguments = 0usize;
    let mut incoming_counts = vec![0usize; body.blocks.len()];
    for block in &body.blocks {
        let terminator = block.terminator.as_ref().expect("checked terminator");
        for_each_terminator_edge(terminator, |target, arguments| {
            edge_count = checked_add(ControlFlowResource::Edges, edge_count, 1)?;
            edge_arguments = checked_add(
                ControlFlowResource::EdgeArguments,
                edge_arguments,
                arguments.len(),
            )?;
            let Some(target_position) = block_positions.get(&target).copied() else {
                return Err(ControlFlowError::UnknownSuccessor {
                    source: block.id,
                    target,
                });
            };
            incoming_counts[target_position] = checked_add(
                ControlFlowResource::Edges,
                incoming_counts[target_position],
                1,
            )?;
            Ok(())
        })?;
    }
    check_limit(ControlFlowResource::Edges, edge_count, limits.edges)?;
    check_limit(
        ControlFlowResource::EdgeArguments,
        edge_arguments,
        limits.edge_arguments,
    )?;

    let mut phi_inputs = 0usize;
    for (block, incoming) in body.blocks.iter().zip(&incoming_counts) {
        let block_phi_inputs = checked_mul(
            ControlFlowResource::PhiInputs,
            block.parameters.len(),
            *incoming,
        )?;
        phi_inputs = checked_add(ControlFlowResource::PhiInputs, phi_inputs, block_phi_inputs)?;
    }
    check_limit(
        ControlFlowResource::PhiInputs,
        phi_inputs,
        limits.phi_inputs,
    )?;

    let mut edges = Vec::with_capacity(edge_count);
    let mut outgoing = Vec::with_capacity(body.blocks.len());
    for (source, block) in body.blocks.iter().enumerate() {
        let start = edges.len();
        let terminator = block.terminator.as_ref().expect("checked terminator");
        let mut ordinal = 0usize;
        for_each_terminator_edge(terminator, |target, arguments| {
            edges.push(IndexedControlFlowEdge {
                source,
                target: block_positions[&target],
                ordinal,
                argument_count: arguments.len(),
            });
            ordinal += 1;
            Ok(())
        })?;
        outgoing.push(start..edges.len());
    }

    let mut incoming = incoming_counts
        .iter()
        .map(|count| Vec::with_capacity(*count))
        .collect::<Vec<_>>();
    for (edge_index, edge) in edges.iter().enumerate() {
        incoming[edge.target].push(edge_index);
    }

    let mut successors = Vec::with_capacity(body.blocks.len());
    for range in &outgoing {
        let mut targets = edges[range.clone()]
            .iter()
            .map(|edge| edge.target)
            .collect::<Vec<_>>();
        targets.sort_unstable();
        targets.dedup();
        successors.push(targets);
    }
    let mut predecessors = vec![Vec::new(); body.blocks.len()];
    for (source, targets) in successors.iter().enumerate() {
        for target in targets {
            predecessors[*target].push(source);
        }
    }

    let mut meter = WorkMeter::new(limits.analysis_work);
    meter.charge_index(
        u64::try_from(body.blocks.len())
            .unwrap_or(u64::MAX)
            .saturating_add(
                u64::try_from(edge_count)
                    .unwrap_or(u64::MAX)
                    .saturating_mul(2),
            )
            .saturating_add(u64::try_from(phi_inputs).unwrap_or(u64::MAX)),
    )?;
    let reachable = compute_reachable(&successors, &mut meter)?;
    let reverse_postorder = compute_reverse_postorder(&successors, &reachable, &mut meter)?;
    let immediate_dominators =
        compute_immediate_dominators(&predecessors, &reachable, &reverse_postorder, &mut meter)?;
    let (dominator_preorder, dominator_postorder) =
        compute_dominator_intervals(&immediate_dominators, &reachable, &mut meter)?;
    let irreducible_blocks = compute_irreducible_blocks(
        &block_ids,
        &successors,
        &reachable,
        &dominator_preorder,
        &dominator_postorder,
        &mut meter,
    )?;

    Ok(IndexedControlFlow {
        block_ids,
        block_positions,
        edges,
        outgoing,
        incoming,
        successors,
        predecessors,
        reachable,
        dominator_preorder,
        dominator_postorder,
        irreducible_blocks,
        edge_arguments,
        phi_inputs,
        work: meter.work,
    })
}

fn compute_reachable(
    successors: &[Vec<usize>],
    meter: &mut WorkMeter,
) -> Result<Vec<bool>, ControlFlowError> {
    let mut reachable = vec![false; successors.len()];
    let mut pending = vec![0usize];
    reachable[0] = true;
    while let Some(block) = pending.pop() {
        for successor in &successors[block] {
            meter.charge_reachability_edge()?;
            if !reachable[*successor] {
                reachable[*successor] = true;
                pending.push(*successor);
            }
        }
    }
    Ok(reachable)
}

fn compute_reverse_postorder(
    successors: &[Vec<usize>],
    reachable: &[bool],
    meter: &mut WorkMeter,
) -> Result<Vec<usize>, ControlFlowError> {
    let mut visited = vec![false; successors.len()];
    let mut postorder = Vec::with_capacity(reachable.iter().filter(|value| **value).count());
    let mut stack = vec![(0usize, 0usize)];
    visited[0] = true;
    while let Some((block, next_successor)) = stack.last_mut() {
        if *next_successor == successors[*block].len() {
            postorder.push(*block);
            stack.pop();
            continue;
        }
        let successor = successors[*block][*next_successor];
        *next_successor += 1;
        meter.charge_depth_first_edge()?;
        if reachable[successor] && !visited[successor] {
            visited[successor] = true;
            stack.push((successor, 0));
        }
    }
    postorder.reverse();
    Ok(postorder)
}

fn compute_immediate_dominators(
    predecessors: &[Vec<usize>],
    reachable: &[bool],
    reverse_postorder: &[usize],
    meter: &mut WorkMeter,
) -> Result<Vec<Option<usize>>, ControlFlowError> {
    let mut order = vec![usize::MAX; predecessors.len()];
    for (position, block) in reverse_postorder.iter().copied().enumerate() {
        order[block] = position;
    }
    let mut dominators = vec![None; predecessors.len()];
    dominators[0] = Some(0);

    loop {
        let mut changed = false;
        for block in reverse_postorder.iter().copied().skip(1) {
            let mut candidates = predecessors[block].iter().copied().filter(|predecessor| {
                reachable[*predecessor] && dominators[*predecessor].is_some()
            });
            let Some(mut next) = candidates.next() else {
                continue;
            };
            meter.charge_dominator_predecessor()?;
            for predecessor in candidates {
                meter.charge_dominator_predecessor()?;
                next = intersect_dominators(next, predecessor, &dominators, &order, meter)?;
            }
            if dominators[block] != Some(next) {
                dominators[block] = Some(next);
                changed = true;
            }
        }
        if !changed {
            break;
        }
    }
    dominators[0] = None;
    Ok(dominators)
}

fn intersect_dominators(
    mut left: usize,
    mut right: usize,
    dominators: &[Option<usize>],
    order: &[usize],
    meter: &mut WorkMeter,
) -> Result<usize, ControlFlowError> {
    while left != right {
        while order[left] > order[right] {
            meter.charge_dominator_climb()?;
            left = dominators[left].unwrap_or(0);
        }
        while order[right] > order[left] {
            meter.charge_dominator_climb()?;
            right = dominators[right].unwrap_or(0);
        }
    }
    Ok(left)
}

fn compute_dominator_intervals(
    immediate_dominators: &[Option<usize>],
    reachable: &[bool],
    meter: &mut WorkMeter,
) -> Result<(Vec<u32>, Vec<u32>), ControlFlowError> {
    let mut children = vec![Vec::new(); immediate_dominators.len()];
    for (block, parent) in immediate_dominators.iter().copied().enumerate().skip(1) {
        if reachable[block] {
            children[parent.expect("reachable non-entry block has an idom")].push(block);
        }
    }
    let mut preorder = vec![0u32; immediate_dominators.len()];
    let mut postorder = vec![0u32; immediate_dominators.len()];
    let mut clock = 0u32;
    let mut stack = vec![(0usize, false)];
    while let Some((block, exiting)) = stack.pop() {
        meter.charge_interval_node()?;
        if exiting {
            postorder[block] = clock;
            clock = clock.saturating_add(1);
            continue;
        }
        preorder[block] = clock;
        clock = clock.saturating_add(1);
        stack.push((block, true));
        stack.extend(children[block].iter().rev().map(|child| (*child, false)));
    }
    Ok((preorder, postorder))
}

fn compute_irreducible_blocks(
    block_ids: &[BlockId],
    successors: &[Vec<usize>],
    reachable: &[bool],
    preorder: &[u32],
    postorder: &[u32],
    meter: &mut WorkMeter,
) -> Result<Vec<BlockId>, ControlFlowError> {
    let dominates = |definition: usize, use_block: usize| {
        if !reachable[use_block] {
            return definition == use_block;
        }
        reachable[definition]
            && preorder[definition] <= preorder[use_block]
            && postorder[use_block] <= postorder[definition]
    };
    let mut forward = vec![Vec::new(); successors.len()];
    let mut indegrees = vec![0usize; successors.len()];
    for (source, targets) in successors.iter().enumerate() {
        for target in targets {
            meter.charge_reducibility_edge()?;
            if dominates(*target, source) {
                continue;
            }
            forward[source].push(*target);
            indegrees[*target] += 1;
        }
    }

    let mut ready = indegrees
        .iter()
        .enumerate()
        .filter_map(|(block, count)| (*count == 0).then_some(block))
        .collect::<VecDeque<_>>();
    let mut visited = vec![false; successors.len()];
    while let Some(block) = ready.pop_front() {
        meter.charge_reducibility_node()?;
        visited[block] = true;
        for successor in &forward[block] {
            let count = &mut indegrees[*successor];
            *count -= 1;
            if *count == 0 {
                ready.push_back(*successor);
            }
        }
    }
    let mut irreducible = block_ids
        .iter()
        .copied()
        .zip(visited)
        .filter_map(|(block, visited)| (!visited).then_some(block))
        .collect::<Vec<_>>();
    irreducible.sort_unstable();
    Ok(irreducible)
}

fn for_each_terminator_edge(
    terminator: &Terminator,
    mut visit: impl FnMut(BlockId, &[ValueId]) -> Result<(), ControlFlowError>,
) -> Result<(), ControlFlowError> {
    match terminator {
        Terminator::Branch { target, arguments } => visit(*target, arguments)?,
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => {
            visit(*then_target, then_arguments)?;
            visit(*else_target, else_arguments)?;
        }
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            for case in cases {
                visit(case.target, &case.arguments)?;
            }
            visit(*default_target, default_arguments)?;
        }
        Terminator::IntegerSwitch {
            cases,
            default_target,
            default_arguments,
            ..
        } => {
            for case in cases {
                visit(case.target, &case.arguments)?;
            }
            visit(*default_target, default_arguments)?;
        }
        Terminator::Return { .. } | Terminator::Unreachable => {}
    }
    Ok(())
}

fn terminator_edge(terminator: &Terminator, ordinal: usize) -> Option<(BlockId, &[ValueId])> {
    match terminator {
        Terminator::Branch { target, arguments } => {
            (ordinal == 0).then_some((*target, arguments.as_slice()))
        }
        Terminator::ConditionalBranch {
            then_target,
            then_arguments,
            else_target,
            else_arguments,
            ..
        } => match ordinal {
            0 => Some((*then_target, then_arguments)),
            1 => Some((*else_target, else_arguments)),
            _ => None,
        },
        Terminator::Switch {
            cases,
            default_target,
            default_arguments,
            ..
        } => cases
            .get(ordinal)
            .map(|case| (case.target, case.arguments.as_slice()))
            .or_else(|| {
                (ordinal == cases.len()).then_some((*default_target, default_arguments.as_slice()))
            }),
        Terminator::IntegerSwitch {
            cases,
            default_target,
            default_arguments,
            ..
        } => cases
            .get(ordinal)
            .map(|case| (case.target, case.arguments.as_slice()))
            .or_else(|| {
                (ordinal == cases.len()).then_some((*default_target, default_arguments.as_slice()))
            }),
        Terminator::Return { .. } | Terminator::Unreachable => None,
    }
}

fn check_limit(
    resource: ControlFlowResource,
    actual: usize,
    limit: usize,
) -> Result<(), ControlFlowError> {
    if actual > limit {
        return Err(ControlFlowError::ResourceLimit {
            resource,
            limit: u64::try_from(limit).unwrap_or(u64::MAX),
            actual: u64::try_from(actual).unwrap_or(u64::MAX),
        });
    }
    Ok(())
}

fn checked_add(
    resource: ControlFlowResource,
    left: usize,
    right: usize,
) -> Result<usize, ControlFlowError> {
    left.checked_add(right)
        .ok_or(ControlFlowError::ArithmeticOverflow(resource))
}

fn checked_mul(
    resource: ControlFlowResource,
    left: usize,
    right: usize,
) -> Result<usize, ControlFlowError> {
    left.checked_mul(right)
        .ok_or(ControlFlowError::ArithmeticOverflow(resource))
}

struct WorkMeter {
    limit: u64,
    work: ControlFlowWork,
}

impl WorkMeter {
    fn new(limit: u64) -> Self {
        Self {
            limit,
            work: ControlFlowWork::default(),
        }
    }

    fn charge_index(&mut self, units: u64) -> Result<(), ControlFlowError> {
        self.work.index_units = self.work.index_units.saturating_add(units);
        self.charge_total(units)
    }

    fn charge_reachability_edge(&mut self) -> Result<(), ControlFlowError> {
        self.work.reachability_edge_visits += 1;
        self.charge_total(1)
    }

    fn charge_depth_first_edge(&mut self) -> Result<(), ControlFlowError> {
        self.work.depth_first_edge_visits += 1;
        self.charge_total(1)
    }

    fn charge_dominator_predecessor(&mut self) -> Result<(), ControlFlowError> {
        self.work.dominator_predecessor_visits += 1;
        self.charge_total(1)
    }

    fn charge_dominator_climb(&mut self) -> Result<(), ControlFlowError> {
        self.work.dominator_climbs += 1;
        self.charge_total(1)
    }

    fn charge_interval_node(&mut self) -> Result<(), ControlFlowError> {
        self.work.interval_node_visits += 1;
        self.charge_total(1)
    }

    fn charge_reducibility_edge(&mut self) -> Result<(), ControlFlowError> {
        self.work.reducibility_edge_visits += 1;
        self.charge_total(1)
    }

    fn charge_reducibility_node(&mut self) -> Result<(), ControlFlowError> {
        self.work.reducibility_node_visits += 1;
        self.charge_total(1)
    }

    fn charge_total(&mut self, units: u64) -> Result<(), ControlFlowError> {
        self.work.total =
            self.work
                .total
                .checked_add(units)
                .ok_or(ControlFlowError::ArithmeticOverflow(
                    ControlFlowResource::AnalysisWork,
                ))?;
        if self.work.total > self.limit {
            return Err(ControlFlowError::ResourceLimit {
                resource: ControlFlowResource::AnalysisWork,
                limit: self.limit,
                actual: self.work.total,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_counters_reject_overflow() {
        assert_eq!(
            checked_add(ControlFlowResource::Edges, usize::MAX, 1),
            Err(ControlFlowError::ArithmeticOverflow(
                ControlFlowResource::Edges
            ))
        );
        assert_eq!(
            checked_mul(ControlFlowResource::PhiInputs, usize::MAX, 2),
            Err(ControlFlowError::ArithmeticOverflow(
                ControlFlowResource::PhiInputs
            ))
        );
    }
}
