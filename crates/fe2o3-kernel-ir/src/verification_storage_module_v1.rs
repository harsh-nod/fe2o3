//! Storage-aware structural/CFG/type admission through the shared verifier.
//! Source validity, initialization, provenance and runtime safety remain unproved.

use std::collections::BTreeSet;

use crate::{
    BorrowedKernelIrVerificationErrorV1 as Error,
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError,
    MeteredKernelIrVerificationErrorV1, Module, StructurallyCheckedModuleStorageV1,
    TargetCapability, VerificationStorageContextV1, verify_depth_bounded_context_with_budget_v1,
};

/// A distinct borrow proving local structure, CFG and types in one storage module.
///
/// This cannot be supplied to legacy analyses requiring VerifiedKernelIrModuleV1.
/// It proves no source-layout/ABI binding, value validity, initialized bytes,
/// active variant, pointer provenance, actual bounds/alignment, allocation
/// lifetime, copy overlap or target/SIM execution support.
///
/// ```compile_fail
/// use fe2o3_kernel_ir::{VerifiedStorageKernelIrModuleV1, VerifiedKernelIrModuleV1};
/// fn legacy<'a>(view: VerifiedStorageKernelIrModuleV1<'a>) -> VerifiedKernelIrModuleV1<'a> {
///     view
/// }
/// ```
#[derive(Debug)]
pub struct VerifiedStorageKernelIrModuleV1<'module> {
    storage: StructurallyCheckedModuleStorageV1<'module>,
}

impl<'module> VerifiedStorageKernelIrModuleV1<'module> {
    pub(crate) fn from_canonical_storage_owner_v18(
        owner: &'module crate::VerifiedCanonicalKernelIrModuleV18,
    ) -> Self {
        Self {
            storage: StructurallyCheckedModuleStorageV1::from_canonical_storage_owner_v18(owner),
        }
    }

    pub fn module(&self) -> &'module Module {
        self.storage.module()
    }

    pub fn storage(&self) -> &StructurallyCheckedModuleStorageV1<'module> {
        &self.storage
    }
}

struct StorageVerificationScope<'a, 'work> {
    budget: &'a mut Budget<'work>,
    floor: Option<usize>,
}

impl StorageVerificationScope<'_, '_> {
    fn finish(&mut self) -> Result<(), ResourceError> {
        match self.floor.take() {
            Some(floor) => self.budget.rollback_storage(floor),
            None => Ok(()),
        }
    }
}

impl Drop for StorageVerificationScope<'_, '_> {
    fn drop(&mut self) {
        // Inner owners unwind first; no callback can swap or undercut this ledger.
        let _ = self.finish();
    }
}

fn storage_verification_headers_v1() -> Result<usize, ResourceError> {
    [
        (1, std::mem::size_of::<StorageVerificationScope<'_, '_>>()),
        (
            1,
            std::mem::size_of::<StructurallyCheckedModuleStorageV1<'_>>(),
        ),
        (
            1,
            std::mem::size_of::<VerifiedStorageKernelIrModuleV1<'_>>(),
        ),
        // Entry, engine, pass, inner pass, dispatch and by-value accessor slots.
        // Header/function dispatch is sequential; no optimized slot reuse assumed.
        (
            6,
            std::mem::size_of::<VerificationStorageContextV1<'_, '_>>(),
        ),
        (
            2,
            std::mem::size_of::<Result<VerifiedStorageKernelIrModuleV1<'_>, Error>>(),
        ),
        (2, std::mem::size_of::<Result<(), Error>>()),
        (
            2,
            std::mem::size_of::<Result<(), MeteredKernelIrVerificationErrorV1>>(),
        ),
        (2, std::mem::size_of::<Result<(), ResourceError>>()),
    ]
    .into_iter()
    .try_fold(0_usize, |total, (count, bytes)| {
        bytes
            .checked_mul(count)
            .and_then(|bytes| total.checked_add(bytes))
            .ok_or(ResourceError::Arithmetic)
    })
}

/// Consumes one prechecked immutable module/table borrow and verifies the module.
///
/// Type-depth preflight, diagnostics, CFG/dominance, definitions, ordinary
/// operations and terminators reuse the existing engine and FunctionPass. The
/// same checked table is shared by both diagnostic passes; it is not revalidated
/// per operation. Only full success constructs this distinct borrowed view.
///
/// Input ownership stays in the caller's floor. Additional fixed typed headers
/// are prepaid, while existing verifier scratch retains its established logical
/// units. Every Result restores the floor; diagnostics transfer to the caller
/// under the existing borrowed-verifier convention. Accepted work, peak and first
/// denial history are never reset. This is not a codec/profile or execution token.
pub fn verify_storage_module_ref_with_budget_v1<'module>(
    checked: StructurallyCheckedModuleStorageV1<'module>,
    supported_capabilities: Option<&BTreeSet<TargetCapability>>,
    budget: &mut Budget<'_>,
) -> Result<VerifiedStorageKernelIrModuleV1<'module>, Error> {
    budget.charge_work(1)?;
    let floor = budget.storage_checkpoint();
    budget.reserve_storage(storage_verification_headers_v1()?)?;
    let mut scope = StorageVerificationScope {
        budget,
        floor: Some(floor),
    };
    let result = (|| {
        crate::verification_borrowed_v1::verify_borrowed_type_depth_v1(
            checked.module(),
            scope.budget,
        )?;
        verify_depth_bounded_context_with_budget_v1(
            VerificationStorageContextV1::Storage(&checked),
            supported_capabilities,
            scope.budget,
        )?;
        Ok(VerifiedStorageKernelIrModuleV1 { storage: checked })
    })();
    let released = scope.finish();
    match result {
        Err(error) => Err(error),
        Ok(view) => {
            released?;
            Ok(view)
        }
    }
}

#[cfg(test)]
#[path = "verification_storage_module_v1_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "verification_storage_module_resource_v1_tests.rs"]
mod resource_tests;

#[cfg(test)]
pub(crate) fn canonical_storage_header_layout_premise_v18() -> usize {
    std::mem::size_of::<StorageVerificationScope<'_, '_>>()
}
