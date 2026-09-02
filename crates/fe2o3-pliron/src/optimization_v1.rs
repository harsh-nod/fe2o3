//! Closed, owner-authenticated execution of the audited Pliron optimization set.
//!
//! This module deliberately does not accept upstream passes, callbacks, pointers,
//! or contexts from callers. Adding a pass requires changing the closed enum and
//! constructing the trusted upstream implementation in [`run_trusted_pass`].

use std::{
    collections::HashSet,
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

use dialect_gpu::{cse_v1::LocalPureCsePassV1, optimization_v1::SelectSameValuePattern};
use pliron::{
    context::Ptr,
    irbuild::{IRStatus, match_rewrite::PassWrapper},
    operation::{Operation, verify_operation},
    opts::{constants::sccp::SCCPPass, dce::DCEPass, simplify_cfg::SimplifyCFGPass},
    pass::{AnalysisManager, Pass, Passes},
};

use crate::{
    HARD_MAX_OPERATION_HANDLES, HARD_MAX_OPERATION_TREE_ITEMS, HARD_MAX_PASSES,
    HARD_MAX_SESSION_OPERATION_TREE_ITEMS, OperationHandle, OperationHandleError, PlironSession,
    inspect_operation_tree_details,
};

/// Maximum number of passes admitted by one closed optimization plan.
pub const HARD_MAX_PLIRON_OPTIMIZATION_PASSES_V1: usize = HARD_MAX_PASSES;

/// Maximum recursively inspected graph work admitted at any pass boundary.
pub const HARD_MAX_PLIRON_OPTIMIZATION_GRAPH_WORK_V1: usize = HARD_MAX_OPERATION_TREE_ITEMS;

/// Maximum conservatively accounted work for one optimization execution.
///
/// The bound covers initial inspection and verification, pass execution plus
/// post-pass inspection and verification, and final handle reconciliation.
pub const HARD_MAX_PLIRON_OPTIMIZATION_WORK_UNITS_V1: usize =
    HARD_MAX_OPERATION_TREE_ITEMS * (3 + 3 * HARD_MAX_PASSES) + HARD_MAX_OPERATION_HANDLES;

/// The only Pliron optimizations callable through the public session boundary.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum PlironOptimizationPassV1 {
    DeadCodeElimination,
    SparseConditionalConstantPropagation,
    SelectSameValueCanonicalization,
    LocalPureCommonSubexpressionElimination,
    SimplifyControlFlow,
}

impl PlironOptimizationPassV1 {
    pub const fn name(self) -> &'static str {
        match self {
            Self::DeadCodeElimination => "dead-code-elimination",
            Self::SparseConditionalConstantPropagation => "sparse-conditional-constant-propagation",
            Self::SelectSameValueCanonicalization => "select-same-value-canonicalization",
            Self::LocalPureCommonSubexpressionElimination => {
                "local-pure-common-subexpression-elimination"
            }
            Self::SimplifyControlFlow => "simplify-control-flow",
        }
    }
}

/// A resource configured for one closed optimization execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlironOptimizationResourceV1 {
    Passes,
    GraphWork,
    WorkUnits,
}

/// Invalid optimization resource configuration.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlironOptimizationLimitErrorV1 {
    Zero(PlironOptimizationResourceV1),
    AboveHardCap(PlironOptimizationResourceV1),
}

impl fmt::Display for PlironOptimizationLimitErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Zero(resource) => write!(formatter, "{resource:?} limit must be non-zero"),
            Self::AboveHardCap(resource) => {
                write!(formatter, "{resource:?} limit exceeds its hard cap")
            }
        }
    }
}

impl Error for PlironOptimizationLimitErrorV1 {}

/// Non-bypassable limits for one closed optimization execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlironOptimizationLimitsV1 {
    max_passes: usize,
    max_graph_work: usize,
    max_work_units: usize,
}

impl PlironOptimizationLimitsV1 {
    pub fn new(
        max_passes: usize,
        max_graph_work: usize,
        max_work_units: usize,
    ) -> Result<Self, PlironOptimizationLimitErrorV1> {
        validate_limit(
            max_passes,
            HARD_MAX_PLIRON_OPTIMIZATION_PASSES_V1,
            PlironOptimizationResourceV1::Passes,
        )?;
        validate_limit(
            max_graph_work,
            HARD_MAX_PLIRON_OPTIMIZATION_GRAPH_WORK_V1,
            PlironOptimizationResourceV1::GraphWork,
        )?;
        validate_limit(
            max_work_units,
            HARD_MAX_PLIRON_OPTIMIZATION_WORK_UNITS_V1,
            PlironOptimizationResourceV1::WorkUnits,
        )?;
        Ok(Self {
            max_passes,
            max_graph_work,
            max_work_units,
        })
    }

