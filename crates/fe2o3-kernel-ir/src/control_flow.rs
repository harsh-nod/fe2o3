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
    block_positions: Vec<(BlockId, usize)>,
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
        if let Ok(position) = usize::try_from(block.0)
            && self.block_ids.get(position) == Some(&block)
        {
            return Some(position);
        }
        self.block_positions
            .binary_search_by_key(&block, |row| row.0)
            .ok()
            .map(|position| self.block_positions[position].1)
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
        self.dominates_positions(definition, use_block)
    }

    fn dominates_positions(&self, definition: usize, use_block: usize) -> bool {
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
    analyze_control_flow_shared_v1(
        function,
        limits,
        &mut ControlFlowResourcesV1 { budget: None },
    )
    .map_err(|error| match error {
        MeteredControlFlowErrorV1::ControlFlow(error) => error,
        MeteredControlFlowErrorV1::Resource(_) => {
            ControlFlowError::ArithmeticOverflow(ControlFlowResource::AnalysisWork)
        }
    })
}

include!("control_flow_resources_v1.rs");
include!("control_flow_build_v1.rs");
include!("control_flow_analysis_v1.rs");

fn for_each_terminator_edge<E>(
    terminator: &Terminator,
    mut visit: impl FnMut(BlockId, &[ValueId]) -> Result<(), E>,
) -> Result<(), E> {
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
#[path = "control_flow_entry_dominators_tests.rs"]
mod entry_dominator_tests;

#[cfg(test)]
mod tests {
    use super::*;

    include!("control_flow_metered_tests.rs");
    include!("control_flow_entry_dominators_01_tests.rs");

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
