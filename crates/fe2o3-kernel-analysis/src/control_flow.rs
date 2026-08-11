use fe2o3_kernel_ir::{BlockId, Function, FunctionId, MAX_BLOCKS_V1, Terminator};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

pub const MAX_CONTROL_FLOW_BLOCKS: usize = MAX_BLOCKS_V1;
pub const MAX_CONTROL_FLOW_EDGES: usize = MAX_BLOCKS_V1 * 2;
pub const MAX_CONTROL_FLOW_NATURAL_LOOPS: usize = MAX_BLOCKS_V1;
pub const MAX_CONTROL_FLOW_LOOP_BODY_MEMBERSHIPS: usize = MAX_BLOCKS_V1;
pub const MAX_CONTROL_FLOW_WORK_UNITS: usize = MAX_BLOCKS_V1 * 128;

/// A separately bounded resource consumed by control-flow analysis.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ControlFlowResource {
    Blocks,
    Edges,
    NaturalLoops,
    NaturalLoopBodyMemberships,
    WorkUnits,
}

impl ControlFlowResource {
    const fn limit(self) -> usize {
        match self {
            Self::Blocks => MAX_CONTROL_FLOW_BLOCKS,
            Self::Edges => MAX_CONTROL_FLOW_EDGES,
            Self::NaturalLoops => MAX_CONTROL_FLOW_NATURAL_LOOPS,
            Self::NaturalLoopBodyMemberships => MAX_CONTROL_FLOW_LOOP_BODY_MEMBERSHIPS,
            Self::WorkUnits => MAX_CONTROL_FLOW_WORK_UNITS,
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::Blocks => "blocks",
            Self::Edges => "edges",
            Self::NaturalLoops => "natural loops",
            Self::NaturalLoopBodyMemberships => "natural-loop body memberships",
            Self::WorkUnits => "natural-loop forest work units",
        }
    }
}

/// Exact deterministic resource counts for one successful analysis.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ControlFlowResourceUsage {
    blocks: usize,
    edges: usize,
    natural_loops: usize,
    natural_loop_body_memberships: usize,
    work_units: usize,
}

impl ControlFlowResourceUsage {
    pub const fn blocks(self) -> usize {
        self.blocks
    }

    pub const fn edges(self) -> usize {
        self.edges
    }

    pub const fn natural_loops(self) -> usize {
        self.natural_loops
    }

    pub const fn natural_loop_body_memberships(self) -> usize {
        self.natural_loop_body_memberships
    }

    pub const fn work_units(self) -> usize {
        self.work_units
    }
}

#[derive(Default)]
struct ControlFlowBudget {
    usage: ControlFlowResourceUsage,
}

impl ControlFlowBudget {
    fn reserve(
        &mut self,
        resource: ControlFlowResource,
        amount: usize,
    ) -> Result<(), ControlFlowDiagnostic> {
        let current = match resource {
            ControlFlowResource::Blocks => &mut self.usage.blocks,
            ControlFlowResource::Edges => &mut self.usage.edges,
            ControlFlowResource::NaturalLoops => &mut self.usage.natural_loops,
            ControlFlowResource::NaturalLoopBodyMemberships => {
                &mut self.usage.natural_loop_body_memberships
            }
            ControlFlowResource::WorkUnits => &mut self.usage.work_units,
        };
        let limit = resource.limit();
        let required = current
            .checked_add(amount)
            .unwrap_or(limit.saturating_add(1));
        if required > limit {
            return Err(ControlFlowDiagnostic::ResourceLimitExceeded {
                resource,
                required,
                limit,
            });
        }
        *current = required;
        Ok(())
    }

    fn work(&mut self, amount: usize) -> Result<(), ControlFlowDiagnostic> {
        self.reserve(ControlFlowResource::WorkUnits, amount)
    }
}

/// A directed edge in a kernel IR control-flow graph.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ControlFlowEdge {
    source: BlockId,
    target: BlockId,
}

impl ControlFlowEdge {
    pub const fn new(source: BlockId, target: BlockId) -> Self {
        Self { source, target }
    }

    pub const fn source(self) -> BlockId {
        self.source
    }

    pub const fn target(self) -> BlockId {
        self.target
    }
}

impl fmt::Display for ControlFlowEdge {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{} -> {}", self.source, self.target)
    }
}

/// A deterministic diagnostic produced before CFG facts can be trusted.
#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum ControlFlowDiagnostic {
    FunctionDeclaration,
    EmptyFunction,
    ResourceLimitExceeded {
        resource: ControlFlowResource,
        required: usize,
        limit: usize,
    },
    DuplicateBlock {
        block: BlockId,
    },
    MissingTerminator {
        block: BlockId,
    },
    UnknownSuccessor {
        edge: ControlFlowEdge,
    },
    /// A strongly connected region has no dominance-based loop structure.
    IrreducibleControlFlow {
        blocks: Vec<BlockId>,
        entry_edges: Vec<ControlFlowEdge>,
    },
}

