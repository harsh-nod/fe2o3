//! Inert, owner-borrowed path-domain refinement of a fresh V1 conflict report.
//! The V1 full-witness envelope and serialized receipt are never modified.

use crate::{
    ControlFlowAnalysis, PresburgerAffineExprV1 as Expr, PresburgerBoxV1,
    PresburgerConstraintV1 as Constraint, PresburgerFailureV1, PresburgerSetDecisionV1,
    PresburgerSetV1, analyze_control_flow,
};
use fe2o3_kernel_ir::{
    Axis, BinaryOp, BlockId, ByteExpression, CastKind, ComparePredicate, Constant,
    ExplicitLaunchExtent1d, FormalIndexWidth, FormalMemoryAccess, FormalMemoryCandidatePairErrorV1,
    FormalMemoryObligationAnalysis, FormalMemoryObligationError, Function, IndexKind,
    IntrinsicKind, KernelId, LaunchDomain, Module, Operation, OperationKind, ScalarType,
    Terminator, Type, UnaryOp, ValueId, VerifiedKernelIrModuleV1,
    bound_formal_memory_candidate_pairs_v1, derive_kernel_memory_obligations_from_verified,
};
use std::{collections::BTreeMap, error::Error as StdError, fmt};

#[path = "formal_path_conflicts_v18.rs"]
mod actual_owner_v18;
pub use actual_owner_v18::FormalPathConflictsV18;

/// Standalone analysis caps. These do not claim production shared-ledger custody.
/// Larger caller values cannot raise the fixed maxima in `Default`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct FormalPathConflictLimitsV1 {
    pub source_items: usize,
    pub construction_steps: usize,
    pub conflicts: usize,
    pub constraints_per_access: usize,
    pub queries: usize,
}
impl Default for FormalPathConflictLimitsV1 {
    fn default() -> Self {
        Self {
            source_items: 16_384,
            construction_steps: 1_048_576,
            conflicts: 1_024,
            constraints_per_access: 120,
            queries: 2_048,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FormalPathConflictResourceV1 {
    SourceItems,
    ConstructionSteps,
    Conflicts,
    Constraints,
    Queries,
}

#[derive(Debug)]
pub enum FormalPathConflictErrorV1 {
    Preflight(FormalMemoryCandidatePairErrorV1),
    Formal(FormalMemoryObligationError),
    /// An exact one-dimensional launch is the only initial coordinate contract.
    UnsupportedLaunch,
    ControlFlow,
    InconsistentReport,
    Limit {
        resource: FormalPathConflictResourceV1,
        actual: usize,
        limit: usize,
    },
}
impl fmt::Display for FormalPathConflictErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "bounded path conflict analysis: {self:?}")
    }
}
impl StdError for FormalPathConflictErrorV1 {}
type Error = FormalPathConflictErrorV1;
type Result<T> = std::result::Result<T, Error>;
type Limits = FormalPathConflictLimitsV1;
type Resource = FormalPathConflictResourceV1;

/// Observations about this report's exact conflict ordinal, never launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FormalPathConflictDecisionV1 {
    /// Both orders of two distinct invocations have an empty overlap relation.
    Disjoint,
    /// A witness in the conservative domain; unsupported predicates may be absent.
    PossibleOverlap {
        left: u64,
        right: u64,
    },
    IncompleteFormalReport,
    UnsupportedAddress,
    IncompletePresburger(PresburgerFailureV1),
}
type Decision = FormalPathConflictDecisionV1;

/// A lexical analysis of an actual immutable verified module. Fresh formal rows
/// are derived internally; callers cannot inject an alleged report or a digest.
/// `belongs_to` uses pointer identity, not structural equality. Coordinates and
/// decisions alone do not authenticate another source or optimized output.
///
/// This object does not implement a receipt, admission or publication interface.
/// Its caps bound analysis shape and the existing solver bounds each query, but
/// its allocations do not participate in the production shared resource ledger.
/// A future admission path must bind actual owner custody, launch and ledger,
/// and introduce an independently checked versioned discharge contract.
///
/// ```compile_fail
/// use fe2o3_kernel_analysis::{FormalPathConflictsV1, FormalPathConflictLimitsV1};
/// use fe2o3_kernel_ir::*;
/// fn escape(module: Module) -> FormalPathConflictsV1<'static> {
///     FormalPathConflictsV1::derive(verify_module_ref(&module).unwrap(),
///         &KernelId::new("kernel"), ExplicitLaunchExtent1d::Exact(64),
///         FormalIndexWidth::Bits64, FormalPathConflictLimitsV1::default()).unwrap()
/// }
/// ```
#[derive(Debug)]
pub struct FormalPathConflictsV1<'owner> {
    owner: VerifiedKernelIrModuleV1<'owner>,
    launch: ExplicitLaunchExtent1d,
    formal: FormalMemoryObligationAnalysis,
    decisions: Vec<Decision>,
    queries: usize,
    construction_steps: usize,
}

