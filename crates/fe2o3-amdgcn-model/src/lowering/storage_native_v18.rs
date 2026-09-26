//! Actual-owner V18 target component. Output is inert LLVM, never source,
//! initialization, lifetime, descriptor or final production authority.

use super::storage_resources_v18::{MeterV18, StorageMeterV18};
use super::*;
use fe2o3_amd_target::ProductionAmdTargetProfileV1;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, StorageLayoutIdV1, StorageLayoutV1,
    VerifiedCanonicalKernelIrModuleV18 as Owner,
};

#[derive(Debug)]
pub(crate) enum StorageLoweringErrorV18 {
    Resource(Resource),
    Lowering(LoweringErrors),
}
impl fmt::Display for StorageLoweringErrorV18 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "V18 storage lowering: {self:?}")
    }
}
impl Error for StorageLoweringErrorV18 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Resource(e) => Some(e),
            Self::Lowering(e) => Some(e),
        }
    }
}

pub(super) struct StorageEmissionContextV18<'a> {
    pub(super) owner: &'a Owner,
    pub(super) target: LoweringTarget,
    pub(super) meter: &'a dyn StorageMeterV18,
    pub(super) root_roles: Option<&'a RootRolesV29<'a>>,
}

impl StorageEmissionContextV18<'_> {
    pub(super) fn charge(&self, amount: usize) -> Result<(), LoweringErrors> {
        self.meter
            .charge(amount)
            .map_err(|_| self.reject("storage target work budget refused"))
    }
    pub(super) fn row(&self, id: StorageLayoutIdV1) -> Result<&StorageLayoutV1, LoweringErrors> {
        self.charge(1)?;
        self.owner
            .module()
            .storage_layouts
            .get(id.0 as usize)
            .ok_or_else(|| self.reject("storage row is outside the actual owner"))
    }
    pub(super) fn reject(&self, reason: &'static str) -> LoweringErrors {
        LoweringErrors::one(
            LoweringLocation::module(self.owner.module()),
            LoweringDiagnosticCode::UnsupportedOperation,
            reason,
        )
    }
    pub(super) fn check_current(
        &self,
        module: &Module,
        target: LoweringTarget,
    ) -> Result<(), LoweringErrors> {
        self.charge(1)?;
        if !std::ptr::eq(module, self.owner.module()) || target != self.target {
            return Err(self.reject("storage lowering requires the actual owner and exact target"));
        }
        Ok(())
    }
}

/// Crate-private until source, exact target binding and descriptor/replay joins
/// are implemented. The caller retains the actual owner and its resource floor.
/// New storage scans/output work share this budget. Shared legacy allocations
/// still use the existing bounded graph/text policy, not a fabricated receipt.
pub(crate) fn lower_canonical_storage_module_v18(
    owner: &Owner,
    profile: ProductionAmdTargetProfileV1,
    budget: &mut Budget<'_>,
) -> Result<String, StorageLoweringErrorV18> {
    lower_storage_module_v29(owner, profile, None, budget)
}

/// Inert target recipe only. Source/runtime/final ABI correspondence is not
/// admitted here, and this entry is not exposed for production publication.
pub(crate) fn lower_canonical_storage_module_with_root_roles_v29(
    owner: &Owner,
    profile: ProductionAmdTargetProfileV1,
    roles: &RootRolesV29<'_>,
    budget: &mut Budget<'_>,
) -> Result<String, StorageLoweringErrorV18> {
    lower_storage_module_v29(owner, profile, Some(roles), budget)
}

fn lower_storage_module_v29(
    owner: &Owner,
    profile: ProductionAmdTargetProfileV1,
    root_roles: Option<&RootRolesV29<'_>>,
    budget: &mut Budget<'_>,
) -> Result<String, StorageLoweringErrorV18> {
    let meter = MeterV18::new(budget).map_err(StorageLoweringErrorV18::Resource)?;
    let target = match profile {
        ProductionAmdTargetProfileV1::Gfx942 => LoweringTarget::Gfx942XnackMinusV1,
        ProductionAmdTargetProfileV1::Gfx950 => LoweringTarget::Gfx950XnackMinusV1,
    };
    let context = StorageEmissionContextV18 {
        owner,
        target,
        meter: &meter,
        root_roles,
    };
    let result = lower_compiler_module_with_ordered_context(
        owner.module(),
        target,
        None,
        Some(SemanticAnchorInputV1::Storage(owner)),
        true,
        Some(OrderedModuleOwner::StorageV18(&context)),
    );
    if let Some(error) = meter.failure() {
        drop(result);
        return Err(StorageLoweringErrorV18::Resource(error));
    }
    result.map_err(StorageLoweringErrorV18::Lowering)
}

#[cfg(test)]
#[path = "storage_native_v18_tests.rs"]
pub(super) mod tests;
