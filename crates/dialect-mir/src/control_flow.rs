use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::executable::terminator_edges;
use crate::{MAX_EXECUTABLE_BLOCKS, MirBlockId, MirBody};

/// Deterministic analysis budget derived from the executable MIR block limit.
pub const MIR_CONTROL_FLOW_WORK_UNITS_PER_BLOCK: usize = 64;
pub const MAX_MIR_CONTROL_FLOW_WORK_UNITS: usize =
    MAX_EXECUTABLE_BLOCKS * MIR_CONTROL_FLOW_WORK_UNITS_PER_BLOCK;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MirControlFlowEdge {
    pub source: MirBlockId,
    pub target: MirBlockId,
}

impl fmt::Display for MirControlFlowEdge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "bb{} -> bb{}", self.source.0, self.target.0)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MirControlFlowError {
    EmptyBody,
    BlockLimitExceeded {
        block_count: usize,
        limit: usize,
    },
    InvalidEntry {
        entry: MirBlockId,
        block_count: usize,
    },
    UnknownSuccessor(MirControlFlowEdge),
    UnreachableBlock(MirBlockId),
    Irreducible {
        blocks: Vec<MirBlockId>,
        entries: Vec<MirControlFlowEdge>,
    },
    WorkBudgetExceeded {
        consumed: usize,
        limit: usize,
    },
}

impl fmt::Display for MirControlFlowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBody => formatter.write_str("control-flow body has no blocks"),
            Self::BlockLimitExceeded { block_count, limit } => write!(
                formatter,
                "control-flow body has {block_count} blocks, exceeding the schema limit {limit}"
            ),
            Self::InvalidEntry { entry, block_count } => write!(
                formatter,
                "entry bb{} is outside the canonical block range 0..{block_count}",
                entry.0
            ),
            Self::UnknownSuccessor(edge) => {
                write!(
                    formatter,
                    "control-flow edge {edge} targets an unknown block"
                )
            }
            Self::UnreachableBlock(block) => {
                write!(formatter, "bb{} is unreachable from the entry", block.0)
            }
            Self::Irreducible { blocks, entries } => write!(
                formatter,
                "irreducible control flow in {}; entries: {}",
                display_blocks(blocks),
                display_edges(entries)
            ),
            Self::WorkBudgetExceeded { consumed, limit } => write!(
                formatter,
                "control-flow analysis consumed {consumed} work units, exceeding the deterministic limit {limit}"
            ),
        }
    }
}

impl std::error::Error for MirControlFlowError {}

/// Deterministic graph facts for a canonical, fully reachable, reducible MIR body.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MirControlFlowAnalysis {
    entry: MirBlockId,
    successors: Vec<BTreeSet<MirBlockId>>,
    predecessors: Vec<BTreeSet<MirBlockId>>,
    immediate_dominators: Vec<Option<MirBlockId>>,
    dominator_tree_children: Vec<BTreeSet<MirBlockId>>,
    dominator_preorder: Vec<usize>,
    dominator_subtree_end: Vec<usize>,
    dominance_frontiers: Vec<BTreeSet<MirBlockId>>,
    backedges: BTreeSet<MirControlFlowEdge>,
    loop_bodies: BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
    loop_latches: BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
    work_units: usize,
}

impl MirControlFlowAnalysis {
    pub const fn entry(&self) -> MirBlockId {
        self.entry
    }

    pub fn block_count(&self) -> usize {
        self.successors.len()
    }

    pub fn successors(&self, block: MirBlockId) -> Option<&BTreeSet<MirBlockId>> {
        self.successors.get(block.0 as usize)
    }

    pub fn predecessors(&self, block: MirBlockId) -> Option<&BTreeSet<MirBlockId>> {
        self.predecessors.get(block.0 as usize)
    }

    /// Materializes this block's dominators on demand from the immediate-
    /// dominator tree. Analysis itself never stores quadratic all-sets.
    pub fn dominators(&self, block: MirBlockId) -> Option<BTreeSet<MirBlockId>> {
        if block.0 as usize >= self.block_count() {
            return None;
        }
        let mut dominators = BTreeSet::new();
        let mut current = block;
        loop {
            dominators.insert(current);
            if current == self.entry {
                return Some(dominators);
            }
            current = self.immediate_dominators[current.0 as usize]?;
        }
    }

