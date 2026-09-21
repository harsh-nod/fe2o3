//! Nondefault source-only nominal ABI continuation. This never lowers LLVM,
//! creates a worker request, or upgrades an inert descriptor to launch authority.
#![allow(
    clippy::result_large_err,
    reason = "Keep exact typed child errors inline without unmetered error allocation."
)]

use super::*;
use crate::compiler_descriptor::nominal_v3::{self, NominalDescriptorErrorV3 as E, scoped};
use fe2o3_kernel_descriptor::{
    DESCRIPTOR_READER_SCRATCH_STORAGE_V3, DESCRIPTOR_TABLE_VIEW_STORAGE_V3,
    DeviceDescriptorTableV3, decode_device_descriptor_table_v3,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use fe2o3_lower_mir_kernel::CanonicalOutputFormalSourceAnchorV1 as Anchor;
use std::mem::size_of;

type R<T> = Result<T, E>;

/// New owner/header/output backing plus the unchanged materialized source floor.
/// Reserve the returned addition before use and drop the owner before refunding.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct NominalSourceAbiStorageV3(usize);
impl NominalSourceAbiStorageV3 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Retains the original compiler transaction, nominal source/N, ranked and
/// general-memory ownership, actual typed roots and rustc target. The new V3
/// bytes are inert: source agreement is not a signed source or native receipt.
/// General matrix, LDS, synchronization and atomic requirement derivation is deferred
/// and receives a typed UnsupportedRequirements refusal at this entry.
pub(crate) struct PreparedNominalSourceAbiV3 {
    source: FormalMemoryAdmittedProductionCompilation,
    wire: Vec<u8>,
    retained_floor: usize,
}
impl PreparedNominalSourceAbiV3 {
    pub(crate) fn canonical_bytes(&self) -> &[u8] {
        &self.wire
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

    /// The callback borrows this owner's fully checked table. A selected row or
    /// cursor cannot escape the local view. Output bytes grant no further token.
    pub(crate) fn with_checked_table<T>(
        &self,
        budget: &mut Budget<'_>,
        use_table: impl for<'a, 'w> FnOnce(&'a DeviceDescriptorTableV3<'w>, &mut Budget<'_>) -> R<T>,
    ) -> R<T> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| {
            budget.charge_work(2)?;
            // Existing semantic/ranked/formal replay keeps its inherited engine
            // domain; all new descriptor rows and bytes use this same budget.
            self.source.admitted.verify_equivalence().map_err(|error| {
                E::Pipeline(ProductionPipelineError::FormalMemoryAdmission(error))
            })?;
            let reproduced = nominal_v3::produce(
                &self.source.bindings.typed_descriptor_roots,
                &self.source.admitted,
                &self.source.bindings.rustc_target,
                budget,
            )?;
            budget.reserve_storage(
                reproduced
                    .capacity()
                    .checked_add(size_of::<Vec<u8>>())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            budget.charge_work(
                reproduced
                    .len()
                    .checked_add(self.wire.len())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            if reproduced != self.wire {
                return Err(E::Mismatch("actual source-derived V3 bytes"));
            }
            drop(reproduced);
            check(&self.source, &self.wire, budget, use_table)
        })
    }
}

fn check<T>(
    source: &FormalMemoryAdmittedProductionCompilation,
    wire: &[u8],
    budget: &mut Budget<'_>,
    use_table: impl for<'a, 'w> FnOnce(&'a DeviceDescriptorTableV3<'w>, &mut Budget<'_>) -> R<T>,
) -> R<T> {
    budget.reserve_storage(
        DESCRIPTOR_TABLE_VIEW_STORAGE_V3
            .checked_add(DESCRIPTOR_READER_SCRATCH_STORAGE_V3)
            .ok_or(Resource::Arithmetic)?,
    )?;
    let table =
        decode_device_descriptor_table_v3(wire, &mut |w| budget.charge_work(w)).map_err(E::Wire)?;
    let (agreement, storage) = fe2o3_verifier::check_nominal_source_abi_v3(
        Anchor::Direct(source.admitted.semantic_kir()),
        &table,
        budget,
    )
    .map_err(E::Agreement)?;
    budget.reserve_storage(storage.retained_storage())?;
    agreement.verify_equivalence(budget).map_err(E::Agreement)?;
    if agreement.grants_artifact_or_launch_authority() {
        return Err(E::Mismatch("inert source ABI receipt"));
    }
    drop(agreement);
    budget.charge_work(1)?;
    use_table(&table, budget)
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Explicit opt-in actual rustc capture. The ordinary importer/default route
    /// is unchanged. Missing runtime, signing or native support is never success
    /// for those separate boundaries.
    pub(crate) fn prepare_nominal_source_abi_v3(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(PreparedNominalSourceAbiV3, NominalSourceAbiStorageV3)> {
        let floor = budget.storage();
        scoped(budget, |budget| {
            budget.charge_work(3)?;
            let admitted = self
                .import_semantic_mir_with_nominal_v35(true)
                .map_err(E::Pipeline)?;
            let ranked = admitted
                .construct_semantic_middle_end()
                .map_err(E::Pipeline)?
                .construct_semantic_ssa()
                .map_err(E::Pipeline)?
                .materialize_target_neutral()
                .map_err(|error| E::Pipeline(*error))?
                .verify_general_kernel_checks()
                .map_err(E::Pipeline)?;
            let source_floor = ranked
                .ranked
                .materialized()
                .unit_local_source_storage_floor_v1()
                .map_err(|error| {
                    E::Pipeline(ProductionPipelineError::TargetNeutralLowering(error))
                })?;
            let header = size_of::<PreparedNominalSourceAbiV3>();
            budget.reserve_storage(
                source_floor
                    .checked_add(header)
                    .ok_or(Resource::Arithmetic)?,
            )?;
            let source = ranked
                .attach_target_neutral_checks()
                .map_err(E::Pipeline)?
                .admit_formal_memory()
                .map_err(E::Pipeline)?;
            let wire = nominal_v3::produce(
                &source.bindings.typed_descriptor_roots,
                &source.admitted,
                &source.bindings.rustc_target,
                budget,
            )?;
            budget.reserve_storage(wire.capacity())?;
            check(&source, &wire, budget, |_, _| Ok(()))?;
            let retained = source_floor
                .checked_add(header)
                .and_then(|v| v.checked_add(wire.capacity()))
                .ok_or(Resource::Arithmetic)?;
            let retained_floor = floor.checked_add(retained).ok_or(Resource::Arithmetic)?;
            budget.charge_work(1)?;
            Ok((
                PreparedNominalSourceAbiV3 {
                    source,
                    wire,
                    retained_floor,
                },
                NominalSourceAbiStorageV3(retained),
            ))
        })
    }
}
