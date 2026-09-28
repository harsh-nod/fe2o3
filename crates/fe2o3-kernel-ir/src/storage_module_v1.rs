//! A structural view of the one table owned by an existing Module.
//! This is not a source-layout, value-validity, target or runtime authority.

use crate::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as ResourceError, Module, StorageLayoutErrorV1,
    StorageLayoutLimitsV1, StructurallyCheckedStorageLayoutsV1, check_storage_layouts_v1,
};

/// Couples a structural layout check to the exact borrowed owning module.
///
/// Row ordinals are not cross-module type identities. Consumers obtain both the
/// module and its table from this view, never from an independently supplied pair.
/// The module and table cannot be mutated while this borrow remains live.
#[derive(Debug)]
pub struct StructurallyCheckedModuleStorageV1<'module> {
    module: &'module Module,
    layouts: StructurallyCheckedStorageLayoutsV1<'module>,
}

impl<'module> StructurallyCheckedModuleStorageV1<'module> {
    pub(crate) fn from_canonical_storage_owner_v18(
        owner: &'module crate::VerifiedCanonicalKernelIrModuleV18,
    ) -> Self {
        Self {
            module: owner.module(),
            layouts: StructurallyCheckedStorageLayoutsV1::from_canonical_storage_owner_v18(owner),
        }
    }

    pub fn module(&self) -> &'module Module {
        self.module
    }

    pub fn layouts(&self) -> &StructurallyCheckedStorageLayoutsV1<'module> {
        &self.layouts
    }
}

struct HeaderScope<'a, 'work> {
    budget: &'a mut Budget<'work>,
    charged: usize,
}

impl HeaderScope<'_, '_> {
    fn finish(&mut self) -> Result<(), ResourceError> {
        let charged = std::mem::replace(&mut self.charged, 0);
        self.budget.release_storage(charged)
    }
}

impl Drop for HeaderScope<'_, '_> {
    fn drop(&mut self) {
        // The core has dropped its scratch before normal or unwind cleanup here.
        let _ = self.finish();
    }
}

fn headers() -> Result<usize, ResourceError> {
    // Full typed caller/callee outcome slots, without assuming stack-slot reuse.
    [
        (1, std::mem::size_of::<HeaderScope<'_, '_>>()),
        (1, std::mem::size_of::<StructurallyCheckedModuleStorageV1<'_>>()),
        (
            2,
            std::mem::size_of::<
                Result<StructurallyCheckedModuleStorageV1<'_>, StorageLayoutErrorV1>,
            >(),
        ),
        (
            2,
            std::mem::size_of::<
                Result<StructurallyCheckedStorageLayoutsV1<'_>, StorageLayoutErrorV1>,
            >(),
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

/// Structurally checks only the table actually owned by `module`.
///
/// The caller owns the input module/table and their live-storage floor. This
/// entry prepays its fixed headers, then invokes the bounded structural checker
/// under the same ledger. All temporary storage is released before returning;
/// the returned borrowed view owns no table allocation. One entry work unit is
/// followed by the unchanged layout-check work schedule.
///
/// This does not verify function bodies or source layout/ABI, initialization,
/// active variants, pointer provenance, allocation lifetime or target lowering.
pub fn check_module_storage_v1<'module>(
    module: &'module Module,
    limits: StorageLayoutLimitsV1,
    budget: &mut Budget<'_>,
) -> Result<StructurallyCheckedModuleStorageV1<'module>, StorageLayoutErrorV1> {
    budget.charge_work(1)?;
    let charged = headers()?;
    budget.reserve_storage(charged)?;
    let mut scope = HeaderScope { budget, charged };
    let result = check_storage_layouts_v1(&module.storage_layouts, limits, scope.budget)
        .map(|layouts| StructurallyCheckedModuleStorageV1 { module, layouts });
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
#[path = "storage_module_v1_tests.rs"]
mod tests;

#[cfg(test)]
pub(crate) fn canonical_storage_header_layout_premise_v18() -> usize {
    std::mem::size_of::<HeaderScope<'_, '_>>()
}