    pub fn dominates(&self, dominator: MirBlockId, block: MirBlockId) -> bool {
        let dominator_index = dominator.0 as usize;
        let block_index = block.0 as usize;
        if dominator_index >= self.block_count() || block_index >= self.block_count() {
            return false;
        }
        let start = self.dominator_preorder[dominator_index];
        let candidate = self.dominator_preorder[block_index];
        start <= candidate && candidate < self.dominator_subtree_end[dominator_index]
    }

    pub fn immediate_dominator(&self, block: MirBlockId) -> Option<Option<MirBlockId>> {
        self.immediate_dominators.get(block.0 as usize).copied()
    }

    pub fn dominator_tree_children(&self, block: MirBlockId) -> Option<&BTreeSet<MirBlockId>> {
        self.dominator_tree_children.get(block.0 as usize)
    }

    pub fn dominance_frontier(&self, block: MirBlockId) -> Option<&BTreeSet<MirBlockId>> {
        self.dominance_frontiers.get(block.0 as usize)
    }

    pub fn iterated_dominance_frontier(
        &self,
        definitions: &BTreeSet<MirBlockId>,
    ) -> Option<BTreeSet<MirBlockId>> {
        if definitions
            .iter()
            .any(|block| block.0 as usize >= self.block_count())
        {
            return None;
        }
        let mut result = BTreeSet::new();
        let mut pending = definitions.clone();
        while let Some(block) = pending.pop_first() {
            for frontier in &self.dominance_frontiers[block.0 as usize] {
                if result.insert(*frontier) && !definitions.contains(frontier) {
                    pending.insert(*frontier);
                }
            }
        }
        Some(result)
    }

    pub fn backedges(&self) -> &BTreeSet<MirControlFlowEdge> {
        &self.backedges
    }

    pub fn loop_headers(&self) -> impl Iterator<Item = MirBlockId> + '_ {
        self.loop_bodies.keys().copied()
    }

    pub fn loop_body(&self, header: MirBlockId) -> Option<&BTreeSet<MirBlockId>> {
        self.loop_bodies.get(&header)
    }

    pub fn loop_latches(&self, header: MirBlockId) -> Option<&BTreeSet<MirBlockId>> {
        self.loop_latches.get(&header)
    }

    /// Deterministic work units consumed while constructing this analysis.
    pub const fn work_units(&self) -> usize {
        self.work_units
    }
}

#[derive(Default)]
struct WorkBudget {
    consumed: usize,
}

impl WorkBudget {
    fn charge(&mut self, units: usize) -> Result<(), MirControlFlowError> {
        let next = self
            .consumed
            .checked_add(units)
            .unwrap_or(MAX_MIR_CONTROL_FLOW_WORK_UNITS + 1);
        if next > MAX_MIR_CONTROL_FLOW_WORK_UNITS {
            return Err(MirControlFlowError::WorkBudgetExceeded {
                consumed: next,
                limit: MAX_MIR_CONTROL_FLOW_WORK_UNITS,
            });
        }
        self.consumed = next;
        Ok(())
    }
}

