//! Fresh actual-output formal extraction with a closed structural guarded-read rule.

use super::*;
use crate::ProductionFormalMemoryErrorV1 as Error;
use fe2o3_kernel_ir::{
    ExplicitLaunchExtent, FormalMemoryObligationAnalysis, VerifiedCanonicalKernelIrModuleV12,
    derive_kernel_memory_obligations_for_launch,
};

/// Only this function creates the fresh report and its complete exception list.
/// Neither an N report nor a caller-selected subset of reasons can be supplied.
/// This establishes the existing structural guarded-load rule on actual O, not
/// source correspondence, arbitrary launch bounds, alias discharge or authority.
/// The formal engine and guarded checker retain their separate bounded work and
/// allocation domains; this is not canonical-ledger or whole-compiler accounting.
pub(crate) fn derive_checked_output_guarded_obligations_v1(
    output: &VerifiedCanonicalKernelIrModuleV12,
    max_operations: usize,
) -> Result<Box<[FormalMemoryObligations]>, Error> {
    let module = output.module();
    if module.kernels.is_empty() {
        return Err(Error::KernelCount { actual: 0 });
    }
    let mut kernels = Vec::with_capacity(module.kernels.len());
    for kernel in &module.kernels {
        let extents = crate::production_formal_memory_v1::witness_extents(&kernel.domain);
        let analysis = derive_kernel_memory_obligations_for_launch(
            module,
            &kernel.id,
            ExplicitLaunchExtent::Exact {
                rank: kernel.domain.rank(),
                extents,
            },
            FormalIndexWidth::Bits64,
        )
        .map_err(Error::Analysis)?;
        let obligations =
            match analysis {
                FormalMemoryObligationAnalysis::Complete(obligations) => obligations,
                FormalMemoryObligationAnalysis::Incomplete { partial, reasons } => {
                    if reasons.is_empty() || reasons.iter().any(|reason| {
                        !matches!(
                            reason,
                            FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof { .. }
                        )
                    }) {
                        return Err(Error::Incomplete {
                            reasons: reasons.into_boxed_slice(),
                        });
                    }
                    let locations = reasons
                        .iter()
                        .filter_map(|reason| match reason {
                            FormalMemoryIncompleteReason::GuardedAccessRequiresRankedProof {
                                location,
                            } => Some(*location),
                            _ => None,
                        })
                        .collect::<Vec<_>>();
                    let result =
                        GuardedAddressProofBudgetV1::new(max_operations).and_then(|mut budget| {
                            guarded_accesses_have_structural_bounds_result(
                                module,
                                kernel,
                                &partial,
                                extents,
                                &locations,
                                max_operations,
                                &mut budget,
                            )
                        });
                    if let Err(detail) = result {
                        return Err(Error::GuardedAccessDischarge {
                            reasons: reasons.into_boxed_slice(),
                            detail,
                        });
                    }
                    partial
                }
            };
        if !obligations.inter_invocation_conflicts().is_empty() {
            return Err(Error::InterInvocationConflicts {
                conflicts: obligations
                    .inter_invocation_conflicts()
                    .to_vec()
                    .into_boxed_slice(),
            });
        }
        kernels.push(obligations);
    }
    Ok(kernels.into_boxed_slice())
}

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as FormalBudget,
    CanonicalKernelIrVerificationResourceErrorV1 as FormalResource,
};