impl<'owner> FormalPathConflictsV1<'owner> {
    pub fn derive(
        owner: VerifiedKernelIrModuleV1<'owner>,
        kernel: &KernelId,
        launch: ExplicitLaunchExtent1d,
        index_width: FormalIndexWidth,
        limits: Limits,
    ) -> Result<Self> {
        let limits = clamp(limits);
        check_source_items(owner.module(), limits)?;
        let selected = owner
            .module()
            .kernels
            .iter()
            .find(|row| &row.id == kernel)
            .ok_or(Error::InconsistentReport)?;
        let ExplicitLaunchExtent1d::Exact(extent) = launch else {
            return Err(Error::UnsupportedLaunch);
        };
        if extent == 0 || !matches!(selected.domain, LaunchDomain::D1 { .. }) {
            return Err(Error::UnsupportedLaunch);
        }
        bound_formal_memory_candidate_pairs_v1(owner, limits.conflicts).map_err(
            |error| match error {
                FormalMemoryCandidatePairErrorV1::Limit { actual, limit } => Error::Limit {
                    resource: Resource::Conflicts,
                    actual,
                    limit,
                },
                other => Error::Preflight(other),
            },
        )?;
        let formal =
            derive_kernel_memory_obligations_from_verified(owner, kernel, launch, index_width)
                .map_err(Error::Formal)?;
        let observed = analyze_fresh_report(owner.module(), &formal, extent, index_width, limits)?;
        Ok(Self {
            owner,
            launch,
            formal,
            decisions: observed.decisions,
            queries: observed.queries,
            construction_steps: observed.construction_steps,
        })
    }

    pub fn belongs_to(&self, owner: VerifiedKernelIrModuleV1<'_>) -> bool {
        std::ptr::eq(self.owner.module(), owner.module())
    }
    pub const fn owner(&self) -> VerifiedKernelIrModuleV1<'owner> {
        self.owner
    }
    pub const fn launch(&self) -> ExplicitLaunchExtent1d {
        self.launch
    }
    pub const fn formal(&self) -> &FormalMemoryObligationAnalysis {
        &self.formal
    }
    pub fn decisions(&self) -> &[Decision] {
        &self.decisions
    }
    pub const fn queries(&self) -> usize {
        self.queries
    }
    /// Local construction visits, excluding independently bounded formal, CFG and solver work.
    pub const fn construction_steps(&self) -> usize {
        self.construction_steps
    }
}

fn check_source_items(module: &Module, limits: Limits) -> Result<()> {
    let mut items = 0usize;
    for function in &module.functions {
        count(&mut items, 1, limits.source_items, Resource::SourceItems)?;
        if let Some(body) = &function.body {
            for block in &body.blocks {
                count(&mut items, 1, limits.source_items, Resource::SourceItems)?;
                count(
                    &mut items,
                    block.operations.len(),
                    limits.source_items,
                    Resource::SourceItems,
                )?;
                count(
                    &mut items,
                    block.parameters.len(),
                    limits.source_items,
                    Resource::SourceItems,
                )?;
            }
        }
    }
    Ok(())
}

struct ObservedConflicts {
    decisions: Vec<Decision>,
    queries: usize,
    construction_steps: usize,
}