impl fmt::Display for ControlFlowDiagnostic {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FunctionDeclaration => formatter.write_str("function is a declaration"),
            Self::EmptyFunction => formatter.write_str("function has no entry block"),
            Self::ResourceLimitExceeded {
                resource,
                required,
                limit,
            } => write!(
                formatter,
                "{} require {required} items, exceeding the deterministic limit {limit}",
                resource.description()
            ),
            Self::DuplicateBlock { block } => write!(formatter, "duplicate block {block}"),
            Self::MissingTerminator { block } => {
                write!(formatter, "block {block} has no terminator")
            }
            Self::UnknownSuccessor { edge } => {
                write!(formatter, "edge {edge} targets an unknown block")
            }
            Self::IrreducibleControlFlow {
                blocks,
                entry_edges,
            } => write!(
                formatter,
                "irreducible control flow in blocks {}; entry edges: {}",
                display_blocks(blocks),
                display_edges(entry_edges)
            ),
        }
    }
}

/// Errors that prevent construction of a trustworthy control-flow analysis.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlFlowErrors {
    function: FunctionId,
    diagnostics: Vec<ControlFlowDiagnostic>,
}

impl ControlFlowErrors {
    pub fn function(&self) -> &FunctionId {
        &self.function
    }

    /// Diagnostics sorted by kind and block identity.
    pub fn diagnostics(&self) -> &[ControlFlowDiagnostic] {
        &self.diagnostics
    }
}

impl fmt::Display for ControlFlowErrors {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            formatter,
            "control-flow analysis of {} failed with {} diagnostic(s)",
            self.function,
            self.diagnostics.len()
        )?;
        for diagnostic in &self.diagnostics {
            writeln!(formatter, "  {diagnostic}")?;
        }
        Ok(())
    }
}

impl Error for ControlFlowErrors {}

/// Validated, deterministic control-flow facts for one function definition.
///
/// Dominance and backedges are defined only for blocks reachable from the
/// function's first block. Predecessors include edges between all defined
/// blocks, including unreachable ones.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlFlowAnalysis {
    function: FunctionId,
    resource_usage: ControlFlowResourceUsage,
    entry: BlockId,
    blocks: BTreeSet<BlockId>,
    predecessors: BTreeMap<BlockId, BTreeSet<BlockId>>,
    reachable: BTreeSet<BlockId>,
    immediate_dominators: BTreeMap<BlockId, Option<BlockId>>,
    dominator_tree_children: BTreeMap<BlockId, BTreeSet<BlockId>>,
    dominator_preorder: BTreeMap<BlockId, usize>,
    dominator_subtree_end: BTreeMap<BlockId, usize>,
    dominance_frontiers: BTreeMap<BlockId, BTreeSet<BlockId>>,
    backedges: BTreeSet<ControlFlowEdge>,
    natural_loop_headers: BTreeSet<BlockId>,
    natural_loop_bodies: BTreeMap<BlockId, BTreeSet<BlockId>>,
    natural_loop_latches: BTreeMap<BlockId, BTreeSet<BlockId>>,
    natural_loop_parents: BTreeMap<BlockId, BlockId>,
    natural_loop_children: BTreeMap<BlockId, BTreeSet<BlockId>>,
    natural_loop_roots: BTreeSet<BlockId>,
    block_loop_nests: BTreeMap<BlockId, Vec<BlockId>>,
}

impl ControlFlowAnalysis {
    pub fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn resource_usage(&self) -> ControlFlowResourceUsage {
        self.resource_usage
    }

    pub const fn entry(&self) -> BlockId {
        self.entry
    }

    pub fn blocks(&self) -> &BTreeSet<BlockId> {
        &self.blocks
    }

    pub fn predecessors(&self, block: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.predecessors.get(&block)
    }

    pub fn reachable_blocks(&self) -> &BTreeSet<BlockId> {
        &self.reachable
    }

    pub fn is_reachable(&self, block: BlockId) -> bool {
        self.reachable.contains(&block)
    }

    /// Returns `None` for an unknown or unreachable block.
    pub fn dominators(&self, block: BlockId) -> Option<BTreeSet<BlockId>> {
        if !self.reachable.contains(&block) {
            return None;
        }
        let mut dominators = BTreeSet::new();
        let mut current = block;
        loop {
            dominators.insert(current);
            if current == self.entry {
                return Some(dominators);
            }
            current = self.immediate_dominators.get(&current).copied().flatten()?;
        }
    }

    pub fn dominates(&self, dominator: BlockId, block: BlockId) -> bool {
        let (Some(start), Some(candidate), Some(end)) = (
            self.dominator_preorder.get(&dominator),
            self.dominator_preorder.get(&block),
            self.dominator_subtree_end.get(&dominator),
        ) else {
            return false;
        };
        start <= candidate && candidate < end
    }

