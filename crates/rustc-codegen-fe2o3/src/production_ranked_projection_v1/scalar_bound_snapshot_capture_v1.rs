//! Explicit capture-only attachment; old ranked/source evidence remains unchanged.
use super::*;
use fe2o3_lower_mir_kernel::ProductionU32BoundSnapshotRecurrenceV1 as SnapshotConsistency;
use fe2o3_mir_model::{
    SemanticU32InductionBoundSnapshotErrorV1 as ModelError,
    SemanticU32InductionBoundSnapshotMeterV1 as ModelMeter,
    SemanticU32InductionBoundSnapshotReportV1 as Report,
    analyze_semantic_u32_induction_bound_snapshots_with_meter_v1,
};

/// Retains the existing source/N/ranked owner exactly once plus new-family reports.
/// Its complete receipt remains RESERVED throughout this attachment's lifetime.
pub(crate) struct CapturedBoundSnapshotSourceV1 {
    captured: CapturedRankedSourceV1,
    reports: Vec<Report>,
    retained: usize,
    ledger: fe2o3_kernel_ir::CanonicalKernelIrWorkLedgerIdentityV1,
    slot: usize,
}
pub(crate) struct SourceBoundSnapshotObservationV1<'source> {
    root: &'source ProductionRankedRootProgramV1,
    report: &'source Report,
    ordinal: Option<usize>,
    outcome: Option<SnapshotConsistency<'source>>,
}
impl<'s> SourceBoundSnapshotObservationV1<'s> {
    pub(crate) fn root(&self) -> &'s ProductionRankedRootProgramV1 {
        self.root
    }
    pub(crate) fn report(&self) -> &'s Report {
        self.report
    }
    pub(crate) fn certificate_ordinal(&self) -> Option<usize> {
        self.ordinal
    }
    pub(crate) fn outcome(&self) -> Option<SnapshotConsistency<'s>> {
        self.outcome
    }
}

struct Meter<'a, 'work>(&'a mut Budget<'work>);
impl ModelMeter for Meter<'_, '_> {
    type Error = Resource;
    fn charge_work(&mut self, amount: usize) -> Result<(), Resource> {
        self.0.charge_work(amount)
    }
    fn reserve_storage(&mut self, amount: usize) -> Result<(), Resource> {
        self.0.reserve_storage(amount)?;
        #[cfg(test)]
        tests::after_model_reservation(self.0)?;
        Ok(())
    }
}

#[cfg(test)]
#[path = "scalar_bound_snapshot_capture_v1_tests.rs"]
mod tests;
fn model_error(error: ModelError<Resource>) -> CaptureError {
    match error {
        ModelError::Analysis(error) => CaptureError::BoundSnapshotAnalysis(error),
        ModelError::Meter(error) => CaptureError::Resource(error),
    }
}
fn table_bytes(capacity: usize) -> Result<usize, CaptureError> {
    capacity
        .checked_mul(size_of::<Report>())
        .ok_or(Resource::Arithmetic.into())
}

