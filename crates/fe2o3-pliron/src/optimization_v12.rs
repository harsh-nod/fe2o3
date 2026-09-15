//! Closed V12 execution on the bridge-owned candidate session.
//!
//! The resource profile is a conservative, versioned logical envelope around
//! opaque upstream passes. It is not allocator/RSS telemetry or a count of
//! upstream primitive steps. Canonical output admission is independently metered.

use std::{error::Error, fmt, mem::size_of};

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
};

use crate::{
    KirBridgeErrorV12, KirPlironGraphV12, PlironOptimizationErrorV1, PlironOptimizationLimitsV1,
    PlironOptimizationPassReportV1, PlironOptimizationPassV1, PlironOptimizationPlanV1,
    PlironOptimizationReportV1,
};

/// Literal policy-2 roster. There is no V12 configurable execution endpoint.
pub const KIR_PLIRON_PRODUCTION_PASSES_V12: [PlironOptimizationPassV1; 7] = [
    PlironOptimizationPassV1::SparseConditionalConstantPropagation,
    PlironOptimizationPassV1::SimplifyControlFlow,
    PlironOptimizationPassV1::SelectSameValueCanonicalization,
    PlironOptimizationPassV1::DeadCodeElimination,
    PlironOptimizationPassV1::LocalPureCommonSubexpressionElimination,
    PlironOptimizationPassV1::DeadCodeElimination,
    PlironOptimizationPassV1::SimplifyControlFlow,
];

/// Prepaid logical execution and payload profile for one complete pass roster.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlironOptimizationResourcesV12 {
    work: usize,
    persistent: usize,
    temporary: usize,
    report: usize,
}

impl PlironOptimizationResourcesV12 {
    pub const fn work(self) -> usize {
        self.work
    }
    /// Arena/interner growth remains charged until the graph/session is dropped.
    pub const fn persistent_storage(self) -> usize {
        self.persistent
    }
    pub const fn temporary_storage(self) -> usize {
        self.temporary
    }
    /// Transferred report payload, to be reserved while the returned report lives.
    pub const fn retained_storage(self) -> usize {
        self.report
    }
}

#[derive(Debug)]
pub enum PlironOptimizationErrorV12 {
    Bridge(KirBridgeErrorV12),
    Resources(CanonicalKernelIrVerificationResourceErrorV1),
    Execution(PlironOptimizationErrorV1),
    Mapping(crate::KirOptimizationMapErrorV12),
    AlreadyExecuted,
    Accounting,
}

impl fmt::Display for PlironOptimizationErrorV12 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bridge(e) => e.fmt(f),
            Self::Resources(e) => e.fmt(f),
            Self::Execution(e) => e.fmt(f),
            Self::Mapping(e) => e.fmt(f),
            Self::AlreadyExecuted => f.write_str("V12 production optimization is one-shot"),
            Self::Accounting => f.write_str("V12 optimization custody accounting mismatch"),
        }
    }
}
impl Error for PlironOptimizationErrorV12 {}

impl From<CanonicalKernelIrVerificationResourceErrorV1> for PlironOptimizationErrorV12 {
    fn from(error: CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resources(error)
    }
}

/// This envelope admits the closed 32,768-item candidate cap, all seven passes,
/// retained arena growth, pass-manager scratch and both report rosters before
/// upstream traversal or mutation. Units intentionally over-approximate logical
/// payload; they do not claim byte-exact coverage of private upstream allocators.
fn execution_resources_v12(
    canonical_bytes: usize,
) -> Result<PlironOptimizationResourcesV12, PlironOptimizationErrorV12> {
    let arithmetic = || {
        PlironOptimizationErrorV12::Resources(
            CanonicalKernelIrVerificationResourceErrorV1::Arithmetic,
        )
    };
    let volume = canonical_bytes
        .checked_add(32_768)
        .and_then(|n| n.checked_add(1))
        .ok_or_else(arithmetic)?;
    let capture = crate::kir_optimization_map_v12::CaptureLimitsV12::for_bytes(canonical_bytes)
        .map_err(PlironOptimizationErrorV12::Mapping)?;
    let capture_work = capture
        .work()
        .map_err(PlironOptimizationErrorV12::Mapping)?;
    let capture_storage = capture
        .storage()
        .map_err(PlironOptimizationErrorV12::Mapping)?;
    let work = volume
        .checked_mul(64)
        .and_then(|n| n.checked_add(25_268_224))
        .and_then(|n| n.checked_add(capture_work))
        .ok_or_else(arithmetic)?;
    let persistent = volume
        .checked_mul(8)
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(arithmetic)?;
    let persistent = persistent
        .checked_add(capture_storage)
        .ok_or_else(arithmetic)?;
    let temporary = volume
        .checked_mul(16)
        .and_then(|n| n.checked_add(4096))
        .ok_or_else(arithmetic)?;
    let report = size_of::<PlironOptimizationReportV1>()
        .checked_add(7 * size_of::<PlironOptimizationPassReportV1>())
        .ok_or_else(arithmetic)?;
    Ok(PlironOptimizationResourcesV12 {
        work,
        persistent,
        temporary,
        report,
    })
}