/// A retained source selects the existing formal engine's operation bound.
/// This anchor is not a proof that the separately borrowed output descends from
/// that source. Complete source/history authentication remains caller-owned.
#[derive(Clone, Copy)]
pub enum CanonicalOutputFormalSourceAnchorV1<'s> {
    /// Connected Direct source with its actual retained ranked checks.
    Direct(&'s ProductionSemanticKirOwnerV1),
    /// Original UnitLocal source with its actual checked N-to-E ownership.
    Erased(&'s ProductionUnitLocalErasedSourceOwnerV1),
}

/// Typed refusal of source replay, fixed-policy extraction, or shared accounting.
#[derive(Debug)]
pub enum CanonicalOutputGuardedFormalMemoryErrorV1 {
    /// The supplied work/storage ledger refused the operation.
    Resource(FormalResource),
    /// The actual retained source failed its existing replay.
    Source(ProductionSemanticKirErrorV1),
    /// Fresh extraction failed the unchanged formal/structural-guard policy.
    Formal(crate::ProductionFormalMemoryErrorV1),
    /// Direct source lacks a connected pre-ranked executable or receipt.
    MissingConnectedSource,
    /// Reports do not cover the complete nonempty ordered output roster.
    KernelRoster,
    /// The analysis unwound without returning a report.
    Panicked,
}
type OutputFormalError = CanonicalOutputGuardedFormalMemoryErrorV1;
impl From<FormalResource> for OutputFormalError {
    fn from(error: FormalResource) -> Self {
        Self::Resource(error)
    }
}
impl std::fmt::Display for OutputFormalError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "canonical output formal analysis: {self:?}")
    }
}
impl std::error::Error for OutputFormalError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Resource(error) => Some(error),
            Self::Source(error) => Some(error),
            Self::Formal(error) => Some(error),
            _ => None,
        }
    }
}

/// Added report header/row storage, excluding borrowed output and source owners.
/// Existing formal/guarded payloads keep their inherited bounded engine domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CanonicalOutputGuardedFormalMemoryStorageV1(usize);
impl CanonicalOutputGuardedFormalMemoryStorageV1 {
    /// Reserve before further controlled work, and release only after report drop.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Fresh fixed-policy reports borrowing the exact analyzed final V12 owner.
/// This supplies neither source-to-output lineage nor signed, typed-rustc,
/// artifact, default-pipeline, or runtime launch authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::CanonicalOutputGuardedFormalMemoryV1;
/// fn duplicate(report: CanonicalOutputGuardedFormalMemoryV1<'_>) {
///     let _ = report.clone();
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{CanonicalOutputFormalSourceAnchorV1,
///     CanonicalOutputGuardedFormalMemoryV1, analyze_canonical_output_guarded_formal_memory_v1};
/// use fe2o3_kernel_ir::{VerifiedCanonicalKernelIrModuleV12,
///     CanonicalKernelIrVerificationResourceBudgetV1 as Budget};
/// fn escape(output: VerifiedCanonicalKernelIrModuleV12,
///     source: CanonicalOutputFormalSourceAnchorV1<'_>, budget: &mut Budget<'_>)
///     -> CanonicalOutputGuardedFormalMemoryV1<'static> {
///     analyze_canonical_output_guarded_formal_memory_v1(&output, source, budget).unwrap().0
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{CanonicalOutputGuardedFormalMemoryV1,
///     ProductionFormalMemoryOwnerV1};
/// fn authority(report: CanonicalOutputGuardedFormalMemoryV1<'_>)
///     -> ProductionFormalMemoryOwnerV1 { report.into() }
/// ```
pub struct CanonicalOutputGuardedFormalMemoryV1<'f> {
    output: &'f VerifiedCanonicalKernelIrModuleV12,
    reports: Box<[FormalMemoryObligations]>,
    max_operations: usize,
    retained: usize,
}
impl<'f> CanonicalOutputGuardedFormalMemoryV1<'f> {
    /// Exact borrowed final graph, not an identity-only replacement.
    pub const fn output(&self) -> &'f VerifiedCanonicalKernelIrModuleV12 {
        self.output
    }
    /// Fresh reports in complete output kernel order.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.reports
    }
    /// Exact operation limit copied from the replayed source, never a wire limit.
    pub const fn max_operations(&self) -> usize {
        self.max_operations
    }
    /// Known returned header and row backing; receipt is initially unreserved.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// A formal report never authorizes an artifact or launch.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

#[allow(
    clippy::result_large_err,
    reason = "Preserve inline typed failures without an unmetered error allocation."
)]
fn output_formal_scoped<'w, T>(
    budget: &mut FormalBudget<'w>,
    run: impl FnOnce(&mut FormalBudget<'w>) -> Result<T, OutputFormalError>,
) -> Result<T, OutputFormalError> {
    use std::panic::{AssertUnwindSafe, catch_unwind};
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let slot = budget as *const FormalBudget<'w> as usize;
    let mut payloads = [None, None];
    let mut result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            payloads[0] = Some(payload);
            Err(OutputFormalError::Panicked)
        }
    };
    if budget.work_ledger_identity_v1() != ledger
        || slot != budget as *const FormalBudget<'w> as usize
        || budget.storage() < floor
    {
        let rejected = std::mem::replace(&mut result, Err(FormalResource::Accounting.into()));
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
            payloads[1] = Some(payload);
        }
    } else if let Err(error) = budget.release_storage(budget.storage() - floor) {
        let rejected = std::mem::replace(&mut result, Err(error.into()));
        if let Err(payload) = catch_unwind(AssertUnwindSafe(|| drop(rejected))) {
            payloads[1] = Some(payload);
        }
    }
    // Caught payload destructors run only after same-ledger floor restoration.
    drop(payloads);
    result
}

