//! Fixed native private coverage; original source roles remain unresolved.
use super::*;
use fe2o3_kernel_analysis::CheckedCanonicalKirPrivateMemoryV18;
use crate::production_analysis::canonical_ranked_checks_v1::private::private_resources::PrivateAnalysisV1;
use crate::production_analysis::pliron_pipeline::canonical_private_v1::CanonicalPrivatePipelineOutcomeV1;

struct PrivateReportRowV18 {
    outcome: CanonicalPrivatePipelineOutcomeV1,
    history: CanonicalRankedPolicyHistoryV1,
}

/// Paired physical-memory and fixed native reports for one exact immutable owner.
/// This scoped observation is not an original source/currentness/RHS proof.
///
/// ```compile_fail
/// use fe2o3_pliron::{PendingCanonicalPrivateMemoryPoliciesV18, CheckedCanonicalRankedPoliciesV18};
/// fn promote(x: PendingCanonicalPrivateMemoryPoliciesV18<'_, '_>)
///     -> CheckedCanonicalRankedPoliciesV18<'_, '_> { x }
/// ```
pub struct PendingCanonicalPrivateMemoryPoliciesV18<'s, 'g> {
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    reports: &'s [Option<PrivateReportRowV18>],
    observation: CanonicalRankedPolicyResourceObservationV1,
    guard: &'s Guard,
    obligations: &'s [CanonicalRankedSourceObligationV18],
}
impl<'g> PendingCanonicalPrivateMemoryPoliciesV18<'_, 'g> {
    pub fn owner(&self, budget: &mut Budget<'_>) -> Result<&'g VerifiedCanonicalKernelIrModuleV18, Failure> {
        self.guard.query(budget)?;
        Ok(self.owner)
    }
    pub fn obligations(&self, budget: &mut Budget<'_>) -> Result<&[CanonicalRankedSourceObligationV18], Failure> {
        self.guard.query(budget)?;
        Ok(self.obligations)
    }
    pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.guard.query(budget)?;
        Ok(self.reports.len())
    }
    fn row(&self, function: usize, budget: &mut Budget<'_>) -> Result<Option<&PrivateReportRowV18>, Failure> {
        self.guard.query(budget)?;
        self.reports.get(function).map(Option::as_ref).ok_or_else(|| self.guard.invalid(function))
    }
    pub fn report(&self, function: usize, budget: &mut Budget<'_>)
        -> Result<Option<&ProductionPlironPreloweringReportV2>, Failure>
    {
        Ok(self.row(function, budget)?.map(|row| row.outcome.report.reports()))
    }
    pub fn paired_stage_count(&self, function: usize, budget: &mut Budget<'_>) -> Result<Option<usize>, Failure> {
        Ok(self.row(function, budget)?.map(|row| row.outcome.report.paired_stage_count()))
    }
    pub fn history(&self, function: usize, budget: &mut Budget<'_>) -> Result<Option<CanonicalRankedPolicyHistoryV1>, Failure> {
        Ok(self.row(function, budget)?.map(|row| row.history))
    }
    pub fn observation(&self, budget: &mut Budget<'_>) -> Result<CanonicalRankedPolicyResourceObservationV1, Failure> {
        self.guard.query(budget)?;
        Ok(self.observation)
    }
    pub fn last_invocation(&self, budget: &mut Budget<'_>) -> Result<Option<CanonicalRankedPolicyHistoryV1>, Failure> {
        self.guard.query(budget)?;
        budget.charge_work(self.reports.len()).map_err(|error| self.guard.resource(error))?;
        Ok(self.reports.iter().rev().find_map(|row| row.as_ref().map(|row| row.history)))
    }
    pub const fn source_roles_are_complete(&self) -> bool { false }
    pub const fn ranked_verification_is_complete(&self) -> bool { false }
    pub const fn grants_artifact_or_launch_authority(&self) -> bool { false }
}

