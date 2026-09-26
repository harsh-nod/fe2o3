//! Fixed scalar execution and independent adoption of its actual V18 output.
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, StorageLayoutLimitsV1,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};
use fe2o3_pliron::{
    CheckedNeutralKernelIrOwnerV18, KirCheckedNeutralOptimizationErrorV1,
    KirNeutralOptimizationErrorV18, optimize_neutral_kernel_ir_v18,
};
use std::{error::Error, fmt};

#[derive(Debug)]
pub enum KernelIrCheckedOptimizationErrorV18 {
    Observation(KirNeutralOptimizationErrorV18),
    Check(KirCheckedNeutralOptimizationErrorV1),
    Resource(Resource),
    InputOwner,
}
impl fmt::Display for KernelIrCheckedOptimizationErrorV18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Observation(error) => error.fmt(f),
            Self::Check(error) => error.fmt(f),
            Self::Resource(error) => error.fmt(f),
            Self::InputOwner => f.write_str("V18 optimizer substituted its input owner"),
        }
    }
}
impl Error for KernelIrCheckedOptimizationErrorV18 {}

/// Executes the literal eight-pass scalar policy and checks its complete V18
/// occurrence relation. Source/ranked/formal and final target admission remain
/// distinct required stages. No caller-selected passes or fallback exist.
/// Reserve input custody before calling, and reserve the returned checked-owner
/// transfer before subsequent controlled allocation. All work shares this ledger.
pub fn optimize_checked_canonical_kernel_ir_v18(
    input: &Owner,
    layouts: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<CheckedNeutralKernelIrOwnerV18, KernelIrCheckedOptimizationErrorV18> {
    use KernelIrCheckedOptimizationErrorV18 as E;
    let observed =
        optimize_neutral_kernel_ir_v18(input, layouts, budget).map_err(E::Observation)?;
    let retained = observed.storage().retained_storage();
    budget.reserve_storage(retained).map_err(E::Resource)?;
    let join = budget.charge_work(1).map_err(E::Resource).and_then(|()| {
        if std::ptr::eq(input, observed.input()) {
            Ok(())
        } else {
            Err(E::InputOwner)
        }
    });
    if let Err(error) = join {
        drop(observed);
        budget.release_storage(retained).map_err(E::Resource)?;
        return Err(error);
    }
    observed.try_check_and_finish_v18(budget).map_err(E::Check)
}

#[cfg(test)]
#[path = "checked_optimization_v18_resources_tests.rs"]
mod resource_tests;
#[cfg(test)]
#[path = "checked_optimization_v18_tests.rs"]
mod tests;