    pub const fn max_passes(self) -> usize {
        self.max_passes
    }

    pub const fn max_graph_work(self) -> usize {
        self.max_graph_work
    }

    pub const fn max_work_units(self) -> usize {
        self.max_work_units
    }
}

impl Default for PlironOptimizationLimitsV1 {
    fn default() -> Self {
        Self {
            max_passes: HARD_MAX_PLIRON_OPTIMIZATION_PASSES_V1,
            max_graph_work: HARD_MAX_PLIRON_OPTIMIZATION_GRAPH_WORK_V1,
            max_work_units: HARD_MAX_PLIRON_OPTIMIZATION_WORK_UNITS_V1,
        }
    }
}

fn validate_limit(
    value: usize,
    hard_cap: usize,
    resource: PlironOptimizationResourceV1,
) -> Result<(), PlironOptimizationLimitErrorV1> {
    if value == 0 {
        return Err(PlironOptimizationLimitErrorV1::Zero(resource));
    }
    if value > hard_cap {
        return Err(PlironOptimizationLimitErrorV1::AboveHardCap(resource));
    }
    Ok(())
}

/// Why construction of a closed optimization plan failed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlironOptimizationPlanErrorV1 {
    TooManyPasses { required: usize, limit: usize },
}

impl fmt::Display for PlironOptimizationPlanErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TooManyPasses { required, limit } => write!(
                formatter,
                "optimization plan requires {required} passes but the limit is {limit}"
            ),
        }
    }
}

impl Error for PlironOptimizationPlanErrorV1 {}

/// An immutable, ordered plan containing only audited optimizer variants.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironOptimizationPlanV1 {
    passes: Vec<PlironOptimizationPassV1>,
    limits: PlironOptimizationLimitsV1,
}

impl PlironOptimizationPlanV1 {
    pub fn new(
        passes: Vec<PlironOptimizationPassV1>,
        limits: PlironOptimizationLimitsV1,
    ) -> Result<Self, PlironOptimizationPlanErrorV1> {
        if passes.len() > limits.max_passes {
            return Err(PlironOptimizationPlanErrorV1::TooManyPasses {
                required: passes.len(),
                limit: limits.max_passes,
            });
        }
        Ok(Self { passes, limits })
    }

    /// The standard deterministic cleanup order.
    pub fn standard() -> Self {
        Self {
            passes: vec![
                PlironOptimizationPassV1::SparseConditionalConstantPropagation,
                PlironOptimizationPassV1::SimplifyControlFlow,
                PlironOptimizationPassV1::SelectSameValueCanonicalization,
                PlironOptimizationPassV1::DeadCodeElimination,
                PlironOptimizationPassV1::LocalPureCommonSubexpressionElimination,
                PlironOptimizationPassV1::DeadCodeElimination,
                PlironOptimizationPassV1::SimplifyControlFlow,
            ],
            limits: PlironOptimizationLimitsV1::default(),
        }
    }

    pub fn dead_code_elimination() -> Self {
        Self {
            passes: vec![PlironOptimizationPassV1::DeadCodeElimination],
            limits: PlironOptimizationLimitsV1::default(),
        }
    }

    pub fn passes(&self) -> &[PlironOptimizationPassV1] {
        &self.passes
    }

    pub const fn limits(&self) -> PlironOptimizationLimitsV1 {
        self.limits
    }
}

/// Failure of a closed optimization execution.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PlironOptimizationErrorV1 {
    Operation(OperationHandleError),
    RootHandleRequired,
    GraphWorkLimitExceeded {
        required: usize,
        limit: usize,
    },
    WorkLimitExceeded {
        required: usize,
        limit: usize,
    },
    SessionGraphCapacityExceeded,
    GraphAccountingMismatch,
    GraphInspectionRejected {
        after: Option<PlironOptimizationPassV1>,
    },
    VerificationRejected {
        after: Option<PlironOptimizationPassV1>,
    },
    PassRejected(PlironOptimizationPassV1),
    UpstreamPanicked {
        during: Option<PlironOptimizationPassV1>,
    },
}

