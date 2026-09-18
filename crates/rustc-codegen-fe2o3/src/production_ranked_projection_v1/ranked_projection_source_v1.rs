//! Private borrowed inputs for the existing source-ranked projector.

use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource, CanonicalKernelIrWorkBudgetV1,
    VerifiedCanonicalKernelIrModuleV12,
};
use fe2o3_lower_mir_kernel::{
    ProductionPreRankedKirOwnerV1, ProductionSourceLaunchRosterV1, SemanticKirAssertOriginsV1,
};
use fe2o3_pliron::ProductionSemanticSsaOwnerV1;

use super::{CanonicalAssertionErrorV1, ProductionRankedProjectionErrorV1 as Error};

pub(super) struct RankedProjectionSourceV1<'s> {
    owner: &'s ProductionPreRankedKirOwnerV1,
    semantic_ssa: &'s ProductionSemanticSsaOwnerV1,
    source_launch: &'s ProductionSourceLaunchRosterV1,
    executable: &'s VerifiedCanonicalKernelIrModuleV12,
    origins: SemanticKirAssertOriginsV1<'s>,
    minimum_storage: usize,
}

impl<'s> RankedProjectionSourceV1<'s> {
    // This borrows inputs, not a helper-effect decision. The materialized join
    // and each ordinary call consume the sealed source relation separately.
    pub(super) fn from_materialized_checked(
        owner: &'s ProductionPreRankedKirOwnerV1,
    ) -> Result<Self, Error> {
        let minimum_storage = owner
            .unit_local_source_storage_floor_v1()
            .map_err(Error::StructuralValidation)?;
        Ok(Self {
            owner,
            semantic_ssa: owner.semantic_ssa(),
            source_launch: owner.source_launch(),
            executable: owner.executable(),
            origins: owner.assert_origins(),
            minimum_storage,
        })
    }

    #[cfg(test)]
    pub(super) fn from_legacy(owner: &'s ProductionPreRankedKirOwnerV1) -> Result<Self, Error> {
        if owner.helper_source_policy_v1()
            != fe2o3_lower_mir_kernel::ProductionHelperSourcePolicyV1::RawEmpty
        {
            return Err(Error::StructuralValidation(
                fe2o3_lower_mir_kernel::ProductionSemanticKirErrorV1::LocalHelperSourceConsumerUnavailable {
                    consumer: "source-ranked projection",
                },
            ));
        }
        let minimum_storage = owner.retained_analysis_storage_v1();
        Ok(Self {
            owner,
            semantic_ssa: owner.semantic_ssa(),
            source_launch: owner.source_launch(),
            executable: owner.executable(),
            origins: owner.assert_origins(),
            minimum_storage,
        })
    }

    pub(super) const fn semantic_ssa(&self) -> &'s ProductionSemanticSsaOwnerV1 {
        self.semantic_ssa
    }

    pub(super) const fn owner(&self) -> &'s ProductionPreRankedKirOwnerV1 {
        self.owner
    }

    pub(super) const fn source_launch(&self) -> &'s ProductionSourceLaunchRosterV1 {
        self.source_launch
    }

    pub(super) const fn executable(&self) -> &'s VerifiedCanonicalKernelIrModuleV12 {
        self.executable
    }

    pub(super) const fn origins(&self) -> SemanticKirAssertOriginsV1<'s> {
        self.origins
    }

    pub(super) fn require_floor(&self, budget: &Budget<'_>) -> Result<(), Error> {
        if budget.storage() < self.minimum_storage {
            return Err(resource(Resource::Accounting));
        }
        Ok(())
    }
}

pub(super) fn resource(error: Resource) -> Error {
    Error::CanonicalAssertions(CanonicalAssertionErrorV1::Resource(error))
}

pub(super) fn with_projection_source_budget_v1<T>(
    source: &RankedProjectionSourceV1<'_>,
    body: impl FnOnce(&mut Budget<'_>) -> Result<T, Error>,
) -> Result<T, Error> {
    let work_limit = usize::try_from(crate::production_canonical_phase_policy_v1::WORK_LIMIT)
        .map_err(|_| resource(Resource::Arithmetic))?;
    let mut work = CanonicalKernelIrWorkBudgetV1::new(work_limit);
    let mut budget = Budget::new(
        &mut work,
        crate::production_canonical_phase_policy_v1::STORAGE_LIMIT,
    );
    budget
        .reserve_storage(source.minimum_storage)
        .map_err(resource)?;
    body(&mut budget)
}
