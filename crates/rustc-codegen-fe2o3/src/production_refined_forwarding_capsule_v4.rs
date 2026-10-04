//! Consuming native capsule continuation. No legacy graph or authority conversion.
#[path = "production_refined_forwarding_capsule_base_v4.rs"]
mod base;
use super::*;
use crate::production_pipeline::ProductionCompilerCustody;
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_DECODE_METADATA_STORAGE_V4 as DECODE_STORAGE,
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_SEAL_STORAGE_V4 as SEAL_STORAGE,
    InertSemanticCompilerModuleHandoffLayoutV4 as HandoffLayout,
    InertSemanticCompilerModuleHandoffV4 as Handoff,
    inert_semantic_compiler_module_handoff_decode_work_v4,
    seal_inert_semantic_compiler_module_handoff_v4,
};
use fe2o3_compiler_lineage::{
    INERT_PRODUCTION_SEMANTIC_CAPSULE_WORKING_STORAGE_V4 as CAPSULE_STORAGE,
    InertProductionSemanticCapsuleLayoutV4 as CapsuleLayout,
    InertProductionSemanticCapsuleV3 as Base, seal_inert_production_semantic_capsule_v4,
};
use fe2o3_rustc_invocation::RustcInvocationDescriptorV3;

/// Additional wrapper, backing growth and decoded metadata. The existing live
/// compiler owner and carrier reservation transfer unchanged. Returned unreserved.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct RefinedForwardingCapsuleStorageV4(usize);
impl RefinedForwardingCapsuleStorageV4 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

/// Retains the actual source/history/F and protected invocation custody. Bytes
/// alone cannot manufacture this private owner, and it has no publication API.
pub(crate) struct PreparedRefinedForwardingCapsuleV4 {
    live: Live,
    handoff: Handoff,
    retained_floor: usize,
}
impl PreparedRefinedForwardingCapsuleV4 {
    pub(crate) fn handoff(&self) -> &Handoff {
        &self.handoff
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
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        scoped(budget, |budget| {
            let carrier = self.handoff.capsule().carrier_bytes();
            verify_live_carrier(&self.live, carrier, budget)?;
            let expected = base::build(
                &self.live,
                carrier,
                invocation(&self.live.bindings.transaction.compiler_custody)?,
                budget,
            )?;
            same(
                expected.canonical_bytes(),
                self.handoff.capsule().base().canonical_bytes(),
                budget,
            )?;
            same(
                self.live.native_output_parts().0.canonical_bytes(),
                self.handoff.module_handoff().canonical_bytes(),
                budget,
            )?;
            Ok(())
        })
    }
}

pub(super) fn invocation(custody: &ProductionCompilerCustody) -> R<&RustcInvocationDescriptorV3> {
    match custody {
        ProductionCompilerCustody::ProtectedV3 { invocation, .. } => Ok(invocation.descriptor()),
        ProductionCompilerCustody::ExtractionOnly => Err(E::Mismatch(
            "native capsule requires retained invocation custody",
        )),
    }
}