// Resource admission only; every variant executes the same literal pass roster.
// The native variant is reachable only from the closed neutral lease entry.
enum ExecutionAdmissionV1<'a> {
    Target {
        occurrences: Option<&'a crate::kir_occurrence_capture_v1::Capture>,
    },
    Native {
        occurrences: &'a crate::kir_occurrence_capture_v1::Capture,
        limits: crate::kir_occurrence_capture_v1::Limits,
    },
}

fn native_execution_resources_v1(
    canonical_bytes: usize,
    registered_node_bound: usize,
) -> Result<
    (
        PlironOptimizationResourcesV12,
        crate::kir_optimization_map_v12::CaptureLimitsV12,
    ),
    PlironOptimizationErrorV12,
> {
    use crate::kir_optimization_map_v12::CaptureLimitsV12;
    // Leave the target formula and its public compatibility surface unchanged.
    let mut profile = execution_resources_v12(canonical_bytes)?;
    let ceiling = CaptureLimitsV12::for_bytes(canonical_bytes)
        .map_err(PlironOptimizationErrorV12::Mapping)?;
    let capture = ceiling
        .for_native_node_bound(registered_node_bound)
        .map_err(PlironOptimizationErrorV12::Mapping)?;
    let old_work = ceiling
        .work()
        .map_err(PlironOptimizationErrorV12::Mapping)?;
    let new_work = capture
        .work()
        .map_err(PlironOptimizationErrorV12::Mapping)?;
    let old_storage = ceiling
        .storage()
        .map_err(PlironOptimizationErrorV12::Mapping)?;
    let new_storage = capture
        .storage()
        .map_err(PlironOptimizationErrorV12::Mapping)?;
    profile.work = profile
        .work
        .checked_sub(old_work)
        .and_then(|n| n.checked_add(new_work))
        .ok_or(PlironOptimizationErrorV12::Accounting)?;
    profile.persistent = profile
        .persistent
        .checked_sub(old_storage)
        .and_then(|n| n.checked_add(new_storage))
        .ok_or(PlironOptimizationErrorV12::Accounting)?;
    Ok((profile, capture))
}

