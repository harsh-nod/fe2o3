//! Native observations with a complete, explicitly unresolved source-role census.
use super::*;

#[cfg(test)]
std::thread_local! {
    static NATIVE_STARTS: Cell<usize> = const { Cell::new(0) };
    static MUTATE_AFTER_NATIVE: Cell<bool> = const { Cell::new(false) };
    static EXHAUST_AFTER_NATIVE: Cell<Option<(bool, usize)>> = const { Cell::new(None) };
}

#[cfg(test)]
pub(super) fn before_function() {
    NATIVE_STARTS.with(|count| count.set(count.get() + 1));
}

#[cfg(test)]
pub(super) fn after_function(graph: &crate::KirPlironGraphV18<'_>) {
    if MUTATE_AFTER_NATIVE.with(|flag| flag.replace(false)) {
        graph.test_ranked_mutate_and_restore_v18();
    }
}

#[cfg(test)]
pub(super) fn before_post_native_snapshot(budget: &mut Budget<'_>) {
    if let Some((storage, limit)) = EXHAUST_AFTER_NATIVE.with(Cell::take) {
        if storage {
            budget.reserve_storage(limit - budget.storage()).unwrap();
        } else {
            budget.charge_work(limit - budget.work()).unwrap();
        }
    }
}

/// Actual fixed-policy observations, with every source-role obligation pending.
/// This view cannot be promoted into the completed-profile Pliron view. Original
/// source services must consume the entire census under their own typed custody.
///
/// ```compile_fail
/// use fe2o3_pliron::{PendingCanonicalRankedPoliciesV18, CheckedCanonicalRankedPoliciesV18};
/// fn complete(x: PendingCanonicalRankedPoliciesV18<'_, '_>) -> CheckedCanonicalRankedPoliciesV18<'_, '_> { x }
/// ```
/// ```compile_fail
/// use fe2o3_pliron::PendingCanonicalRankedPoliciesV18;
/// fn forge() -> PendingCanonicalRankedPoliciesV18<'static, 'static> {
///     PendingCanonicalRankedPoliciesV18 { obligations: &[] }
/// }
/// ```
pub struct PendingCanonicalRankedPoliciesV18<'s, 'g> {
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    reports: &'s [Option<ReportRow>],
    observation: CanonicalRankedPolicyResourceObservationV1,
    guard: &'s Guard,
    obligations: &'s [CanonicalRankedSourceObligationV18],
}