impl fmt::Display for PlironOptimizationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Operation(error) => write!(formatter, "operation authentication failed: {error}"),
            Self::RootHandleRequired => formatter.write_str("optimization requires a root handle"),
            Self::GraphWorkLimitExceeded { required, limit } => write!(
                formatter,
                "operation graph requires {required} work units but the graph limit is {limit}"
            ),
            Self::WorkLimitExceeded { required, limit } => write!(
                formatter,
                "optimization requires {required} work units but the limit is {limit}"
            ),
            Self::SessionGraphCapacityExceeded => {
                formatter.write_str("optimization may exceed the session graph hard cap")
            }
            Self::GraphAccountingMismatch => {
                formatter.write_str("registered operation graph accounting is inconsistent")
            }
            Self::GraphInspectionRejected { after } => {
                write!(
                    formatter,
                    "operation graph inspection failed after {after:?}"
                )
            }
            Self::VerificationRejected { after } => {
                write!(formatter, "recursive verification failed after {after:?}")
            }
            Self::PassRejected(pass) => write!(formatter, "{} was rejected", pass.name()),
            Self::UpstreamPanicked { during } => {
                write!(formatter, "upstream Pliron panicked during {during:?}")
            }
        }
    }
}

impl Error for PlironOptimizationErrorV1 {}

impl From<OperationHandleError> for PlironOptimizationErrorV1 {
    fn from(error: OperationHandleError) -> Self {
        Self::Operation(error)
    }
}

/// Immutable accounting for one completed pass.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlironOptimizationPassReportV1 {
    pass: PlironOptimizationPassV1,
    changed: bool,
    input_graph_work: usize,
    output_graph_work: usize,
    work_units: usize,
}

impl PlironOptimizationPassReportV1 {
    pub const fn pass(self) -> PlironOptimizationPassV1 {
        self.pass
    }

    pub const fn changed(self) -> bool {
        self.changed
    }

    pub const fn input_graph_work(self) -> usize {
        self.input_graph_work
    }

    pub const fn output_graph_work(self) -> usize {
        self.output_graph_work
    }

    pub const fn work_units(self) -> usize {
        self.work_units
    }
}

/// Immutable report published only after every pass and reconciliation succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironOptimizationReportV1 {
    initial_graph_work: usize,
    final_graph_work: usize,
    invalidated_handle_count: usize,
    work_units: usize,
    passes: Vec<PlironOptimizationPassReportV1>,
}

impl PlironOptimizationReportV1 {
    pub const fn initial_graph_work(&self) -> usize {
        self.initial_graph_work
    }

    pub const fn final_graph_work(&self) -> usize {
        self.final_graph_work
    }

    pub const fn invalidated_handle_count(&self) -> usize {
        self.invalidated_handle_count
    }

    pub const fn work_units(&self) -> usize {
        self.work_units
    }

    pub fn passes(&self) -> &[PlironOptimizationPassReportV1] {
        &self.passes
    }
}

