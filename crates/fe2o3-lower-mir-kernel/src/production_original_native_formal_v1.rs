//! Borrowed original-N formal extraction, never E/I or compatibility admission.
use super::{
    ProductionSemanticKirErrorV1, ProductionSemanticKirOwnerV1,
    ProductionUnitLocalErasedSourceOwnerV1, derive_checked_output_guarded_obligations_v1,
};
use crate::ProductionFormalMemoryErrorV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, FormalMemoryObligations,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use std::{
    fmt,
    mem::size_of,
    panic::{AssertUnwindSafe, catch_unwind},
};

#[path = "production_original_unit_local_formal_v1.rs"]
mod unit_local;
#[cfg(test)]
pub(super) use unit_local::exercise_call_joins_v1;

/// Refusal of source custody, the fixed formal policy, or its wrapper accounting.
#[derive(Debug)]
pub enum OriginalNativeFormalMemoryErrorV1 {
    /// Shared work or logical-storage accounting failed.
    Resource(Resource),
    /// Complete retained source replay failed.
    Source(Box<ProductionSemanticKirErrorV1>),
    /// Existing closed formal/structural-guarded extraction failed.
    Formal(Box<ProductionFormalMemoryErrorV1>),
    /// The source has no actual connected V12 N owner.
    MissingConnectedSource,
    /// Fresh reports do not cover the exact nonempty N kernel roster.
    KernelRoster,
    /// A same-owner original-N call could not be joined to its live source token.
    CallRelation(&'static str),
    /// An internal analysis unwound; no witness is returned.
    Panicked,
}
type E = OriginalNativeFormalMemoryErrorV1;
impl From<Resource> for E {
    fn from(error: Resource) -> Self {
        Self::Resource(error)
    }
}
impl fmt::Display for E {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "original native formal analysis: {self:?}")
    }
}
impl std::error::Error for E {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Source(error) => Some(error.as_ref()),
            Self::Formal(error) => Some(error.as_ref()),
            _ => None,
        }
    }
}

/// Known owned report-row and wrapper storage, excluding the borrowed source.
/// Formal/guarded report payloads retain the existing separate bounded domain.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OriginalNativeFormalMemoryStorageV1(usize);
impl OriginalNativeFormalMemoryStorageV1 {
    /// Reserve this returned receipt before further controlled allocation.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Fresh fixed-policy obligations borrowing one exact original V12 N owner.
/// This is not runtime launch safety, a signed proof, or publication authority.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::OriginalNativeFormalMemoryV1;
/// fn copy(value: OriginalNativeFormalMemoryV1<'_>) { let _ = value.clone(); }
/// ```
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionSemanticKirOwnerV1, OriginalNativeFormalMemoryV1,
///     analyze_original_native_formal_memory_v1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn detach(source: ProductionSemanticKirOwnerV1, budget: &mut Budget<'_>)
///     -> OriginalNativeFormalMemoryV1<'static> {
///     analyze_original_native_formal_memory_v1(&source, budget).unwrap().0
/// }
/// ```
pub struct OriginalNativeFormalMemoryV1<'n> {
    original: &'n Graph,
    reports: Box<[FormalMemoryObligations]>,
    retained: usize,
}
impl<'n> OriginalNativeFormalMemoryV1<'n> {
    /// Exact original V12 N; never the compatibility identity or erased E.
    pub const fn original(&self) -> &'n Graph {
        self.original
    }
    /// Fresh reports in the actual original module's kernel order.
    pub fn kernels(&self) -> &[FormalMemoryObligations] {
        &self.reports
    }
    /// The live known wrapper/report-row reservation.
    pub const fn retained_storage(&self) -> usize {
        self.retained
    }
    /// This borrowed analysis grants no artifact or runtime authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn scoped<'w, T>(
    budget: &mut Budget<'w>,
    run: impl FnOnce(&mut Budget<'w>) -> Result<T, E>,
) -> Result<T, E> {
    let floor = budget.storage();
    let ledger = budget.work_ledger_identity_v1();
    let mut deferred_panic = None;
    let result = match catch_unwind(AssertUnwindSafe(|| run(budget))) {
        Ok(result) => result,
        Err(payload) => {
            deferred_panic = Some(payload);
            Err(E::Panicked)
        }
    };
    if budget.work_ledger_identity_v1() != ledger || budget.storage() < floor {
        drop(result);
        drop(deferred_panic);
        return Err(Resource::Accounting.into());
    }
    if let Err(error) = budget.release_storage(budget.storage() - floor) {
        drop(result);
        drop(deferred_panic);
        return Err(error.into());
    }
    // A caught payload's destructor must not skip restoration of our floor.
    drop(deferred_panic);
    result
}