impl PreparedRefinedForwardingWireV1 {
    /// Extends the already verified production owner without caller-selectable
    /// graphs, target, descriptor, invocation, or replacement output bytes.
    pub(crate) fn into_inert_semantic_handoff_v4(
        self,
        budget: &mut Budget<'_>,
    ) -> R<(
        PreparedRefinedForwardingCapsuleV4,
        RefinedForwardingCapsuleStorageV4,
    )> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        let floor = budget.storage();
        scoped(budget, |budget| {
            // Reject extraction custody before expensive replay or output allocation.
            invocation(&self.live.bindings.transaction.compiler_custody)?;
            self.verify_equivalence(budget)?;
            let header = size_of::<PreparedRefinedForwardingCapsuleV4>()
                .checked_sub(size_of::<PreparedRefinedForwardingWireV1>())
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(header)?;
            let old_capacity = self.carrier.capacity();
            let PreparedRefinedForwardingWireV1 { live, carrier, .. } = self;
            let backing = scoped(budget, |budget| {
                let base = base::build(
                    &live,
                    &carrier,
                    invocation(&live.bindings.transaction.compiler_custody)?,
                    budget,
                )?;
                pack(carrier, &base, live.native_output_parts().0, budget)
            })?;
            let growth = backing
                .capacity()
                .checked_sub(old_capacity)
                .ok_or(Resource::Accounting)?;
            budget.reserve_storage(growth)?;
            let handoff = decode(backing, budget)?;
            let added = header
                .checked_add(growth)
                .and_then(|n| n.checked_add(DECODE_STORAGE))
                .ok_or(Resource::Arithmetic)?;
            let retained_floor = floor.checked_add(added).ok_or(Resource::Arithmetic)?;
            budget.charge_work(1)?;
            Ok((
                PreparedRefinedForwardingCapsuleV4 {
                    live,
                    handoff,
                    retained_floor,
                },
                RefinedForwardingCapsuleStorageV4(added),
            ))
        })
    }
}

fn decode(backing: Vec<u8>, budget: &mut Budget<'_>) -> R<Handoff> {
    if budget.storage() < backing.capacity() {
        return Err(Resource::Accounting.into());
    }
    budget.reserve_storage(DECODE_STORAGE)?;
    budget.charge_work(
        inert_semantic_compiler_module_handoff_decode_work_v4(backing.len()).map_err(E::Handoff)?,
    )?;
    Handoff::decode_owned(backing).map_err(E::Handoff)
}

// The carrier allocation becomes the final outer allocation. Reallocation may
// move it; no pointer-stability claim is made. Its old capacity remains prepaid.
fn pack(
    mut bytes: Vec<u8>,
    base: &Base,
    module: &fe2o3_compiler_ffi::CompilerModuleHandoffV2,
    budget: &mut Budget<'_>,
) -> R<Vec<u8>> {
    let capsule =
        CapsuleLayout::new(base.canonical_bytes().len(), bytes.len()).map_err(E::Capsule)?;
    let outer = HandoffLayout::new(capsule.encoded_len(), module.canonical_bytes().len())
        .map_err(E::Handoff)?;
    let old_len = bytes.len();
    let old_capacity = bytes.capacity();
    let new_capacity = old_capacity.max(outer.encoded_len());
    budget.reserve_storage(new_capacity - old_capacity)?;
    budget.charge_work(1)?;
    bytes
        .try_reserve_exact(outer.encoded_len() - old_len)
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        bytes
            .capacity()
            .checked_sub(new_capacity)
            .ok_or(Resource::Accounting)?,
    )?;
    budget.charge_work(outer.encoded_len() - old_len)?;
    bytes.resize(outer.encoded_len(), 0);
    budget.charge_work(old_len)?;
    bytes.copy_within(
        0..old_len,
        outer.capsule_range().start + capsule.carrier_range().start,
    );
    budget.charge_work(base.canonical_bytes().len())?;
    bytes[outer.capsule_range()][capsule.base_range()].copy_from_slice(base.canonical_bytes());
    budget.charge_work(module.canonical_bytes().len())?;
    bytes[outer.module_handoff_range()].copy_from_slice(module.canonical_bytes());
    budget.reserve_storage(CAPSULE_STORAGE + SEAL_STORAGE)?;
    let limit = budget.storage_limit();
    let capsule_identity = seal_inert_production_semantic_capsule_v4(
        capsule,
        &mut bytes[outer.capsule_range()],
        limit,
        |w| budget.charge_work(w),
    )
    .map_err(E::Capsule)?;
    seal_inert_semantic_compiler_module_handoff_v4(
        outer,
        &mut bytes,
        capsule_identity,
        module.identity(),
        |w| budget.charge_work(w),
    )
    .map_err(E::Seal)?;
    Ok(bytes)
}

#[cfg(test)]
#[path = "production_refined_forwarding_capsule_v4_tests.rs"]
mod tests;
