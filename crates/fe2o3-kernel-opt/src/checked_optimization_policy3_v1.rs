//! Versioned fixed policy-3 execution followed by the existing independent
//! V12 semantic checker. Historical policy-2 endpoints are not redirected here.

use crate::KernelIrCheckedOptimizationErrorV1 as Error;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    VerifiedCanonicalKernelIrModuleV12 as Owner,
};
use fe2o3_pliron::{
    CheckedNeutralKernelIrOwnerPolicy3V1, optimize_native_neutral_kernel_ir_policy3_v1,
};

/// Imports and executes the trusted eight-pass policy on this exact B, extracts
/// its actual O once, and independently checks the observed complete relation.
/// One ledger covers every stage. The caller reserves B beforehand and reserves
/// the returned owner receipt before further allocation; failure restores the
/// entry floor after dropping all private candidate/session state. This is not
/// target binding, ranked/formal authority, or backend default activation.
///
/// ```compile_fail
/// use fe2o3_pliron::{CheckedNeutralKernelIrOwnerV1, CheckedNeutralKernelIrOwnerPolicy3V1};
/// fn historical(owner: CheckedNeutralKernelIrOwnerPolicy3V1) -> CheckedNeutralKernelIrOwnerV1 {
///     owner
/// }
/// ```
pub fn optimize_checked_canonical_kernel_ir_policy3_v1(
    input: &Owner,
    budget: &mut Budget<'_>,
) -> Result<CheckedNeutralKernelIrOwnerPolicy3V1, Error> {
    let observed =
        optimize_native_neutral_kernel_ir_policy3_v1(input, budget).map_err(Error::Observation)?;
    let retained = observed.storage().retained_storage();
    budget.reserve_storage(retained).map_err(Error::Resource)?;
    let input_check = budget
        .charge_work(1)
        .map_err(Error::Resource)
        .and_then(|()| {
            if std::ptr::eq(input, observed.input()) {
                Ok(())
            } else {
                Err(Error::InputOwner)
            }
        });
    if let Err(error) = input_check {
        drop(observed);
        budget.release_storage(retained).map_err(Error::Resource)?;
        return Err(error);
    }
    observed
        .try_check_and_finish_v1(budget)
        .map_err(Error::Check)
}