#[allow(
    clippy::result_large_err,
    reason = "Preserve inline typed failures without an unmetered error allocation."
)]
fn output_formal_source_limit(
    source: CanonicalOutputFormalSourceAnchorV1<'_>,
    budget: &mut FormalBudget<'_>,
) -> Result<usize, OutputFormalError> {
    match source {
        CanonicalOutputFormalSourceAnchorV1::Direct(source) => {
            source
                .pre_ranked_executable()
                .ok_or(OutputFormalError::MissingConnectedSource)?;
            let required = source
                .pre_ranked_retained_analysis_storage_v1()
                .ok_or(OutputFormalError::MissingConnectedSource)?;
            if budget.storage() < required {
                return Err(FormalResource::Accounting.into());
            }
            source
                .verify_equivalence_with_budget_v1(budget)
                .map_err(OutputFormalError::Source)?;
            Ok(source.limits.max_operations)
        }
        CanonicalOutputFormalSourceAnchorV1::Erased(source) => {
            if budget.storage() < source.retained_storage_floor_v1() {
                return Err(FormalResource::Accounting.into());
            }
            source
                .verify_equivalence(budget)
                .map_err(OutputFormalError::Source)?;
            Ok(source.original_source().limits.max_operations)
        }
    }
}

#[allow(
    clippy::result_large_err,
    reason = "Preserve inline typed failures without an unmetered error allocation."
)]
fn output_formal_reserve(
    output: &VerifiedCanonicalKernelIrModuleV12,
    budget: &mut FormalBudget<'_>,
) -> Result<usize, OutputFormalError> {
    let count = output.module().kernels.len();
    if count == 0 {
        return Err(OutputFormalError::KernelRoster);
    }
    let retained = count
        .checked_mul(std::mem::size_of::<FormalMemoryObligations>())
        .and_then(|bytes| {
            bytes.checked_add(std::mem::size_of::<CanonicalOutputGuardedFormalMemoryV1<'_>>())
        })
        .ok_or(FormalResource::Arithmetic)?;
    budget.reserve_storage(retained)?;
    budget.charge_work(count.checked_add(3).ok_or(FormalResource::Arithmetic)?)?;
    // Pay the complete ordered comparison before invoking the unchanged engine.
    for kernel in &output.module().kernels {
        let work = kernel
            .id
            .as_str()
            .len()
            .checked_add(kernel.entry.as_str().len())
            .and_then(|bytes| bytes.checked_add(3))
            .ok_or(FormalResource::Arithmetic)?;
        budget.charge_work(work)?;
    }
    Ok(retained)
}

#[allow(
    clippy::result_large_err,
    reason = "Preserve inline typed failures without an unmetered error allocation."
)]
fn output_formal_finish<'f>(
    output: &'f VerifiedCanonicalKernelIrModuleV12,
    reports: Box<[FormalMemoryObligations]>,
    max_operations: usize,
    retained: usize,
    budget: &mut FormalBudget<'_>,
) -> Result<
    (
        CanonicalOutputGuardedFormalMemoryV1<'f>,
        CanonicalOutputGuardedFormalMemoryStorageV1,
    ),
    OutputFormalError,