/// Analyzes a body without trusting its edge metadata.
///
/// Dead blocks and irreducible regions are rejected. This matches executable
/// MIR's canonical policy and prevents later passes from silently assigning
/// semantics to source-order-only blocks.
pub fn analyze_mir_control_flow(
    body: &MirBody,
) -> Result<MirControlFlowAnalysis, MirControlFlowError> {
    let block_count = body.blocks.len();
    if block_count == 0 {
        return Err(MirControlFlowError::EmptyBody);
    }
    if block_count > MAX_EXECUTABLE_BLOCKS {
        return Err(MirControlFlowError::BlockLimitExceeded {
            block_count,
            limit: MAX_EXECUTABLE_BLOCKS,
        });
    }
    if body.entry.0 as usize >= block_count {
        return Err(MirControlFlowError::InvalidEntry {
            entry: body.entry,
            block_count,
        });
    }

    let mut budget = WorkBudget::default();
    budget.charge(block_count)?;
    let mut successors = vec![BTreeSet::new(); block_count];
    for (source_index, block) in body.blocks.iter().enumerate() {
        let source = MirBlockId(source_index as u32);
        for edge in terminator_edges(&block.terminator.kind) {
            budget.charge(1)?;
            if edge.target.0 as usize >= block_count {
                return Err(MirControlFlowError::UnknownSuccessor(MirControlFlowEdge {
                    source,
                    target: edge.target,
                }));
            }
            successors[source_index].insert(edge.target);
        }
    }

    let mut reachable = BTreeSet::new();
    let mut pending = VecDeque::from([body.entry]);
    while let Some(block) = pending.pop_front() {
        budget.charge(1)?;
        if reachable.insert(block) {
            for successor in &successors[block.0 as usize] {
                budget.charge(1)?;
                pending.push_back(*successor);
            }
        }
    }
    if reachable.len() != block_count {
        let block = (0..block_count)
            .map(|index| MirBlockId(index as u32))
            .find(|block| !reachable.contains(block))
            .expect("a short reachable set omits a block");
        return Err(MirControlFlowError::UnreachableBlock(block));
    }

    let mut predecessors = vec![BTreeSet::new(); block_count];
    for (source_index, targets) in successors.iter().enumerate() {
        let source = MirBlockId(source_index as u32);
        for target in targets {
            budget.charge(1)?;
            predecessors[target.0 as usize].insert(source);
        }
    }
    let reverse_postorder = compute_reverse_postorder(body.entry, &successors, &mut budget)?;
    let immediate_dominators =
        compute_immediate_dominators(body.entry, &predecessors, &reverse_postorder, &mut budget)?;
    let dominator_tree_children = compute_dominator_tree(&immediate_dominators);
    let (dominator_preorder, dominator_subtree_end) =
        compute_dominator_intervals(body.entry, &dominator_tree_children, &mut budget)?;
    let dominance_frontiers = compute_frontiers(
        &successors,
        &immediate_dominators,
        &dominator_tree_children,
        body.entry,
        &mut budget,
    )?;
    let backedges = compute_backedges(
        &successors,
        &dominator_preorder,
        &dominator_subtree_end,
        &mut budget,
    )?;
    if let Some(error) = find_irreducible(&successors, &backedges, &mut budget)? {
        return Err(error);
    }
    let (loop_bodies, loop_latches) =
        compute_natural_loops(&predecessors, &backedges, &mut budget)?;

    Ok(MirControlFlowAnalysis {
        entry: body.entry,
        successors,
        predecessors,
        immediate_dominators,
        dominator_tree_children,
        dominator_preorder,
        dominator_subtree_end,
        dominance_frontiers,
        backedges,
        loop_bodies,
        loop_latches,
        work_units: budget.consumed,
    })
}

fn compute_reverse_postorder(
    entry: MirBlockId,
    successors: &[BTreeSet<MirBlockId>],
    budget: &mut WorkBudget,
) -> Result<Vec<MirBlockId>, MirControlFlowError> {
    let mut visited = vec![false; successors.len()];
    let mut postorder = Vec::with_capacity(successors.len());
    let mut pending = vec![(entry, false)];
    while let Some((block, finish)) = pending.pop() {
        budget.charge(1)?;
        if finish {
            postorder.push(block);
        } else if !visited[block.0 as usize] {
            visited[block.0 as usize] = true;
            pending.push((block, true));
            for successor in successors[block.0 as usize].iter().rev() {
                budget.charge(1)?;
                if !visited[successor.0 as usize] {
                    pending.push((*successor, false));
                }
            }
        }
    }
    postorder.reverse();
    Ok(postorder)
}

fn compute_immediate_dominators(
    entry: MirBlockId,
    predecessors: &[BTreeSet<MirBlockId>],
    reverse_postorder: &[MirBlockId],
    budget: &mut WorkBudget,
) -> Result<Vec<Option<MirBlockId>>, MirControlFlowError> {
    let mut rpo_index = vec![usize::MAX; predecessors.len()];
    for (index, block) in reverse_postorder.iter().enumerate() {
        rpo_index[block.0 as usize] = index;
    }
    let mut immediate = vec![None; predecessors.len()];
    immediate[entry.0 as usize] = Some(entry);

    loop {
        budget.charge(1)?;
        let mut changed = false;
        for block in reverse_postorder.iter().copied().skip(1) {
            budget.charge(1)?;
            let mut processed = predecessors[block.0 as usize]
                .iter()
                .copied()
                .filter(|predecessor| immediate[predecessor.0 as usize].is_some());
            let Some(mut next) = processed.next() else {
                continue;
            };
            for predecessor in processed {
                budget.charge(1)?;
                next =
                    intersect_dominator_paths(predecessor, next, &immediate, &rpo_index, budget)?;
            }
            if immediate[block.0 as usize] != Some(next) {
                immediate[block.0 as usize] = Some(next);
                changed = true;
            }
        }
        if !changed {
            immediate[entry.0 as usize] = None;
            return Ok(immediate);
        }
    }
}