// Only the nominal owner entrances supply a freshly bound report to this engine.
// It is deliberately private: an arbitrary Module/report pair is not authority.
fn analyze_fresh_report(
    module: &Module,
    formal: &FormalMemoryObligationAnalysis,
    extent: u64,
    index_width: FormalIndexWidth,
    limits: Limits,
) -> Result<ObservedConflicts> {
    let conflicts = formal.obligations().inter_invocation_conflicts();
    check(conflicts.len(), limits.conflicts, Resource::Conflicts)?;
    let mut decisions = Vec::with_capacity(conflicts.len());
    if !formal.is_complete() {
        decisions.resize(conflicts.len(), Decision::IncompleteFormalReport);
        return Ok(ObservedConflicts {
            decisions,
            queries: 0,
            construction_steps: 0,
        });
    }
    let function = module
        .functions
        .iter()
        .find(|row| &row.id == formal.obligations().entry())
        .ok_or(Error::InconsistentReport)?;
    let cfg = analyze_control_flow(function).map_err(|_| Error::ControlFlow)?;
    let mut engine = Engine::new(function, cfg, extent, index_width, limits)?;
    let accesses = formal.obligations().accesses();
    let mut by_location = BTreeMap::new();
    for (ordinal, access) in accesses.iter().enumerate() {
        engine.step()?;
        if by_location.insert(access.location(), ordinal).is_some() {
            return Err(Error::InconsistentReport);
        }
    }
    for conflict in conflicts {
        engine.step()?;
        let left = accesses
            .get(
                *by_location
                    .get(&conflict.left())
                    .ok_or(Error::InconsistentReport)?,
            )
            .ok_or(Error::InconsistentReport)?;
        let right = accesses
            .get(
                *by_location
                    .get(&conflict.right())
                    .ok_or(Error::InconsistentReport)?,
            )
            .ok_or(Error::InconsistentReport)?;
        if left.allocation() != conflict.allocation() || right.allocation() != conflict.allocation()
        {
            return Err(Error::InconsistentReport);
        }
        decisions.push(engine.compare(left, right)?);
    }
    Ok(ObservedConflicts {
        decisions,
        queries: engine.queries,
        construction_steps: engine.steps,
    })
}

