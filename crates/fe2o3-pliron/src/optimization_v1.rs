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
    HARD_MAX_SESSION_OPERATION_TREE_ITEMS, OperationGraphEpochV1, OperationGraphReplayIdentityV1,
    OperationHandle, OperationHandleError, PlironSession, inspect_operation_tree_details,
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
    /// Only the trusted, same-ledger fixed policy-3 entrypoint executes this pass.
    DominancePureCommonSubexpressionElimination,
    /// Only the fixed Policy6 continuation can invoke this same-ledger pass.
    IntegerNeutralCanonicalization,
    /// Only the fixed V18 Policy9 continuation executes the def-use worklist.
    IntegerNeutralWorklistCanonicalization,
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
            Self::DominancePureCommonSubexpressionElimination => {
                "dominance-pure-common-subexpression-elimination"
            }
            Self::IntegerNeutralCanonicalization => "integer-neutral-canonicalization",
            Self::IntegerNeutralWorklistCanonicalization => {
                "integer-neutral-worklist-canonicalization"
            }
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
    FixedpointRoundLimitExceeded {
        completed: usize,
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
            Self::FixedpointRoundLimitExceeded { completed, limit } => write!(
                formatter,
                "fixed policy did not reach an unchanged round after {completed} of {limit} rounds"
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
    input_epoch: OperationGraphEpochV1,
    output_epoch: OperationGraphEpochV1,
    invalidated_analysis_count: usize,
    preserved_analysis_count: usize,
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

    pub const fn input_epoch(self) -> OperationGraphEpochV1 {
        self.input_epoch
    }

    pub const fn output_epoch(self) -> OperationGraphEpochV1 {
        self.output_epoch
    }

    pub const fn invalidated_analysis_count(self) -> usize {
        self.invalidated_analysis_count
    }

    pub const fn preserved_analysis_count(self) -> usize {
        self.preserved_analysis_count
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
    final_graph_identity: OperationGraphReplayIdentityV1,
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

    pub(crate) fn pass_capacity(&self) -> usize {
        self.passes.capacity()
    }

    pub const fn final_graph_identity(&self) -> OperationGraphReplayIdentityV1 {
        self.final_graph_identity
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
        self.execute_optimization_impl_v1(root, plan, None, None, None, None)
    }

    pub(crate) fn execute_optimization_with_capture_v12(
        &mut self,
        root: &OperationHandle,
        plan: &PlironOptimizationPlanV1,
        capture: &crate::kir_optimization_map_v12::CaptureV12,
    ) -> Result<PlironOptimizationReportV1, PlironOptimizationErrorV1> {
        self.execute_optimization_impl_v1(root, plan, Some(capture), None, None, None)
    }

    pub(crate) fn execute_optimization_with_occurrences_v1(
        &mut self,
        root: &OperationHandle,
        plan: &PlironOptimizationPlanV1,
        capture: &crate::kir_optimization_map_v12::CaptureV12,
        occurrences: &crate::kir_occurrence_capture_v1::Capture,
    ) -> Result<PlironOptimizationReportV1, PlironOptimizationErrorV1> {
        self.execute_optimization_impl_v1(root, plan, Some(capture), Some(occurrences), None, None)
    }

    pub(crate) fn execute_fixed_policy3_v1(
        &mut self,
        root: &OperationHandle,
        plan: &PlironOptimizationPlanV1,
        capture: &crate::kir_optimization_map_v12::CaptureV12,
        occurrences: &crate::kir_occurrence_capture_v1::Capture,
        ledger: &mut crate::fixed_policy_v3::CseLedger<'_, '_>,
    ) -> Result<PlironOptimizationReportV1, PlironOptimizationErrorV1> {
        if plan.passes.as_slice() != crate::fixed_policy_v3::POLICY3_PASSES {
            return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
        }
        self.execute_optimization_impl_v1(
            root,
            plan,
            Some(capture),
            Some(occurrences),
            Some(ledger),
            None,
        )
    }

    fn execute_optimization_impl_v1(
        &mut self,
        root: &OperationHandle,
        plan: &PlironOptimizationPlanV1,
        capture: Option<&crate::kir_optimization_map_v12::CaptureV12>,
        occurrences: Option<&crate::kir_occurrence_capture_v1::Capture>,
        mut cse: Option<&mut crate::fixed_policy_v3::CseLedger<'_, '_>>,
        fixedpoint: Option<crate::fixed_policy_v3::FixedpointRoundResourcesV18>,
    ) -> Result<PlironOptimizationReportV1, PlironOptimizationErrorV1> {
        let pointer = self.with_operation(root, |pointer, _| pointer)?;
        if cse.is_none() {
            for pass in [
                PlironOptimizationPassV1::DominancePureCommonSubexpressionElimination,
                PlironOptimizationPassV1::IntegerNeutralCanonicalization,
                PlironOptimizationPassV1::IntegerNeutralWorklistCanonicalization,
            ] {
                if plan.passes.contains(&pass) {
                    return Err(PlironOptimizationErrorV1::PassRejected(pass));
                }
            }
        }
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
        let pass_count = if fixedpoint.is_some() {
            crate::fixed_policy_v3::FixedPolicy::MixedFixedpoint11.max_passes()
        } else {
            plan.passes.len()
        };
        let preflight_work = optimization_work_preflight(
            charged_root_work,
            pass_count,
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
        self.analyze_operation_graph_v1(root)?;

        let mut analyses = AnalysisManager::default();
        let mut current_graph_work = initial_graph_work;
        let mut work_units = initial_graph_work
            .checked_mul(2)
            .ok_or(PlironOptimizationErrorV1::GraphAccountingMismatch)?;
        let mut reports = if fixedpoint.is_some() {
            let ledger = cse.as_deref_mut().ok_or_else(|| {
                self.poisoned = true;
                PlironOptimizationErrorV1::GraphAccountingMismatch
            })?;
            let mut rows = Vec::new();
            rows.try_reserve_exact(plan.passes.len()).map_err(|_| {
                self.poisoned = true;
                ledger.record_core_error(
                    dialect_gpu::dominance_cse_v1::DominanceCseErrorV1::Allocation,
                );
                PlironOptimizationErrorV1::GraphAccountingMismatch
            })?;
            let excess = rows
                .capacity()
                .checked_sub(plan.passes.len())
                .and_then(|n| n.checked_mul(std::mem::size_of::<PlironOptimizationPassReportV1>()))
                .ok_or_else(|| {
                    self.poisoned = true;
                    PlironOptimizationErrorV1::GraphAccountingMismatch
                })?;
            ledger.admit_fixedpoint_round(0, excess).map_err(|_| {
                self.poisoned = true;
                PlironOptimizationErrorV1::GraphAccountingMismatch
            })?;
            rows
        } else {
            Vec::with_capacity(plan.passes.len())
        };

        let mut round_changed = false;
        let mut round_presentation = None;
        for ordinal in 0..pass_count {
            if ordinal != 0 && ordinal % plan.passes.len() == 0 {
                let admission = (|| {
                    let failure = || PlironOptimizationErrorV1::GraphAccountingMismatch;
                    let envelope = fixedpoint.ok_or_else(failure)?;
                    let ledger = cse.as_deref_mut().ok_or_else(failure)?;
                    let required = reports
                        .len()
                        .checked_add(plan.passes.len())
                        .ok_or_else(failure)?;
                    // Account for a replacement allocation and the retained
                    // report copy before growing this actual round's rows.
                    let row_bytes = std::mem::size_of::<PlironOptimizationPassReportV1>();
                    let rows = required
                        .checked_mul(2)
                        .and_then(|n| n.checked_mul(row_bytes))
                        .ok_or_else(failure)?;
                    ledger
                        .admit_fixedpoint_round(envelope.work, rows)
                        .map_err(|_| failure())?;
                    capture
                        .ok_or_else(failure)?
                        .admit_fixedpoint_round(ledger)
                        .map_err(|_| failure())?;
                    occurrences
                        .ok_or_else(failure)?
                        .admit_fixedpoint_round(envelope.occurrence_work)
                        .map_err(|_| failure())?;
                    reports.try_reserve_exact(plan.passes.len()).map_err(|_| {
                        ledger.record_core_error(
                            dialect_gpu::dominance_cse_v1::DominanceCseErrorV1::Allocation,
                        );
                        failure()
                    })?;
                    let excess = reports
                        .capacity()
                        .checked_sub(required)
                        .and_then(|n| n.checked_mul(2))
                        .and_then(|n| n.checked_mul(row_bytes))
                        .ok_or_else(failure)?;
                    ledger
                        .admit_fixedpoint_round(0, excess)
                        .map_err(|_| failure())?;
                    Ok::<(), PlironOptimizationErrorV1>(())
                })();
                if let Err(error) = admission {
                    self.poisoned = true;
                    return Err(error);
                }
                round_changed = false;
            }
            let pass = plan.passes[ordinal % plan.passes.len()];
            let input_graph_work = current_graph_work;
            let input_snapshot = self.operation_graph_snapshot_v1(root)?;
            if let Some(envelope) = fixedpoint.filter(|_| ordinal % plan.passes.len() == 0) {
                let ledger = cse.as_deref_mut().ok_or_else(|| {
                    self.poisoned = true;
                    PlironOptimizationErrorV1::GraphAccountingMismatch
                })?;
                round_presentation = Some((
                    input_snapshot,
                    self.fixedpoint_presentation_v18(pointer, envelope.presentation_limit, ledger)?,
                ));
            }
            if capture.is_some_and(|capture| !capture.begin_pass(pass, input_snapshot.epoch())) {
                self.poisoned = true;
                return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
            }
            if occurrences.is_some_and(|capture| !capture.begin_pass(pass, input_snapshot.epoch()))
            {
                self.poisoned = true;
                return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
            }
            let transaction = self.begin_checked_operation_graph_mutation_v1(root)?;
            let changed = match catch_unwind(AssertUnwindSafe(|| match capture {
                Some(capture) => run_observed_pass_v12(
                    pass,
                    pointer,
                    &mut self.context,
                    &mut analyses,
                    match occurrences {
                        Some(occurrences) => occurrences.observer(capture.observer()),
                        None => capture.observer(),
                    },
                    cse.as_deref_mut(),
                ),
                None => run_trusted_pass(pass, pointer, &mut self.context, &mut analyses),
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
            // This closed roster only erases/aliases, except a checked integer
            // operation may become one operand-free false constant. Tree work
            // omits operands/results, so that replacement can have equal work.
            if fixedpoint.is_some()
                && ((changed && output_graph_work > input_graph_work)
                    || (!changed && output_graph_work != input_graph_work))
            {
                self.poisoned = true;
                return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
            }
            enforce_graph_limit_after_mutation(
                self,
                output_graph_work,
                plan.limits.max_graph_work,
            )?;
            self.verify_optimization_graph(pointer, Some(pass))?;
            let commit = self
                .commit_checked_operation_graph_mutation_v1(transaction, changed)
                .map_err(PlironOptimizationErrorV1::Operation)?;
            let output_snapshot = commit.snapshot();
            if capture
                .is_some_and(|capture| !capture.end_pass(&self.context, output_snapshot.epoch()))
            {
                self.poisoned = true;
                return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
            }

            if occurrences
                .is_some_and(|capture| !capture.end_pass(&self.context, output_snapshot.epoch()))
            {
                self.poisoned = true;
                return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
            }

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
                input_epoch: input_snapshot.epoch(),
                output_epoch: output_snapshot.epoch(),
                invalidated_analysis_count: commit.invalidated_analysis_count(),
                preserved_analysis_count: commit.preserved_analysis_count(),
            });
            current_graph_work = output_graph_work;
            final_operations = operations;
            round_changed |= changed;
            if fixedpoint.is_some() && (ordinal + 1) % plan.passes.len() == 0 {
                let (round_snapshot, before) = round_presentation.take().ok_or_else(|| {
                    self.poisoned = true;
                    PlironOptimizationErrorV1::GraphAccountingMismatch
                })?;
                let ledger = cse.as_deref_mut().ok_or_else(|| {
                    self.poisoned = true;
                    PlironOptimizationErrorV1::GraphAccountingMismatch
                })?;
                if !round_changed {
                    let after = self.fixedpoint_presentation_v18(
                        pointer,
                        fixedpoint.expect("fixed policy round").presentation_limit,
                        ledger,
                    )?;
                    // Local same-context equality supplements the per-pass
                    // mutation checks. These bytes are never durable authority.
                    if round_snapshot != output_snapshot || before != after {
                        self.poisoned = true;
                        return Err(PlironOptimizationErrorV1::GraphAccountingMismatch);
                    }
                    let bytes =
                        before
                            .capacity()
                            .checked_add(after.capacity())
                            .ok_or_else(|| {
                                self.poisoned = true;
                                PlironOptimizationErrorV1::GraphAccountingMismatch
                            })?;
                    drop(after);
                    drop(before);
                    ledger.release_fixedpoint_scratch(bytes).map_err(|_| {
                        self.poisoned = true;
                        PlironOptimizationErrorV1::GraphAccountingMismatch
                    })?;
                    break;
                }
                let bytes = before.capacity();
                drop(before);
                ledger.release_fixedpoint_scratch(bytes).map_err(|_| {
                    self.poisoned = true;
                    PlironOptimizationErrorV1::GraphAccountingMismatch
                })?;
                if ordinal + 1 == pass_count {
                    self.poisoned = true;
                    return Err(PlironOptimizationErrorV1::FixedpointRoundLimitExceeded {
                        completed: pass_count / plan.passes.len(),
                        limit: crate::fixed_policy_v3::POLICY11_MAX_ROUNDS,
                    });
                }
            }
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
        let final_graph_identity = self.analyze_operation_graph_v1(root)?.replay_identity();

        Ok(PlironOptimizationReportV1 {
            initial_graph_work,
            final_graph_work: current_graph_work,
            invalidated_handle_count,
            work_units,
            passes: reports,
            final_graph_identity,
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
        PlironOptimizationPassV1::DominancePureCommonSubexpressionElimination
        | PlironOptimizationPassV1::IntegerNeutralCanonicalization
        | PlironOptimizationPassV1::IntegerNeutralWorklistCanonicalization => {
            return Err(TrustedPassFailure);
        }
    }
    passes
        .run(pointer, context, analyses)
        .map(|report| report.ir_changed == IRStatus::Changed)
        .map_err(|_| TrustedPassFailure)
}

// This wrapper retains the pinned passes' analysis preservation contracts and
// uses the same Passes manager. It does not clone or reimplement the algorithms.
fn run_observed_pass_v12(
    pass: PlironOptimizationPassV1,
    pointer: Ptr<Operation>,
    context: &mut pliron::context::Context,
    analyses: &mut AnalysisManager,
    observer: Box<dyn pliron::irbuild::observer::RewriteObserver>,
    cse: Option<&mut crate::fixed_policy_v3::CseLedger<'_, '_>>,
) -> Result<bool, TrustedPassFailure> {
    if pass == PlironOptimizationPassV1::IntegerNeutralWorklistCanonicalization {
        return run_observed_integer_identity::<true>(
            pointer,
            context,
            analyses,
            observer,
            cse.ok_or(TrustedPassFailure)?,
        );
    }
    if pass == PlironOptimizationPassV1::IntegerNeutralCanonicalization {
        return run_observed_integer_identity_v1(
            pointer,
            context,
            analyses,
            observer,
            cse.ok_or(TrustedPassFailure)?,
        );
    }
    if pass == PlironOptimizationPassV1::DominancePureCommonSubexpressionElimination {
        return run_observed_dominance_cse_v1(
            pointer,
            context,
            analyses,
            observer,
            cse.ok_or(TrustedPassFailure)?,
        );
    }
    struct Observed {
        pass: PlironOptimizationPassV1,
        observer: Option<Box<dyn pliron::irbuild::observer::RewriteObserver>>,
    }
    impl Pass for Observed {
        fn name(&self) -> &str {
            match self.pass {
                PlironOptimizationPassV1::SparseConditionalConstantPropagation => "sccp",
                PlironOptimizationPassV1::DeadCodeElimination => "dce",
                PlironOptimizationPassV1::SimplifyControlFlow => "simplify-cfg",
                PlironOptimizationPassV1::SelectSameValueCanonicalization => {
                    "gpu-select-same-value-v1"
                }
                PlironOptimizationPassV1::LocalPureCommonSubexpressionElimination => {
                    "gpu-local-pure-cse-v1"
                }
                PlironOptimizationPassV1::DominancePureCommonSubexpressionElimination => {
                    "gpu-dominance-pure-cse-v1"
                }
                PlironOptimizationPassV1::IntegerNeutralCanonicalization => {
                    "gpu-integer-neutral-v1"
                }
                PlironOptimizationPassV1::IntegerNeutralWorklistCanonicalization => {
                    "gpu-integer-neutral-worklist-v2"
                }
            }
        }
        fn run(
            &mut self,
            root: Ptr<Operation>,
            context: &mut pliron::context::Context,
            _analyses: &mut AnalysisManager,
        ) -> pliron::result::Result<pliron::pass::PassResult> {
            let observer = self.observer.take().expect("one observed pass execution");
            let mut result = pliron::pass::PassResult::default();
            result.ir_changed = match self.pass {
                PlironOptimizationPassV1::SparseConditionalConstantPropagation => {
                    pliron::opts::constants::sccp::sccp_with_observer(root, context, observer)?
                }
                PlironOptimizationPassV1::DeadCodeElimination => {
                    pliron::opts::dce::dce_with_observer(root, context, observer)?
                }
                PlironOptimizationPassV1::SimplifyControlFlow => {
                    pliron::opts::simplify_cfg::simplify_cfg_with_observer(root, context, observer)?
                }
                PlironOptimizationPassV1::SelectSameValueCanonicalization => {
                    pliron::irbuild::match_rewrite::apply_match_rewrite_with_observer(
                        context,
                        &mut SelectSameValuePattern,
                        pliron::irbuild::match_rewrite::RewriterOrder::default(),
                        root,
                        observer,
                    )?
                }
                PlironOptimizationPassV1::LocalPureCommonSubexpressionElimination => {
                    dialect_gpu::cse_v1::local_pure_cse_with_observer_v12(root, context, observer)
                }
                PlironOptimizationPassV1::DominancePureCommonSubexpressionElimination
                | PlironOptimizationPassV1::IntegerNeutralCanonicalization
                | PlironOptimizationPassV1::IntegerNeutralWorklistCanonicalization => {
                    return Err(pliron::input_error_noloc!("missing policy-3 ledger"));
                }
            };
            if matches!(
                self.pass,
                PlironOptimizationPassV1::SparseConditionalConstantPropagation
                    | PlironOptimizationPassV1::DeadCodeElimination
            ) {
                result.set_preserved::<pliron::graph::dominance::DomInfo>();
            }
            Ok(result)
        }
    }
    let mut passes = Passes::default();
    passes.add_pass(Observed {
        pass,
        observer: Some(observer),
    });
    passes
        .run(pointer, context, analyses)
        .map(|report| report.ir_changed == IRStatus::Changed)
        .map_err(|_| TrustedPassFailure)
}

struct TrustedPassFailure;

fn run_observed_dominance_cse_v1(
    pointer: Ptr<Operation>,
    context: &mut pliron::context::Context,
    analyses: &mut AnalysisManager,
    observer: Box<dyn pliron::irbuild::observer::RewriteObserver>,
    ledger: &mut crate::fixed_policy_v3::CseLedger<'_, '_>,
) -> Result<bool, TrustedPassFailure> {
    use pliron::{
        graph::dominance::DomInfo,
        pass::{PassManager, PassResult},
    };
    struct Observed<'a, 'budget, 'work> {
        ledger: &'a mut crate::fixed_policy_v3::CseLedger<'budget, 'work>,
        observer: Option<Box<dyn pliron::irbuild::observer::RewriteObserver>>,
    }
    impl Pass for Observed<'_, '_, '_> {
        fn name(&self) -> &str {
            "gpu-dominance-pure-cse-v1"
        }
        fn run(
            &mut self,
            root: Ptr<Operation>,
            context: &mut pliron::context::Context,
            analyses: &mut AnalysisManager,
        ) -> pliron::result::Result<PassResult> {
            let mut dominance = analyses.get_analysis_mut::<DomInfo>(root, context)?;
            let observer = self
                .observer
                .take()
                .expect("one fixed-policy CSE invocation");
            let changed = dialect_gpu::dominance_cse_v1::dominance_pure_cse_with_observer_v1(
                root,
                context,
                &mut dominance,
                self.ledger,
                observer,
            )
            .map_err(|error| {
                let error = self.ledger.record_core_error(error);
                pliron::input_error_noloc!(error)
            })?;
            self.ledger
                .finish()
                .map_err(|error| pliron::input_error_noloc!(error))?;
            let mut result = PassResult::default();
            result.ir_changed = changed;
            result.set_preserved::<DomInfo>();
            Ok(result)
        }
    }
    // Run through the existing manager hooks without its 'static boxed list:
    // this pass must borrow the caller's one live canonical ledger.
    let result = <Passes as PassManager>::run_pass(
        &mut Observed {
            ledger,
            observer: Some(observer),
        },
        pointer,
        context,
        analyses,
    )
    .map_err(|_| TrustedPassFailure)?;
    analyses.retain_preserved(&result);
    Ok(result.ir_changed == IRStatus::Changed)
}

#[cfg(test)]
#[path = "optimization_v1/graph_custody_tests_v1.rs"]
mod graph_custody_tests_v1;

include!("optimization_integer_continuation_v1.rs");
include!("optimization_commutative_owner_v1.rs");

#[cfg(test)]
#[path = "optimization_mixed_fixedpoint_v18_tests.rs"]
mod mixed_fixedpoint_v18_tests;