fn intersect_dominator_paths(
    mut left: MirBlockId,
    mut right: MirBlockId,
    immediate: &[Option<MirBlockId>],
    rpo_index: &[usize],
    budget: &mut WorkBudget,
) -> Result<MirBlockId, MirControlFlowError> {
    while left != right {
        budget.charge(1)?;
        while rpo_index[left.0 as usize] > rpo_index[right.0 as usize] {
            budget.charge(1)?;
            left = immediate[left.0 as usize]
                .expect("processed CHK predecessor has an immediate dominator");
        }
        while rpo_index[right.0 as usize] > rpo_index[left.0 as usize] {
            budget.charge(1)?;
            right = immediate[right.0 as usize]
                .expect("processed CHK predecessor has an immediate dominator");
        }
    }
    Ok(left)
}

fn compute_dominator_tree(
    immediate_dominators: &[Option<MirBlockId>],
) -> Vec<BTreeSet<MirBlockId>> {
    let mut children = vec![BTreeSet::new(); immediate_dominators.len()];
    for (index, parent) in immediate_dominators.iter().enumerate() {
        if let Some(parent) = parent {
            children[parent.0 as usize].insert(MirBlockId(index as u32));
        }
    }
    children
}

fn compute_dominator_intervals(
    entry: MirBlockId,
    children: &[BTreeSet<MirBlockId>],
    budget: &mut WorkBudget,
) -> Result<(Vec<usize>, Vec<usize>), MirControlFlowError> {
    let mut preorder = vec![0; children.len()];
    let mut subtree_end = vec![0; children.len()];
    let mut clock = 0;
    let mut pending = vec![(entry, false)];
    while let Some((block, finish)) = pending.pop() {
        budget.charge(1)?;
        if finish {
            subtree_end[block.0 as usize] = clock;
        } else {
            preorder[block.0 as usize] = clock;
            clock += 1;
            pending.push((block, true));
            for child in children[block.0 as usize].iter().rev() {
                pending.push((*child, false));
            }
        }
    }
    Ok((preorder, subtree_end))
}

fn compute_frontiers(
    successors: &[BTreeSet<MirBlockId>],
    immediate: &[Option<MirBlockId>],
    children: &[BTreeSet<MirBlockId>],
    entry: MirBlockId,
    budget: &mut WorkBudget,
) -> Result<Vec<BTreeSet<MirBlockId>>, MirControlFlowError> {
    let mut tree_postorder = Vec::with_capacity(children.len());
    let mut pending = vec![(entry, false)];
    while let Some((block, finish)) = pending.pop() {
        budget.charge(1)?;
        if finish {
            tree_postorder.push(block);
        } else {
            pending.push((block, true));
            for child in children[block.0 as usize].iter().rev() {
                pending.push((*child, false));
            }
        }
    }

    let mut frontiers = vec![BTreeSet::<MirBlockId>::new(); successors.len()];
    for block in tree_postorder {
        let mut frontier = BTreeSet::new();
        for successor in &successors[block.0 as usize] {
            budget.charge(1)?;
            if immediate[successor.0 as usize] != Some(block) {
                frontier.insert(*successor);
            }
        }
        for child in &children[block.0 as usize] {
            for candidate in &frontiers[child.0 as usize] {
                budget.charge(1)?;
                if immediate[candidate.0 as usize] != Some(block) {
                    frontier.insert(*candidate);
                }
            }
        }
        frontiers[block.0 as usize] = frontier;
    }
    Ok(frontiers)
}

fn compute_backedges(
    successors: &[BTreeSet<MirBlockId>],
    dominator_preorder: &[usize],
    dominator_subtree_end: &[usize],
    budget: &mut WorkBudget,
) -> Result<BTreeSet<MirControlFlowEdge>, MirControlFlowError> {
    let mut backedges = BTreeSet::new();
    for (source_index, targets) in successors.iter().enumerate() {
        let source = MirBlockId(source_index as u32);
        for target in targets {
            budget.charge(1)?;
            let start = dominator_preorder[target.0 as usize];
            let candidate = dominator_preorder[source_index];
            if start <= candidate && candidate < dominator_subtree_end[target.0 as usize] {
                backedges.insert(MirControlFlowEdge {
                    source,
                    target: *target,
                });
            }
        }
    }
    Ok(backedges)
}

type NaturalLoops = (
    BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
    BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
);

