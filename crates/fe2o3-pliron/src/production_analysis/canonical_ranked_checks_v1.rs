//! Fixed policy checks over one authoritative neutral graph.
//!
//! The native projection is private and never optimized. This entry point can
//! consume a later optimized-neutral owner once its source metadata has been
//! independently transported; it does not require an old ranked recipe.
//! Structural coverage and policy reports do not discharge compiler obligations.
use super::pliron_pipeline::invocation_receipt_v1::{InvocationObservationV1, InvocationReceiptV1};
use super::pliron_pipeline::{
    ProductionPlironPreloweringErrorV2, ProductionPlironPreloweringOutcomeV1,
    ProductionPlironPreloweringReportV2, require_production_pliron_checks_with_observation_v1,
};
use super::pliron_resource_envelope::{
    ProductionAnalysisResourceContractV1 as Contract, ProductionAnalysisResourceLimitV1 as Limit,
    ProductionAnalysisResourceLimitsV1 as Limits, ProductionAnalysisResourcePhaseV1 as Phase,
    ProductionAnalysisResourceUpperBoundV1 as Bound,
};
use crate::kir_bridge_v1::canonical_ranked_v1::NativeCanonicalRankedProjectionV1 as Projection;
use fe2o3_kernel_analysis::{
    CanonicalRankedObligationV1 as Obligation, CanonicalRankedObligationsV1 as Obligations,
    CanonicalRankedViewErrorV1, CheckedCanonicalRankedViewV1,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    CanonicalKernelIrWorkLedgerIdentityV1 as Ledger, VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    cell::Cell,
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Actual accepted analysis-domain history. Storage units are NOT KIR bytes.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct CanonicalRankedPolicyResourceObservationV1 {
    work: usize,
    retained: usize,
    peak: usize,
    first_denial: Option<(Phase, &'static str)>,
    caught_panic: bool,
}
impl CanonicalRankedPolicyResourceObservationV1 {
    pub const fn work_upper_bound(self) -> usize {
        self.work
    }
    pub const fn retained_storage_units(self) -> usize {
        self.retained
    }
    pub const fn peak_storage_units(self) -> usize {
        self.peak
    }
    pub const fn first_denial(self) -> Option<(Phase, &'static str)> {
        self.first_denial
    }
    pub const fn caught_panic(self) -> bool {
        self.caught_panic
    }
}

/// One genuine ordinary invocation and its previously accepted cumulative floor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalRankedPolicyHistoryV1 {
    function: usize,
    floor: CanonicalRankedPolicyResourceObservationV1,
    invocation: CanonicalRankedPolicyResourceObservationV1,
}
impl CanonicalRankedPolicyHistoryV1 {
    pub const fn function(self) -> usize {
        self.function
    }
    pub const fn floor(self) -> CanonicalRankedPolicyResourceObservationV1 {
        self.floor
    }
    pub const fn invocation(self) -> CanonicalRankedPolicyResourceObservationV1 {
        self.invocation
    }
}

/// Typed refusal. No case grants source, target, refinement or launch authority.
#[derive(Debug)]
pub enum CanonicalRankedPolicyFailureV1 {
    Resource(Resource),
    View(CanonicalRankedViewErrorV1),
    Bridge(crate::KirBridgeErrorV12),
    UnsupportedGraph {
        function: usize,
        block: Option<usize>,
        operation: Option<usize>,
    },
    NativeSchema,
    ExactGraph,
    Mutation,
    Analysis {
        function: usize,
        cause: ProductionPlironPreloweringErrorV2,
    },
    AnalysisLimit {
        phase: Phase,
        resource: &'static str,
    },
    InvocationAccounting,
    InvalidQuery {
        function: usize,
    },
    Callback(&'static str),
    Panicked,
}
type Failure = CanonicalRankedPolicyFailureV1;
impl From<Resource> for Failure {
    fn from(value: Resource) -> Self {
        Self::Resource(value)
    }
}
impl From<CanonicalRankedViewErrorV1> for Failure {
    fn from(value: CanonicalRankedViewErrorV1) -> Self {
        Self::View(value)
    }
}
impl From<crate::KirBridgeErrorV12> for Failure {
    fn from(value: crate::KirBridgeErrorV12) -> Self {
        Self::Bridge(value)
    }
}
impl From<Limit> for Failure {
    fn from(value: Limit) -> Self {
        Self::AnalysisLimit {
            phase: value.phase,
            resource: value.resource,
        }
    }
}
impl fmt::Display for Failure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "canonical ranked policy: {self:?}")
    }
}
impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::View(error) => Some(error),
            Self::Bridge(error) => Some(error),
            Self::Analysis { cause, .. } => Some(cause),
            Self::UnsupportedGraph { .. }
            | Self::NativeSchema
            | Self::ExactGraph
            | Self::Mutation
            | Self::AnalysisLimit { .. }
            | Self::InvocationAccounting
            | Self::InvalidQuery { .. }
            | Self::Callback(_)
            | Self::Panicked => None,
        }
    }
}

