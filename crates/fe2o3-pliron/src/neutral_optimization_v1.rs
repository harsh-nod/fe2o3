//! Closed neutral execution and consuming extraction on one bridge session.
//! The output is an observed candidate for independent semantic checking.

use crate::{
    KirBridgeErrorV12, KirBridgeOptimizedReceiptV1, KirMappedExtractionErrorV12,
    KirOptimizationMapErrorV12, KirOptimizationMapV12, KirPlironGraphV12,
    PlironOptimizationErrorV12, PlironOptimizationReportV1,
    kir_occurrence_capture_v1::{Capture, KirNeutralOccurrenceRowsV1, Limits},
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use std::{
    error::Error,
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[derive(Debug)]
pub enum KirNeutralOptimizationErrorV1 {
    Bridge(KirBridgeErrorV12),
    Execution(PlironOptimizationErrorV12),
    Extraction(KirMappedExtractionErrorV12),
    Occurrences(KirOptimizationMapErrorV12),
    Resource(ResourceError),
    Panicked,
}
impl fmt::Display for KirNeutralOptimizationErrorV1 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Bridge(e) => e.fmt(f),
            Self::Execution(e) => e.fmt(f),
            Self::Extraction(e) => e.fmt(f),
            Self::Occurrences(e) => e.fmt(f),
            Self::Resource(e) => e.fmt(f),
            Self::Panicked => f.write_str("neutral occurrence capture/extraction panicked"),
        }
    }
}
impl Error for KirNeutralOptimizationErrorV1 {}
impl From<ResourceError> for KirNeutralOptimizationErrorV1 {
    fn from(e: ResourceError) -> Self {
        Self::Resource(e)
    }
}
type Result<T> = std::result::Result<T, KirNeutralOptimizationErrorV1>;

/// Exclusive, one-shot execution custody. Its capture and report stay reserved
/// in this very ledger until consuming extraction or drop. Graph/session growth
/// remains in the caller's graph receipt, including after a failed pass.
pub struct KirNeutralOptimizationLeaseV1<'graph, 'input, 'budget, 'work> {
    graph: &'graph mut KirPlironGraphV12<'input>,
    budget: &'budget mut Budget<'work>,
    capture: Option<Capture>,
    report: Option<PlironOptimizationReportV1>,
    held: usize,
    report_storage: usize,
    limits: Limits,
}

/// Move-only output of actual observed execution. No method claims equivalence,
/// ranked verification, source/ISA projection, proof or executable authority.
pub struct KirNeutralOptimizationOutputV1<'input> {
    input: &'input Owner,
    owner: Owner,
    report: PlironOptimizationReportV1,
    bridge: KirBridgeOptimizedReceiptV1,
    map: KirOptimizationMapV12,
    occurrences: KirNeutralOccurrenceRowsV1,
    storage: KirNeutralOptimizationStorageV1,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KirNeutralOptimizationStorageV1 {
    retained: usize,
}
impl KirNeutralOptimizationStorageV1 {
    pub const fn retained_storage(self) -> usize {
        self.retained
    }
}
// Component receipts already include their inline headers. Charge only the
// remainder of this wrapper, including borrowed-input/receipt fields and padding.
fn output_wrapper_storage_v1() -> Option<usize> {
    let mut remainder = std::mem::size_of::<KirNeutralOptimizationOutputV1<'_>>();
    for header in [
        std::mem::size_of::<Owner>(),
        std::mem::size_of::<PlironOptimizationReportV1>(),
        std::mem::size_of::<KirBridgeOptimizedReceiptV1>(),
        std::mem::size_of::<KirOptimizationMapV12>(),
        std::mem::size_of::<KirNeutralOccurrenceRowsV1>(),
    ] {
        remainder = remainder.checked_sub(header)?;
    }
    Some(remainder)
}
impl<'input> KirNeutralOptimizationOutputV1<'input> {
    pub const fn input(&self) -> &'input Owner {
        self.input
    }
    pub const fn owner(&self) -> &Owner {
        &self.owner
    }
    pub const fn report(&self) -> &PlironOptimizationReportV1 {
        &self.report
    }
    pub const fn bridge(&self) -> &KirBridgeOptimizedReceiptV1 {
        &self.bridge
    }
    pub const fn map(&self) -> &KirOptimizationMapV12 {
        &self.map
    }
    pub const fn occurrences(&self) -> &KirNeutralOccurrenceRowsV1 {
        &self.occurrences
    }
    /// Reserve before any further allocation/checking; release after this owner
    /// drops. Input custody is borrowed and excluded. Graph/session custody is
    /// separately retained until that graph drops.
    pub const fn storage(&self) -> KirNeutralOptimizationStorageV1 {
        self.storage
    }
}