impl<'g> PendingCanonicalRankedPoliciesV18<'_, 'g> {
    /// Exact immutable canonical owner observed by the native stages.
    pub fn owner(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&'g VerifiedCanonicalKernelIrModuleV18, Failure> {
        self.guard.query(budget)?;
        Ok(self.owner)
    }
    /// Complete occurrence-ordered obligations, not caller-provided annotations.
    pub fn obligations(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&[CanonicalRankedSourceObligationV18], Failure> {
        self.guard.query(budget)?;
        Ok(self.obligations)
    }
    /// Declaration-order function count, including absent declaration reports.
    pub fn function_count(&self, budget: &mut Budget<'_>) -> Result<usize, Failure> {
        self.guard.query(budget)?;
        Ok(self.reports.len())
    }
    /// A report is an observation, not source-role or ranked completion.
    pub fn report(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<&ProductionPlironPreloweringReportV2>, Failure> {
        self.guard.query(budget)?;
        self.reports
            .get(function)
            .map(|row| row.as_ref().map(|row| &row.outcome.report))
            .ok_or_else(|| self.guard.invalid(function))
    }
    /// Accepted invocation history, absent for canonical declarations.
    pub fn history(
        &self,
        function: usize,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalRankedPolicyHistoryV1>, Failure> {
        self.guard.query(budget)?;
        self.reports
            .get(function)
            .map(|row| row.as_ref().map(|row| row.history))
            .ok_or_else(|| self.guard.invalid(function))
    }
    /// Cumulative native analysis-domain resource observation.
    pub fn observation(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<CanonicalRankedPolicyResourceObservationV1, Failure> {
        self.guard.query(budget)?;
        Ok(self.observation)
    }
    /// Last accepted invocation, retained as a diagnostic rather than authority.
    pub fn last_invocation(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<Option<CanonicalRankedPolicyHistoryV1>, Failure> {
        self.guard.query(budget)?;
        budget
            .charge_work(self.reports.len())
            .map_err(|error| self.guard.resource(error))?;
        Ok(self
            .reports
            .iter()
            .rev()
            .find_map(|row| row.as_ref().map(|row| row.history)))
    }
    /// Source roles are always pending in this Pliron-only view.
    pub const fn source_roles_are_complete(&self) -> bool {
        false
    }
    /// Native observations do not complete ranked verification.
    pub const fn ranked_verification_is_complete(&self) -> bool {
        false
    }
    /// No publication or device-launch authority is granted.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn source_obligations(
    owner: &VerifiedCanonicalKernelIrModuleV18,
    budget: &mut Budget<'_>,
) -> Result<Vec<CanonicalRankedSourceObligationV18>, Failure> {
    let mut count = 0usize;
    visit_source_requirements(owner, budget, |_, _| {
        count = count.checked_add(1).ok_or(Resource::Arithmetic)?;
        Ok(())
    })?;
    let mut rows = reserve_rows(count, budget)?;
    visit_source_requirements(owner, budget, |obligation, _| {
        if rows.len() == count || rows.len() == rows.capacity() {
            return Err(Failure::ExactGraph);
        }
        rows.push(obligation);
        Ok(())
    })?;
    if rows.len() != count {
        return Err(Failure::ExactGraph);
    }
    Ok(rows)
}

/// Complete source-role census and one retained exact native graph epoch.
/// No native stage has run merely because this scope exists. Native observations
/// remain a separate non-authoritative phase; no source completion setter exists.
pub struct PendingCanonicalRankedSourceRolesV18<'s, 'g> {
    owner: &'g VerifiedCanonicalKernelIrModuleV18,
    obligations: &'s [CanonicalRankedSourceObligationV18],
    graph: &'s mut crate::KirPlironGraphV18<'g>,
    epoch: u64,
    layouts: StorageLayoutLimitsV1,
    guard: &'s Guard,
    refund_denied: &'s Cell<bool>,
}

impl<'g> PendingCanonicalRankedSourceRolesV18<'_, 'g> {
    fn check(&self, budget: &mut Budget<'_>) -> Result<(), Failure> {
        self.guard.query(budget)?;
        self.graph
            .check_ranked_policy_epoch_v18(self.epoch)
            .map_err(|_| self.guard.mutation())
    }

    /// Immutable owner of the retained graph and the complete role census.
    pub fn owner(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&'g VerifiedCanonicalKernelIrModuleV18, Failure> {
        self.check(budget)?;
        Ok(self.owner)
    }

    /// Complete occurrence-ordered source roles, all still unresolved here.
    pub fn obligations(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<&[CanonicalRankedSourceObligationV18], Failure> {
        self.check(budget)?;
        Ok(self.obligations)
    }

    /// This preflight scope never discharges source roles.
    pub const fn source_roles_are_complete(&self) -> bool {
        false
    }
    /// No publication or device-launch authority is granted.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }

    /// Runs actual fixed stages on the same retained graph/epoch. The resulting
    /// observations still cannot complete a source role or become the existing
    /// completed-profile Pliron type. Callback output must be prepaid outside
    /// this scope and the callback must restore its exact incoming floor.
    pub fn with_native_observations<'w, T>(
        &mut self,
        budget: &mut Budget<'w>,
        callback: impl for<'s> FnOnce(
            &PendingCanonicalRankedPoliciesV18<'s, 'g>,
            &mut Budget<'w>,
        ) -> Result<T, Failure>,
    ) -> Result<T, CanonicalRankedPolicyChecksErrorV1> {
        let mut analysis = AnalysisState::new(Limits::production_hard_ceiling());
        let entered = self.check(budget);
        if let Err(failure) = entered {
            return Err(CanonicalRankedPolicyChecksErrorV1 {
                failure,
                observation: analysis.observation(),
                last_invocation: analysis.last,
            });
        }
        let result = protected(budget, |budget| {
            budget.reserve_storage(checked_add(
                size_of::<&[CanonicalRankedSourceObligationV18]>(),
                checked_add(
                    std::mem::size_of_val(&callback),
                    std::mem::align_of_val(&callback)
                        .checked_mul(2)
                        .ok_or(Resource::Arithmetic)?,
                )?,
            )?)?;
            let mut reports = prepare_native_report_rows_v18::<T>(self.owner, budget)?;
            let result = execute_fixed_native_reports_v18(
                self.owner,
                self.layouts,
                &mut analysis,
                self.graph,
                self.epoch,
                &mut reports,
                budget,
                |owner, reports, observation, guard, budget| {
                    let view = PendingCanonicalRankedPoliciesV18 {
                        owner,
                        reports,
                        observation,
                        guard,
                        obligations: self.obligations,
                    };
                    callback(&view, budget)
                },
            )?;
            drop(reports);
            analysis.release_reports()?;
            self.graph.check_ranked_policy_epoch_v18(self.epoch)?;
            result
        });
        result.map_err(|failure| {
            // A swallowed phase-two resource refusal remains a refusal of this
            // pending scope. Native semantic findings keep their own history;
            // they are not converted into unrelated query failures.
            let failure = match failure {
                Failure::Resource(error) => self.guard.resource(error),
                Failure::Mutation => self.guard.mutation(),
                Failure::View(CanonicalRankedViewErrorV1::Resource(error)) => {
                    self.guard.resource(error)
                }
                Failure::StorageBridge(crate::KirBridgeErrorV18::Canonical(
                    fe2o3_kernel_ir::CanonicalKernelIrReplayAdmissionErrorV18::Resource(error),
                )) => self.guard.resource(error),
                other => other,
            };
            CanonicalRankedPolicyChecksErrorV1 {
                failure,
                observation: analysis.observation(),
                last_invocation: analysis.last,
            }
        })
    }
}

/// Derives every pending source role and establishes the exact graph epoch, but
/// runs no native policy stage. The source consumer can refuse unresolved roles
/// before requesting native observations. No public completion callback,
/// annotation list, allowlist or conversion into completed reports is accepted.
pub fn with_pending_canonical_ranked_source_roles_v18<'w, T>(
    checked: &mut CheckedCanonicalRankedViewV18<'_, '_, '_, '_>,
    layouts: StorageLayoutLimitsV1,
    budget: &mut Budget<'w>,
    callback: impl for<'s, 'g> FnOnce(
        &mut PendingCanonicalRankedSourceRolesV18<'s, 'g>,
        &mut Budget<'w>,
    ) -> Result<T, Failure>,
) -> Result<T, Failure> {
    let owner = checked.inventory(budget)?.owner();
    let refund_denied = Cell::new(false);
    protected_retained_v18(budget, &refund_denied, |budget| {
        let headers = checked_add(
            size_of::<Vec<CanonicalRankedSourceObligationV18>>(),
            checked_add(
                size_of::<PendingCanonicalRankedSourceRolesV18<'_, '_>>(),
                checked_add(
                    size_of::<Guard>(),
                    checked_add(
                        size_of::<std::thread::Result<Result<T, Failure>>>(),
                        checked_add(
                            size_of::<Result<Vec<CanonicalRankedSourceObligationV18>, Failure>>(),
                            drain_header(),
                        )?,
                    )?,
                )?,
            )?,
        )?;
        budget.reserve_storage(checked_add(
            headers,
            checked_add(
                std::mem::size_of_val(&callback),
                std::mem::align_of_val(&callback)
                    .checked_mul(2)
                    .ok_or(Resource::Arithmetic)?,
            )?,
        )?)?;
        budget.reserve_storage(pending_refund_headers_v18()?)?;
        let obligations = source_obligations(owner, budget)?;
        let (mut graph, storage) = crate::KirPlironGraphV18::import(owner, budget)?;
        budget.reserve_storage(storage.retained_storage())?;
        let epoch = graph.ranked_policy_epoch_v18()?;
        exact_snapshot(&mut graph, layouts, epoch, budget)?;
        let guard = Guard::new(budget);
        let mut view = PendingCanonicalRankedSourceRolesV18 {
            owner,
            obligations: &obligations,
            graph: &mut graph,
            epoch,
            layouts,
            guard: &guard,
            refund_denied: &refund_denied,
        };
        let result = guard.callback(budget, |budget| callback(&mut view, budget));
        drop(view);
        if let Err(error) = graph.check_ranked_policy_epoch_v18(epoch) {
            drop(result);
            return Err(error);
        }
        drop(graph);
        drop(obligations);
        result
    })
}

#[cfg(test)]
#[path = "canonical_ranked_pending_v18_tests.rs"]
mod tests;

fn pending_refund_headers_v18() -> Result<usize, Failure> {
    checked_add(size_of::<Cell<bool>>(), size_of::<(&Cell<bool>, Ledger, usize, usize)>())
}

#[path = "canonical_ranked_private_pending_v18.rs"]
mod private_memory;
pub use private_memory::PendingCanonicalPrivateMemoryPoliciesV18;

#[path = "canonical_global_pending_v18.rs"]
mod global_memory;
pub use global_memory::PendingCanonicalGlobalAccessesV18;