> {
    if reports.len() != output.module().kernels.len() || reports.is_empty() {
        return Err(OutputFormalError::KernelRoster);
    }
    for (kernel, report) in output.module().kernels.iter().zip(reports.iter()) {
        if report.kernel() != &kernel.id
            || report.entry() != &kernel.entry
            || !report.inter_invocation_conflicts().is_empty()
        {
            return Err(OutputFormalError::KernelRoster);
        }
    }
    budget.charge_work(1)?;
    Ok((
        CanonicalOutputGuardedFormalMemoryV1 {
            output,
            reports,
            max_operations,
            retained,
        },
        CanonicalOutputGuardedFormalMemoryStorageV1(retained),
    ))
}

/// Replays the actual source, then freshly analyzes the exact borrowed output
/// using that source's retained operation limit and the unchanged guarded rule.
/// All new wrapper work and known header/row storage use the supplied budget.
/// Formal/guarded engine scratch and nested report payloads retain their existing
/// bounded domain; this API introduces no broader metering or allocator claim.
/// Caller-owned input/source/sibling backing must already be reserved. Success
/// restores the entry floor and returns an unreserved added report receipt.
/// This does not prove source-to-output lineage or authenticate either input.
#[allow(
    clippy::result_large_err,
    reason = "Preserve inline typed failures without an unmetered error allocation."
)]
pub fn analyze_canonical_output_guarded_formal_memory_v1<'f>(
    output: &'f VerifiedCanonicalKernelIrModuleV12,
    source: CanonicalOutputFormalSourceAnchorV1<'_>,
    budget: &mut FormalBudget<'_>,
) -> Result<
    (
        CanonicalOutputGuardedFormalMemoryV1<'f>,
        CanonicalOutputGuardedFormalMemoryStorageV1,
    ),
    OutputFormalError,
