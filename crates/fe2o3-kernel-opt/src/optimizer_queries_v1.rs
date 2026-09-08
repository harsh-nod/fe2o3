//! Epoch-bound, resource-bounded queries used by target-neutral transforms.
//!
//! Query receipts are evidence about one immutable canonical graph. They are
//! invalid after mutation and never grant semantic or production authority.

use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    error::Error,
    fmt,
};

use fe2o3_kernel_analysis::{PresburgerFailureV1, PresburgerSetDecisionV1, PresburgerSetV1};
use fe2o3_kernel_ir::{
    BlockId, ComparePredicate, Constant, Function, FunctionId, Module, OperationKind, Terminator,
    ValueId, VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrIdentityV13,
    VerifiedCanonicalKernelIrV13, analyze_control_flow,
};

pub const HARD_MAX_OPTIMIZER_QUERY_GRAPH_WORK_V1: usize = 1_000_000;
pub const HARD_MAX_OPTIMIZER_PRESBURGER_POINTS_V1: usize = 1_048_576;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OptimizerQueryLimitsV1 {
    max_graph_work: usize,
    max_presburger_points: usize,
}

impl OptimizerQueryLimitsV1 {
    pub fn new(
        max_graph_work: usize,
        max_presburger_points: usize,
    ) -> Result<Self, OptimizerQueryFailureV1> {
        if max_graph_work == 0
            || max_graph_work > HARD_MAX_OPTIMIZER_QUERY_GRAPH_WORK_V1
            || max_presburger_points == 0
            || max_presburger_points > HARD_MAX_OPTIMIZER_PRESBURGER_POINTS_V1
        {
            return Err(OptimizerQueryFailureV1::InvalidLimits);
        }
        Ok(Self {
            max_graph_work,
            max_presburger_points,
        })
    }

    pub const fn max_graph_work(self) -> usize {
        self.max_graph_work
    }

    pub const fn max_presburger_points(self) -> usize {
        self.max_presburger_points
    }
}