fn pending_private_headers_v18<T>(capture: usize, alignment: usize) -> Result<usize, Failure> {
    fn h<T>() -> Result<usize, Failure> {
        checked_add(size_of::<T>(), size_of::<Result<T, Failure>>().checked_mul(2).ok_or(Resource::Arithmetic)?)
    }
    let mut total = checked_add(capture, alignment.checked_mul(2).ok_or(Resource::Arithmetic)?)?;
    for amount in [
        h::<PrivateAnalysisV1>()?, h::<Guard>()?, h::<PendingCanonicalPrivateMemoryPoliciesV18<'_, '_>>()?,
        h::<Vec<Option<PrivateReportRowV18>>>()?, h::<PrivateReportRowV18>()?,
        h::<CanonicalPrivatePipelineOutcomeV1>()?, h::<CanonicalRankedPolicyHistoryV1>()?,
        h::<CanonicalRankedPolicyResourceObservationV1>()?, h::<Option<&PrivateReportRowV18>>()?,
        h::<Option<&Option<PrivateReportRowV18>>>()?, h::<&PrivateReportRowV18>()?,
        h::<Option<&mut Option<PrivateReportRowV18>>>()?, h::<&mut Option<PrivateReportRowV18>>()?,
        h::<Option<&ProductionPlironPreloweringReportV2>>()?, h::<Option<usize>>()?,
        h::<Option<CanonicalRankedPolicyHistoryV1>>()?, h::<usize>()?, h::<()>()?,
        h::<std::iter::Rev<std::slice::Iter<'_, Option<PrivateReportRowV18>>>>()?,
        h::<&Option<PrivateReportRowV18>>()?,
        h::<&VerifiedCanonicalKernelIrModuleV18>()?, h::<&[CanonicalRankedSourceObligationV18]>()?,
        h::<&[Option<PrivateReportRowV18>]>()?, h::<&Guard>()?, h::<&mut Budget<'_>>()?,
        h::<Result<T, Failure>>()?, h::<std::thread::Result<Result<T, Failure>>>()?,
        h::<(&mut PrivateAnalysisV1, &mut [Option<PrivateReportRowV18>])>()?,
        h::<(&PendingCanonicalPrivateMemoryPoliciesV18<'_, '_>, &mut Budget<'_>)>()?,
        h::<(&mut PendingCanonicalRankedSourceRolesV18<'_, '_>, &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
             &mut PrivateAnalysisV1, usize, usize)>()?, drain_header(),
    ] { total = checked_add(total, amount)?; }
    Ok(total)
}

impl<'g> PendingCanonicalRankedSourceRolesV18<'_, 'g> {
    /// Runs the fixed nine stages with independent exact private-memory coverage
    /// at each checkpoint. The proof must borrow this graph's immutable owner.
    /// This does not discharge any source obligation. The callback follows the
    /// same exact-floor/prepaid-output contract as with_native_observations.
    pub fn with_private_memory_observations_v18<'w, T>(
        &mut self,
        physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        budget: &mut Budget<'w>,
        callback: impl for<'s> FnOnce(&PendingCanonicalPrivateMemoryPoliciesV18<'s, 'g>, &mut Budget<'w>) -> Result<T, Failure>,
    ) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
        self.with_private_memory_observations_with_limits_v18(physical, Limits::production_hard_ceiling(), budget, callback)
    }

    fn with_private_memory_observations_with_limits_v18<'w, T>(
        &mut self,
        physical: &CheckedCanonicalKirPrivateMemoryV18<'_, '_>,
        limits: Limits,
        budget: &mut Budget<'w>,
        callback: impl for<'s> FnOnce(&PendingCanonicalPrivateMemoryPoliciesV18<'s, 'g>, &mut Budget<'w>) -> Result<T, Failure>,
    ) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
        let mut analysis = PrivateAnalysisV1::new(limits);
        let entered = self.check(budget).and_then(|()| {
            if std::ptr::eq(physical.inventory().owner(), self.owner) { Ok(()) }
            else { Err(Failure::ExactGraph) }
        });
        if let Err(failure) = entered {
            return Err(CanonicalRankedPolicyChecksErrorV1 { failure, observation: analysis.observation(), last_invocation: analysis.last });
        }
        let capture = std::mem::size_of_val(&callback);
        let alignment = std::mem::align_of_val(&callback);
        let result = protected(budget, |budget| {
            budget.reserve_storage(pending_private_headers_v18::<T>(capture, alignment)?)?;
            let count = self.owner.module().functions.len();
            let mut reports = reserve_rows::<Option<PrivateReportRowV18>>(count, budget)?;
            budget.charge_work(count)?;
            reports.resize_with(count, || None);
            exact_snapshot(self.graph, self.layouts, self.epoch, budget)?;
            self.graph.visit_private_policy_functions_v18(physical, self.epoch, budget, |ordinal, input| {
                let slot = reports.get_mut(ordinal).ok_or(Failure::ExactGraph)?;
                if slot.is_some() { return Err(Failure::ExactGraph); }
                let outcome = analysis.invoke(input)?;
                let history = analysis.last.ok_or(Failure::InvocationAccounting)?;
                *slot = Some(PrivateReportRowV18 { outcome, history });
                Ok(())
            })?;
            exact_snapshot(self.graph, self.layouts, self.epoch, budget)?;
            let guard = Guard::new(budget);
            let view = PendingCanonicalPrivateMemoryPoliciesV18 {
                owner: self.owner, reports: &reports, observation: analysis.observation(),
                guard: &guard, obligations: self.obligations,
            };
            let result = guard.callback(budget, |budget| callback(&view, budget));
            // The view and every actual report die before their own credits.
            drop(view);
            let epoch_result = self.graph.check_ranked_policy_epoch_v18(self.epoch);
            drop(reports);
            analysis.release_reports()?;
            epoch_result?;
            result
        });
        result.map_err(|failure| {
            let failure = match failure {
                Failure::Resource(error) => self.guard.resource(error),
                Failure::Mutation => self.guard.mutation(),
                Failure::View(CanonicalRankedViewErrorV1::Resource(error)) => self.guard.resource(error),
                Failure::StorageBridge(crate::KirBridgeErrorV18::Canonical(
                    fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Resource(error))) => self.guard.resource(error),
                other => other,
            };
            CanonicalRankedPolicyChecksErrorV1 { failure, observation: analysis.observation(), last_invocation: analysis.last }
        })
    }
}

#[cfg(test)]
#[path = "canonical_ranked_private_pending_v18_tests.rs"]
mod private_tests;