> {
    output_formal_scoped(budget, |budget| {
        budget.charge_work(4)?;
        let max_operations = output_formal_source_limit(source, budget)?;
        let retained = output_formal_reserve(output, budget)?;
        let reports = derive_checked_output_guarded_obligations_v1(output, max_operations)
            .map_err(OutputFormalError::Formal)?;
        output_formal_finish(output, reports, max_operations, retained, budget)
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_kernel_ir::{
        BasicBlock, BlockId, CanonicalKernelIrWorkBudgetV1 as Work, Function, Kernel, LaunchDomain,
        LaunchExtent, Module, Signature, Terminator,
    };
    const WORK: usize = 1_000_000_000;
    const STORAGE: usize = 512 * 1024 * 1024;
    const FLOOR: usize = 71;

    fn graph(count: usize) -> (VerifiedCanonicalKernelIrModuleV12, usize) {
        let mut module = Module::new("output-formal-adapter");
        for i in 0..count {
            let name = format!("root_{i}");
            let mut block = BasicBlock::new(BlockId(0));
            block.terminator = Some(Terminator::Return { values: vec![] });
            module.functions.push(Function::kernel_entry(
                name.as_str(),
                Signature::new(vec![], vec![]),
                vec![],
                vec![block],
            ));
            module.kernels.push(Kernel::new(
                name.as_str(),
                name.as_str(),
                LaunchDomain::D1 {
                    x: LaunchExtent::Static(1),
                },
            ));
        }
        let mut work = Work::new(WORK);
        let mut budget = FormalBudget::new(&mut work, STORAGE);
        let (output, receipt) =
            VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(
                &module,
                &mut budget,
            )
            .unwrap();
        (output, receipt.retained_storage())
    }

    #[test]
    fn output_formal_roster_rejects_empty_omitted_reordered_and_wrong_entry_reports() {
        let (output, output_storage) = graph(2);
        let (empty, empty_storage) = graph(0);
        let floor = FLOOR + output_storage + empty_storage;
        let mut work = Work::new(WORK);
        let mut budget = FormalBudget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        assert!(matches!(
            output_formal_reserve(&empty, &mut budget),
            Err(OutputFormalError::KernelRoster)
        ));
        assert_eq!(
            (budget.work(), budget.storage(), budget.peak_storage()),
            (0, floor, floor)
        );
        for case in 0..3 {
            let result = output_formal_scoped(&mut budget, |budget| {
                let retained = output_formal_reserve(&output, budget)?;
                let mut reports =
                    derive_checked_output_guarded_obligations_v1(&output, 100).unwrap();
                match case {
                    0 => reports = reports.into_vec().into_iter().take(1).collect(),
                    1 => reports.swap(0, 1),
                    _ => {
                        let mut other = output.module().clone();
                        let first = other.kernels[0].entry.clone();
                        other.kernels[0].entry = other.kernels[1].entry.clone();
                        other.kernels[1].entry = first;
                        let (other, receipt) = VerifiedCanonicalKernelIrModuleV12::from_module_ref_with_verification_budget_v12(&other, budget).unwrap();
                        budget.reserve_storage(receipt.retained_storage()).unwrap();
                        reports =
                            derive_checked_output_guarded_obligations_v1(&other, 100).unwrap();
                        drop(other);
                    }
                }
                output_formal_finish(&output, reports, 100, retained, budget)
            });
            assert!(matches!(result, Err(OutputFormalError::KernelRoster)));
            assert_eq!(budget.storage(), floor);
        }
    }

    #[test]
    fn output_formal_first_header_rows_reservation_is_exact_and_fail_closed() {
        let (output, input_storage) = graph(2);
        let floor = FLOOR + input_storage;
        let expected = std::mem::size_of::<CanonicalOutputGuardedFormalMemoryV1<'_>>()
            + 2 * std::mem::size_of::<FormalMemoryObligations>();
        let mut work = Work::new(WORK);
        work.charge_work(13).unwrap();
        let mut budget = FormalBudget::new(&mut work, floor);
        budget.reserve_storage(floor).unwrap();
        let result = output_formal_scoped(&mut budget, |budget| {
            budget.charge_work(4)?;
            output_formal_reserve(&output, budget)
        });
        let Err(OutputFormalError::Resource(FormalResource::Storage(error))) = result else {
            panic!("exact first report header/row reservation")
        };
        assert_eq!((error.actual(), error.limit()), (floor + expected, floor));
        assert_eq!(
            (
                budget.work(),
                budget.storage(),
                budget.peak_storage(),
                budget.failed_storage()
            ),
            (17, floor, floor, Some(floor + expected))
        );
    }

    #[test]
    fn output_formal_exact_work_peak_and_final_work_short_preserve_floor() {
        let (output, input_storage) = graph(2);
        let floor = FLOOR + input_storage;
        let run = |work_limit, storage_limit| {
            let mut work = Work::new(work_limit);
            work.charge_work(13).unwrap();
            let values = {
                let mut budget = FormalBudget::new(&mut work, storage_limit);
                budget.reserve_storage(floor).unwrap();
                let ledger = budget.work_ledger_identity_v1();
                let result = output_formal_scoped(&mut budget, |budget| {
                    budget.charge_work(4)?;
                    let retained = output_formal_reserve(&output, budget)?;
                    let reports =
                        derive_checked_output_guarded_obligations_v1(&output, 100).unwrap();
                    output_formal_finish(&output, reports, 100, retained, budget)
                })
                .map(|(report, receipt)| {
                    assert!(std::ptr::eq(report.output(), &output));
                    assert_eq!(report.max_operations(), 100);
                    assert_eq!(report.retained_storage(), receipt.retained_storage());
                    assert_eq!(report.kernels().len(), 2);
                    assert!(!report.grants_artifact_or_launch_authority());
                    drop(report);
                });
                assert_eq!(budget.storage(), floor);
                assert!(budget.work_ledger_identity_v1() == ledger);
                (
                    result,
                    budget.work(),
                    budget.peak_storage(),
                    budget.failed_storage(),
                )
            };
            (values, work.failed_work())
        };
        let ((result, used, peak, failure), failed_work) = run(WORK, STORAGE);
        result.unwrap();
        assert_eq!((failure, failed_work), (None, None));
        let ((result, exact_work, exact_peak, failure), failed_work) = run(used, peak);
        result.unwrap();
        assert_eq!(
            (exact_work, exact_peak, failure, failed_work),
            (used, peak, None, None)
        );
        let ((result, short_work, short_peak, failure), failed_work) = run(used - 1, peak);
        let Err(OutputFormalError::Resource(FormalResource::Work(error))) = result else {
            panic!("final adapter work charge")
        };
        assert_eq!((error.actual(), error.limit()), (used, used - 1));
        assert_eq!(
            (short_work, short_peak, failure, failed_work),
            (used - 1, peak, None, Some(used))
        );
    }

    #[test]
    fn output_formal_scope_drops_rows_and_preserves_sibling_on_error_and_panic() {
        let (output, input_storage) = graph(2);
        let sibling = vec![0x53u8; 37];
        let floor = FLOOR + input_storage + std::mem::size_of_val(&sibling) + sibling.capacity();
        let mut work = Work::new(WORK);
        let mut budget = FormalBudget::new(&mut work, STORAGE);
        budget.reserve_storage(floor).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        for panic in [false, true] {
            let result: Result<(), OutputFormalError> =
                output_formal_scoped(&mut budget, |budget| {
                    let retained = output_formal_reserve(&output, budget)?;
                    let reports =
                        derive_checked_output_guarded_obligations_v1(&output, 100).unwrap();
                    let (report, _) =
                        output_formal_finish(&output, reports, 100, retained, budget)?;
                    assert_eq!(report.kernels().len(), 2);
                    if panic {
                        panic!("after prepaid actual report allocation");
                    }
                    drop(report);
                    Err(OutputFormalError::KernelRoster)
                });
            assert!(matches!(
                (panic, result),
                (true, Err(OutputFormalError::Panicked))
                    | (false, Err(OutputFormalError::KernelRoster))
            ));
            assert_eq!(budget.storage(), floor);
            assert!(budget.work_ledger_identity_v1() == ledger);
            assert_eq!(sibling, [0x53; 37]);
        }
    }

    #[test]
    fn output_formal_scope_refuses_foreign_ledger_without_refunding_it() {
        let mut work = Work::new(WORK);
        let mut foreign_work = Work::new(WORK);
        let mut budget = FormalBudget::new(&mut work, STORAGE);
        let mut foreign = FormalBudget::new(&mut foreign_work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        foreign.reserve_storage(FLOOR + 7).unwrap();
        let ledger = budget.work_ledger_identity_v1();
        let foreign_ledger = foreign.work_ledger_identity_v1();
        let result = output_formal_scoped(&mut budget, |budget| {
            budget.charge_work(4)?;
            std::mem::swap(budget, &mut foreign);
            Ok(())
        });
        assert!(matches!(
            result,
            Err(OutputFormalError::Resource(FormalResource::Accounting))
        ));
        assert!(budget.work_ledger_identity_v1() == foreign_ledger);
        assert_eq!(budget.storage(), FLOOR + 7);
        std::mem::swap(&mut budget, &mut foreign);
        assert!(budget.work_ledger_identity_v1() == ledger);
        assert_eq!((budget.storage(), budget.work()), (FLOOR, 4));
        assert_eq!(foreign.storage(), FLOOR + 7);
    }

    #[test]
    fn output_formal_panic_payload_drop_cannot_skip_floor_restoration() {
        struct Payload;
        impl Drop for Payload {
            fn drop(&mut self) {
                panic!("payload drop");
            }
        }
        let mut work = Work::new(WORK);
        let mut budget = FormalBudget::new(&mut work, STORAGE);
        budget.reserve_storage(FLOOR).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: Result<(), OutputFormalError> = output_formal_scoped(&mut budget, |budget| {
                budget.reserve_storage(17)?;
                std::panic::panic_any(Payload);
            });
        }));
        assert!(result.is_err());
        assert_eq!(budget.storage(), FLOOR);
        assert_eq!(budget.peak_storage(), FLOOR + 17);
    }
}
