use std::{collections::BTreeSet, error::Error, fmt};

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1, CanonicalKernelIrVerificationResourceErrorV1,
    DiagnosticCode, MeteredKernelIrVerificationErrorV1, Module, TargetCapability,
    VerificationDiagnosticCollectorV1, VerificationErrors, VerifiedKernelIrModuleV1,
    first_excessive_type_depth_location_with_visits_v1, verify_depth_bounded_module_with_budget_v1,
};

/// A semantic or resource rejection from borrowed, budgeted verification.
#[derive(Debug, Eq, PartialEq)]
pub enum BorrowedKernelIrVerificationErrorV1 {
    /// The complete, ordered diagnostics produced by the shared verifier.
    Verification(VerificationErrors),
    /// The next traversal, allocation, or diagnostic publication was denied.
    Resource(CanonicalKernelIrVerificationResourceErrorV1),
}

impl fmt::Display for BorrowedKernelIrVerificationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Verification(error) => error.fmt(formatter),
            Self::Resource(error) => error.fmt(formatter),
        }
    }
}

impl Error for BorrowedKernelIrVerificationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Verification(error) => Some(error),
            Self::Resource(error) => Some(error),
        }
    }
}

impl From<CanonicalKernelIrVerificationResourceErrorV1> for BorrowedKernelIrVerificationErrorV1 {
    fn from(error: CanonicalKernelIrVerificationResourceErrorV1) -> Self {
        Self::Resource(error)
    }
}

impl From<MeteredKernelIrVerificationErrorV1> for BorrowedKernelIrVerificationErrorV1 {
    fn from(error: MeteredKernelIrVerificationErrorV1) -> Self {
        match error {
            MeteredKernelIrVerificationErrorV1::Verification(error) => Self::Verification(error),
            MeteredKernelIrVerificationErrorV1::Resource(error) => Self::Resource(error),
        }
    }
}

/// Verifies the exact borrowed module under the caller's work/storage ledger.
///
/// This performs the same type-depth preflight and shared semantic verification
/// as `verify_module_ref`, with optional target-capability validation. Resource
/// exhaustion can precede semantic diagnostics. No module clone, serialization,
/// or caller-supplied verification claim is involved.
///
/// On every ordinary `Result` path, verifier-local scratch returns to the input
/// storage checkpoint. Accepted cumulative work, peak storage, and rejected
/// reservation history remain observable through `budget`; the shared work
/// meter also retains its first rejected charge. Returned diagnostics transfer
/// to the caller, as in canonical verification, and are included in the peak.
/// Existing meter overflow uses `usize::MAX` as a saturation sentinel, not an
/// exact mathematical attempted sum. This API does not reset either meter.
pub fn verify_module_ref_with_budget_v1<'module>(
    module: &'module Module,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerifiedKernelIrModuleV1<'module>, BorrowedKernelIrVerificationErrorV1> {
    let checkpoint = budget.storage_checkpoint();
    let result = verify_borrowed_inner_v1(module, supported_capabilities, budget);
    budget.rollback_storage(checkpoint)?;
    result
}

fn verify_borrowed_inner_v1<'module>(
    module: &'module Module,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
    budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>,
) -> Result<VerifiedKernelIrModuleV1<'module>, BorrowedKernelIrVerificationErrorV1> {
    if let Some(location) =
        first_excessive_type_depth_location_with_visits_v1(module, &mut |amount| {
            budget.charge_work(amount)
        })?
    {
        let mut diagnostics = VerificationDiagnosticCollectorV1::materialize(1, budget)?;
        let emitted = diagnostics.emit(
            location,
            DiagnosticCode::ResourceLimit,
            crate::PUBLIC_VERIFIER_TYPE_DEPTH_MESSAGE_V1.len(),
            format_args!("{}", crate::PUBLIC_VERIFIER_TYPE_DEPTH_MESSAGE_V1),
            budget,
        );
        if let Err(error) = emitted {
            let _ = diagnostics.abandon(budget);
            return Err(error.into());
        }
        let diagnostics = diagnostics.finish_materialized(budget)?;
        return Err(BorrowedKernelIrVerificationErrorV1::Verification(
            VerificationErrors::from_sorted_diagnostics_v1(diagnostics),
        ));
    }
    verify_depth_bounded_module_with_budget_v1(module, supported_capabilities, budget)
        .map_err(Into::into)
}

#[cfg(test)]
#[path = "verification_borrowed_v1_tests.rs"]
mod tests;