fn reserve_reports(original: &Graph, budget: &mut Budget<'_>) -> Result<usize, E> {
    let count = original.module().kernels.len();
    budget.charge_work(count.checked_add(3).ok_or(Resource::Arithmetic)?)?;
    if count == 0 {
        return Err(E::KernelRoster);
    }
    let retained = count
        .checked_mul(size_of::<FormalMemoryObligations>())
        .and_then(|bytes| bytes.checked_add(size_of::<OriginalNativeFormalMemoryV1<'_>>()))
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(retained)?;
    Ok(retained)
}

fn finish_reports<'n>(
    original: &'n Graph,
    reports: Box<[FormalMemoryObligations]>,
    retained: usize,
    budget: &mut Budget<'_>,
) -> Result<
    (
        OriginalNativeFormalMemoryV1<'n>,
        OriginalNativeFormalMemoryStorageV1,
    ),
    E,
> {
    if reports.len() != original.module().kernels.len() {
        return Err(E::KernelRoster);
    }
    for (kernel, report) in original.module().kernels.iter().zip(reports.iter()) {
        budget.charge_work(
            kernel
                .id
                .as_str()
                .len()
                .checked_add(kernel.entry.as_str().len())
                .and_then(|n| n.checked_add(3))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if report.kernel() != &kernel.id || report.entry() != &kernel.entry {
            return Err(E::KernelRoster);
        }
    }
    Ok((
        OriginalNativeFormalMemoryV1 {
            original,
            reports,
            retained,
        },
        OriginalNativeFormalMemoryStorageV1(retained),
    ))
}

fn extract<'n>(
    original: &'n Graph,
    max_operations: usize,
    budget: &mut Budget<'_>,
) -> Result<
    (
        OriginalNativeFormalMemoryV1<'n>,
        OriginalNativeFormalMemoryStorageV1,
    ),
    E,
> {
    let retained = reserve_reports(original, budget)?;
    // The existing formal/guarded engines own their separately bounded payload
    // allocation/work domain. Only known returned rows/header are charged here.
    let reports = derive_checked_output_guarded_obligations_v1(original, max_operations)
        .map_err(|error| E::Formal(Box::new(error)))?;
    finish_reports(original, reports, retained, budget)
}

/// Replays a retained connected Direct source and extracts fresh obligations
/// from its exact V12 N under the existing closed structural-guarded policy.
/// Incoming source/capture reservations remain caller-owned on every exit.
/// Success returns its additional row/header receipt unreserved. No broader
/// ranked/collective exception, graph clone, or semantic import is performed.
pub fn analyze_original_native_formal_memory_v1<'n>(
    source: &'n ProductionSemanticKirOwnerV1,
    budget: &mut Budget<'_>,
) -> Result<
    (
        OriginalNativeFormalMemoryV1<'n>,
        OriginalNativeFormalMemoryStorageV1,
    ),
    E,
> {
    scoped(budget, |budget| {
        budget.charge_work(4)?;
        let original = source
            .pre_ranked_executable()
            .ok_or(E::MissingConnectedSource)?;
        let floor = source
            .pre_ranked_retained_analysis_storage_v1()
            .ok_or(E::MissingConnectedSource)?;
        if budget.storage() < floor {
            return Err(Resource::Accounting.into());
        }
        source
            .verify_equivalence_with_budget_v1(budget)
            .map_err(|error| E::Source(Box::new(error)))?;
        extract(original, source.limits.max_operations, budget)
    })
}

/// Replays retained UnitLocal source/ranked/N-to-E custody but analyzes only
/// `original_source().executable()`. E/I reports never replace original N.
/// Only exact original call gaps backed by live source-qualified silent Unit
/// deletion are discharged; the generic formal engine's purity rule is unchanged.
/// Storage, fixed policy, and separate engine domains match the Direct entry.
///
/// ```compile_fail
/// use fe2o3_lower_mir_kernel::{ProductionUnitLocalErasedSourceOwnerV1,
///     OriginalNativeFormalMemoryV1, analyze_original_unit_local_formal_memory_v1};
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn detach(source: ProductionUnitLocalErasedSourceOwnerV1, budget: &mut Budget<'_>)
///     -> OriginalNativeFormalMemoryV1<'static> {
///     analyze_original_unit_local_formal_memory_v1(&source, budget).unwrap().0
/// }
/// ```
pub fn analyze_original_unit_local_formal_memory_v1<'n>(
    source: &'n ProductionUnitLocalErasedSourceOwnerV1,
    budget: &mut Budget<'_>,
) -> Result<
    (
        OriginalNativeFormalMemoryV1<'n>,
        OriginalNativeFormalMemoryStorageV1,
    ),
    E,
> {
    scoped(budget, |budget| {
        budget.charge_work(4)?;
        if budget.storage() < source.retained_storage_floor_v1() {
            return Err(Resource::Accounting.into());
        }
        let original = source.original_source().executable();
        // Escaping rows/header are prepaid outside the checked-erasure scope.
        // That scope performs complete source/N/E replay exactly once.
        let retained = reserve_reports(original, budget)?;
        let reports = source
            .with_checked_erasure_v1(budget, |deletion, budget| {
                Ok(scoped(budget, |budget| {
                    unit_local::derive(original, deletion, budget)
                }))
            })
            .map_err(|error| E::Source(Box::new(error)))??;
        finish_reports(original, reports, retained, budget)
    })
}

#[cfg(test)]
#[path = "production_original_native_formal_v1_tests.rs"]
mod tests;