impl PlironSession {
    /// Executes an immutable closed optimization plan on an authenticated root.
    ///
    /// The root is recursively verified before the first pass and after every
    /// pass. Any upstream failure after execution begins poisons the session,
    /// because Pliron passes do not provide transactional rollback.
    pub fn execute_optimization_v1(
        &mut self,
        root: &OperationHandle,
        plan: &PlironOptimizationPlanV1,
    ) -> Result<PlironOptimizationReportV1, PlironOptimizationErrorV1> {
        let pointer = self.with_operation(root, |pointer, _| pointer)?;
        let Some(owner_root) = self.operation_roots.get(&root.identity).copied() else {
            self.poisoned = true;
            return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
        };
        if owner_root != root.identity {
            return Err(PlironOptimizationErrorV1::RootHandleRequired);
        }
        let Some(charged_root_work) = self.owned_tree_work.get(&root.identity).copied() else {
            self.poisoned = true;
            return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
        };

        let registered_handle_count = self
            .operation_roots
            .values()
            .filter(|registered_root| **registered_root == root.identity)
            .count();

        // Existing root accounting lets all configured limits fail before the
        // recursive inspector, verifier, analysis cache, or pass manager allocate.
        enforce_graph_limit(charged_root_work, plan.limits.max_graph_work)?;
        self.operation_tree_work
            .checked_sub(charged_root_work)
            .and_then(|work| work.checked_add(plan.limits.max_graph_work))
            .filter(|work| *work <= HARD_MAX_SESSION_OPERATION_TREE_ITEMS)
            .ok_or(PlironOptimizationErrorV1::SessionGraphCapacityExceeded)?;
        let preflight_work = optimization_work_preflight(
            charged_root_work,
            plan.passes.len(),
            plan.limits.max_graph_work,
            registered_handle_count,
        )?;
        if preflight_work > plan.limits.max_work_units {
            return Err(PlironOptimizationErrorV1::WorkLimitExceeded {
                required: preflight_work,
                limit: plan.limits.max_work_units,
            });
        }

        let (initial_graph_work, mut final_operations) =
            self.inspect_optimization_graph(pointer, None)?;
        if initial_graph_work != charged_root_work {
            self.poisoned = true;
            return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
        }
        self.verify_optimization_graph(pointer, None)?;

        let mut analyses = AnalysisManager::default();
        let mut current_graph_work = initial_graph_work;
        let mut work_units = initial_graph_work
            .checked_mul(2)
            .ok_or(PlironOptimizationErrorV1::GraphAccountingMismatch)?;
        let mut reports = Vec::with_capacity(plan.passes.len());

        for pass in plan.passes.iter().copied() {
            let input_graph_work = current_graph_work;
            let changed = match catch_unwind(AssertUnwindSafe(|| {
                run_trusted_pass(pass, pointer, &mut self.context, &mut analyses)
            })) {
                Ok(Ok(changed)) => changed,
                Ok(Err(TrustedPassFailure)) => {
                    self.poisoned = true;
                    return Err(PlironOptimizationErrorV1::PassRejected(pass));
                }
                Err(_) => {
                    self.poisoned = true;
                    return Err(PlironOptimizationErrorV1::UpstreamPanicked { during: Some(pass) });
                }
            };

            let (output_graph_work, operations) =
                self.inspect_optimization_graph(pointer, Some(pass))?;
            enforce_graph_limit_after_mutation(
                self,
                output_graph_work,
                plan.limits.max_graph_work,
            )?;
            self.verify_optimization_graph(pointer, Some(pass))?;

            let pass_work = input_graph_work
                .checked_add(output_graph_work.checked_mul(2).ok_or_else(|| {
                    self.poisoned = true;
                    PlironOptimizationErrorV1::GraphAccountingMismatch
                })?)
                .ok_or_else(|| {
                    self.poisoned = true;
                    PlironOptimizationErrorV1::GraphAccountingMismatch
                })?;
            work_units = work_units.checked_add(pass_work).ok_or_else(|| {
                self.poisoned = true;
                PlironOptimizationErrorV1::GraphAccountingMismatch
            })?;
            reports.push(PlironOptimizationPassReportV1 {
                pass,
                changed,
                input_graph_work,
                output_graph_work,
                work_units: pass_work,
            });
            current_graph_work = output_graph_work;
            final_operations = operations;
        }

        let reconciliation_work = current_graph_work
            .checked_add(registered_handle_count)
            .ok_or_else(|| {
                self.poisoned = true;
                PlironOptimizationErrorV1::GraphAccountingMismatch
            })?;
        work_units = work_units.checked_add(reconciliation_work).ok_or_else(|| {
            self.poisoned = true;
            PlironOptimizationErrorV1::GraphAccountingMismatch
        })?;
        if work_units > plan.limits.max_work_units {
            self.poisoned = true;
            return Err(PlironOptimizationErrorV1::WorkLimitExceeded {
                required: work_units,
                limit: plan.limits.max_work_units,
            });
        }

        let invalidated_handle_count = self.reconcile_optimized_root(
            root,
            pointer,
            charged_root_work,
            current_graph_work,
            &final_operations,
        )?;

        Ok(PlironOptimizationReportV1 {
            initial_graph_work,
            final_graph_work: current_graph_work,
            invalidated_handle_count,
            work_units,
            passes: reports,
        })
    }

    fn inspect_optimization_graph(
        &mut self,
        pointer: Ptr<Operation>,
        after: Option<PlironOptimizationPassV1>,
    ) -> Result<(usize, Vec<Ptr<Operation>>), PlironOptimizationErrorV1> {
        match catch_unwind(AssertUnwindSafe(|| {
            inspect_operation_tree_details(pointer, &mut self.context)
        })) {
            Ok(Ok(inspection)) => Ok(inspection),
            Ok(Err(_)) => {
                self.poisoned = true;
                Err(PlironOptimizationErrorV1::GraphInspectionRejected { after })
            }
            Err(_) => {
                self.poisoned = true;
                Err(PlironOptimizationErrorV1::UpstreamPanicked { during: after })
            }
        }
    }

    fn verify_optimization_graph(
        &mut self,
        pointer: Ptr<Operation>,
        after: Option<PlironOptimizationPassV1>,
    ) -> Result<(), PlironOptimizationErrorV1> {
        match catch_unwind(AssertUnwindSafe(|| {
            verify_operation(pointer, &self.context)
        })) {
            Ok(Ok(())) => Ok(()),
            Ok(Err(_)) => {
                self.poisoned = true;
                Err(PlironOptimizationErrorV1::VerificationRejected { after })
            }
            Err(_) => {
                self.poisoned = true;
                Err(PlironOptimizationErrorV1::UpstreamPanicked { during: after })
            }
        }
    }