    /// The block's immediate dominator.
    ///
    /// The outer `Option` is `None` for an unknown or unreachable block. The
    /// entry block is represented by `Some(None)` because it has no immediate
    /// dominator.
    pub fn immediate_dominator(&self, block: BlockId) -> Option<Option<BlockId>> {
        self.immediate_dominators.get(&block).copied()
    }

    /// Children of a reachable block in deterministic block-identity order.
    ///
    /// Returns `None` for an unknown or unreachable block.
    pub fn dominator_tree_children(&self, block: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.dominator_tree_children.get(&block)
    }

    /// The block's dominance frontier in deterministic block-identity order.
    ///
    /// Returns `None` for an unknown or unreachable block.
    pub fn dominance_frontier(&self, block: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.dominance_frontiers.get(&block)
    }

    /// Computes the iterated dominance frontier for SSA phi placement.
    ///
    /// Returns `None` if any definition block is unknown or unreachable. The
    /// result is not pruned by liveness; callers should intersect it with
    /// variable-specific live-in blocks when constructing pruned SSA.
    pub fn iterated_dominance_frontier(
        &self,
        definition_blocks: &BTreeSet<BlockId>,
    ) -> Option<BTreeSet<BlockId>> {
        if !definition_blocks
            .iter()
            .all(|block| self.reachable.contains(block))
        {
            return None;
        }

        let mut phi_blocks = BTreeSet::new();
        let mut pending = definition_blocks.clone();
        while let Some(block) = pending.pop_first() {
            for frontier in &self.dominance_frontiers[&block] {
                if phi_blocks.insert(*frontier) && !definition_blocks.contains(frontier) {
                    pending.insert(*frontier);
                }
            }
        }
        Some(phi_blocks)
    }

    /// Edges whose target dominates their source.
    pub fn backedges(&self) -> &BTreeSet<ControlFlowEdge> {
        &self.backedges
    }

    /// Headers of all reachable natural loops.
    ///
    /// Multiple backedges to one header are represented by one loop whose
    /// body and latch sets are the union of those natural loops.
    pub fn natural_loop_headers(&self) -> &BTreeSet<BlockId> {
        &self.natural_loop_headers
    }

    /// Body of the natural loop headed by `header`, including the header.
    pub fn natural_loop_body(&self, header: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.natural_loop_bodies.get(&header)
    }

    /// Sources of backedges targeting `header`.
    pub fn natural_loop_latches(&self, header: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.natural_loop_latches.get(&header)
    }

    /// Root loop headers in deterministic block-identity order.
    pub fn natural_loop_roots(&self) -> &BTreeSet<BlockId> {
        &self.natural_loop_roots
    }

    /// Immediate containing loop, or `None` for a root or non-header block.
    ///
    /// Use [`Self::natural_loop_body`] to distinguish roots from non-headers.
    pub fn natural_loop_parent(&self, header: BlockId) -> Option<BlockId> {
        self.natural_loop_parents.get(&header).copied()
    }

    /// Immediately nested loop headers, or `None` if `header` is not a loop.
    pub fn natural_loop_children(&self, header: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.natural_loop_children.get(&header)
    }

    /// Loops containing a reachable block, ordered outermost to innermost.
    ///
    /// Returns `None` for an unknown or unreachable block and an empty slice
    /// for a reachable block outside every natural loop.
    pub fn containing_natural_loops(&self, block: BlockId) -> Option<&[BlockId]> {
        self.block_loop_nests.get(&block).map(Vec::as_slice)
    }

    /// Number of natural loops containing a reachable block.
    pub fn natural_loop_depth(&self, block: BlockId) -> Option<usize> {
        self.block_loop_nests.get(&block).map(Vec::len)
    }
}

