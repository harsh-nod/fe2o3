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
    backedges: BTreeSet<ControlFlowEdge>,
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

    /// Edges whose target dominates their source.
    pub fn backedges(&self) -> &BTreeSet<ControlFlowEdge> {
        &self.backedges
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
    let backedges = compute_backedges(&reachable, &successors, &dominators);
    let irreducible = irreducible_diagnostics(&reachable, &successors, &backedges);
    if !irreducible.is_empty() {
        return Err(errors(function, irreducible));
    }

    Ok(ControlFlowAnalysis {
        function: function.id.clone(),
        entry,
        blocks: block_ids,
        predecessors,
        reachable,
        dominators,
        backedges,
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

    loop {
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
