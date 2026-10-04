//! Closed source-owned composition entry. This is not a caller-supplied law.
//! The fixed production caller remains closed until forwarding is composed.
use super::*;
use crate::mixed_optimizer_refinement_v26::semantics::allocation_bridge_v48::AllocationBridgeV48;
use fe2o3_kernel_analysis::{
    CanonicalKirInventoryV18 as Inventory, CanonicalKirPrivateByteAnalysisV38 as Physical,
    CheckedCanonicalKirLicmV18 as Licm, CheckedCanonicalKirTransitionV18 as Prefix,
};

pub(super) struct Tail<'a, 'owner, 'rows> {
    prefix: &'a Prefix<'a, 'owner, 'owner, 'rows>,
    licm: &'a Licm<'owner>,
    output: &'a Inventory<'owner>,
    forwarding: Option<(
        &'a fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV18<'owner>,
        &'a Inventory<'owner>,
    )>,
}

impl<'a, 'owner, 'rows> Tail<'a, 'owner, 'rows> {
    pub(super) fn derive(
        relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
        prefix: &'a Prefix<'a, 'owner, 'owner, 'rows>,
        licm: &'a Licm<'owner>,
        output: &'a Inventory<'owner>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let inventory = relation.inventory(out.budget)?;
        out.budget.charge_work(3)?;
        if !std::ptr::eq(inventory.owner(), prefix.input().owner())
            || !std::ptr::eq(prefix.output().owner(), licm.input())
            || !output.belongs_to(licm.output())
        {
            return Err(mismatch());
        }
        out.budget
            .reserve_storage(size_of::<Self>() + 2 * size_of::<Result<Self>>())?;
        Ok(Self {
            prefix,
            licm,
            output,
            forwarding: None,
        })
    }

    pub(super) fn derive_final(
        relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
        prefix: &'a Prefix<'a, 'owner, 'owner, 'rows>,
        licm: &'a Licm<'owner>,
        relocated: &'a Inventory<'owner>,
        forwarding: &'a fe2o3_kernel_analysis::CheckedCanonicalKirCrossBlockForwardingV18<'owner>,
        final_inventory: &'a Inventory<'owner>,
        out: &mut Writer<'_, '_>,
    ) -> Result<Self> {
        let mut tail = Self::derive(relation, prefix, licm, relocated, out)?;
        out.budget.charge_work(2)?;
        if !relocated.belongs_to(forwarding.input())
            || !final_inventory.belongs_to(forwarding.output())
        {
            return Err(mismatch());
        }
        tail.forwarding = Some((forwarding, final_inventory));
        Ok(tail)
    }

    pub(super) fn emit(
        &self,
        relation: &fe2o3_lower_mir_kernel::ProductionSourceCorrespondenceV18<'_>,
        slots: &slots::SourceSlots<'_, '_>,
        original_physical: &Physical<'_, '_>,
        original_contracts: &TargetContracts<'_, '_>,
        emitted_original: &EmittedByteFunctionsV55<'_, '_, slots::SourceSlots<'_, '_>>,
        width: fe2o3_kernel_ir::FormalIndexWidth,
        out: &mut Writer<'_, '_>,
    ) -> Result<()> {
        slots.with_source_query_v42(out, |out| {
            let floor = out.budget.storage();
            let result = (|| {
                let bridge =
                    AllocationBridgeV48::derive(self.prefix, self.licm, self.output, slots, out)?;
                let limits = fe2o3_kernel_analysis::CanonicalKirPrivateByteLimitsV38 {
                    max_boundaries: MAX_PRIVATE_BYTE_BOUNDARIES_V38,
                };
                let (prefix_physical, prefix_storage) =
                    fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                        self.prefix.output(),
                        limits,
                        out.budget,
                    )?;
                out.budget
                    .reserve_storage(prefix_storage.retained_storage())?;
                let (output_physical, output_storage) =
                    fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                        self.output,
                        limits,
                        out.budget,
                    )?;
                out.budget
                    .reserve_storage(output_storage.retained_storage())?;
                let prefix_contracts = TargetContracts::derive(self.prefix.output(), width, out)?;
                let output_contracts = TargetContracts::derive(self.output, width, out)?;
                let (forwarding, final_inventory) = self.forwarding.ok_or_else(mismatch)?;
                let (final_physical, final_storage) =
                    fe2o3_kernel_analysis::analyze_canonical_kir_private_bytes_v38(
                        final_inventory,
                        limits,
                        out.budget,
                    )?;
                out.budget
                    .reserve_storage(final_storage.retained_storage())?;
                let final_contracts = TargetContracts::derive(final_inventory, width, out)?;
                bridge.emit_typed_source_chain_v49(
                    relation,
                    original_physical,
                    &prefix_physical,
                    &output_physical,
                    original_contracts,
                    &prefix_contracts,
                    &output_contracts,
                    emitted_original,
                    final_inventory,
                    forwarding,
                    &final_physical,
                    &final_contracts,
                    width,
                    TARGET_TAG_NAMESPACE_V40,
                    out,
                )?;
                drop((
                    final_contracts,
                    final_physical,
                    output_contracts,
                    prefix_contracts,
                    output_physical,
                    prefix_physical,
                    bridge,
                ));
                Ok(())
            })();
            out.budget.release_storage(
                out.budget
                    .storage()
                    .checked_sub(floor)
                    .ok_or(Resource::Accounting)?,
            )?;
            result
        })
    }
}
