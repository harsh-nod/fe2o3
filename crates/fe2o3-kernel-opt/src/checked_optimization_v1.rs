//! Fixed observed V12 execution followed by independent local-rule checking.
//!
//! For an already target-bound input B this checks B -> O. It does not perform
//! or certify neutral-to-target binding, reinterpret historical V2/V3 reports,
//! or activate a backend, serialized receipt, or final optimized-graph proof.

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::{
    CheckedNeutralKernelIrOwnerV1, KirCheckedNeutralOptimizationErrorV1,
    KirNeutralOptimizationErrorV1, KirNeutralOptimizationOutputV1,
    optimize_native_neutral_kernel_ir_v1,
};
use std::{error::Error, fmt};

/// Failures do not return an unchecked output or a substitute input graph.
#[derive(Debug)]
pub enum KernelIrCheckedOptimizationErrorV1 {
    Observation(KirNeutralOptimizationErrorV1),
    InputOwner,
    Resource(Resource),
    Check(KirCheckedNeutralOptimizationErrorV1),
}

impl fmt::Display for KernelIrCheckedOptimizationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observation(error) => error.fmt(formatter),
            Self::InputOwner => formatter.write_str("observed optimizer input owner differs"),
            Self::Resource(error) => error.fmt(formatter),
            Self::Check(error) => error.fmt(formatter),
        }
    }
}

impl Error for KernelIrCheckedOptimizationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Observation(error) => Some(error),
            Self::InputOwner => None,
            Self::Resource(error) => Some(error),
            Self::Check(error) => Some(error),
        }
    }
}

/// Run the fixed seven-pass scalar/CFG roster on this exact admitted V12 input
/// and consume its actual observed output through the independent checker.
/// The checker preserves exact module, kernel, capability, and function
/// metadata. It certifies only its named input-to-final local rewrite rules,
/// not general semantic equivalence or the provenance of target binding.
///
/// The caller must already reserve the input owner's transferred receipt on
/// this ledger. Import, live session growth, extraction, observation, and
/// checked adoption use the same ledger without resets. Every returned exit
/// restores the entry storage floor after dropping rejected candidates and
/// session state. Accepted work, peak, and first-failure history remain.
/// Reserve the returned owner's `storage().retained_storage()` before another
/// controlled allocation, and release it only after that owner is dropped.
/// There is no additional adapter-owned payload or transfer receipt.
///
/// The native bridge profile is structural accounting, not NativeSource proof
/// admission. This API accepts neither legacy canonical bytes nor V2/V3
/// optimizer reports. It does not authorize ranked/formal checks, compiler
/// artifacts, launches, or the final target backend.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{
///     CanonicalKernelIrVerificationResourceBudgetV1, VerifiedCanonicalKernelIrV10,
/// };
/// use fe2o3_kernel_opt::optimize_checked_canonical_kernel_ir_v1;
/// fn legacy(input: &VerifiedCanonicalKernelIrV10,
///           budget: &mut CanonicalKernelIrVerificationResourceBudgetV1<'_>) {
///     let _ = optimize_checked_canonical_kernel_ir_v1(input, budget);
/// }
/// ```
pub fn optimize_checked_canonical_kernel_ir_v1(
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<CheckedNeutralKernelIrOwnerV1, KernelIrCheckedOptimizationErrorV1> {
    let observed = optimize_native_neutral_kernel_ir_v1(input, budget)
        .map_err(KernelIrCheckedOptimizationErrorV1::Observation)?;
    finish_observed_v1(input, observed, budget)
}

fn finish_observed_v1(
    input: &Owner,
    observed: KirNeutralOptimizationOutputV1<'_>,
    budget: &mut Budget<'_>,
) -> Result<CheckedNeutralKernelIrOwnerV1, KernelIrCheckedOptimizationErrorV1> {
    let retained = observed.storage().retained_storage();
    budget
        .reserve_storage(retained)
        .map_err(KernelIrCheckedOptimizationErrorV1::Resource)?;
    // One fixed pointer-identity comparison, after admitting the observation.
    let input_check = budget
        .charge_work(1)
        .map_err(KernelIrCheckedOptimizationErrorV1::Resource)
        .and_then(|()| {
            if std::ptr::eq(input, observed.input()) {
                Ok(())
            } else {
                Err(KernelIrCheckedOptimizationErrorV1::InputOwner)
            }
        });
    if let Err(error) = input_check {
        drop(observed);
        budget
            .release_storage(retained)
            .map_err(KernelIrCheckedOptimizationErrorV1::Resource)?;
        return Err(error);
    }
    // Consuming adoption releases the observed reservation on every exit and
    // transfers only a successfully checked owner. No fallback is available.
    observed
        .try_check_and_finish_v1(budget)
        .map_err(KernelIrCheckedOptimizationErrorV1::Check)
}

#[cfg(test)]
#[path = "checked_optimization_v1_tests.rs"]
mod tests;
