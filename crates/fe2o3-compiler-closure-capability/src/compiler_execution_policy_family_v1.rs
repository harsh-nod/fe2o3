//! Metered sealed-header inspection; the matching full decoder remains mandatory.
use crate::{
    CompilerExecutionPolicyCapabilityV3 as Policy,
    native_capability::{CompilerExecutionCapabilityErrorV2 as Error, ENTRY_WORK},
    sealed_image::SealedCapabilityImage,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V3 as BYTES, CompilerExecutionPolicyFamilyV1 as Family,
};
use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
use std::fs::File;

/// Inspects a borrowed, prepaid `PolicyV3::FILE_STORAGE` image. Charges the
/// existing `PolicyV3::IO_WORK` and `IO_STORAGE` envelope on the original ledger.
/// The same held descriptor is checked for seals, mode, length and identity
/// before and after a fixed positional read. No descriptor is duplicated.
///
/// This result is inert: it selects exactly one nominal constructor, which must
/// independently decode/revalidate the full image. A constructor failure is not
/// permission to retry a different family, including the equal-sized V1/V3 pair.
pub fn inspect_compiler_execution_policy_family_v1(
    file: &File,
    budget: &mut Budget<'_>,
) -> Result<Family, Error> {
    budget.with_prepaid_scope(
        Policy::FILE_STORAGE,
        ENTRY_WORK,
        Policy::IO_WORK,
        Policy::IO_STORAGE,
        |_| {
            let bytes = SealedCapabilityImage::read_borrowed_fixed::<BYTES>(file)?;
            Family::inspect_header(&bytes)
                .map_err(|_| Error::Rejected("unsupported or malformed sealed policy header"))
        },
    )
}

#[cfg(test)]
#[path = "compiler_execution_policy_family_v1_tests.rs"]
mod tests;