fn compute_natural_loops(
    predecessors: &[BTreeSet<MirBlockId>],
    backedges: &BTreeSet<MirControlFlowEdge>,
    budget: &mut WorkBudget,
) -> Result<NaturalLoops, MirControlFlowError> {
    let mut bodies = BTreeMap::<_, BTreeSet<_>>::new();
    let mut latches = BTreeMap::<_, BTreeSet<_>>::new();
    for edge in backedges {
        budget.charge(1)?;
        let mut body = BTreeSet::from([edge.target, edge.source]);
        let mut pending = VecDeque::from([edge.source]);
        while let Some(block) = pending.pop_front() {
            if block == edge.target {
                continue;
            }
            for predecessor in &predecessors[block.0 as usize] {
                budget.charge(1)?;
                if body.insert(*predecessor) {
                    pending.push_back(*predecessor);
                }
            }
        }
        bodies.entry(edge.target).or_default().extend(body);
        latches.entry(edge.target).or_default().insert(edge.source);
    }
    Ok((bodies, latches))
}

fn find_irreducible(
    successors: &[BTreeSet<MirBlockId>],
    backedges: &BTreeSet<MirControlFlowEdge>,
    budget: &mut WorkBudget,
) -> Result<Option<MirControlFlowError>, MirControlFlowError> {
    let forward = successors
        .iter()
        .enumerate()
        .map(|(source_index, targets)| {
            let source = MirBlockId(source_index as u32);
            targets
                .iter()
                .copied()
                .filter(|target| {
                    !backedges.contains(&MirControlFlowEdge {
                        source,
                        target: *target,
                    })
                })
                .collect::<BTreeSet<_>>()
        })
        .collect::<Vec<_>>();
    budget.charge(
        forward
            .iter()
            .map(BTreeSet::len)
            .sum::<usize>()
            .saturating_add(forward.len()),
    )?;
    for blocks in strongly_connected_components(&forward) {
        budget.charge(blocks.len())?;
        if blocks.len() == 1 && !forward[blocks[0].0 as usize].contains(&blocks[0]) {
            continue;
        }
        let members = blocks.iter().copied().collect::<BTreeSet<_>>();
        let mut entries = Vec::new();
        for (source_index, targets) in successors.iter().enumerate() {
            let source = MirBlockId(source_index as u32);
            for target in targets {
                budget.charge(1)?;
                if !members.contains(&source) && members.contains(target) {
                    entries.push(MirControlFlowEdge {
                        source,
                        target: *target,
                    });
                }
            }
        }
        return Ok(Some(MirControlFlowError::Irreducible { blocks, entries }));
    }
    Ok(None)
}

fn strongly_connected_components(successors: &[BTreeSet<MirBlockId>]) -> Vec<Vec<MirBlockId>> {
    let mut visited = vec![false; successors.len()];
    let mut finish_order = Vec::with_capacity(successors.len());
    for block_index in 0..successors.len() {
        if visited[block_index] {
            continue;
        }
        let mut pending = vec![(MirBlockId(block_index as u32), false)];
        while let Some((block, finish)) = pending.pop() {
            if finish {
                finish_order.push(block);
            } else if !visited[block.0 as usize] {
                visited[block.0 as usize] = true;
                pending.push((block, true));
                pending.extend(
                    successors[block.0 as usize]
                        .iter()
                        .rev()
                        .copied()
                        .map(|successor| (successor, false)),
                );
            }
        }
    }

    let mut reverse = vec![BTreeSet::new(); successors.len()];
    for (source_index, targets) in successors.iter().enumerate() {
        for target in targets {
            reverse[target.0 as usize].insert(MirBlockId(source_index as u32));
        }
    }
    visited.fill(false);
    let mut components = Vec::new();
    for root in finish_order.into_iter().rev() {
        if visited[root.0 as usize] {
            continue;
        }
        visited[root.0 as usize] = true;
        let mut component = Vec::new();
        let mut pending = vec![root];
        while let Some(block) = pending.pop() {
            component.push(block);
            for predecessor in reverse[block.0 as usize].iter().rev() {
                if !visited[predecessor.0 as usize] {
                    visited[predecessor.0 as usize] = true;
                    pending.push(*predecessor);
                }
            }
        }
        component.sort();
        components.push(component);
    }
    components.sort();
    components
}

fn display_blocks(blocks: &[MirBlockId]) -> String {
    blocks
        .iter()
        .map(|block| format!("bb{}", block.0))
        .collect::<Vec<_>>()
        .join(", ")
}

fn display_edges(edges: &[MirControlFlowEdge]) -> String {
    edges
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}
