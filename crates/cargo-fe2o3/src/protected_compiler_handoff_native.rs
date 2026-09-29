//! Metered use of the same parent invocation and sealed V3 descriptor.
use super::ParentRustcInvocationCustody;
use fe2o3_build_authority::CompilerClosureV2;
use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2 as Error;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_rustc_invocation::RustcInvocationDescriptorV3;
use std::{
    mem::size_of,
    os::fd::{BorrowedFd, OwnedFd},
};

// Fixed descriptor checks, no path lookup, allocation or retries. Each syscall
// costs 1024 logical work; the frame covers stat/flags and bounded error results.
const INPUT_FRAME: usize = 4096;
const DIRECTORY_WORK: usize = 8 + 3 * 1024;
const TRANSFER_WORK: usize = 8 + 6 * 1024;
const STDIO_WORK: usize = 6 * 1024;

impl ParentRustcInvocationCustody {
    /// Prepay before transferring the enclosing attempt's account into native
    /// readiness custody. Capture/spawn/provenance remain the wrapper's duties.
    pub(crate) fn native_retained_storage(&self) -> Result<usize, Error> {
        // The capability's conservative quote covers a full decoded descriptor
        // and its backing. Use the same bound for the independent parent copy.
        // Self includes the fixed-size stdio and pinned-directory owners. Their
        // descriptors stay live until custody drops; kernel memory is not metered.
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

    /// Borrows the actual prepared directory on the enclosing attempt's account.
    /// Keep this custody and its reservation live through staging and transfer.
    /// This neither joins the object to cwd text nor approves its contents. The
    /// caller excludes descriptor/flag mutation and prepays duplication/cleanup.
    pub(crate) fn native_working_directory(
        &self,
        b: &mut Budget<'_>,
    ) -> Result<BorrowedFd<'_>, Error> {
        b.with_prepaid_scope(
            self.native_retained_storage()?,
            8,
            DIRECTORY_WORK,
            INPUT_FRAME,
            |_| {
                self.working_directory
                    .native_source()
                    .map_err(|error| input_error("parent working directory", error))
            },
        )
    }

    /// Checks a duplicate staged from `native_working_directory` on the same
    /// account. Its OwnedFd charge must coexist with the retained parent floor.
    /// The adapter owns duplication provenance: matching object/flags alone is
    /// not proof of a shared open-file description or protected admission.
    #[allow(dead_code)] // Native coordinator staging is a separate integration.
    pub(crate) fn validate_native_working_directory_transfer(
        &self,
        transfer: BorrowedFd<'_>,
        b: &mut Budget<'_>,
    ) -> Result<(), Error> {
        let floor = self
            .native_retained_storage()?
            .checked_add(size_of::<OwnedFd>())
            .ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, TRANSFER_WORK, INPUT_FRAME, |_| {
            self.working_directory
                .validate_native_transfer(transfer)
                .map_err(|error| input_error("parent working directory transfer", error))
        })
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
        let work = floor.checked_add(STDIO_WORK).ok_or(Resource::Arithmetic)?;
        b.with_prepaid_scope(floor, 8, work, INPUT_FRAME, |b| {
            self.native_working_directory(b)?;
            if let Some(stdio) = &self.stdio {
                stdio
                    .revalidate()
                    .map_err(|error| input_error("parent stdio", error))?;
            }
            self.capability.revalidate_native(b)?;
            let selected = self.capability.descriptor();
            if self.invocation.descriptor() != selected || observed != selected {
                return Err(Error::Rejected("parent rustc invocation differs"));
            }
            Ok(*selected.compiler_closure())
        })
    }
}

fn input_error(operation: &'static str, error: std::io::Error) -> Error {
    Error::Io {
        operation,
        errno: error.raw_os_error().unwrap_or(libc::EIO),
    }
}

#[cfg(test)]
#[path = "protected_compiler_handoff_native_tests.rs"]
mod tests;
