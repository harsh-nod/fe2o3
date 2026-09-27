//! Metered use of the same parent invocation and sealed V3 descriptor.
use super::ParentRustcInvocationCustody;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2 as Error;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_rustc_invocation::RustcInvocationDescriptorV3;
use std::mem::size_of;

impl ParentRustcInvocationCustody {
    /// Prepay before transferring the enclosing attempt's account into native
    /// readiness custody. Capture/spawn/provenance remain the wrapper's duties.
    pub(crate) fn native_retained_storage(&self) -> Result<usize, Error> {
        // The capability's conservative quote covers a full decoded descriptor
        // and its backing. Use the same bound for the independent parent copy.
        self.capability
            .native_retained_storage()?
            .checked_mul(2)
            .and_then(|n| n.checked_add(size_of::<Self>()))
            .ok_or_else(|| Resource::Arithmetic.into())
    }

    pub(crate) fn revalidate_native(&self, b: &mut Budget<'_>) -> Result<(), Error> {
        self.match_native_invocation(self.invocation.descriptor(), b)
            .map(|_| ())
    }

    /// Compares the complete immutable descriptor, not only its closure/digest.
    /// The enclosing source owner of `observed` stays prepaid on this account.
    /// This agreement does not authenticate capture or protected execution.
    pub(crate) fn match_native_invocation(
        &self,
        observed: &RustcInvocationDescriptorV3,
        b: &mut Budget<'_>,
    ) -> Result<CompilerClosureV2, Error> {
        let floor = self.native_retained_storage()?;
        // Both comparisons are bounded by the retained descriptor: collection
        // and string length mismatches reject before traversing foreign bytes.
        // The native storage quote's 64x wire allowance covers these visits.
        b.with_prepaid_scope(floor, 8, floor, 4096, |b| {
            self.capability.revalidate_native(b)?;
            let selected = self.capability.descriptor();
            if self.invocation.descriptor() != selected || observed != selected {
                return Err(Error::Rejected("parent rustc invocation differs"));
            }
            Ok(*selected.compiler_closure())
        })
    }
}

#[cfg(test)]
#[path = "protected_compiler_handoff_native_tests.rs"]
mod tests;