impl<'input> KirPlironGraphV12<'input> {
    /// Executes the same literal seven-pass roster, with additional occurrence
    /// observations. There is no caller-selected pass, optimizer, or fallback.
    /// The caller must already reserve the imported graph and input owner.
    pub fn execute_production_neutral_optimization_v1<'graph, 'budget, 'work>(
        &'graph mut self,
        budget: &'budget mut Budget<'work>,
    ) -> Result<KirNeutralOptimizationLeaseV1<'graph, 'input, 'budget, 'work>> {
        self.validate_custody_v12()
            .map_err(KirNeutralOptimizationErrorV1::Bridge)?;
        if self.optimization_started {
            return Err(KirNeutralOptimizationErrorV1::Execution(
                PlironOptimizationErrorV12::AlreadyExecuted,
            ));
        }
        let limits = self
            .neutral_occurrence_limits_v1(budget)
            .map_err(KirNeutralOptimizationErrorV1::Occurrences)?;
        let storage = limits
            .storage()
            .map_err(KirNeutralOptimizationErrorV1::Occurrences)?;
        budget.charge_work(
            limits
                .work()
                .map_err(KirNeutralOptimizationErrorV1::Occurrences)?,
        )?;
        let floor = budget.storage();
        let prior_graph_storage = self.retained_storage;
        budget.reserve_storage(storage)?;
        let result = catch_unwind(AssertUnwindSafe(|| {
            let capture = self
                .begin_neutral_occurrence_capture_v1(limits)
                .map_err(KirNeutralOptimizationErrorV1::Occurrences)?;
            let execution = self.execute_native_neutral_policy_v1(budget, &capture, limits);
            if let Some(error) = capture.failure() {
                return Err(KirNeutralOptimizationErrorV1::Occurrences(error));
            }
            let (report, profile) = execution.map_err(KirNeutralOptimizationErrorV1::Execution)?;
            // The target executor transfers its report at return. No allocation
            // intervenes before admitting the exact coexisting report payload.
            let held = storage
                .checked_add(profile.retained_storage())
                .ok_or(ResourceError::Arithmetic)?;
            budget.reserve_storage(profile.retained_storage())?;
            Ok((capture, report, profile.retained_storage(), held))
        }));
        match result {
            Ok(Ok((capture, report, report_storage, held))) => Ok(KirNeutralOptimizationLeaseV1 {
                graph: self,
                budget,
                capture: Some(capture),
                report: Some(report),
                held,
                report_storage,
                limits,
            }),
            failed => {
                // Closure-local capture/report owners have dropped before
                // release. The target executor's persistent session growth is
                // deliberately not released by this neutral capture rollback.
                self.session.poisoned = true;
                let keep = self
                    .retained_storage
                    .checked_sub(prior_graph_storage)
                    .ok_or(ResourceError::Accounting)?;
                let release = budget
                    .storage()
                    .checked_sub(floor)
                    .and_then(|n| n.checked_sub(keep))
                    .ok_or(ResourceError::Accounting)?;
                budget.release_storage(release)?;
                match failed {
                    Ok(Err(error)) => Err(error),
                    Err(_) => Err(KirNeutralOptimizationErrorV1::Panicked),
                    _ => unreachable!(),
                }
            }
        }
    }
}
impl<'input> KirNeutralOptimizationLeaseV1<'_, 'input, '_, '_> {
    /// Extracts live SSA once, creates connected canonical output once, and
    /// returns observed rows beside the unchanged old-map evidence. This consumes
    /// execution custody; failures poison the candidate rather than permit reuse.
    pub fn extract(self) -> Result<KirNeutralOptimizationOutputV1<'input>> {
        self.extract_admitted_v1(None)
    }

    fn extract_admitted_v1(
        mut self,
        native: Option<&crate::kir_bridge_v1::NativeBridgeWitnessV1>,
    ) -> Result<KirNeutralOptimizationOutputV1<'input>> {
        let floor = self.budget.storage();
        let result = catch_unwind(AssertUnwindSafe(|| {
            let (owner, bridge, map, extracted) = self
                .graph
                .extract_admitted_canonical_with_map_v1(self.budget, native)
                .map_err(KirNeutralOptimizationErrorV1::Extraction)?;
            self.budget.reserve_storage(extracted.retained_storage())?;
            let scratch = self
                .limits
                .storage()
                .map_err(KirNeutralOptimizationErrorV1::Occurrences)?;
            self.budget.reserve_storage(scratch)?;
            // The new profile already precharged this census and row derivation;
            // all live owners remain reserved during their coexistence.
            let capture = self.capture.as_ref().ok_or(ResourceError::Accounting)?;
            let roster = capture
                .with_roster_meter(|meter| {
                    self.graph.neutral_live_roster_v1(self.limits.nodes, meter)
                })
                .map_err(KirNeutralOptimizationErrorV1::Occurrences)?;
            let rows = capture
                .finish(&self.graph.session.context, &roster, &map, owner.module())
                .map_err(KirNeutralOptimizationErrorV1::Occurrences)?;
            if !map.matches_execution(self.report.as_ref().ok_or(ResourceError::Accounting)?) {
                return Err(KirNeutralOptimizationErrorV1::Occurrences(
                    KirOptimizationMapErrorV12::Passes,
                ));
            }
            let row_storage = rows
                .retained_storage()
                .map_err(KirNeutralOptimizationErrorV1::Occurrences)?;
            let retained = extracted
                .retained_storage()
                .checked_add(row_storage)
                .and_then(|n| n.checked_add(self.report_storage))
                .and_then(|n| n.checked_add(output_wrapper_storage_v1()?))
                .ok_or(ResourceError::Arithmetic)?;
            drop(roster);
            Ok((owner, bridge, map, rows, retained))
        }));
        // Error owners and every closure-local scratch buffer have dropped.
        // Success transfers exactly the returned payload after this final
        // allocation; no checker runs while its reservation is absent.
        let release = self
            .budget
            .storage()
            .checked_sub(floor)
            .ok_or(ResourceError::Accounting)?;
        self.budget.release_storage(release)?;
        let (owner, bridge, map, occurrences, retained) = match result {
            Ok(Ok(output)) => output,
            failed => {
                self.graph.session.poisoned = true;
                return match failed {
                    Ok(Err(error)) => Err(error),
                    Err(_) => Err(KirNeutralOptimizationErrorV1::Panicked),
                    _ => unreachable!(),
                };
            }
        };
        let report = self.report.take().ok_or(ResourceError::Accounting)?;
        let input = self.graph.neutral_input_v1();
        drop(self.capture.take());
        self.budget.release_storage(self.held)?;
        self.held = 0;
        Ok(KirNeutralOptimizationOutputV1 {
            input,
            owner,
            report,
            bridge,
            map,
            occurrences,
            storage: KirNeutralOptimizationStorageV1 { retained },
        })
    }
}
impl Drop for KirNeutralOptimizationLeaseV1<'_, '_, '_, '_> {
    fn drop(&mut self) {
        drop(self.capture.take());
        drop(self.report.take());
        if self.held != 0 {
            // Dropping an unconsumed lease cannot reopen legacy extraction of
            // a candidate whose complete neutral observation was abandoned.
            self.graph.session.poisoned = true;
            let _ = self.budget.release_storage(self.held);
        }
    }
}

