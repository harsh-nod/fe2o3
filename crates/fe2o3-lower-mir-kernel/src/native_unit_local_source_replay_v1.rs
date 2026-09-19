//! Additive UnitLocal source/N and exact silent-call N/E replay.
use super::*;
use crate::{ProductionRankedSemanticProjectionRootV1, ProductionUnitLocalErasedSourceOwnerV1};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrModuleV12;

/// Normal-constructor UnitLocal N, never a legacy RawEmpty replay owner.
pub struct ReplayedUnitLocalNativeSourceV1 {
    source: ProductionPreRankedKirOwnerV1,
    catalog: Catalog,
    retained_storage: usize,
}
impl ReplayedUnitLocalNativeSourceV1 {
    /// Freshly reconstructed original source and neutral N.
    pub fn source(&self) -> &ProductionPreRankedKirOwnerV1 {
        &self.source
    }
    /// Catalog checked against the reconstructed original graph.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    /// Logical reconstruction payload retained under the replay contract.
    pub const fn retained_storage(&self) -> usize {
        self.retained_storage
    }
    /// Reconstruction alone confers no artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Reconstructs exact original UnitLocal N. No erased graph or detached
/// deletion claims are accepted here. Legacy replay remains RawEmpty-only.
pub fn replay_native_unit_local_source_correspondence_v1(
    semantic_bytes: &[u8],
    native_n_bytes: &[u8],
    catalog_bytes: &[u8],
    launch_inputs: &[ProductionSourceLaunchRootInputV1<'_>],
    budget: &mut Budget<'_>,
) -> Result<(ReplayedUnitLocalNativeSourceV1, NativeSourceReplayStorageV1), NativeSourceReplayErrorV1>
{
    let (parts, storage) = replay_native_source_parts_v1(
        semantic_bytes,
        native_n_bytes,
        catalog_bytes,
        launch_inputs,
        NativeSourceReplayRouteV1::UnitLocal,
        std::mem::size_of::<ReplayedUnitLocalNativeSourceV1>(),
        budget,
    )?;
    Ok((
        ReplayedUnitLocalNativeSourceV1 {
            source: parts.source,
            catalog: parts.catalog,
            retained_storage: storage.retained_storage(),
        },
        storage,
    ))
}

/// Owned original source/N, freshly checked ranked roots and independently
/// admitted exact E. Empty catalog is checked against both actual graphs.
pub struct ReplayedUnitLocalErasedNativeSourceV1 {
    source: ProductionUnitLocalErasedSourceOwnerV1,
    catalog: Catalog,
}
impl ReplayedUnitLocalErasedNativeSourceV1 {
    /// Original source/N and independently admitted erased E.
    pub fn source(&self) -> &ProductionUnitLocalErasedSourceOwnerV1 {
        &self.source
    }
    /// Catalog checked against both original N and actual erased E.
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }
    /// Source-erasure replay confers no artifact or launch authority.
    pub const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

/// Additional copied root maps/text, wrapper, and newly admitted E/deletion
/// payload. Original reconstruction and fresh lowerings remain separately
/// transferred input reservations; this is not complete heap/RSS accounting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeUnitLocalErasedAttachmentStorageV1(usize);
impl NativeUnitLocalErasedAttachmentStorageV1 {
    /// Additional retained payload to reserve after replay restores its floor.
    pub const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Replays original source/ranked and exact N/E against actual caller E. This
/// calls only the checked candidate constructor, never the erasure producer.
/// Inputs are consumed on error; their incoming reservations stay caller-owned.
/// Result/error/unwind restore the incoming floor, and the returned additional
/// receipt must be reserved before any subsequent allocation.
pub fn attach_replayed_native_unit_local_erasure_v1(
    replayed: ReplayedUnitLocalNativeSourceV1,
    candidates: &[NativeRankedSourceCandidateV1<'_>],
    lowerings: Vec<ProductionRankedKernelLoweringInputV1>,
    actual_e: &VerifiedCanonicalKernelIrModuleV12,
    budget: &mut Budget<'_>,
) -> Result<
    (
        ReplayedUnitLocalErasedNativeSourceV1,
        NativeUnitLocalErasedAttachmentStorageV1,
    ),
    NativeSourceReplayErrorV1,
> {
    budget.charge_work(8)?;
    if budget.storage() < replayed.retained_storage {
        return Err(Resource::Accounting.into());
    }
    native_source_transfer_v1(budget, move |budget| {
        let semantic_roots = replayed.source.semantic_ssa().source_semantic().roots();
        if candidates.is_empty()
            || candidates.len() != semantic_roots.len()
            || candidates.len() != lowerings.len()
        {
            return Err(NativeSourceReplayErrorV1::Mismatch(
                "complete fresh UnitLocal ranked roster",
            ));
        }
        let wrapper = std::mem::size_of::<ReplayedUnitLocalErasedNativeSourceV1>();
        let roots_header = std::mem::size_of::<Vec<ProductionRankedSemanticProjectionRootV1>>();
        budget.reserve_storage(
            wrapper
                .checked_add(roots_header)
                .ok_or(Resource::Arithmetic)?,
        )?;
        let mut roots = native_source_vector_v1(candidates.len(), budget)?;
        let roots_capacity = native_source_vector_capacity_v1(&roots)?;
        let mut retained = wrapper
            .checked_add(roots_header)
            .and_then(|n| n.checked_add(roots_capacity))
            .ok_or(Resource::Arithmetic)?;
        for ((candidate, lowering), semantic_root) in
            candidates.iter().zip(lowerings).zip(semantic_roots)
        {
            budget.charge_work(5)?;
            if candidate.semantic_root() != semantic_root.index()
                || candidate.kernel() != lowering.kernel()
            {
                return Err(NativeSourceReplayErrorV1::Mismatch(
                    "exact fresh UnitLocal ranked root/recipe",
                ));
            }
            let map_headers = std::mem::size_of::<Vec<crate::ProductionRankedAccessSourceV1>>()
                .checked_add(std::mem::size_of::<
                    Vec<crate::ProductionRankedExecutableEffectSourceV1>,
                >())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(map_headers)?;
            let mut access_sources =
                native_source_vector_v1(candidate.access_sources().len(), budget)?;
            budget.charge_work(candidate.access_sources().len())?;
            access_sources.extend_from_slice(candidate.access_sources());
            let mut effects =
                native_source_vector_v1(candidate.executable_effect_sources().len(), budget)?;
            budget.charge_work(candidate.executable_effect_sources().len())?;
            effects.extend_from_slice(candidate.executable_effect_sources());
            let text = candidate.ranked_ir();
            budget.charge_work(text.len())?;
            budget.reserve_storage(text.len())?;
            let mut ranked_ir = String::new();
            ranked_ir
                .try_reserve_exact(text.len())
                .map_err(|_| Resource::Allocation)?;
            budget.reserve_storage(
                ranked_ir
                    .capacity()
                    .checked_sub(text.len())
                    .ok_or(Resource::Accounting)?,
            )?;
            ranked_ir.push_str(text);
            let access_capacity = native_source_vector_capacity_v1(&access_sources)?;
            let effect_capacity = native_source_vector_capacity_v1(&effects)?;
            let boxed_maps = std::mem::size_of_val(access_sources.as_slice())
                .checked_add(std::mem::size_of_val(effects.as_slice()))
                .ok_or(Resource::Arithmetic)?;
            budget.charge_work(
                access_sources
                    .len()
                    .checked_add(effects.len())
                    .ok_or(Resource::Arithmetic)?,
            )?;
            // Vec-to-Box conversion may allocate a second buffer when shrinking.
            // Keep both original capacities live until the constructor consumes them.
            budget.reserve_storage(boxed_maps)?;
            retained = retained
                .checked_add(boxed_maps)
                .and_then(|n| n.checked_add(ranked_ir.capacity()))
                .ok_or(Resource::Arithmetic)?;
            roots.push(ProductionRankedSemanticProjectionRootV1::new(
                *semantic_root,
                candidate.launch_rank(),
                lowering,
                ranked_ir,
                access_sources,
                effects,
            ));
            budget.release_storage(
                map_headers
                    .checked_add(access_capacity)
                    .and_then(|value| value.checked_add(effect_capacity))
                    .ok_or(Resource::Arithmetic)?,
            )?;
        }
        let ReplayedUnitLocalNativeSourceV1 {
            source, catalog, ..
        } = replayed;
        let (source, storage) = ProductionUnitLocalErasedSourceOwnerV1::try_from_candidate_v1(
            source,
            roots,
            actual_e.module(),
            budget,
        )
        .map_err(NativeSourceReplayErrorV1::Materialize)?;
        budget.reserve_storage(storage.retained_storage())?;
        retained = retained
            .checked_add(storage.retained_storage())
            .ok_or(Resource::Arithmetic)?;
        let fresh_bytes = source.erased().canonical().canonical_bytes();
        let actual_bytes = actual_e.canonical().canonical_bytes();
        budget.charge_work(
            fresh_bytes
                .len()
                .checked_add(actual_bytes.len())
                .ok_or(Resource::Arithmetic)?,
        )?;
        if source.erased().canonical().identity() != actual_e.canonical().identity()
            || fresh_bytes != actual_bytes
        {
            return Err(NativeSourceReplayErrorV1::Mismatch(
                "complete freshly admitted E bytes",
            ));
        }
        let (inventory, inventory_storage) =
            CanonicalKirInventoryV1::derive(source.erased(), budget)
                .map_err(NativeSourceReplayErrorV1::Inventory)?;
        budget.reserve_storage(inventory_storage.retained_storage())?;
        let _ = check_kernel_ir_contract_catalog_v1(&inventory, &catalog, budget)
            .map_err(NativeSourceReplayErrorV1::CatalogBinding)?;
        drop(inventory);
        budget.release_storage(inventory_storage.retained_storage())?;
        // The candidate constructor has already reconstructed original source/N,
        // checked the ranked stage and replayed exact deletion. Nothing above
        // mutates either retained graph; the catalog join adds its own check.
        Ok((
            ReplayedUnitLocalErasedNativeSourceV1 { source, catalog },
            NativeUnitLocalErasedAttachmentStorageV1(retained),
        ))
    })
}