/// Refusal plus accepted cumulative history, including failed/panicking calls.
/// Diagnostics retain their existing analysis-domain envelope; this is not a
/// transfer of KIR byte credits or permission to execute another analysis.
#[derive(Debug)]
pub struct CanonicalRankedPolicyChecksErrorV1 {
    failure: Failure,
    observation: CanonicalRankedPolicyResourceObservationV1,
    last_invocation: Option<CanonicalRankedPolicyHistoryV1>,
}
impl CanonicalRankedPolicyChecksErrorV1 {
    pub const fn failure(&self) -> &Failure {
        &self.failure
    }
    pub const fn observation(&self) -> CanonicalRankedPolicyResourceObservationV1 {
        self.observation
    }
    pub const fn last_invocation(&self) -> Option<CanonicalRankedPolicyHistoryV1> {
        self.last_invocation
    }
}
impl fmt::Display for CanonicalRankedPolicyChecksErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.failure.fmt(f)
    }
}
impl std::error::Error for CanonicalRankedPolicyChecksErrorV1 {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.failure)
    }
}

struct ReportRow {
    outcome: ProductionPlironPreloweringOutcomeV1,
    history: CanonicalRankedPolicyHistoryV1,
}

/// Short immutable reports over the exact graph supplied to this invocation.
/// No Context, mutable IR, optimizer handle, completed-ranked conversion or
/// authority is exposed. Returned report references borrow this view.
///
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalRankedPoliciesV1;
/// fn clone_it(x: &CheckedCanonicalRankedPoliciesV1<'_, '_>) { let _ = (*x).clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalRankedPoliciesV1;
/// fn edit(x: &CheckedCanonicalRankedPoliciesV1<'_, '_>) { let _ = x.context(); }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::{CheckedCanonicalRankedPoliciesV1, ProductionPlironPreloweringReportV2};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn escape(x: &CheckedCanonicalRankedPoliciesV1<'_, '_>, b: &mut Budget<'_>)
///     -> &'static ProductionPlironPreloweringReportV2 { x.report(0, b).unwrap() }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::CheckedCanonicalRankedPoliciesV1;
/// fn promote(x: &CheckedCanonicalRankedPoliciesV1<'_, '_>) { let _ = x.into_verified_ranked(); }
/// ```
pub struct CheckedCanonicalRankedPoliciesV1<'s, 'g> {
    owner: &'g Owner,
    reports: &'s [ReportRow],
    observation: CanonicalRankedPolicyResourceObservationV1,
    guard: &'s Guard,
}
impl<'g> CheckedCanonicalRankedPoliciesV1<'_, 'g> {
    /// Original immutable subject borrow; no new source or refinement authority.
    pub fn owner(&self, budget: &mut Budget<'_>) -> Result<&'g Owner, Failure> {
        self.guard.query(budget)?;
        Ok(self.owner)
    }
    pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.guard.query(budget)?;
        Ok(self.reports.len())
    }
    /// Exact all-nine-stage report for the canonical function ordinal.
    pub fn report(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<&ProductionPlironPreloweringReportV2, Failure> {
        self.guard.query(budget)?;
        self.reports
            .get(function)
            .map(|row| &row.outcome.report)
            .ok_or_else(|| self.guard.invalid(function))
    }
    pub fn history(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalRankedPolicyHistoryV1, Failure> {
        self.guard.query(budget)?;
        self.reports
            .get(function)
            .map(|row| row.history)
            .ok_or_else(|| self.guard.invalid(function))
    }
    pub fn observation(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalRankedPolicyResourceObservationV1, Failure> {
        self.guard.query(budget)?;
        Ok(self.observation)
    }
    /// All full-compiler obligations remain pending, including source equivalence.
    pub const fn pending_obligations(&self) -> Obligations {
        Obligations::NONE
            .with(Obligation::ExactScalarSemantics)
            .with(Obligation::Control)
            .with(Obligation::Bounds)
            .with(Obligation::Provenance)
            .with(Obligation::Initialization)
            .with(Obligation::RaceFreedom)
            .with(Obligation::Lifetime)
            .with(Obligation::Ordering)
            .with(Obligation::Convergence)
            .with(Obligation::TrapBehavior)
            .with(Obligation::CallEffects)
            .with(Obligation::CallControl)
            .with(Obligation::Launch)
            .with(Obligation::Target)
            .with(Obligation::Tensor)
            .with(Obligation::Assembly)
            .with(Obligation::Contract)
            .with(Obligation::ReferenceRefinement)
            .with(Obligation::SourceMetadata)
    }
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[path = "canonical_ranked_checks_resource_v1.rs"]
mod resources;
use resources::{AnalysisState, Guard, checked_add, protected, reserve_rows};

/// Imports one temporary native view, runs the fixed nine-stage policy on every
/// defined function in canonical order, and checks exact bytes/schema/epoch
/// before and after. Both successful and rejected callbacks are short-lived.
/// Caller-owned escaping values require reservation BEFORE this invocation;
/// temporary callback allocations must be released before return. The callback
/// must restore its exact entry storage and preserve the Budget slot/ledger.
///
/// KIR metering and the existing cumulative analysis-domain envelope remain
/// separate. Neither is a unified whole-compiler or physical-memory bound.
pub fn with_canonical_ranked_policy_checks_v1<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
    budget: &mut Budget<'w>,
    callback: impl for<'s, 'g> FnOnce(
        &CheckedCanonicalRankedPoliciesV1<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
    with_checks(checked, budget, Limits::production_hard_ceiling(), callback)
}

fn with_checks<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV1<'_, '_, '_, '_>,
    budget: &mut Budget<'w>,
    limits: Limits,
    callback: impl for<'s, 'g> FnOnce(
        &CheckedCanonicalRankedPoliciesV1<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
    let mut analysis = AnalysisState::new(limits);
    let result = protected(budget, |budget| {
        // Checked coverage queries require their exact entry floor. Borrow the
        // authenticated owner before reserving this consumer's scratch.
        let owner = checked.inventory(budget)?.owner();
        budget.reserve_storage(checked_add(
            size_of::<AnalysisState>(),
            checked_add(
                size_of::<Guard>(),
                checked_add(
                    size_of::<CheckedCanonicalRankedPoliciesV1<'_, '_>>(),
                    size_of::<std::thread::Result<Result<T, Failure>>>(),
                )?,
            )?,
        )?)?;
        let mut reports = reserve_rows::<ReportRow>(owner.module().functions.len(), budget)?;
        let mut projection = Projection::import(owner, budget)?;
        for ordinal in 0..owner.module().functions.len() {
            let outcome = projection.with_function(ordinal, budget, |context, function| {
                analysis.invoke(ordinal, context, function)
            })??;
            let history = analysis.last.ok_or(Failure::InvocationAccounting)?;
            reports.push(ReportRow { outcome, history });
            projection.check_epoch()?;
            #[cfg(test)]
            tests::between_functions(&projection, ordinal);
        }
        projection.check(budget)?;
        let guard = Guard::new(budget);
        let view = CheckedCanonicalRankedPoliciesV1 {
            owner,
            reports: &reports,
            observation: analysis.observation(),
            guard: &guard,
        };
        let result = guard.callback(budget, |budget| callback(&view, budget));
        if let Err(error) = projection.check_epoch() {
            drop(result);
            return Err(error);
        }
        // Destructors precede either-domain release; the session never escapes.
        drop(projection);
        drop(reports);
        analysis.release_reports()?;
        result
    });
    result.map_err(|failure| CanonicalRankedPolicyChecksErrorV1 {
        failure,
        observation: analysis.observation(),
        last_invocation: analysis.last,
    })
}

#[cfg(test)]
#[path = "canonical_ranked_checks_v1_tests.rs"]
mod tests;
