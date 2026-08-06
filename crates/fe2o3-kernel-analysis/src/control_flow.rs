use fe2o3_kernel_ir::{BlockId, Function, FunctionId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::error::Error;
use std::fmt;

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
    entry: BlockId,
    blocks: BTreeSet<BlockId>,
    predecessors: BTreeMap<BlockId, BTreeSet<BlockId>>,
    reachable: BTreeSet<BlockId>,
    dominators: BTreeMap<BlockId, BTreeSet<BlockId>>,
    immediate_dominators: BTreeMap<BlockId, Option<BlockId>>,
    dominator_tree_children: BTreeMap<BlockId, BTreeSet<BlockId>>,
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
    pub fn dominators(&self, block: BlockId) -> Option<&BTreeSet<BlockId>> {
        self.dominators.get(&block)
    }

    pub fn dominates(&self, dominator: BlockId, block: BlockId) -> bool {
        self.dominators
            .get(&block)
            .is_some_and(|dominators| dominators.contains(&dominator))
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
        for target in terminator.successors() {
            let edge = ControlFlowEdge::new(block.id, target);
            if block_ids.contains(&target) {
                successors
                    .get_mut(&block.id)
                    .expect("defined block has successor storage")
                    .insert(target);
            } else {
                diagnostics.insert(ControlFlowDiagnostic::UnknownSuccessor { edge });
            }
        }
    }
    if !diagnostics.is_empty() {
        return Err(errors(function, diagnostics));
    }

    let entry = body.blocks[0].id;
    let predecessors = compute_predecessors(&block_ids, &successors);
    let reachable = compute_reachable(entry, &successors);
    let dominators = compute_dominators(entry, &reachable, &predecessors);
    let immediate_dominators = compute_immediate_dominators(entry, &reachable, &dominators);
    let dominator_tree_children =
        compute_dominator_tree_children(&reachable, &immediate_dominators);
    let dominance_frontiers = compute_dominance_frontiers(&reachable, &predecessors, &dominators);
    let backedges = compute_backedges(&reachable, &successors, &dominators);
    let irreducible = irreducible_diagnostics(&reachable, &successors, &backedges);
    if !irreducible.is_empty() {
        return Err(errors(function, irreducible));
    }
    let natural_loops = compute_natural_loops(&reachable, &predecessors, &backedges);

    Ok(ControlFlowAnalysis {
        function: function.id.clone(),
        entry,
        blocks: block_ids,
        predecessors,
        reachable,
        dominators,
        immediate_dominators,
        dominator_tree_children,
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

fn compute_dominators(
    entry: BlockId,
    reachable: &BTreeSet<BlockId>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    let mut dominators = reachable
        .iter()
        .copied()
        .map(|block| {
            let initial = if block == entry {
                BTreeSet::from([entry])
            } else {
                reachable.clone()
            };
            (block, initial)
        })
        .collect::<BTreeMap<_, _>>();

    // Every successful pass removes at least one candidate dominator. The
    // quadratic limit is therefore a conservative finite bound.
    let maximum_passes = reachable.len().saturating_mul(reachable.len()).max(1);
    for _ in 0..maximum_passes {
        let mut changed = false;
        for block in reachable.iter().copied().filter(|block| *block != entry) {
            let mut incoming = predecessors[&block]
                .iter()
                .filter(|predecessor| reachable.contains(predecessor));
            let mut next = incoming
                .next()
                .map(|predecessor| dominators[predecessor].clone())
                .unwrap_or_default();
            for predecessor in incoming {
                next.retain(|candidate| dominators[predecessor].contains(candidate));
            }
            next.insert(block);
            if dominators[&block] != next {
                dominators.insert(block, next);
                changed = true;
            }
        }
        if !changed {
            return dominators;
        }
    }

    // The monotone intersection above must converge within the number of
    // removable `(block, candidate)` facts.
    unreachable!("bounded dominator fixed point did not converge")
}

fn compute_immediate_dominators(
    entry: BlockId,
    reachable: &BTreeSet<BlockId>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, Option<BlockId>> {
    reachable
        .iter()
        .copied()
        .map(|block| {
            if block == entry {
                return (block, None);
            }

            let strict_dominators = dominators[&block]
                .iter()
                .copied()
                .filter(|dominator| *dominator != block)
                .collect::<BTreeSet<_>>();
            let immediate = strict_dominators.iter().copied().find(|candidate| {
                strict_dominators.iter().all(|dominator| {
                    dominator == candidate || dominators[candidate].contains(dominator)
                })
            });
            (block, immediate)
        })
        .collect()
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

fn compute_dominance_frontiers(
    reachable: &BTreeSet<BlockId>,
    predecessors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeMap<BlockId, BTreeSet<BlockId>> {
    reachable
        .iter()
        .copied()
        .map(|dominator| {
            let frontier = reachable
                .iter()
                .copied()
                .filter(|candidate| {
                    let strictly_dominates_candidate =
                        *candidate != dominator && dominators[candidate].contains(&dominator);
                    !strictly_dominates_candidate
                        && predecessors[candidate]
                            .iter()
                            .filter(|predecessor| reachable.contains(predecessor))
                            .any(|predecessor| dominators[predecessor].contains(&dominator))
                })
                .collect();
            (dominator, frontier)
        })
        .collect()
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
) -> NaturalLoopForest {
    let mut bodies = BTreeMap::<BlockId, BTreeSet<BlockId>>::new();
    let mut latches = BTreeMap::<BlockId, BTreeSet<BlockId>>::new();

    for edge in backedges {
        let header = edge.target();
        let latch = edge.source();
        let mut body = BTreeSet::from([header, latch]);
        let mut pending = if latch == header {
            Vec::new()
        } else {
            vec![latch]
        };
        while let Some(block) = pending.pop() {
            for predecessor in predecessors[&block]
                .iter()
                .rev()
                .filter(|predecessor| reachable.contains(predecessor))
            {
                if body.insert(*predecessor) && *predecessor != header {
                    pending.push(*predecessor);
                }
            }
        }
        bodies.entry(header).or_default().extend(body);
        latches.entry(header).or_default().insert(latch);
    }

    let headers = bodies.keys().copied().collect::<BTreeSet<_>>();
    let mut parents = BTreeMap::new();
    for header in &headers {
        let body = &bodies[header];
        let parent = headers
            .iter()
            .copied()
            .filter(|candidate| {
                candidate != header
                    && body.len() < bodies[candidate].len()
                    && body.is_subset(&bodies[candidate])
            })
            .min_by_key(|candidate| (bodies[candidate].len(), *candidate));
        if let Some(parent) = parent {
            parents.insert(*header, parent);
        }
    }

    let mut children = headers
        .iter()
        .copied()
        .map(|header| (header, BTreeSet::new()))
        .collect::<BTreeMap<_, _>>();
    for (child, parent) in &parents {
        children
            .get_mut(parent)
            .expect("a natural-loop parent is a loop header")
            .insert(*child);
    }
    let roots = headers
        .iter()
        .copied()
        .filter(|header| !parents.contains_key(header))
        .collect();

    let block_nests = reachable
        .iter()
        .copied()
        .map(|block| {
            let mut containing = headers
                .iter()
                .copied()
                .filter(|header| bodies[header].contains(&block))
                .collect::<Vec<_>>();
            containing.sort_by_key(|header| (std::cmp::Reverse(bodies[header].len()), *header));
            (block, containing)
        })
        .collect();

    NaturalLoopForest {
        bodies,
        latches,
        parents,
        children,
        roots,
        block_nests,
    }
}

fn compute_backedges(
    reachable: &BTreeSet<BlockId>,
    successors: &BTreeMap<BlockId, BTreeSet<BlockId>>,
    dominators: &BTreeMap<BlockId, BTreeSet<BlockId>>,
) -> BTreeSet<ControlFlowEdge> {
    let mut backedges = BTreeSet::new();
    for source in reachable {
        for target in &successors[source] {
            if dominators[source].contains(target) {
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