impl KirPlironGraphV12<'_> {
    /// Runs the fixed production policy once on this exact V12 candidate.
    ///
    /// The caller must have reserved the import receipt. On accepted execution,
    /// persistent growth is added to this graph's retained payload and remains
    /// live in the ledger on both success and failure. Drop the graph/session
    /// before releasing that reservation. On success the report payload is
    /// transferred separately; reserve it before the next allocation.
    ///
    /// A failed pass never publishes a candidate or rewinds a mutation. The
    /// private session remains poisoned or execution-exhausted and is discarded.
    pub fn execute_production_optimization_v12(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
    ) -> Result<
        (PlironOptimizationReportV1, PlironOptimizationResourcesV12),
        PlironOptimizationErrorV12,
    > {
        self.execute_production_optimization_with_occurrences_v1(budget, None)
    }

    // The target endpoint passes None: no neutral capture, census, allocation,
    // or occurrence event stream is constructed on the frozen target path.
    pub(crate) fn execute_production_optimization_with_occurrences_v1(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        occurrences: Option<&crate::kir_occurrence_capture_v1::Capture>,
    ) -> Result<
        (PlironOptimizationReportV1, PlironOptimizationResourcesV12),
        PlironOptimizationErrorV12,
    > {
        self.execute_admitted_production_optimization_v1(
            budget,
            ExecutionAdmissionV1::Target { occurrences },
        )
    }

    pub(crate) fn execute_native_neutral_policy_v1(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        occurrences: &crate::kir_occurrence_capture_v1::Capture,
        limits: crate::kir_occurrence_capture_v1::Limits,
    ) -> Result<
        (PlironOptimizationReportV1, PlironOptimizationResourcesV12),
        PlironOptimizationErrorV12,
    > {
        self.execute_admitted_production_optimization_v1(
            budget,
            ExecutionAdmissionV1::Native {
                occurrences,
                limits,
            },
        )
    }

    fn execute_admitted_production_optimization_v1(
        &mut self,
        budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
        admission: ExecutionAdmissionV1<'_>,
    ) -> Result<
        (PlironOptimizationReportV1, PlironOptimizationResourcesV12),
        PlironOptimizationErrorV12,
    > {
        self.validate_custody_v12()
            .map_err(PlironOptimizationErrorV12::Bridge)?;
        if self.optimization_started {
            return Err(PlironOptimizationErrorV12::AlreadyExecuted);
        }
        let canonical_bytes = usize::try_from(self.input().canonical_bytes())
            .map_err(|_| PlironOptimizationErrorV12::Accounting)?;
        let (profile, native_capture, occurrences) = match admission {
            ExecutionAdmissionV1::Target { occurrences } => {
                (execution_resources_v12(canonical_bytes)?, None, occurrences)
            }
            ExecutionAdmissionV1::Native {
                occurrences,
                limits,
            } => {
                let (profile, capture) =
                    native_execution_resources_v1(canonical_bytes, limits.nodes)?;
                (profile, Some(capture), Some(occurrences))
            }
        };
        let retained = self
            .retained_storage
            .checked_add(profile.persistent)
            .ok_or(PlironOptimizationErrorV12::Accounting)?;
        let reservation = profile
            .persistent
            .checked_add(profile.temporary)
            .and_then(|n| n.checked_add(profile.report))
            .ok_or(PlironOptimizationErrorV12::Accounting)?;
        let floor = budget.storage();
        budget.charge_work(profile.work)?;
        budget.reserve_storage(reservation)?;
        let result = (|| {
            let mut passes = Vec::new();
            passes
                .try_reserve_exact(7)
                .map_err(|_| CanonicalKernelIrVerificationResourceErrorV1::Allocation)?;
            passes.extend(KIR_PLIRON_PRODUCTION_PASSES_V12);
            let limits = PlironOptimizationLimitsV1::new(256, 32_768, 25_268_224)
                .map_err(|_| PlironOptimizationErrorV12::Accounting)?;
            let plan = PlironOptimizationPlanV1::new(passes, limits)
                .map_err(|_| PlironOptimizationErrorV12::Accounting)?;
            let capture = match native_capture {
                Some(limits) => self.begin_optimization_capture_with_limits_v1(limits),
                None => self.begin_optimization_capture_v12(),
            }
            .map_err(PlironOptimizationErrorV12::Mapping)?;
            self.retained_storage = retained;
            self.optimization_started = true;
            let result = match occurrences {
                None => self
                    .session
                    .execute_optimization_with_capture_v12(&self.root, &plan, &capture),
                Some(occurrences) => self.session.execute_optimization_with_occurrences_v1(
                    &self.root,
                    &plan,
                    &capture,
                    occurrences,
                ),
            };
            if let Some(error) = capture.failure() {
                return Err(PlironOptimizationErrorV12::Mapping(error));
            }
            result.map_err(PlironOptimizationErrorV12::Execution)
        })();
        let keep = if self.optimization_started {
            profile.persistent
        } else {
            0
        };
        let release = budget
            .storage()
            .checked_sub(floor)
            .and_then(|n| n.checked_sub(keep))
            .ok_or(PlironOptimizationErrorV12::Accounting)?;
        // Pass-local objects have dropped; only session growth and, on success,
        // the explicitly transferred immutable report remain.
        budget.release_storage(release)?;
        result.map(|report| (report, profile))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    include!("optimization_v12_native_profile_tests.rs");

    #[test]
    fn envelope_literals_and_arithmetic_are_independent() {
        let profile = execution_resources_v12(31).unwrap();
        assert_eq!(profile.work(), 28_391_552);
        assert_eq!(profile.persistent_storage(), 464_128);
        assert_eq!(profile.temporary_storage(), 528_896);
        assert_eq!(
            profile.retained_storage(),
            size_of::<PlironOptimizationReportV1>()
                + 7 * size_of::<PlironOptimizationPassReportV1>()
        );
        assert!(matches!(
            execution_resources_v12(usize::MAX),
            Err(PlironOptimizationErrorV12::Resources(
                CanonicalKernelIrVerificationResourceErrorV1::Arithmetic
            ))
        ));
    }

    fn input() -> fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12 {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(294);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
        let (input, _) = fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12::
            from_module_ref_with_verification_budget_v12(
                &fe2o3_kernel_ir::Module::new("m"), &mut budget).unwrap();
        assert_eq!(input.canonical().canonical_bytes().len(), 37);
        input
    }

    fn imported(
        input: &fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12,
    ) -> KirPlironGraphV12<'_> {
        let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(1_000_000);
        let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 1_000_000);
        let (graph, receipt) = KirPlironGraphV12::import(input, &mut budget).unwrap();
        assert_eq!(budget.storage(), 0);
        assert_eq!(receipt.retained_storage(), graph.retained_storage());
        graph
    }

    #[test]
    fn actual_execution_work_exact_and_one_under_preserve_custody() {
        // wire37: base 27367808, plus capture 138*1104*8+64*138=1227648.
        const WORK: usize = 28_595_456;
        let input = input();
        for allowance in [WORK - 1, WORK] {
            let mut graph = imported(&input);
            let imported_storage = graph.retained_storage();
            let floor = 7 + imported_storage;
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(11 + allowance);
            work.charge_work(11).unwrap();
            let mut budget =
                CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, 2_000_000);
            budget.reserve_storage(floor).unwrap();
            let result = graph.execute_production_optimization_v12(&mut budget);
            if allowance == WORK {
                let (report, receipt) = result.unwrap();
                assert_eq!(report.passes().len(), 7);
                assert_eq!(receipt.work(), WORK);
                assert_eq!(budget.work(), 11 + WORK);
                assert_eq!(budget.storage(), floor + 482_608);
                assert_eq!(graph.retained_storage(), imported_storage + 482_608);
                assert!(matches!(
                    graph.execute_production_optimization_v12(&mut budget),
                    Err(PlironOptimizationErrorV12::AlreadyExecuted)
                ));
                drop(report);
            } else {
                assert!(matches!(result, Err(PlironOptimizationErrorV12::Resources(
                    CanonicalKernelIrVerificationResourceErrorV1::Work(error)))
                    if error.actual() == 11 + WORK && error.limit() == 11 + WORK - 1));
                assert_eq!(budget.work(), 11);
                assert_eq!(budget.storage(), floor);
                assert_eq!(graph.retained_storage(), imported_storage);
                assert!(!graph.optimization_started);
            }
            let retained = graph.retained_storage();
            drop(graph);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), 7);
        }
    }

    #[test]
    fn actual_execution_storage_exact_and_one_under_fail_before_mutation() {
        let input = input();
        let report_storage = size_of::<PlironOptimizationReportV1>()
            + 7 * size_of::<PlironOptimizationPassReportV1>();
        for one_under in [true, false] {
            let mut graph = imported(&input);
            let imported_storage = graph.retained_storage();
            let floor = 7 + imported_storage;
            // Persistent: base 266544 plus capture 216064; scratch 528992, plus report.
            let required = floor + 482_608 + 528_992 + report_storage;
            let limit = required - usize::from(one_under);
            let mut work = fe2o3_kernel_ir::CanonicalKernelIrWorkBudgetV1::new(28_595_456);
            let mut budget = CanonicalKernelIrVerificationResourceBudgetV1::new(&mut work, limit);
            budget.reserve_storage(floor).unwrap();
            let result = graph.execute_production_optimization_v12(&mut budget);
            assert_eq!(budget.work(), 28_595_456);
            if one_under {
                assert!(matches!(result, Err(PlironOptimizationErrorV12::Resources(
                    CanonicalKernelIrVerificationResourceErrorV1::Storage(error)))
                    if error.actual() == required && error.limit() == required - 1));
                assert_eq!(budget.storage(), floor);
                assert!(!graph.optimization_started);
                assert_eq!(graph.retained_storage(), imported_storage);
            } else {
                let (report, receipt) = result.unwrap();
                assert_eq!(receipt.retained_storage(), report_storage);
                assert_eq!(budget.peak_storage(), required);
                assert_eq!(budget.storage(), floor + 482_608);
                drop(report);
            }
            let retained = graph.retained_storage();
            drop(graph);
            budget.release_storage(retained).unwrap();
            assert_eq!(budget.storage(), 7);
        }
    }
}