impl Default for OptimizerQueryLimitsV1 {
    fn default() -> Self {
        Self {
            max_graph_work: HARD_MAX_OPTIMIZER_QUERY_GRAPH_WORK_V1,
            max_presburger_points: HARD_MAX_OPTIMIZER_PRESBURGER_POINTS_V1,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OptimizerQueryCoordinateV1 {
    function: FunctionId,
    block: BlockId,
    operation: u32,
}

impl OptimizerQueryCoordinateV1 {
    pub const fn function(&self) -> &FunctionId {
        &self.function
    }

    pub const fn block(&self) -> BlockId {
        self.block
    }

    pub const fn operation(&self) -> u32 {
        self.operation
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CheckedOptimizerQueryKindV1 {
    DominatingUnsignedLessThan {
        destination: BlockId,
        lhs: ValueId,
        rhs: ValueId,
        guard: OptimizerQueryCoordinateV1,
    },
    DominatingIndexEqualsConstant {
        destination: BlockId,
        value: ValueId,
        constant: u64,
        guard: OptimizerQueryCoordinateV1,
    },
    PresburgerSetEmpty,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedOptimizerQueryV1 {
    graph_identity: VerifiedCanonicalKernelIrIdentityV13,
    graph_epoch: u64,
    kind: CheckedOptimizerQueryKindV1,
    work_units: usize,
}

impl CheckedOptimizerQueryV1 {
    pub const fn graph_identity(&self) -> &VerifiedCanonicalKernelIrIdentityV13 {
        &self.graph_identity
    }

    pub const fn graph_epoch(&self) -> u64 {
        self.graph_epoch
    }

    pub const fn kind(&self) -> &CheckedOptimizerQueryKindV1 {
        &self.kind
    }

    pub const fn work_units(&self) -> usize {
        self.work_units
    }

    pub const fn grants_semantic_authority(&self) -> bool {
        false
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptimizerQueryDecisionV1 {
    Proved(CheckedOptimizerQueryV1),
    Refuted { witness: Vec<i128> },
    Incomplete(OptimizerQueryFailureV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum OptimizerQueryFailureV1 {
    InvalidLimits,
    Input(VerifiedCanonicalKernelIrErrorV13),
    MissingFunction(FunctionId),
    GraphWorkLimit { required: usize, limit: usize },
    PresburgerPointLimit { required: usize, limit: usize },
    Presburger(PresburgerFailureV1),
    CoordinateOverflow,
    Unsupported,
}

impl fmt::Display for OptimizerQueryFailureV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => formatter.write_str("optimizer query limits are invalid"),
            Self::Input(error) => error.fmt(formatter),
            Self::MissingFunction(function) => write!(formatter, "missing function {function}"),
            Self::GraphWorkLimit { required, limit } => write!(
                formatter,
                "optimizer query requires {required} graph units but the limit is {limit}"
            ),
            Self::PresburgerPointLimit { required, limit } => write!(
                formatter,
                "optimizer Presburger domain has {required} points but the limit is {limit}"
            ),
            Self::Presburger(error) => error.fmt(formatter),
            Self::CoordinateOverflow => {
                formatter.write_str("optimizer query coordinate overflowed")
            }
            Self::Unsupported => formatter.write_str("optimizer query is unsupported"),
        }
    }
}

impl Error for OptimizerQueryFailureV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Input(error) => Some(error),
            _ => None,
        }
    }
}

pub struct CheckedOptimizerQuerySessionV1<'module> {
    module: &'module Module,
    graph_identity: VerifiedCanonicalKernelIrIdentityV13,
    graph_epoch: u64,
    limits: OptimizerQueryLimitsV1,
}

impl<'module> CheckedOptimizerQuerySessionV1<'module> {
    pub fn new(
        module: &'module Module,
        graph_epoch: u64,
        limits: OptimizerQueryLimitsV1,
    ) -> Result<Self, OptimizerQueryFailureV1> {
        let canonical = VerifiedCanonicalKernelIrV13::from_module(module.clone())
            .map_err(OptimizerQueryFailureV1::Input)?;
        Ok(Self {
            module,
            graph_identity: *canonical.identity(),
            graph_epoch,
            limits,
        })
    }

    pub fn prove_dominating_unsigned_less_than(
        &self,
        function: &FunctionId,
        destination: BlockId,
        lhs: ValueId,
        rhs: ValueId,
    ) -> OptimizerQueryDecisionV1 {
        let Some(function) = self.module.function(function) else {
            return OptimizerQueryDecisionV1::Incomplete(OptimizerQueryFailureV1::MissingFunction(
                function.clone(),
            ));
        };
        match find_dominating_unsigned_less_than_guard_v1(
            function,
            destination,
            lhs,
            rhs,
            self.limits.max_graph_work,
        ) {
            Ok(Some((guard, work_units))) => {
                OptimizerQueryDecisionV1::Proved(CheckedOptimizerQueryV1 {
                    graph_identity: self.graph_identity,
                    graph_epoch: self.graph_epoch,
                    kind: CheckedOptimizerQueryKindV1::DominatingUnsignedLessThan {
                        destination,
                        lhs,
                        rhs,
                        guard,
                    },
                    work_units,
                })
            }
            Ok(None) => OptimizerQueryDecisionV1::Incomplete(OptimizerQueryFailureV1::Unsupported),
            Err(error) => OptimizerQueryDecisionV1::Incomplete(error),
        }
    }

    pub fn prove_presburger_set_empty(&self, set: &PresburgerSetV1) -> OptimizerQueryDecisionV1 {
        let points = match finite_domain_points(set, self.limits.max_presburger_points) {
            Ok(points) => points,
            Err(error) => return OptimizerQueryDecisionV1::Incomplete(error),
        };
        match set.find_witness() {
            PresburgerSetDecisionV1::Empty => {
                OptimizerQueryDecisionV1::Proved(CheckedOptimizerQueryV1 {
                    graph_identity: self.graph_identity,
                    graph_epoch: self.graph_epoch,
                    kind: CheckedOptimizerQueryKindV1::PresburgerSetEmpty,
                    work_units: points,
                })
            }
            PresburgerSetDecisionV1::Witness(witness) => OptimizerQueryDecisionV1::Refuted {
                witness: witness.point().to_vec(),
            },
            PresburgerSetDecisionV1::Incomplete(error) => {
                OptimizerQueryDecisionV1::Incomplete(OptimizerQueryFailureV1::Presburger(error))
            }
        }
    }

    pub fn prove_dominating_index_equals_constant(
        &self,
        function: &FunctionId,
        destination: BlockId,
        value: ValueId,
    ) -> OptimizerQueryDecisionV1 {
        let Some(function) = self.module.function(function) else {
            return OptimizerQueryDecisionV1::Incomplete(OptimizerQueryFailureV1::MissingFunction(
                function.clone(),
            ));
        };
        match find_dominating_index_equals_constant_guard_v1(
            function,
            destination,
            value,
            self.limits.max_graph_work,
        ) {
            Ok(Some((guard, constant, work_units))) => {
                OptimizerQueryDecisionV1::Proved(CheckedOptimizerQueryV1 {
                    graph_identity: self.graph_identity,
                    graph_epoch: self.graph_epoch,
                    kind: CheckedOptimizerQueryKindV1::DominatingIndexEqualsConstant {
                        destination,
                        value,
                        constant,
                        guard,
                    },
                    work_units,
                })
            }
            Ok(None) => OptimizerQueryDecisionV1::Incomplete(OptimizerQueryFailureV1::Unsupported),
            Err(error) => OptimizerQueryDecisionV1::Incomplete(error),
        }
    }
}

pub(crate) fn find_dominating_index_equals_constant_guard_v1(
    function: &Function,
    destination: BlockId,
    value: ValueId,
    max_graph_work: usize,
) -> Result<Option<(OptimizerQueryCoordinateV1, u64, usize)>, OptimizerQueryFailureV1> {
    let Some(body) = &function.body else {
        return Ok(None);
    };
    let cfg = analyze_control_flow(function).map_err(|_| OptimizerQueryFailureV1::Unsupported)?;
    let constants = body
        .blocks
        .iter()
        .flat_map(|block| &block.operations)
        .filter_map(
            |operation| match (operation.results.as_slice(), &operation.kind) {
                ([result], OperationKind::Constant(Constant::Index(constant))) => {
                    Some((result.id, *constant))
                }
                _ => None,
            },
        )
        .collect::<BTreeMap<_, _>>();
    let mut work = 0_usize;
    for block in &body.blocks {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            charge_graph_work(&mut work, max_graph_work)?;
            let OperationKind::Compare {
                predicate: ComparePredicate::Equal,
                lhs,
                rhs,
            } = operation.kind
            else {
                continue;
            };
            let constant = if lhs == value {
                constants.get(&rhs).copied()
            } else if rhs == value {
                constants.get(&lhs).copied()
            } else {
                None
            };
            let Some(constant) = constant else {
                continue;
            };
            let Some(condition) = operation.results.first().map(|result| result.id) else {
                continue;
            };
            let Some(Terminator::ConditionalBranch {
                condition: branch_condition,
                then_target,
                else_target,
                ..
            }) = &block.terminator
            else {
                continue;
            };
            if *branch_condition == condition
                && cfg.dominates(block.id, destination)
                && cfg.dominates(*then_target, destination)
                && !can_reach_block(&cfg, *else_target, destination, &mut work, max_graph_work)?
            {
                let operation = u32::try_from(operation_index)
                    .map_err(|_| OptimizerQueryFailureV1::CoordinateOverflow)?;
                return Ok(Some((
                    OptimizerQueryCoordinateV1 {
                        function: function.id.clone(),
                        block: block.id,
                        operation,
                    },
                    constant,
                    work,
                )));
            }
        }
    }
    Ok(None)
}

fn charge_graph_work(work: &mut usize, limit: usize) -> Result<(), OptimizerQueryFailureV1> {
    *work = work
        .checked_add(1)
        .ok_or(OptimizerQueryFailureV1::GraphWorkLimit {
            required: usize::MAX,
            limit,
        })?;
    if *work > limit {
        return Err(OptimizerQueryFailureV1::GraphWorkLimit {
            required: *work,
            limit,
        });
    }
    Ok(())
}

fn can_reach_block(
    cfg: &fe2o3_kernel_ir::IndexedControlFlow,
    source: BlockId,
    destination: BlockId,
    work: &mut usize,
    work_limit: usize,
) -> Result<bool, OptimizerQueryFailureV1> {
    let mut pending = VecDeque::from([source]);
    let mut visited = BTreeSet::new();
    while let Some(block) = pending.pop_front() {
        charge_graph_work(work, work_limit)?;
        if block == destination {
            return Ok(true);
        }
        if !visited.insert(block) {
            continue;
        }
        let successors = cfg
            .successor_blocks(block)
            .ok_or(OptimizerQueryFailureV1::Unsupported)?;
        for successor in successors {
            charge_graph_work(work, work_limit)?;
            if successor == destination {
                return Ok(true);
            }
            if !visited.contains(&successor) {
                pending.push_back(successor);
            }
        }
    }
    Ok(false)
}

pub(crate) fn find_dominating_unsigned_less_than_guard_v1(
    function: &Function,
    destination: BlockId,
    lhs: ValueId,
    rhs: ValueId,
    max_graph_work: usize,
) -> Result<Option<(OptimizerQueryCoordinateV1, usize)>, OptimizerQueryFailureV1> {
    let Some(body) = &function.body else {
        return Ok(None);
    };
    let cfg = analyze_control_flow(function).map_err(|_| OptimizerQueryFailureV1::Unsupported)?;
    let mut work = 0_usize;
    for block in &body.blocks {
        for (operation_index, operation) in block.operations.iter().enumerate() {
            work = work
                .checked_add(1)
                .ok_or(OptimizerQueryFailureV1::GraphWorkLimit {
                    required: usize::MAX,
                    limit: max_graph_work,
                })?;
            if work > max_graph_work {
                return Err(OptimizerQueryFailureV1::GraphWorkLimit {
                    required: work,
                    limit: max_graph_work,
                });
            }
            let OperationKind::Compare {
                predicate: ComparePredicate::LessThan,
                lhs: guard_lhs,
                rhs: guard_rhs,
            } = operation.kind
            else {
                continue;
            };
            if guard_lhs != lhs || guard_rhs != rhs {
                continue;
            }
            let Some(condition) = operation.results.first().map(|result| result.id) else {
                continue;
            };
            let Some(Terminator::ConditionalBranch {
                condition: branch_condition,
                then_target,
                else_target,
                ..
            }) = &block.terminator
            else {
                continue;
            };
            if *branch_condition == condition
                && cfg.dominates(block.id, destination)
                && cfg.dominates(*then_target, destination)
                && !can_reach_block(&cfg, *else_target, destination, &mut work, max_graph_work)?
            {
                let operation = u32::try_from(operation_index)
                    .map_err(|_| OptimizerQueryFailureV1::CoordinateOverflow)?;
                return Ok(Some((
                    OptimizerQueryCoordinateV1 {
                        function: function.id.clone(),
                        block: block.id,
                        operation,
                    },
                    work,
                )));
            }
        }
    }
    Ok(None)
}

fn finite_domain_points(
    set: &PresburgerSetV1,
    limit: usize,
) -> Result<usize, OptimizerQueryFailureV1> {
    let mut points = 1_usize;
    for (lower, upper) in set
        .domain()
        .lower()
        .iter()
        .zip(set.domain().upper_exclusive())
    {
        let extent = upper.saturating_sub(*lower);
        if extent == 0 {
            return Ok(0);
        }
        let Ok(extent) = usize::try_from(extent) else {
            return Err(OptimizerQueryFailureV1::PresburgerPointLimit {
                required: usize::MAX,
                limit,
            });
        };
        points =
            points
                .checked_mul(extent)
                .ok_or(OptimizerQueryFailureV1::PresburgerPointLimit {
                    required: usize::MAX,
                    limit,
                })?;
        if points > limit {
            return Err(OptimizerQueryFailureV1::PresburgerPointLimit {
                required: points,
                limit,
            });
        }
    }
    Ok(points)
}