impl CapturedBoundSnapshotSourceV1 {
    /// Consumes an already RESERVED legacy capture without repeating import,
    /// materialization or ranked projection. Success keeps the complete new
    /// receipt RESERVED on this same ledger. Error/unwind drops owned stages
    /// before restoring the unrelated floor; work is never reset or released.
    pub(crate) fn try_attach_v1(
        captured: CapturedRankedSourceV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, CaptureError> {
        let consumed = captured.retained_storage();
        let floor = budget
            .storage()
            .checked_sub(consumed)
            .ok_or(Resource::Accounting)?;
        let slot = budget as *mut _ as usize;
        let ledger = budget.work_ledger_identity_v1();
        let mut payload = None;
        let mut outcome = match catch_unwind(AssertUnwindSafe(|| -> Result<Self, CaptureError> {
            let delta = size_of::<Self>()
                .checked_sub(size_of::<CapturedRankedSourceV1>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(delta)?;
            let mut reports = Vec::new();
            let count = captured.roots.len();
            budget.charge_work(count)?;
            let requested = table_bytes(count)?;
            budget.reserve_storage(requested)?;
            reports
                .try_reserve_exact(count)
                .map_err(|_| Resource::Allocation)?;
            let capacity = table_bytes(reports.capacity())?;
            budget.reserve_storage(
                capacity
                    .checked_sub(requested)
                    .ok_or(Resource::Accounting)?,
            )?;
            let mut retained = consumed
                .checked_add(delta)
                .and_then(|n| n.checked_add(capacity))
                .ok_or(Resource::Arithmetic)?;
            let source = captured.capture.original().semantic_ssa().source_semantic();
            for root in &captured.roots {
                budget.charge_work(4)?;
                // The retained ranked owner already selected this actual body.
                // Reuse its closed binding, then C independently joins root/body
                // to the retained emission alias; do not rerun wrapper selection.
                let selected = root.semantic_u32_induction.function();
                let declaration = source
                    .functions()
                    .get(selected.index() as usize)
                    .ok_or(CaptureError::Mismatch("snapshot retained selected body"))?;
                if root.semantic_u32_induction.function_identity() != declaration.identity()
                    || root.semantic_u32_induction.semantic_mir_sha256() != source.semantic_sha256()
                {
                    return Err(CaptureError::Mismatch(
                        "snapshot retained ranked/source report identity",
                    ));
                }
                let entry = budget.storage();
                let report = analyze_semantic_u32_induction_bound_snapshots_with_meter_v1(
                    source,
                    selected,
                    Default::default(),
                    &mut Meter(budget),
                )
                .map_err(model_error)?;
                // The vector already owns/prepays this header. Keep only the
                // transferred certificate capacity after moving the report.
                let payload = report
                    .retained_storage()
                    .checked_sub(size_of::<Report>())
                    .ok_or(Resource::Accounting)?;
                if reports.len() == reports.capacity() {
                    return Err(Resource::Accounting.into());
                }
                reports.push(report);
                retained = retained.checked_add(payload).ok_or(Resource::Arithmetic)?;
                let required = entry.checked_add(payload).ok_or(Resource::Arithmetic)?;
                let scratch = budget
                    .storage()
                    .checked_sub(required)
                    .ok_or(Resource::Accounting)?;
                budget.release_storage(scratch)?;
            }
            if budget.storage() != floor.checked_add(retained).ok_or(Resource::Arithmetic)? {
                return Err(Resource::Accounting.into());
            }
            Ok(Self {
                captured,
                reports,
                retained,
                ledger,
                slot,
            })
        })) {
            Ok(result) => Some(result),
            Err(value) => {
                payload = Some(value);
                None
            }
        };
        if budget as *mut _ as usize != slot
            || budget.work_ledger_identity_v1() != ledger
            || budget.storage() < floor
        {
            drop((outcome, payload));
            return Err(Resource::Accounting.into());
        }
        if outcome.as_ref().is_some_and(|result| result.is_ok()) {
            // All scratch has already dropped and been released; the successful
            // source/reports remain fully reserved, not an unreserved handoff.
            return outcome.take().unwrap();
        }
        budget.release_storage(budget.storage() - floor)?;
        drop(payload);
        outcome.unwrap_or(Err(CaptureError::Panicked))
    }

    pub(crate) fn capture(&self) -> &Capture {
        self.captured.capture()
    }
    pub(crate) fn retained_storage(&self) -> usize {
        self.retained
    }
    pub(crate) const fn grants_authority(&self) -> bool {
        false
    }

    pub(crate) fn observation_count_v1(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<usize, CaptureError> {
        if budget as *mut _ as usize != self.slot
            || budget.work_ledger_identity_v1() != self.ledger
            || budget.storage() < self.retained
        {
            return Err(Resource::Accounting.into());
        }
        budget.charge_work(self.reports.len())?;
        self.reports.iter().try_fold(0usize, |sum, report| {
            sum.checked_add(report.certificates().len().max(1))
                .ok_or(Resource::Arithmetic.into())
        })
    }

    /// One shared existing A+B+C scope, observing the complete new report roster.
    /// New facts retain Statement/Entry distinctions and all old C exclusions.
    pub(crate) fn with_observations_v1<'source, 'work>(
        &'source self,
        budget: &mut Budget<'work>,
        mut observe: impl FnMut(
            SourceBoundSnapshotObservationV1<'source>,
            &mut Budget<'work>,
        ) -> Result<(), CaptureError>,
    ) -> Result<(), CaptureError> {
        if budget as *mut _ as usize != self.slot
            || budget.work_ledger_identity_v1() != self.ledger
            || budget.storage() < self.retained
            || self.reports.len() != self.captured.roots.len()
        {
            return Err(Resource::Accounting.into());
        }
        self.capture()
            .with_u32_recurrences_v1(Default::default(), budget, |query, budget| {
                let ledger = budget.work_ledger_identity_v1();
                let slot = budget as *mut _ as usize;
                let floor = budget.storage();
                let guard = |budget: &mut Budget<'_>| -> Result<(), CaptureError> {
                    if budget as *mut _ as usize != slot
                        || budget.work_ledger_identity_v1() != ledger
                        || budget.storage() != floor
                    {
                        return Err(Resource::Accounting.into());
                    }
                    Ok(())
                };
                for (root, report) in self.captured.roots.iter().zip(&self.reports) {
                    guard(budget)?;
                    budget.charge_work(1)?;
                    if report.certificates().is_empty() {
                        observe(
                            SourceBoundSnapshotObservationV1 {
                                root,
                                report,
                                ordinal: None,
                                outcome: None,
                            },
                            budget,
                        )?;
                    } else {
                        for (ordinal, certificate) in report.certificates().iter().enumerate() {
                            guard(budget)?;
                            budget.charge_work(1)?;
                            let outcome = query.check_u32_bound_snapshot_certificate_v1(
                                root.semantic_root,
                                certificate,
                                budget,
                            )?;
                            observe(
                                SourceBoundSnapshotObservationV1 {
                                    root,
                                    report,
                                    ordinal: Some(ordinal),
                                    outcome: Some(outcome),
                                },
                                budget,
                            )?;
                        }
                    }
                }
                guard(budget)
            })
    }
}