/// Computes validated CFG facts and rejects reachable irreducible control flow.
///
/// The analysis fails closed on structurally malformed functions even when the
/// malformed block is unreachable. Callers may run this independently of the
/// kernel IR verifier, although that verifier remains responsible for SSA,
/// type, and branch-argument validation.
pub fn analyze_control_flow(function: &Function) -> Result<ControlFlowAnalysis, ControlFlowErrors> {
    let Some(body) = &function.body else {
        return Err(errors(
            function,
            [ControlFlowDiagnostic::FunctionDeclaration],
        ));
    };
    if body.blocks.is_empty() {
        return Err(errors(function, [ControlFlowDiagnostic::EmptyFunction]));
    }
    let mut budget = ControlFlowBudget::default();
    if let Err(diagnostic) = budget.reserve(ControlFlowResource::Blocks, body.blocks.len()) {
        return Err(errors(function, [diagnostic]));
    }

    let mut diagnostics = BTreeSet::new();
    let mut blocks = BTreeMap::new();
    for block in &body.blocks {
        if blocks.insert(block.id, block).is_some() {
            diagnostics.insert(ControlFlowDiagnostic::DuplicateBlock { block: block.id });
        }
        if block.terminator.is_none() {
            diagnostics.insert(ControlFlowDiagnostic::MissingTerminator { block: block.id });
        }
    }

    let block_ids = blocks.keys().copied().collect::<BTreeSet<_>>();
    let mut successors = block_ids
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for block in blocks.values() {
        let Some(terminator) = &block.terminator else {
            continue;
        };
        let result = visit_successors(terminator, |target| {
            budget.reserve(ControlFlowResource::Edges, 1)?;
            let edge = ControlFlowEdge::new(block.id, target);
            if block_ids.contains(&target) {
                successors
                    .get_mut(&block.id)
                    .expect("defined block has successor storage")
                    .insert(target);
            } else {
                diagnostics.insert(ControlFlowDiagnostic::UnknownSuccessor { edge });
            }
            Ok(())
        });
        if let Err(diagnostic) = result {
            return Err(errors(function, [diagnostic]));
        }
    }
    if !diagnostics.is_empty() {
        return Err(errors(function, diagnostics));
    }

    let entry = body.blocks[0].id;
    let predecessors = compute_predecessors(&block_ids, &successors);
    let reachable = compute_reachable(entry, &successors);
    let reverse_postorder = compute_reverse_postorder(entry, &successors);
    let immediate_dominators =
        compute_immediate_dominators(entry, &reachable, &predecessors, &reverse_postorder);
    let dominator_tree_children =
        compute_dominator_tree_children(&reachable, &immediate_dominators);
    let (dominator_preorder, dominator_subtree_end) =
        compute_dominator_intervals(entry, &dominator_tree_children);
    let dominance_frontiers = compute_dominance_frontiers(
        entry,
        &reachable,
        &successors,
        &immediate_dominators,
        &dominator_tree_children,
    );
    let backedges = compute_backedges(
        &reachable,
        &successors,
        &dominator_preorder,
        &dominator_subtree_end,
    );
    let irreducible = irreducible_diagnostics(&reachable, &successors, &backedges);
    if !irreducible.is_empty() {
        return Err(errors(function, irreducible));
    }
    let natural_loops = compute_natural_loops(&reachable, &predecessors, &backedges, &mut budget)
        .map_err(|diagnostic| errors(function, [diagnostic]))?;

    Ok(ControlFlowAnalysis {
        function: function.id.clone(),
        resource_usage: budget.usage,
        entry,
        blocks: block_ids,
        predecessors,
        reachable,
        immediate_dominators,
        dominator_tree_children,
        dominator_preorder,
        dominator_subtree_end,
        dominance_frontiers,
        backedges,
        natural_loop_headers: natural_loops.bodies.keys().copied().collect(),
        natural_loop_bodies: natural_loops.bodies,
        natural_loop_latches: natural_loops.latches,
        natural_loop_parents: natural_loops.parents,
        natural_loop_children: natural_loops.children,
        natural_loop_roots: natural_loops.roots,
        block_loop_nests: natural_loops.block_nests,
    })
}

fn visit_successors(
    terminator: &Terminator,
    mut visit: impl FnMut(BlockId) -> Result<(), ControlFlowDiagnostic>,
) -> Result<(), ControlFlowDiagnostic> {
    match terminator {
        Terminator::Branch { target, .. } => visit(*target),
        Terminator::ConditionalBranch {
            then_target,
            else_target,
            ..
        } => {
            visit(*then_target)?;
            visit(*else_target)
        }
        Terminator::Switch {
            cases,
            default_target,
            ..
        } => {
            for case in cases {
                visit(case.target)?;
            }
            visit(*default_target)
        }
        Terminator::IntegerSwitch {
            cases,
            default_target,
            ..
        } => {
            for case in cases {
                visit(case.target)?;
            }
            visit(*default_target)
        }
        Terminator::Return { .. } | Terminator::Unreachable => Ok(()),
    }
}

fn errors(
    function: &Function,
    diagnostics: impl IntoIterator<Item = ControlFlowDiagnostic>,
) -> ControlFlowErrors {
    let mut diagnostics = diagnostics.into_iter().collect::<Vec<_>>();
    diagnostics.sort();
    diagnostics.dedup();
    ControlFlowErrors {
        function: function.id.clone(),
        diagnostics,
    }
}

fn compute_predecessors(
    blocks: &BTreeSet<BlockId>,
    successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let mut predecessors = blocks
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (source, targets) in successors {
        for target in targets {
            predecessors
                .get_mut(target)
                .expect("successor validation rejected unknown blocks")
                .insert(*source);
        }
    }
    predecessors
}

