use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::fmt;

use crate::executable::terminator_edges;
use crate::{MirBlockId, MirBody};

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
}

impl fmt::Display for MirControlFlowError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyBody => formatter.write_str("control-flow body has no blocks"),
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
    dominators: Vec<BTreeSet<MirBlockId>>,
    immediate_dominators: Vec<Option<MirBlockId>>,
    dominator_tree_children: Vec<BTreeSet<MirBlockId>>,
    dominance_frontiers: Vec<BTreeSet<MirBlockId>>,
    backedges: BTreeSet<MirControlFlowEdge>,
    loop_bodies: BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
    loop_latches: BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
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

    pub fn dominators(&self, block: MirBlockId) -> Option<&BTreeSet<MirBlockId>> {
        self.dominators.get(block.0 as usize)
    }

    pub fn dominates(&self, dominator: MirBlockId, block: MirBlockId) -> bool {
        self.dominators(block)
            .is_some_and(|set| set.contains(&dominator))
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
    if body.entry.0 as usize >= block_count {
        return Err(MirControlFlowError::InvalidEntry {
            entry: body.entry,
            block_count,
        });
    }

    let mut successors = vec![BTreeSet::new(); block_count];
    for (source_index, block) in body.blocks.iter().enumerate() {
        let source = MirBlockId(source_index as u32);
        for edge in terminator_edges(&block.terminator.kind) {
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
        if reachable.insert(block) {
            pending.extend(successors[block.0 as usize].iter().copied());
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
            predecessors[target.0 as usize].insert(source);
        }
    }
    let dominators = compute_dominators(body.entry, &predecessors);
    let immediate_dominators = compute_immediate_dominators(body.entry, &dominators);
    let dominator_tree_children = compute_dominator_tree(&immediate_dominators);
    let dominance_frontiers = compute_frontiers(&predecessors, &dominators);
    let backedges = compute_backedges(&successors, &dominators);
    if let Some(error) = find_irreducible(&successors, &backedges) {
        return Err(error);
    }
    let (loop_bodies, loop_latches) = compute_natural_loops(&predecessors, &backedges);

    Ok(MirControlFlowAnalysis {
        entry: body.entry,
        successors,
        predecessors,
        dominators,
        immediate_dominators,
        dominator_tree_children,
        dominance_frontiers,
        backedges,
        loop_bodies,
        loop_latches,
    })
}

fn compute_dominators(
    entry: MirBlockId,
    predecessors: &[BTreeSet<MirBlockId>],
) -> Vec<BTreeSet<MirBlockId>> {
    let all = (0..predecessors.len())
        .map(|index| MirBlockId(index as u32))
        .collect::<BTreeSet<_>>();
    let mut result = (0..predecessors.len())
        .map(|index| {
            let block = MirBlockId(index as u32);
            if block == entry {
                BTreeSet::from([entry])
            } else {
                all.clone()
            }
        })
        .collect::<Vec<_>>();

    loop {
        let mut changed = false;
        for index in 0..predecessors.len() {
            let block = MirBlockId(index as u32);
            if block == entry {
                continue;
            }
            let mut incoming = predecessors[index].iter();
            let mut next = incoming
                .next()
                .map(|predecessor| result[predecessor.0 as usize].clone())
                .unwrap_or_default();
            for predecessor in incoming {
                next.retain(|candidate| result[predecessor.0 as usize].contains(candidate));
            }
            next.insert(block);
            changed |= result[index] != next;
            result[index] = next;
        }
        if !changed {
            return result;
        }
    }
}

fn compute_immediate_dominators(
    entry: MirBlockId,
    dominators: &[BTreeSet<MirBlockId>],
) -> Vec<Option<MirBlockId>> {
    dominators
        .iter()
        .enumerate()
        .map(|(index, set)| {
            let block = MirBlockId(index as u32);
            if block == entry {
                return None;
            }
            set.iter()
                .copied()
                .filter(|candidate| *candidate != block)
                .find(|candidate| {
                    set.iter().all(|other| {
                        other == &block
                            || other == candidate
                            || dominators[candidate.0 as usize].contains(other)
                    })
                })
        })
        .collect()
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

fn compute_frontiers(
    predecessors: &[BTreeSet<MirBlockId>],
    dominators: &[BTreeSet<MirBlockId>],
) -> Vec<BTreeSet<MirBlockId>> {
    (0..predecessors.len())
        .map(|dominator_index| {
            let dominator = MirBlockId(dominator_index as u32);
            (0..predecessors.len())
                .map(|index| MirBlockId(index as u32))
                .filter(|candidate| {
                    let strictly_dominates = candidate != &dominator
                        && dominators[candidate.0 as usize].contains(&dominator);
                    !strictly_dominates
                        && predecessors[candidate.0 as usize]
                            .iter()
                            .any(|predecessor| {
                                dominators[predecessor.0 as usize].contains(&dominator)
                            })
                })
                .collect()
        })
        .collect()
}

fn compute_backedges(
    successors: &[BTreeSet<MirBlockId>],
    dominators: &[BTreeSet<MirBlockId>],
) -> BTreeSet<MirControlFlowEdge> {
    successors
        .iter()
        .enumerate()
        .flat_map(|(source_index, targets)| {
            let source = MirBlockId(source_index as u32);
            targets.iter().copied().filter_map(move |target| {
                dominators[source_index]
                    .contains(&target)
                    .then_some(MirControlFlowEdge { source, target })
            })
        })
        .collect()
}

fn compute_natural_loops(
    predecessors: &[BTreeSet<MirBlockId>],
    backedges: &BTreeSet<MirControlFlowEdge>,
) -> (
    BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
    BTreeMap<MirBlockId, BTreeSet<MirBlockId>>,
) {
    let mut bodies = BTreeMap::<_, BTreeSet<_>>::new();
    let mut latches = BTreeMap::<_, BTreeSet<_>>::new();
    for edge in backedges {
        let mut body = BTreeSet::from([edge.target, edge.source]);
        let mut pending = VecDeque::from([edge.source]);
        while let Some(block) = pending.pop_front() {
            if block == edge.target {
                continue;
            }
            for predecessor in &predecessors[block.0 as usize] {
                if body.insert(*predecessor) {
                    pending.push_back(*predecessor);
                }
            }
        }
        bodies.entry(edge.target).or_default().extend(body);
        latches.entry(edge.target).or_default().insert(edge.source);
    }
    (bodies, latches)
}

fn find_irreducible(
    successors: &[BTreeSet<MirBlockId>],
    backedges: &BTreeSet<MirControlFlowEdge>,
) -> Option<MirControlFlowError> {
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
    for blocks in strongly_connected_components(&forward) {
        if blocks.len() == 1 && !forward[blocks[0].0 as usize].contains(&blocks[0]) {
            continue;
        }
        let members = blocks.iter().copied().collect::<BTreeSet<_>>();
        let entries = successors
            .iter()
            .enumerate()
            .flat_map(|(source_index, targets)| {
                let source = MirBlockId(source_index as u32);
                targets
                    .iter()
                    .copied()
                    .map(move |target| MirControlFlowEdge { source, target })
            })
            .filter(|edge| !members.contains(&edge.source) && members.contains(&edge.target))
            .collect::<Vec<_>>();
        return Some(MirControlFlowError::Irreducible { blocks, entries });
    }
    None
}

struct TarjanTraversal<'a> {
    successors: &'a [BTreeSet<MirBlockId>],
    index: usize,
    indices: Vec<Option<usize>>,
    lowlinks: Vec<usize>,
    stack: Vec<MirBlockId>,
    on_stack: BTreeSet<MirBlockId>,
    components: Vec<Vec<MirBlockId>>,
}

impl<'a> TarjanTraversal<'a> {
    fn new(successors: &'a [BTreeSet<MirBlockId>]) -> Self {
        Self {
            successors,
            index: 0,
            indices: vec![None; successors.len()],
            lowlinks: vec![0; successors.len()],
            stack: Vec::new(),
            on_stack: BTreeSet::new(),
            components: Vec::new(),
        }
    }

    fn visit(&mut self, block: MirBlockId) {
        let current = self.index;
        self.index += 1;
        self.indices[block.0 as usize] = Some(current);
        self.lowlinks[block.0 as usize] = current;
        self.stack.push(block);
        self.on_stack.insert(block);

        for successor in &self.successors[block.0 as usize] {
            if self.indices[successor.0 as usize].is_none() {
                self.visit(*successor);
                self.lowlinks[block.0 as usize] =
                    self.lowlinks[block.0 as usize].min(self.lowlinks[successor.0 as usize]);
            } else if self.on_stack.contains(successor) {
                self.lowlinks[block.0 as usize] = self.lowlinks[block.0 as usize].min(
                    self.indices[successor.0 as usize].expect("visited successor has an index"),
                );
            }
        }

        if self.lowlinks[block.0 as usize] == current {
            let mut component = Vec::new();
            loop {
                let member = self.stack.pop().expect("SCC root remains on the stack");
                self.on_stack.remove(&member);
                component.push(member);
                if member == block {
                    break;
                }
            }
            component.sort();
            self.components.push(component);
        }
    }
}

fn strongly_connected_components(successors: &[BTreeSet<MirBlockId>]) -> Vec<Vec<MirBlockId>> {
    let mut traversal = TarjanTraversal::new(successors);
    for block_index in 0..successors.len() {
        if traversal.indices[block_index].is_none() {
            traversal.visit(MirBlockId(block_index as u32));
        }
    }
    traversal.components.sort();
    traversal.components
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