fn clamp(value: Limits) -> Limits {
    let cap = Limits::default();
    Limits {
        source_items: value.source_items.min(cap.source_items),
        construction_steps: value.construction_steps.min(cap.construction_steps),
        conflicts: value.conflicts.min(cap.conflicts),
        constraints_per_access: value.constraints_per_access.min(cap.constraints_per_access),
        queries: value.queries.min(cap.queries),
    }
}
fn check(actual: usize, limit: usize, resource: Resource) -> Result<()> {
    if actual > limit {
        Err(Error::Limit {
            resource,
            actual,
            limit,
        })
    } else {
        Ok(())
    }
}
fn count(current: &mut usize, amount: usize, limit: usize, resource: Resource) -> Result<()> {
    let actual = current.saturating_add(amount);
    check(actual, limit, resource)?;
    *current = actual;
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Affine {
    constant: i128,
    coefficient: i128,
}
impl Affine {
    fn constant(value: i128) -> Self {
        Self {
            constant: value,
            coefficient: 0,
        }
    }
    fn add(self, other: Self) -> Option<Self> {
        Some(Self {
            constant: self.constant.checked_add(other.constant)?,
            coefficient: self.coefficient.checked_add(other.coefficient)?,
        })
    }
    fn scale(self, factor: i128) -> Option<Self> {
        Some(Self {
            constant: self.constant.checked_mul(factor)?,
            coefficient: self.coefficient.checked_mul(factor)?,
        })
    }
    fn subtract(self, other: Self) -> Option<Self> {
        self.add(other.scale(-1)?)
    }
    fn fits(self, maximum: u64, extent: u64) -> bool {
        let last = self
            .coefficient
            .checked_mul(i128::from(extent - 1))
            .and_then(|offset| self.constant.checked_add(offset));
        let max = i128::from(maximum);
        self.constant >= 0 && self.constant <= max && last.is_some_and(|v| v >= 0 && v <= max)
    }
    fn lift(self, dimension: usize) -> std::result::Result<Expr, PresburgerFailureV1> {
        let mut coefficients = vec![0; 2];
        coefficients[dimension] = self.coefficient;
        Expr::new(self.constant, coefficients)
    }
}

#[derive(Clone, Copy, Debug)]
struct Fact {
    expression: Affine,
    equality: bool,
}
#[derive(Clone, Copy, Debug)]
struct Guard {
    target: BlockId,
    condition: ValueId,
    truth: bool,
}

struct Engine<'g> {
    cfg: ControlFlowAnalysis,
    operations: BTreeMap<ValueId, &'g Operation>,
    affine_cache: BTreeMap<ValueId, Option<Affine>>,
    guards: Vec<Guard>,
    domains: BTreeMap<BlockId, Vec<Fact>>,
    extent: u64,
    index_width: FormalIndexWidth,
    limits: Limits,
    steps: usize,
    queries: usize,
}
impl<'g> Engine<'g> {
    fn new(
        function: &'g Function,
        cfg: ControlFlowAnalysis,
        extent: u64,
        index_width: FormalIndexWidth,
        limits: Limits,
    ) -> Result<Self> {
        let mut this = Self {
            cfg,
            operations: BTreeMap::new(),
            affine_cache: BTreeMap::new(),
            guards: Vec::new(),
            domains: BTreeMap::new(),
            extent,
            index_width,
            limits,
            steps: 0,
            queries: 0,
        };
        let body = function.body.as_ref().ok_or(Error::InconsistentReport)?;
        for block in &body.blocks {
            this.step()?;
            for operation in &block.operations {
                this.step()?;
                if let [result] = operation.results.as_slice() {
                    if this.operations.insert(result.id, operation).is_some() {
                        return Err(Error::InconsistentReport);
                    }
                }
            }
        }
        for block in &body.blocks {
            this.step()?;
            if !this.cfg.is_reachable(block.id) {
                continue;
            }
            let branch = match block.terminator.as_ref().ok_or(Error::InconsistentReport)? {
                Terminator::ConditionalBranch {
                    condition,
                    then_target,
                    else_target,
                    ..
                } => Some((*condition, *then_target, *else_target)),
                Terminator::Switch {
                    selector,
                    cases,
                    default_target,
                    ..
                } => {
                    if let Some(condition) = this.boolean_selector(*selector) {
                        let destination = |value| {
                            cases
                                .iter()
                                .find(|case| case.value == value)
                                .map_or(*default_target, |case| case.target)
                        };
                        Some((condition, destination(1), destination(0)))
                    } else {
                        None
                    }
                }
                _ => None,
            };
            if let Some((condition, yes, no)) = branch {
                if yes == no {
                    continue;
                }
                for (target, truth) in [(yes, true), (no, false)] {
                    this.step()?;
                    // A sole predecessor is insufficient if both branch edges
                    // target it. Distinct targets above authenticate the edge.
                    if target != this.cfg.entry()
                        && this
                            .cfg
                            .predecessors(target)
                            .is_some_and(|preds| preds.len() == 1 && preds.contains(&block.id))
                    {
                        this.guards.push(Guard {
                            target,
                            condition,
                            truth,
                        });
                    }
                }
            }
        }
        Ok(this)
    }
    fn step(&mut self) -> Result<()> {
        count(
            &mut self.steps,
            1,
            self.limits.construction_steps,
            Resource::ConstructionSteps,
        )
    }
    fn boolean_selector(&self, value: ValueId) -> Option<ValueId> {
        let op = self.operations.get(&value)?;
        match &op.kind {
            OperationKind::Cast {
                kind: CastKind::ZeroExtend,
                value,
                ..
            } => {
                let input = self.operations.get(value)?;
                (input.results[0].ty == Type::BOOL).then_some(*value)
            }
            _ => None,
        }
    }
    fn maximum(&self, ty: &Type) -> Option<u64> {
        match ty {
            Type::Scalar(ScalarType::Index) => match self.index_width {
                FormalIndexWidth::Unknown => None,
                FormalIndexWidth::Bits32 => Some(u64::from(u32::MAX)),
                FormalIndexWidth::Bits64 => Some(u64::MAX),
            },
            Type::Scalar(ScalarType::U8) => Some(u64::from(u8::MAX)),
            Type::Scalar(ScalarType::U16) => Some(u64::from(u16::MAX)),
            Type::Scalar(ScalarType::U32) => Some(u64::from(u32::MAX)),
            Type::Scalar(ScalarType::U64) => Some(u64::MAX),
            _ => None,
        }
    }
    fn affine(&mut self, value: ValueId, depth: usize) -> Result<Option<Affine>> {
        self.step()?;
        if depth >= 32 {
            return Ok(None);
        }
        if let Some(cached) = self.affine_cache.get(&value) {
            return Ok(*cached);
        }
        let Some(&op) = self.operations.get(&value) else {
            return Ok(None);
        };
        let Some(maximum) = self.maximum(&op.results[0].ty) else {
            return Ok(None);
        };
        let candidate = match &op.kind {
            OperationKind::Constant(value) => match value {
                Constant::Index(v) | Constant::U64(v) => Some(Affine::constant(i128::from(*v))),
                Constant::U32(v) => Some(Affine::constant(i128::from(*v))),
                Constant::U16(v) => Some(Affine::constant(i128::from(*v))),
                Constant::U8(v) => Some(Affine::constant(i128::from(*v))),
                _ => None,
            },
            OperationKind::Intrinsic(intrinsic)
                if matches!(
                    intrinsic.kind,
                    IntrinsicKind::InvocationIndex {
                        kind: IndexKind::Global,
                        axis: Axis::X
                    }
                ) =>
            {
                Some(Affine {
                    constant: 0,
                    coefficient: 1,
                })
            }
            OperationKind::Binary { op, lhs, rhs } => {
                let a = self.affine(*lhs, depth + 1)?;
                let b = self.affine(*rhs, depth + 1)?;
                match (a, b) {
                    (Some(a), Some(b)) => match op {
                        BinaryOp::Add => a.add(b),
                        BinaryOp::Subtract => a.subtract(b),
                        BinaryOp::Multiply if b.coefficient == 0 => a.scale(b.constant),
                        BinaryOp::Multiply if a.coefficient == 0 => b.scale(a.constant),
                        _ => None,
                    },
                    _ => None,
                }
            }
            OperationKind::Cast { kind, value, to } => {
                let input = self.operations.get(value).map(|op| &op.results[0].ty);
                let admitted = match (kind, input, to) {
                    (CastKind::ZeroExtend, Some(input), _) => self.maximum(input).is_some(),
                    (CastKind::Bitcast, Some(Type::Scalar(from)), Type::Scalar(to)) => {
                        self.index_width == FormalIndexWidth::Bits64
                            && matches!(
                                (from, to),
                                (ScalarType::Index, ScalarType::U64)
                                    | (ScalarType::U64, ScalarType::Index)
                            )
                    }
                    _ => false,
                };
                if admitted {
                    self.affine(*value, depth + 1)?
                } else {
                    None
                }
            }
            _ => None,
        }
        .filter(|value| value.fits(maximum, self.extent));
        self.affine_cache.insert(value, candidate);
        Ok(candidate)
    }
    fn predicate(
        &mut self,
        value: ValueId,
        truth: bool,
        depth: usize,
        facts: &mut Vec<Fact>,
    ) -> Result<()> {
        self.step()?;
        if depth >= 32 {
            return Ok(());
        }
        let Some(&op) = self.operations.get(&value) else {
            return Ok(());
        };
        if op.results[0].ty != Type::BOOL {
            return Ok(());
        }
        let fact = match &op.kind {
            OperationKind::Constant(Constant::Bool(actual)) if *actual != truth => Some(Fact {
                expression: Affine::constant(1),
                equality: true,
            }),
            OperationKind::Unary {
                op: UnaryOp::Not,
                operand,
            } => {
                self.predicate(*operand, !truth, depth + 1, facts)?;
                None
            }
            OperationKind::Binary {
                op: BinaryOp::BitAnd,
                lhs,
                rhs,
            } if truth => {
                self.predicate(*lhs, true, depth + 1, facts)?;
                self.predicate(*rhs, true, depth + 1, facts)?;
                None
            }
            OperationKind::Binary {
                op: BinaryOp::BitOr,
                lhs,
                rhs,
            } if !truth => {
                self.predicate(*lhs, false, depth + 1, facts)?;
                self.predicate(*rhs, false, depth + 1, facts)?;
                None
            }
            OperationKind::Compare {
                predicate,
                lhs,
                rhs,
            } => {
                let left = self.affine(*lhs, 0)?;
                let right = self.affine(*rhs, 0)?;
                match (left, right) {
                    (Some(left), Some(right)) => {
                        let predicate = if truth {
                            *predicate
                        } else {
                            negate(*predicate)
                        };
                        let (a, b, strict, equality) = match predicate {
                            ComparePredicate::Equal => (left, right, false, true),
                            ComparePredicate::LessThan => (left, right, true, false),
                            ComparePredicate::LessThanOrEqual => (left, right, false, false),
                            ComparePredicate::GreaterThan => (right, left, true, false),
                            ComparePredicate::GreaterThanOrEqual => (right, left, false, false),
                            ComparePredicate::NotEqual => return Ok(()),
                        };
                        a.subtract(b)
                            .and_then(|v| v.add(Affine::constant(i128::from(strict))))
                            .map(|expression| Fact {
                                expression,
                                equality,
                            })
                    }
                    _ => None,
                }
            }
            _ => None,
        };
        if let Some(fact) = fact {
            check(
                facts.len().saturating_add(1),
                self.limits.constraints_per_access,
                Resource::Constraints,
            )?;
            facts.push(fact);
        }
        Ok(())
    }
    fn domain(&mut self, block: BlockId) -> Result<()> {
        if self.domains.contains_key(&block) {
            return Ok(());
        }
        let mut facts = Vec::new();
        for index in 0..self.guards.len() {
            self.step()?;
            let guard = self.guards[index];
            if self.cfg.dominates(guard.target, block) {
                self.predicate(guard.condition, guard.truth, 0, &mut facts)?;
            }
        }
        self.domains.insert(block, facts);
        Ok(())
    }
    fn compare(
        &mut self,
        left: &FormalMemoryAccess,
        right: &FormalMemoryAccess,
    ) -> Result<Decision> {
        self.domain(left.location().block)?;
        self.domain(right.location().block)?;
        let (Some(a), Some(b)) = (address(left), address(right)) else {
            return Ok(Decision::UnsupportedAddress);
        };
        let relation = || -> std::result::Result<_, PresburgerFailureV1> {
            let mut constraints = Vec::new();
            for (dimension, access) in [(0, left), (1, right)] {
                for fact in &self.domains[&access.location().block] {
                    let expression = fact.expression.lift(dimension)?;
                    constraints.push(if fact.equality {
                        Constraint::EqualZero(expression)
                    } else {
                        Constraint::LessEqualZero(expression)
                    });
                }
            }
            let a = a.lift(0)?;
            let b = b.lift(1)?;
            constraints
                .push(Constraint::LessEqualZero(a.checked_sub(&b)?.checked_add(
                    &Expr::constant(1 - i128::from(right.byte_width()), 2)?,
                )?));
            constraints
                .push(Constraint::LessEqualZero(b.checked_sub(&a)?.checked_add(
                    &Expr::constant(1 - i128::from(left.byte_width()), 2)?,
                )?));
            Ok(constraints)
        }();
        let constraints = match relation {
            Ok(v) => v,
            Err(error) => return Ok(Decision::IncompletePresburger(error)),
        };
        // Disequality is two independently checked orders, not an omitted row.
        for coefficients in [[1, -1], [-1, 1]] {
            count(&mut self.queries, 1, self.limits.queries, Resource::Queries)?;
            let query = || -> std::result::Result<_, PresburgerFailureV1> {
                let mut constraints = constraints.clone();
                constraints.push(Constraint::LessEqualZero(Expr::new(
                    1,
                    coefficients.to_vec(),
                )?));
                let domain = PresburgerBoxV1::new(
                    vec![
                        i128::from(left.invocations().start()),
                        i128::from(right.invocations().start()),
                    ],
                    vec![
                        i128::from(left.invocations().end_exclusive()),
                        i128::from(right.invocations().end_exclusive()),
                    ],
                )?;
                Ok(PresburgerSetV1::new(domain, constraints)?.find_witness())
            }();
            match query {
                Ok(PresburgerSetDecisionV1::Empty) => {}
                Ok(PresburgerSetDecisionV1::Witness(witness)) => {
                    return Ok(Decision::PossibleOverlap {
                        left: u64::try_from(witness.point()[0])
                            .map_err(|_| Error::InconsistentReport)?,
                        right: u64::try_from(witness.point()[1])
                            .map_err(|_| Error::InconsistentReport)?,
                    });
                }
                Ok(PresburgerSetDecisionV1::Incomplete(error)) | Err(error) => {
                    return Ok(Decision::IncompletePresburger(error));
                }
            }
        }
        Ok(Decision::Disjoint)
    }
}
fn negate(predicate: ComparePredicate) -> ComparePredicate {
    match predicate {
        ComparePredicate::Equal => ComparePredicate::NotEqual,
        ComparePredicate::NotEqual => ComparePredicate::Equal,
        ComparePredicate::LessThan => ComparePredicate::GreaterThanOrEqual,
        ComparePredicate::LessThanOrEqual => ComparePredicate::GreaterThan,
        ComparePredicate::GreaterThan => ComparePredicate::LessThanOrEqual,
        ComparePredicate::GreaterThanOrEqual => ComparePredicate::LessThan,
    }
}
fn address(access: &FormalMemoryAccess) -> Option<Affine> {
    if access.byte_width() == 0 {
        return None;
    }
    match access.byte_offset() {
        ByteExpression::Affine {
            constant,
            invocation_coefficient,
        } => Some(Affine {
            constant: i128::from(constant),
            coefficient: i128::from(invocation_coefficient),
        }),
        ByteExpression::Unbounded => None,
    }
}

#[cfg(test)]
#[path = "formal_path_conflicts_v1_tests.rs"]
mod tests;