    fn reconcile_optimized_root(
        &mut self,
        root: &OperationHandle,
        pointer: Ptr<Operation>,
        old_graph_work: usize,
        new_graph_work: usize,
        final_operations: &[Ptr<Operation>],
    ) -> Result<usize, PlironOptimizationErrorV1> {
        let live = final_operations.iter().copied().collect::<HashSet<_>>();
        if !live.contains(&pointer) {
            self.poisoned = true;
            return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
        }
        let stale = self
            .operation_roots
            .iter()
            .filter_map(|(identity, registered_root)| {
                if *registered_root != root.identity {
                    return None;
                }
                let registered = self.operations.get(identity)?;
                (!live.contains(registered)).then_some(*identity)
            })
            .collect::<Vec<_>>();
        if stale.contains(&root.identity) {
            self.poisoned = true;
            return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
        }

        let new_session_work = self
            .operation_tree_work
            .checked_sub(old_graph_work)
            .and_then(|work| work.checked_add(new_graph_work))
            .filter(|work| *work <= HARD_MAX_SESSION_OPERATION_TREE_ITEMS)
            .ok_or_else(|| {
                self.poisoned = true;
                PlironOptimizationErrorV1::SessionGraphCapacityExceeded
            })?;

        for identity in &stale {
            self.operations.remove(identity);
            self.operation_roots.remove(identity);
        }
        self.owned_tree_work.insert(root.identity, new_graph_work);
        self.operation_tree_work = new_session_work;
        Ok(stale.len())
    }
}

fn optimization_work_preflight(
    initial_graph_work: usize,
    pass_count: usize,
    max_graph_work: usize,
    registered_handle_count: usize,
) -> Result<usize, PlironOptimizationErrorV1> {
    let initial = initial_graph_work
        .checked_mul(2)
        .ok_or(PlironOptimizationErrorV1::GraphAccountingMismatch)?;
    let passes = max_graph_work
        .checked_mul(3)
        .and_then(|work| work.checked_mul(pass_count))
        .ok_or(PlironOptimizationErrorV1::GraphAccountingMismatch)?;
    initial
        .checked_add(passes)
        .and_then(|work| work.checked_add(max_graph_work))
        .and_then(|work| work.checked_add(registered_handle_count))
        .ok_or(PlironOptimizationErrorV1::GraphAccountingMismatch)
}

fn enforce_graph_limit(graph_work: usize, limit: usize) -> Result<(), PlironOptimizationErrorV1> {
    if graph_work > limit {
        return Err(PlironOptimizationErrorV1::GraphWorkLimitExceeded {
            required: graph_work,
            limit,
        });
    }
    Ok(())
}

fn enforce_graph_limit_after_mutation(
    session: &mut PlironSession,
    graph_work: usize,
    limit: usize,
) -> Result<(), PlironOptimizationErrorV1> {
    if graph_work > limit {
        session.poisoned = true;
        return Err(PlironOptimizationErrorV1::GraphWorkLimitExceeded {
            required: graph_work,
            limit,
        });
    }
    Ok(())
}

fn run_trusted_pass(
    pass: PlironOptimizationPassV1,
    pointer: Ptr<Operation>,
    context: &mut pliron::context::Context,
    analyses: &mut AnalysisManager,
) -> Result<bool, TrustedPassFailure> {
    let mut passes = Passes::default();
    match pass {
        PlironOptimizationPassV1::DeadCodeElimination => passes.add_pass(DCEPass),
        PlironOptimizationPassV1::SparseConditionalConstantPropagation => passes.add_pass(SCCPPass),
        PlironOptimizationPassV1::SelectSameValueCanonicalization => passes.add_pass(
            PassWrapper::new("gpu-select-same-value-v1", SelectSameValuePattern),
        ),
        PlironOptimizationPassV1::LocalPureCommonSubexpressionElimination => {
            passes.add_pass(LocalPureCsePassV1)
        }
        PlironOptimizationPassV1::SimplifyControlFlow => passes.add_pass(SimplifyCFGPass),
    }
    passes
        .run(pointer, context, analyses)
        .map(|report| report.ir_changed == IRStatus::Changed)
        .map_err(|_| TrustedPassFailure)
}

struct TrustedPassFailure;