fn compute_reachable(
    entry: BlockId,
    successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeSet<BlockId> {
    let mut reachable = BTreeSet::new();
    let mut pending = VecDeque::from([entry]);
    while let Some(block) = pending.pop_front() {
        if !reachable.insert(block) {
            continue;
        }
        pending.extend(successors[&block].iter().copied());
    }
    reachable
}

fn compute_reverse_postorder(
    entry: BlockId,
    successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Vec<BlockId> {
    let mut visited = BTreeSet::new();
    let mut postorder = Vec::new();
    let mut pending = vec![(entry, false)];
    while let Some((block, finish)) = pending.pop() {
        if finish {
            postorder.push(block);
        } else if visited.insert(block) {
            pending.push((block, true));
            pending.extend(
                successors[&block]
                    .iter()
                    .rev()
                    .copied()
                    .map(|successor| (successor, false)),
            );
        }
    }
    postorder.reverse();
    postorder
}

fn compute_immediate_dominators(
    entry: BlockId,
    reachable: &BTreeSet<BlockId>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    reverse_postorder: &[BlockId],
) -> BTreeMap<BlockId, Option<BlockId>> {
    let rpo_index = reverse_postorder
        .iter()
        .copied()
        .enumerate()
        .map(|(index, block)| (block, index))
        .collect::<BTreeMap<_, _>>();
    let mut immediate = reachable
        .iter()
        .copied()
        .map(|block| (block, None))
        .collect::<BTreeMap<_, _>>();
    immediate.insert(entry, Some(entry));

    loop {
        let mut changed = false;
        for block in reverse_postorder.iter().copied().skip(1) {
            let mut processed = predecessors[&block]
                .iter()
                .copied()
                .filter(|predecessor| immediate[predecessor].is_some());
            let Some(mut next) = processed.next() else {
                continue;
            };
            for predecessor in processed {
                next = intersect_dominator_paths(predecessor, next, &immediate, &rpo_index);
            }
            if immediate[&block] != Some(next) {
                immediate.insert(block, Some(next));
                changed = true;
            }
        }
        if !changed {
            immediate.insert(entry, None);
            return immediate;
        }
    }
}

fn intersect_dominator_paths(
    mut left: BlockId,
    mut right: BlockId,
    immediate: &BTreeMap<BlockId, Option<BlockId>>,
    rpo_index: &BTreeMap<BlockId, usize>,
) -> BlockId {
    while left != right {
        while rpo_index[&left] > rpo_index[&right] {
            left = immediate[&left].expect("processed CHK predecessor has an immediate dominator");
        }
        while rpo_index[&right] > rpo_index[&left] {
            right =
                immediate[&right].expect("processed CHK predecessor has an immediate dominator");
        }
    }
    left
}

fn compute_dominator_tree_children(
    reachable: &BTreeSet<BlockId>,
    immediate_dominators: &BTreeMap<BlockId, Option<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let mut children = reachable
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (block, immediate) in immediate_dominators {
        if let Some(parent) = immediate {
            children
                .get_mut(parent)
                .expect("an immediate dominator is reachable")
                .insert(*block);
        }
    }
    children
}

fn compute_dominator_intervals(
    entry: BlockId,
    children: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> (BTreeMap<BlockId, usize>, BTreeMap<BlockId, usize>) {
    let mut preorder = BTreeMap::new();
    let mut subtree_end = BTreeMap::new();
    let mut clock = 0_usize;
    let mut pending = vec![(entry, false)];
    while let Some((block, finish)) = pending.pop() {
        if finish {
            subtree_end.insert(block, clock);
        } else {
            preorder.insert(block, clock);
            clock += 1;
            pending.push((block, true));
            pending.extend(
                children[&block]
                    .iter()
                    .rev()
                    .copied()
                    .map(|child| (child, false)),
            );
        }
    }
    (preorder, subtree_end)
}

fn compute_dominance_frontiers(
    entry: BlockId,
    reachable: &BTreeSet<BlockId>,
    successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    immediate: &BTreeMap<BlockId, Option<BlockId>>,
    children: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let mut tree_postorder = Vec::with_capacity(reachable.len());
    let mut pending = vec![(entry, false)];
    while let Some((block, finish)) = pending.pop() {
        if finish {
            tree_postorder.push(block);
        } else {
            pending.push((block, true));
            pending.extend(
                children[&block]
                    .iter()
                    .rev()
                    .copied()
                    .map(|child| (child, false)),
            );
        }
    }

    let mut frontiers = reachable
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for block in tree_postorder {
        let mut frontier = BTreeSet::new();
        for successor in &successors[&block] {
            if reachable.contains(successor) && immediate[successor] != Some(block) {
                frontier.insert(*successor);
            }
        }
        for child in &children[&block] {
            for candidate in &frontiers[child] {
                if immediate[candidate] != Some(block) {
                    frontier.insert(*candidate);
                }
            }
        }
        frontiers.insert(block, frontier);
    }
    frontiers
}

#[derive(Debug)]
struct NaturalLoopForest {
    bodies: BTreeMap<BlockId, BTreeSet<BlockId>>,
    latches: BTreeMap<BlockId, BTreeSet<BlockId>>,
    parents: BTreeMap<BlockId, BlockId>,
    children: BTreeMap<BlockId, BTreeSet<BlockId>>,
    roots: BTreeSet<BlockId>,
    block_nests: BTreeMap<BlockId, Vec<BlockId>>,
}

fn compute_natural_loops(
    reachable: &BTreeSet<BlockId>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    backedges: &BTreeSet<ControlFlowEdge>,
    budget: &mut ControlFlowBudget,
) -> Result<NaturalLoopForest, ControlFlowDiagnostic> {
    let block_ids = reachable.iter().copied().collect::<Vec<_>>();
    budget.work(block_ids.len())?;
    let block_indices = block_ids
        .iter()
        .copied()
        .enumerate()
        .map(|(index, block)| (block, index))
        .collect::<BTreeMap<_, _>>();

    let mut predecessor_counts = vec![0_usize; block_ids.len()];
    for (target_index, target) in block_ids.iter().copied().enumerate() {
        for _predecessor in predecessors[&target]
            .iter()
            .filter(|predecessor| reachable.contains(predecessor))
        {
            budget.work(1)?;
            predecessor_counts[target_index] += 1;
        }
    }
    budget.work(block_ids.len())?;
    let mut predecessor_offsets = Vec::with_capacity(block_ids.len() + 1);
    predecessor_offsets.push(0_usize);
    for count in predecessor_counts {
        let next = predecessor_offsets
            .last()
            .copied()
            .expect("predecessor offsets contain their zero origin")
            + count;
        predecessor_offsets.push(next);
    }
    let mut predecessor_indices =
        Vec::with_capacity(predecessor_offsets.last().copied().unwrap_or_default());
    for target in &block_ids {
        for predecessor in predecessors[target]
            .iter()
            .filter(|predecessor| reachable.contains(predecessor))
        {
            budget.work(1)?;
            predecessor_indices.push(block_indices[predecessor]);
        }
    }

    let mut latch_counts = vec![0_usize; block_ids.len()];
    for edge in backedges {
        budget.work(1)?;
        latch_counts[block_indices[&edge.target()]] += 1;
    }
    budget.work(block_ids.len())?;
    let loop_count = latch_counts.iter().filter(|count| **count != 0).count();
    budget.reserve(ControlFlowResource::NaturalLoops, loop_count)?;

    let mut latch_offsets = Vec::with_capacity(block_ids.len() + 1);
    latch_offsets.push(0_usize);
    for count in &latch_counts {
        latch_offsets.push(latch_offsets.last().copied().unwrap_or_default() + count);
    }
    let mut latch_indices = vec![usize::MAX; backedges.len()];
    let mut latch_cursors = latch_offsets[..block_ids.len()].to_vec();
    for edge in backedges {
        budget.work(1)?;
        let header = block_indices[&edge.target()];
        let cursor = &mut latch_cursors[header];
        latch_indices[*cursor] = block_indices[&edge.source()];
        *cursor += 1;
    }

    #[derive(Debug)]
    struct IndexedNaturalLoop {
        header: usize,
        latches: Vec<usize>,
        body: Vec<usize>,
    }

    let mut loops = Vec::with_capacity(loop_count);
    let mut marks = vec![usize::MAX; block_ids.len()];
    for header in 0..block_ids.len() {
        if latch_counts[header] == 0 {
            continue;
        }
        let generation = loops.len();
        let latches = latch_indices[latch_offsets[header]..latch_offsets[header + 1]].to_vec();
        let mut body = Vec::new();
        let mut pending = Vec::new();

        budget.reserve(ControlFlowResource::NaturalLoopBodyMemberships, 1)?;
        marks[header] = generation;
        body.push(header);
        for latch in &latches {
            budget.work(1)?;
            if marks[*latch] != generation {
                budget.reserve(ControlFlowResource::NaturalLoopBodyMemberships, 1)?;
                marks[*latch] = generation;
                body.push(*latch);
                if *latch != header {
                    pending.push(*latch);
                }
            }
        }

        while let Some(block) = pending.pop() {
            budget.work(1)?;
            for predecessor in
                &predecessor_indices[predecessor_offsets[block]..predecessor_offsets[block + 1]]
            {
                budget.work(1)?;
                if marks[*predecessor] != generation {
                    budget.reserve(ControlFlowResource::NaturalLoopBodyMemberships, 1)?;
                    marks[*predecessor] = generation;
                    body.push(*predecessor);
                    if *predecessor != header {
                        pending.push(*predecessor);
                    }
                }
            }
        }
        loops.push(IndexedNaturalLoop {
            header,
            latches,
            body,
        });
    }

    let mut block_membership_counts = vec![0_usize; block_ids.len()];
    for natural_loop in &loops {
        budget.work(natural_loop.body.len())?;
        for block in &natural_loop.body {
            block_membership_counts[*block] += 1;
        }
    }
    budget.work(block_ids.len())?;
    let mut block_membership_offsets = Vec::with_capacity(block_ids.len() + 1);
    block_membership_offsets.push(0_usize);
    for count in block_membership_counts {
        block_membership_offsets
            .push(block_membership_offsets.last().copied().unwrap_or_default() + count);
    }
    let membership_count = budget.usage.natural_loop_body_memberships;
    let mut block_memberships = vec![usize::MAX; membership_count];
    let mut membership_cursors = block_membership_offsets[..block_ids.len()].to_vec();
    for (loop_index, natural_loop) in loops.iter().enumerate() {
        budget.work(natural_loop.body.len())?;
        for block in &natural_loop.body {
            let cursor = &mut membership_cursors[*block];
            block_memberships[*cursor] = loop_index;
            *cursor += 1;
        }
    }

    let memberships_for = |block: usize| {
        &block_memberships[block_membership_offsets[block]..block_membership_offsets[block + 1]]
    };
    let mut indexed_parents = vec![None; loops.len()];
    for (loop_index, natural_loop) in loops.iter().enumerate() {
        let mut parent: Option<usize> = None;
        for candidate in memberships_for(natural_loop.header) {
            budget.work(1)?;
            if *candidate == loop_index || loops[*candidate].body.len() <= natural_loop.body.len() {
                continue;
            }
            // Reachable irreducible regions were rejected above, so natural
            // loops containing this header form its strict containment chain.
            if parent.is_none_or(|current| {
                (
                    loops[*candidate].body.len(),
                    block_ids[loops[*candidate].header],
                ) < (loops[current].body.len(), block_ids[loops[current].header])
            }) {
                parent = Some(*candidate);
            }
        }
        indexed_parents[loop_index] = parent;
    }

    let mut indexed_block_nests = Vec::with_capacity(block_ids.len());
    for block in 0..block_ids.len() {
        let mut containing = memberships_for(block).to_vec();
        budget.work(sort_work(containing.len()))?;
        containing.sort_by_key(|loop_index| {
            (
                std::cmp::Reverse(loops[*loop_index].body.len()),
                block_ids[loops[*loop_index].header],
            )
        });
        indexed_block_nests.push(containing);
    }

    let mut bodies = BTreeMap::new();
    let mut latches = BTreeMap::new();
    let mut parents = BTreeMap::new();
    let mut children = BTreeMap::new();
    let mut roots = BTreeSet::new();
    for (loop_index, natural_loop) in loops.iter().enumerate() {
        budget.work(2 + natural_loop.body.len() + natural_loop.latches.len())?;
        let header = block_ids[natural_loop.header];
        bodies.insert(
            header,
            natural_loop
                .body
                .iter()
                .map(|block| block_ids[*block])
                .collect(),
        );
        latches.insert(
            header,
            natural_loop
                .latches
                .iter()
                .map(|latch| block_ids[*latch])
                .collect(),
        );
        children.insert(header, BTreeSet::new());
        if let Some(parent) = indexed_parents[loop_index] {
            parents.insert(header, block_ids[loops[parent].header]);
        } else {
            roots.insert(header);
        }
    }
    budget.work(indexed_parents.len())?;
    for (child, parent) in indexed_parents.iter().copied().enumerate() {
        if let Some(parent) = parent {
            children
                .get_mut(&block_ids[loops[parent].header])
                .expect("an indexed natural-loop parent is a loop header")
                .insert(block_ids[loops[child].header]);
        }
    }
    budget.work(block_ids.len() + membership_count)?;
    let block_nests = block_ids
        .iter()
        .copied()
        .zip(indexed_block_nests)
        .map(|(block, containing)| {
            (
                block,
                containing
                    .into_iter()
                    .map(|loop_index| block_ids[loops[loop_index].header])
                    .collect(),
            )
        })
        .collect();

    Ok(NaturalLoopForest {
        bodies,
        latches,
        parents,
        children,
        roots,
        block_nests,
    })
}

fn binary_search_work(items: usize) -> usize {
    if items <= 1 {
        1
    } else {
        usize::BITS as usize - (items - 1).leading_zeros() as usize
    }
}

fn sort_work(items: usize) -> usize {
    items
        .saturating_mul(binary_search_work(items))
        .saturating_mul(2)
}

fn compute_backedges(
    reachable: &BTreeSet<BlockId>,
    successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    dominator_preorder: &BTreeMap<BlockId, usize>,
    dominator_subtree_end: &BTreeMap<BlockId, usize>,
) -> BTreeSet<ControlFlowEdge> {
    let mut backedges = BTreeSet::new();
    for source in reachable {
        for target in &successors[source] {
            let Some(start) = dominator_preorder.get(target) else {
                continue;
            };
            let candidate = dominator_preorder[source];
            if *start <= candidate && candidate < dominator_subtree_end[target] {
                backedges.insert(ControlFlowEdge::new(*source, *target));
            }
        }
    }
    backedges
}

fn irreducible_diagnostics(
    reachable: &BTreeSet<BlockId>,
    successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    backedges: &BTreeSet<ControlFlowEdge>,
) -> Vec<ControlFlowDiagnostic> {
    let forward = reachable
        .iter()
        .copied()
        .map(|source| {
            let targets = successors[&source]
                .iter()
                .copied()
                .filter(|target| {
                    !backedges.contains(&ControlFlowEdge::new(source, *target))
                        && reachable.contains(target)
                })
                .collect();
            (source, targets)
        })
        .collect::<BTreeMap<_, BTreeSet<_>>>();

    strongly_connected_components(reachable, &forward)
        .into_iter()
        .filter(|component| component.len() > 1 || forward[&component[0]].contains(&component[0]))
        .map(|blocks| {
            let members = blocks.iter().copied().collect::<BTreeSet<_>>();
            let entry_edges = reachable
                .iter()
                .copied()
                .flat_map(|source| {
                    successors[&source]
                        .iter()
                        .copied()
                        .map(move |target| ControlFlowEdge::new(source, target))
                })
                .filter(|edge| {
                    !members.contains(&edge.source()) && members.contains(&edge.target())
                })
                .collect();
            ControlFlowDiagnostic::IrreducibleControlFlow {
                blocks,
                entry_edges,
            }
        })
        .collect()
}

fn strongly_connected_components(
    blocks: &BTreeSet<BlockId>,
    successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> Vec<Vec<BlockId>> {
    let mut visited = BTreeSet::new();
    let mut finish_order = Vec::new();
    for root in blocks {
        if visited.contains(root) {
            continue;
        }
        let mut stack = vec![(*root, false)];
        while let Some((block, finish)) = stack.pop() {
            if finish {
                finish_order.push(block);
            } else if visited.insert(block) {
                stack.push((block, true));
                stack.extend(
                    successors[&block]
                        .iter()
                        .rev()
                        .copied()
                        .map(|successor| (successor, false)),
                );
            }
        }
    }

    let mut reverse = blocks
        .iter()
        .copied()
        .map(|block| (block, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (source, targets) in successors {
        for target in targets {
            reverse
                .get_mut(target)
                .expect("SCC graph is closed over its blocks")
                .insert(*source);
        }
    }

    visited.clear();
    let mut components = Vec::new();
    for root in finish_order.into_iter().rev() {
        if !visited.insert(root) {
            continue;
        }
        let mut component = Vec::new();
        let mut stack = vec![root];
        while let Some(block) = stack.pop() {
            component.push(block);
            for predecessor in reverse[&block].iter().rev() {
                if visited.insert(*predecessor) {
                    stack.push(*predecessor);
                }
            }
        }
        component.sort();
        components.push(component);
    }
    components.sort();
    components
}

fn display_blocks(blocks: &[BlockId]) -> String {
    blocks
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

fn display_edges(edges: &[ControlFlowEdge]) -> String {
    edges
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod resource_budget_tests {
    use super::*;

    #[test]
    fn every_resource_accepts_its_boundary_and_rejects_the_next_item() {
        let resources = [
            ControlFlowResource::Blocks,
            ControlFlowResource::Edges,
            ControlFlowResource::NaturalLoops,
            ControlFlowResource::NaturalLoopBodyMemberships,
            ControlFlowResource::WorkUnits,
        ];

        for resource in resources {
            let mut budget = ControlFlowBudget::default();
            budget.reserve(resource, resource.limit()).unwrap();
            assert_eq!(
                budget.reserve(resource, 1),
                Err(ControlFlowDiagnostic::ResourceLimitExceeded {
                    resource,
                    required: resource.limit() + 1,
                    limit: resource.limit(),
                })
            );
        }
    }

    #[test]
    fn resource_diagnostic_text_is_stable() {
        let diagnostic = ControlFlowDiagnostic::ResourceLimitExceeded {
            resource: ControlFlowResource::NaturalLoopBodyMemberships,
            required: MAX_CONTROL_FLOW_LOOP_BODY_MEMBERSHIPS + 1,
            limit: MAX_CONTROL_FLOW_LOOP_BODY_MEMBERSHIPS,
        };
        assert_eq!(
            diagnostic.to_string(),
            "natural-loop body memberships require 65537 items, exceeding the deterministic limit 65536"
        );
    }
}
