//! Actual scalar target dispatch, distinct from the original source inventory.
//! This owner-bound view emits target semantics; it does not establish source
//! cut correspondence or source-to-target refinement.
use super::{
    ByteContext, Error, MAX_PRIVATE_BYTE_BOUNDARIES_V38, Resource, Result,
    TARGET_TAG_NAMESPACE_V40, TargetContracts, Writer,
    slots::{SourceSlots, TileAllocationSlotsV164},
    vector,
};
use crate::mixed_optimizer_refinement_v26::semantics::byte_function_v30::{
    ByteAllocationResolverV30, ByteFunctionV30,
};
use fe2o3_kernel_analysis::CanonicalKirInventoryV18 as Inventory;
use fe2o3_kernel_ir::{
    CanonicalKirFunctionCoordinateV1 as Function, FormalIndexWidth, FunctionRole,
};
use std::mem::size_of;

#[path = "original_semantic_mir_tile_microcuts_v180.rs"]
mod microcuts;
pub(super) use microcuts::TileMicroCutsV180;

pub(super) struct TileTargetV176<'slots, 'view, 'source> {
    slots: &'slots SourceSlots<'view, 'source>,
    inventory: Inventory<'view>,
    allocations: TileAllocationSlotsV164<'slots, 'view, 'source>,
    roots: Vec<Function>,
    required: usize,
}

fn mismatch() -> Error {
    Error::Statement("expanded tile target differs from its retained source owner")
}

impl<'slots, 'view, 'source> TileTargetV176<'slots, 'view, 'source> {
    pub(super) fn derive(
        slots: &'slots SourceSlots<'view, 'source>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        slots.with_source_query_v42(out, |out| {
            out.budget.reserve_storage(
                size_of::<Self>()
                    + 2 * size_of::<Result<Self>>()
                    + size_of::<fe2o3_lower_mir_kernel::ProductionOptimizedSourceCfgRootV18<'_, '_>>()
                    + 24 * size_of::<usize>(),
            )?;
            let tile = slots.tile_owner_v176(out)?;
            let (inventory, receipt) = Inventory::derive_v18(tile.output(out.budget)?, out.budget)?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            let allocations = TileAllocationSlotsV164::derive(slots, &inventory, out)?;
            let relation = slots.correspondence(out)?;
            let source = relation.source(out.budget)?;
            let neutral = tile.neutral_source_v162(out.budget)?;
            let root_count = source.root_count(out.budget)?;
            let mut roots = vector(root_count, out)?;
            let mut seen = vector(inventory.functions().len(), out)?;
            out.budget.charge_work(inventory.functions().len())?;
            seen.resize(inventory.functions().len(), false);
            for root in 0..root_count {
                let cfg = neutral.output_root_cfg_v18(root, out.budget)?;
                let function = cfg.function();
                let coordinate = function.coordinate;
                out.budget.charge_work(5)?;
                let actual = inventory
                    .functions()
                    .get(coordinate.0 as usize)
                    .ok_or_else(mismatch)?;
                out.budget.charge_work(
                    actual.function.id.as_str().len()
                        .checked_add(function.function.id.as_str().len())
                        .ok_or(Resource::Arithmetic)?,
                )?;
                let visited = seen.get_mut(coordinate.0 as usize).ok_or_else(mismatch)?;
                if *visited
                    || actual.coordinate != coordinate
                    || actual.function.role != FunctionRole::KernelEntry
                    || actual.function.id != function.function.id
                {
                    return Err(mismatch());
                }
                // The scalar transaction preserves functions. The neutral
                // declaration relation, not the original ordinal, locates it.
                if let Some((selected, _, _)) = tile.root_policy_v162(root, out.budget)?
                    && selected != coordinate
                {
                    return Err(mismatch());
                }
                *visited = true;
                roots.push(coordinate);
            }
            for (function, visited) in inventory.functions().iter().zip(&seen) {
                out.budget.charge_work(1)?;
                if *visited != (function.function.role == FunctionRole::KernelEntry) {
                    return Err(mismatch());
                }
            }
            let temporary = seen.capacity().checked_mul(size_of::<bool>()).ok_or(Resource::Arithmetic)?;
            drop(seen);
            out.budget.release_storage(temporary)?;
            allocations.check_owner(inventory.owner(), out)?;
            Ok(Self {
                slots,
                inventory,
                allocations,
                roots,
                required: out.budget.storage(),
            })
        })
    }

    fn check(&self, out: &mut Writer<'_, '_>) -> Result<()> {
        self.slots
            .check_query_storage_floor(self.required, out.budget)?;
        self.allocations.check_owner(self.inventory.owner(), out)
    }

    pub(super) fn inventory(&self, out: &mut Writer<'_, '_>) -> Result<&Inventory<'view>> {
        self.check(out)?;
        Ok(&self.inventory)
    }

    pub(super) fn source_slots(
        &self,
        out: &mut Writer<'_, '_>,
    ) -> Result<&'slots SourceSlots<'view, 'source>> {
        self.check(out)?;
        Ok(self.slots)
    }

    pub(super) fn allocation_site(
        &self,
        operation: fe2o3_kernel_ir::CanonicalKirOperationCoordinateV1,
        out: &mut Writer<'_, '_>,
    ) -> Result<
        crate::mixed_optimizer_refinement_v26::semantics::byte_function_v30::ByteAllocationSiteV30,
    > {
        self.check(out)?;
        self.allocations.site(operation, out)
    }

    pub(super) fn root_function(&self, root: usize, out: &mut Writer<'_, '_>) -> Result<Function> {
        self.check(out)?;
        out.budget.charge_work(1)?;
        self.roots.get(root).copied().ok_or_else(mismatch)
    }

    pub(super) fn emit(&self, width: FormalIndexWidth, out: &mut Writer<'_, '_>) -> Result<()> {
        self.slots.with_source_query_v42(out, |out| {
            self.check(out)?;
            let contracts = TargetContracts::derive(&self.inventory, width, out)?;
            let (physical, receipt) =
                fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                    &self.inventory,
                    fe2o3_kernel_analysis::CanonicalKirPrivateByteLimitsV38 {
                        max_boundaries: MAX_PRIVATE_BYTE_BOUNDARIES_V38,
                    },
                    out.budget,
                )?;
            out.budget.reserve_storage(receipt.retained_storage())?;
            contracts.emit(TARGET_TAG_NAMESPACE_V40, out)?;
            for root in 0..self.roots.len() {
                // Source tag pairing belongs to the eventual paired relation,
                // not this independently owner-bound target semantics emitter.
                contracts.check_owner_width_v39(self.inventory.owner(), width, out)?;
                let function = self.root_function(root, out)?;
                let actual = ByteFunctionV30::derive(
                    &self.inventory,
                    &physical,
                    function,
                    ByteContext::classified(width, &contracts, TARGET_TAG_NAMESPACE_V40),
                    &self.allocations,
                    out,
                )?;
                actual.emit(root, out)?;
            }
            self.check(out)?;
            drop(physical);
            out.budget.release_storage(receipt.retained_storage())?;
            Ok(())
        })
    }
}
