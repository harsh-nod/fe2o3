//! Consuming nominal source-to-P4-O descriptor continuation. No native authority.
#![allow(clippy::result_large_err)]

use super::*;
use crate::compiler_descriptor::checked_output_policy3_v1::nominal_policy4_v3 as descriptor;
use crate::compiler_descriptor::nominal_v3::{NominalDescriptorErrorV3 as E, scoped};
use crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1 as Roster;
use fe2o3_compiler_ffi::CompilerDescriptorSourceV3;
use fe2o3_kernel_descriptor::DeviceDescriptorTableV3;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
    VerifiedCanonicalKernelIrModuleV12 as Graph,
};
use fe2o3_lower_mir_kernel::{
    ProductionCheckedOutputOwnerPolicy4V1 as Direct, ProductionHelperSourcePolicyV1 as Policy,
    ProductionUnitLocalErasedCheckedOutputOwnerPolicy4V1 as Erased,
};
use std::mem::size_of;

type R<T> = Result<T, E>;
enum Output {
    Direct(Direct),
    Erased(Erased),
}
impl Output {
    fn view(&self) -> descriptor::OwnerRef<'_> {
        match self {
            Self::Direct(owner) => descriptor::OwnerRef::Direct(owner),
            Self::Erased(owner) => descriptor::OwnerRef::Erased(owner),
        }
    }
    fn output(&self) -> &Graph {
        match self {
            Self::Direct(owner) => owner.output(),
            Self::Erased(owner) => owner.output(),
        }
    }
    fn additional_header(&self) -> R<usize> {
        // Preserve inherited source/formal accounting. Erasure's original
        // backend receipt already pays the verification-roster header/backing.
        let transferred = match self {
            Self::Direct(_) => size_of::<Direct>(),
            Self::Erased(_) => size_of::<Erased>()
                .checked_add(size_of::<Roster>())
                .ok_or(Resource::Arithmetic)?,
        };
        size_of::<PreparedNominalPolicy4AbiV3>()
            .checked_sub(transferred)
            .and_then(|n| n.checked_sub(size_of::<CompilerDescriptorSourceV3>()))
            .ok_or(Resource::Arithmetic.into())
    }
}

/// Additional complete transferred reservations, not a reconstructed P4 minimum.
#[derive(Clone, Copy)]
pub(crate) struct NominalPolicy4AbiStorageV3(usize);
impl NominalPolicy4AbiStorageV3 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Original collector, ranked, target and source ownership move together with O.
/// Descriptor bytes prove neither protected compiler origin nor GPU execution.
pub(crate) struct PreparedNominalPolicy4AbiV3 {
    admitted: Output,
    ranked_verification: Roster,
    bindings: AuthenticatedProductionBindings,
    descriptor: CompilerDescriptorSourceV3,
    retained_floor: usize,
}
impl RankedVerifiedProductionCompilation {
    pub(crate) fn lower_nominal_policy4_abi_v3(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(PreparedNominalPolicy4AbiV3, NominalPolicy4AbiStorageV3)> {
        let floor = budget.storage();
        scoped(budget, move |budget| {
            let (admitted, ranked_verification, bindings) =
                match self.ranked.materialized().helper_source_policy_v1() {
                    Policy::RawEmpty => {
                        let stage = self
                            .prepare_admitted_policy4_v1(budget)
                            .map_err(E::Pipeline)?;
                        (
                            Output::Direct(stage.admitted),
                            stage.ranked_verification,
                            stage.bindings,
                        )
                    }
                    Policy::UnitLocal => {
                        let stage = self
                            .prepare_admitted_erased_policy4_v1(budget)
                            .map_err(E::Pipeline)?;
                        (
                            Output::Erased(stage.admitted),
                            stage.ranked_verification,
                            stage.bindings,
                        )
                    }
                };
            budget.reserve_storage(admitted.additional_header()?)?;
            let (descriptor, storage) = descriptor::produce(
                admitted.view(),
                &bindings.typed_descriptor_roots,
                &bindings.rustc_target,
                budget,
            )?;
            budget.reserve_storage(storage.retained_storage())?;
            let value = PreparedNominalPolicy4AbiV3 {
                admitted,
                ranked_verification,
                bindings,
                descriptor,
                retained_floor: budget.storage(),
            };
            value.verify_equivalence(budget)?;
            let retained = budget
                .storage()
                .checked_sub(floor)
                .ok_or(Resource::Accounting)?;
            Ok((value, NominalPolicy4AbiStorageV3(retained)))
        })
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    pub(crate) fn prepare_nominal_policy4_abi_v3(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(PreparedNominalPolicy4AbiV3, NominalPolicy4AbiStorageV3)> {
        scoped(budget, move |budget| {
            budget.charge_work(3)?;
            self.import_nominal_ranked_v3()?
                .lower_nominal_policy4_abi_v3(budget)
        })
    }
}

impl PreparedNominalPolicy4AbiV3 {
    #[cfg(test)]
    pub(crate) fn source_test_is_erased_v3(&self) -> bool {
        matches!(self.admitted, Output::Erased(_))
    }

    pub(crate) fn output(&self) -> &Graph {
        self.admitted.output()
    }
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        self.descriptor.canonical_bytes()
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) const fn authenticates_execution(&self) -> bool {
        false
    }

    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        self.with_checked_table(budget, |_, _| Ok(()))
    }

    pub(crate) fn with_checked_table(
        &self,
        budget: &mut Budget<'_>,
        observe: impl for<'a, 'w> FnOnce(&'a DeviceDescriptorTableV3<'w>, &mut Budget<'_>) -> R<()>,
    ) -> R<()> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| {
            budget.charge_work(3)?;
            if self
                .bindings
                .rustc_preflight_plan
                .rustc_identity_inventory_sha256()
                != self.bindings.rustc_identity_inventory.sha256()
                || self.ranked_verification.root_count() != self.output().module().kernels.len()
                || !self
                    .ranked_verification
                    .every_functional_verification_is_coherent()
            {
                return Err(E::Mismatch("complete original rustc/ranked P4 custody"));
            }
            descriptor::with_checked_table(
                self.admitted.view(),
                &self.bindings.typed_descriptor_roots,
                &self.bindings.rustc_target,
                &self.descriptor,
                budget,
                observe,
            )
        })
    }
}
