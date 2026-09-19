//! Genuine source/N attachment retained beside the already-computed ranked reports.
// This explicit capture-only API is not yet selected by a shipping driver.
#![allow(dead_code)]
use super::*;
use crate::production_pipeline::ProductionPipelineError as Error;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::{
    ProductionScalarSsaEmissionErrorV1 as CaptureError,
    ProductionScalarSsaEmissionOwnerV1 as Capture,
    ProductionU32RecurrenceConsistencyV1 as Consistency,
};
use std::{
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

/// Owns one genuine source/N attachment and the existing projected root reports.
/// Ranked proof authentication, guard transport and final-output admission have
/// not occurred. No self-borrow, raw-parts constructor or Clone is provided.
pub(crate) struct CapturedRankedSourceV1 {
    capture: Capture,
    roots: Box<[ProductionRankedRootProgramV1]>,
    retained: usize,
}

/// Inert observation of an actual root/report and its source/N query outcome.
/// No-certificate roots are explicit, not silently omitted or successful joins.
pub(crate) struct SourceLoopObservationV1<'source> {
    root: &'source ProductionRankedRootProgramV1,
    ordinal: Option<usize>,
    outcome: Option<Consistency<'source>>,
}
impl<'s> SourceLoopObservationV1<'s> {
    /// Borrows the actual projected root in source roster order.
    pub(crate) fn root(&self) -> &'s ProductionRankedRootProgramV1 {
        self.root
    }
    /// Borrows the exact report computed by the existing root projector once.
    pub(crate) fn report(&self) -> &'s fe2o3_mir_model::SemanticU32InductionNoOverflowReportV1 {
        &self.root.semantic_u32_induction
    }
    /// None means this root's existing source report contains no certificate.
    pub(crate) fn certificate_ordinal(&self) -> Option<usize> {
        self.ordinal
    }
    /// Joined, unavailable, or None for an empty source report; never authority.
    pub(crate) fn outcome(&self) -> Option<Consistency<'s>> {
        self.outcome
    }
}

fn capture_error(error: impl Into<CaptureError>) -> Error {
    Error::ScalarEmissionCapture(Box::new(error.into()))
}

impl CapturedRankedSourceV1 {
    /// Consumes an already-reserved capture exactly once. All exits consume its
    /// reservation and restore the unrelated entry floor. Success transfers the
    /// whole returned receipt UNRESERVED. Ranked projection keeps its inherited
    /// bounded allocation domain; the new receipt covers the wrapper delta and
    /// capture, not those pre-existing report payloads or allocator/RSS behavior.
    pub(crate) fn try_project_v1(
        capture: Capture,
        inputs: &[ProductionRankedRootInputV1],
        references: &crate::reference_effect_v1::AuthenticatedReferenceEffectBindingsV1,
        budget: &mut Budget<'_>,
    ) -> Result<Self, Error> {
        let consumed = capture.retained_analysis_storage_v1();
        let floor = budget
            .storage()
            .checked_sub(consumed)
            .ok_or_else(|| capture_error(Resource::Accounting))?;
        let ledger = budget.work_ledger_identity_v1();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let delta = size_of::<Self>()
                .checked_sub(size_of::<Capture>())
                .ok_or_else(|| capture_error(Resource::Arithmetic))?;
            budget.reserve_storage(delta).map_err(capture_error)?;
            let roots = {
                let source =
                    RankedProjectionSourceV1::from_materialized_checked(capture.original())
                        .map_err(Error::RankedProjection)?;
                project_ranked_roots_v1(&source, inputs, references, budget)
                    .map_err(Error::RankedProjection)?
            };
            let retained = consumed
                .checked_add(delta)
                .ok_or_else(|| capture_error(Resource::Arithmetic))?;
            Ok(Self {
                capture,
                roots,
                retained,
            })
        }));
        if budget.work_ledger_identity_v1() != ledger || budget.storage() < floor {
            drop(result);
            return Err(capture_error(Resource::Accounting));
        }
        // Failed setup locals have dropped; successful owners are transferred.
        budget
            .release_storage(budget.storage() - floor)
            .map_err(capture_error)?;
        match result {
            Ok(result) => result,
            Err(payload) => {
                drop(payload);
                Err(capture_error(CaptureError::Panicked))
            }
        }
    }

    /// Borrows the actual emitted N and retained source owner; never a replay graph.
    pub(crate) fn capture(&self) -> &Capture {
        &self.capture
    }
    /// Complete new wrapper/capture transfer, excluding inherited ranked payloads.
    pub(crate) fn retained_storage(&self) -> usize {
        self.retained
    }
    /// Number of distinct roots actually projected from the admitted source roster.
    pub(crate) fn root_count(&self) -> usize {
        self.roots.len()
    }
    /// Always false. This stage cannot be passed to an artifact or optimizer API.
    pub(crate) const fn grants_authority(&self) -> bool {
        false
    }

    /// Counts fixed observation slots before the caller prepays output staging.
    /// Empty reports still produce one explicit no-certificate observation.
    pub(crate) fn observation_count_v1(
        &self,
        budget: &mut Budget<'_>,
    ) -> Result<usize, CaptureError> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        budget.charge_work(self.roots.len())?;
        self.roots.iter().try_fold(0usize, |sum, root| {
            sum.checked_add(root.semantic_u32_induction.certificates().len().max(1))
                .ok_or_else(|| Resource::Arithmetic.into())
        })
    }

    /// Replays A+B+C once outside the projector's masked-assertion callbacks and
    /// visits every actual root/certificate with the same live budget. The caller
    /// reserves this stage and any output staging before entry. Callback outputs
    /// may borrow source facts but cannot retain analysis resources. Unsupported
    /// mappings are distinct from errors; callbacks cannot suppress query failure.
    pub(crate) fn with_observations_v1<'source, 'work>(
        &'source self,
        budget: &mut Budget<'work>,
        mut observe: impl FnMut(
            SourceLoopObservationV1<'source>,
            &mut Budget<'work>,
        ) -> Result<(), CaptureError>,
    ) -> Result<(), CaptureError> {
        if budget.storage() < self.retained {
            return Err(Resource::Accounting.into());
        }
        self.capture
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
                for root in &self.roots {
                    guard(budget)?;
                    budget.charge_work(1)?;
                    let report = &root.semantic_u32_induction;
                    if report.certificates().is_empty() {
                        observe(
                            SourceLoopObservationV1 {
                                root,
                                ordinal: None,
                                outcome: None,
                            },
                            budget,
                        )?;
                    } else {
                        for (ordinal, certificate) in report.certificates().iter().enumerate() {
                            guard(budget)?;
                            budget.charge_work(1)?;
                            let outcome = query.check_u32_certificate_v1(
                                root.semantic_root,
                                certificate,
                                budget,
                            )?;
                            observe(
                                SourceLoopObservationV1 {
                                    root,
                                    ordinal: Some(ordinal),
                                    outcome: Some(outcome),
                                },
                                budget,
                            )?;
                        }
                    }
                }
                Ok(())
            })
    }
}

#[cfg(test)]
#[path = "scalar_emission_capture_v1_tests.rs"]
mod tests;