/// Imports, executes the fixed seven neutral passes, and extracts through the
/// native structural bridge profile in one private session. Target entry points
/// and their resource/report profiles are unchanged. This returns an observed
/// candidate, not checked equivalence, proof, or executable authority.
///
/// The caller must reserve the input owner before calling. The private graph,
/// immutable signature witness and all session growth drop before restoring
/// the entry storage floor. Reserve the returned output's transfer receipt
/// before further allocation or checking. Accepted work and peak are retained.
pub fn optimize_native_neutral_kernel_ir_v1<'input>(
    input: &'input Owner,
    budget: &mut Budget<'_>,
) -> Result<KirNeutralOptimizationOutputV1<'input>> {
    let floor = budget.storage();
    let result = catch_unwind(AssertUnwindSafe(|| {
        let (mut graph, witness) = crate::kir_bridge_v1::import_native_neutral_v1(input, budget)
            .map_err(KirNeutralOptimizationErrorV1::Bridge)?;
        let output = graph
            .execute_production_neutral_optimization_v1(budget)?
            .extract_admitted_v1(Some(&witness))?;
        drop(witness);
        drop(graph);
        Ok(output)
    }));
    let release = budget
        .storage()
        .checked_sub(floor)
        .ok_or(ResourceError::Accounting)?;
    budget.release_storage(release)?;
    match result {
        Ok(result) => result,
        Err(_) => Err(KirNeutralOptimizationErrorV1::Panicked),
    }
}

#[path = "checked_neutral_optimization_v1.rs"]
mod checked_neutral_optimization_v1;
#[path = "neutral_integer_continuation_v1.rs"]
mod integer_continuation;
#[path = "neutral_optimization_policy3_v1.rs"]
mod policy3;
pub use checked_neutral_optimization_v1::{
    CheckedNeutralKernelIrOwnerV1, KirCheckedNeutralOptimizationErrorV1,
    KirCheckedNeutralOptimizationStorageV1, KirNeutralOwnedOriginStorageV1,
};
pub use integer_continuation::{
    CheckedNeutralKernelIrOwnerIntegerContinuationV1,
    KirNeutralOptimizationOutputIntegerContinuationV1,
    optimize_native_neutral_kernel_ir_integer_continuation_v1,
};
pub use policy3::{
    CheckedNeutralKernelIrOwnerPolicy3V1, KirNeutralOptimizationOutputPolicy3V1,
    optimize_native_neutral_kernel_ir_policy3_v1,
};

#[cfg(test)]
#[path = "neutral_optimization_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "neutral_optimization_transition_v1_tests.rs"]
mod transition_tests;
