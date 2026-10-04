//! Typed final-K receipt custody. The underlying codecs are unchanged inert bytes.
use super::*;
use crate::production_pipeline::native_checked_output_handoff_v1::policy6::final_receipts::{
    NativeFinalOutputReceiptsPolicy6V1 as Payloads, joins,
};

/// A distinct type prevents accidental use of a historical I/J endpoint.
/// It grants no authority and never constructs a signed native owner.
pub(crate) struct NativeFinalOutputReceiptsPolicy8V1(Payloads);
impl NativeFinalOutputReceiptsPolicy8V1 {
    /// Caller backing is borrowed/external; the returned receipt covers the new
    /// owned copy only. Reserve it before later work and drop before releasing.
    pub(crate) fn try_from_preimages_v1(
        kernel_ir: &[u8],
        formal_memory: &[u8],
        budget: &mut Budget<'_>,
    ) -> Result8<(Self, Policy8NativeStorageV1)> {
        scoped(0, budget, |budget| {
            let (payloads, receipt) =
                Payloads::try_from_preimages_v1(kernel_ir, formal_memory, budget)
                    .map_err(native_error)?;
            let extra = size_of::<Self>().saturating_sub(size_of::<Payloads>());
            let retained = receipt
                .retained_storage()
                .checked_add(extra)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(retained).map_err(resource)?;
            Ok((Self(payloads), Policy8NativeStorageV1(retained)))
        })
    }
    pub(crate) fn kernel_ir(&self) -> &fe2o3_compiler_lineage::InertKernelIrReceiptV3 {
        self.0.kernel_ir()
    }
    pub(crate) fn formal_memory(&self) -> &fe2o3_compiler_lineage::InertFormalMemoryReceiptV3 {
        self.0.formal_memory()
    }
    pub(crate) fn retained_storage(&self) -> usize {
        // Checked once at construction; this newtype never grows later.
        self.0.retained_storage() + size_of::<Self>().saturating_sub(size_of::<Payloads>())
    }
}

pub(super) fn require_pair(left: usize, right: usize, budget: &mut Budget<'_>) -> Result8<usize> {
    budget.charge_work(3).map_err(resource)?;
    let required = left
        .checked_add(right)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    if budget.storage() < required {
        return Err(resource(Resource::Accounting));
    }
    Ok(required)
}
pub(super) fn finish_pair<T, A, B>(
    incoming: usize,
    budget: &mut Budget<'_>,
) -> Result8<(usize, Policy8NativeStorageV1)> {
    let credited = size_of::<A>()
        .checked_add(size_of::<B>())
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    let delta = size_of::<T>().saturating_sub(credited);
    let retained = incoming
        .checked_add(delta)
        .ok_or_else(|| resource(Resource::Arithmetic))?;
    budget.reserve_storage(delta).map_err(resource)?;
    Ok((retained, Policy8NativeStorageV1(delta)))
}

fn check(
    native: &PreparedNativeCheckedOutputWorkerHandoffPolicy8V1,
    receipts: &NativeFinalOutputReceiptsPolicy8V1,
    budget: &mut Budget<'_>,
) -> Result8<usize> {
    scoped(0, budget, |budget| {
        let required = require_pair(native.retained_floor, receipts.retained_storage(), budget)?;
        // There is no unsigned constructor or receipt-only shortcut across this
        // boundary. Full signed N/source/native replay precedes final-K joins.
        native.verify_equivalence(budget)?;
        let inputs = native.inputs();
        joins::check_policy8(
            inputs.output,
            native.ranked(),
            &receipts.0,
            &inputs.bindings.typed_descriptor_roots,
            budget,
        )
        .map_err(native_error)?;
        Ok(required)
    })
}

/// Retains actual signed N/(E), checked history through K, and exact K receipts.
/// This is not a signed final refinement proof or heterogeneous wire admission.
pub(crate) struct PreparedNativeFinalOutputPolicy8V1 {
    pub(super) native: PreparedNativeCheckedOutputWorkerHandoffPolicy8V1,
    receipts: NativeFinalOutputReceiptsPolicy8V1,
    retained_floor: usize,
}
impl PreparedNativeCheckedOutputWorkerHandoffPolicy8V1 {
    /// Consume two already-reserved owners. Return only wrapper growth,
    /// unreserved; incoming reservations remain caller-owned on every exit.
    pub(crate) fn try_admit_final_output_receipts_v1(
        self,
        receipts: NativeFinalOutputReceiptsPolicy8V1,
        budget: &mut Budget<'_>,
    ) -> Result8<(PreparedNativeFinalOutputPolicy8V1, Policy8NativeStorageV1)> {
        scoped(0, budget, move |budget| {
            let incoming = check(&self, &receipts, budget)?;
            let (retained_floor, receipt) = finish_pair::<
                PreparedNativeFinalOutputPolicy8V1,
                Self,
                NativeFinalOutputReceiptsPolicy8V1,
            >(incoming, budget)?;
            Ok((
                PreparedNativeFinalOutputPolicy8V1 {
                    native: self,
                    receipts,
                    retained_floor,
                },
                receipt,
            ))
        })
    }
}
impl PreparedNativeFinalOutputPolicy8V1 {
    pub(crate) fn output(&self) -> &Graph {
        self.native.output()
    }
    pub(crate) fn original(&self) -> &Graph {
        self.native.original()
    }
    pub(crate) const fn policy_version(&self) -> u16 {
        8
    }
    pub(crate) fn prefix_execution(&self) -> &Policy7ExecutionWitnessV1 {
        self.native.prefix_execution()
    }
    pub(crate) fn handoff(&self) -> &fe2o3_compiler_ffi::CompilerModuleHandoffV2 {
        self.native.handoff()
    }
    pub(crate) fn original_kernel_ir_preimage(&self) -> &[u8] {
        self.native.original_kernel_ir_preimage()
    }
    pub(crate) fn receipts(&self) -> &NativeFinalOutputReceiptsPolicy8V1 {
        &self.receipts
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> Result8<()> {
        scoped(self.retained_floor, budget, |budget| {
            check(&self.native, &self.receipts, budget).map(|_| ())
        })
    }
}

#[cfg(test)]
pub(super) fn check_component(
    artifacts: &PreparedPolicy8ArtifactsV1,
    ranked: &Ranked,
    typed: &[crate::compiler_descriptor::TypedDescriptorRootV1],
    receipts: &NativeFinalOutputReceiptsPolicy8V1,
    budget: &mut Budget<'_>,
) -> Result8<()> {
    joins::check_policy8(
        artifacts.native_worker_output_v1(),
        ranked,
        &receipts.0,
        typed,
        budget,
    )
    .map_err(native_error)
}
