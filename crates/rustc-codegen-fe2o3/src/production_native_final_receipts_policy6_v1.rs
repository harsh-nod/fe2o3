//! Fixed in-process final-I receipt admission, not F2NOUT1 or publication.
//! Original-N formal input and heterogeneous serialized-chain admission remain
//! separate gaps. No signed source owner is constructed from these inert bytes.
use super::*;
use crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1 as Ranked;
use fe2o3_compiler_lineage::{
    InertFormalMemoryReceiptV3, InertKernelIrReceiptV3, MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
};

#[path = "production_native_final_receipt_joins_policy6_v1.rs"]
mod joins;
#[cfg(test)]
#[path = "production_native_final_receipts_policy6_v1_tests.rs"]
pub(crate) mod tests;

/// Logical owned headers and exact canonical payloads. No shared backing is
/// accepted: the constructor below copies into the existing owned codecs.
pub(crate) struct NativeFinalOutputReceiptsPolicy6V1 {
    kernel_ir: InertKernelIrReceiptV3,
    formal_memory: InertFormalMemoryReceiptV3,
    retained: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FinalReceiptStoragePolicy6V1(usize);
impl FinalReceiptStoragePolicy6V1 {
    pub(crate) const fn retained_storage(self) -> usize {
        self.0
    }
}

impl NativeFinalOutputReceiptsPolicy6V1 {
    /// Copies capped inert preimages; this does not admit their contents. Caller
    /// input bytes remain reserved separately. Reserve the returned full receipt
    /// before any later controlled allocation and drop this owner before release.
    pub(crate) fn try_from_preimages_v1(
        kernel_ir: &[u8],
        formal_memory: &[u8],
        budget: &mut Budget<'_>,
    ) -> R<(Self, FinalReceiptStoragePolicy6V1)> {
        scoped(budget, |budget| {
            budget.charge_work(4)?;
            if [kernel_ir, formal_memory]
                .iter()
                .any(|b| b.is_empty() || b.len() > MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3)
            {
                return Err(E::Mismatch("final receipt preimage extent"));
            }
            let bytes = kernel_ir
                .len()
                .checked_add(formal_memory.len())
                .ok_or(Resource::Arithmetic)?;
            let retained = size_of::<Self>()
                .checked_add(bytes)
                .ok_or(Resource::Arithmetic)?;
            // Vector-to-box shrinking may transiently retain both buffers.
            budget.reserve_storage(retained.checked_add(bytes).ok_or(Resource::Arithmetic)?)?;
            budget.charge_work(bytes.checked_mul(3).ok_or(Resource::Arithmetic)?)?;
            let copy = |bytes: &[u8]| -> R<Vec<u8>> {
                let mut out = Vec::new();
                out.try_reserve_exact(bytes.len())
                    .map_err(|_| Resource::Allocation)?;
                out.extend_from_slice(bytes);
                Ok(out)
            };
            let kernel = copy(kernel_ir)?;
            let formal = copy(formal_memory)?;
            let excess = kernel
                .capacity()
                .checked_sub(kernel_ir.len())
                .and_then(|n| n.checked_add(formal.capacity().checked_sub(formal_memory.len())?))
                .ok_or(Resource::Accounting)?;
            budget.reserve_storage(excess)?;
            let value = Self {
                kernel_ir: InertKernelIrReceiptV3::from_canonical_preimage(kernel)
                    .map_err(|_| E::Mismatch("final KernelIr receipt identity"))?,
                formal_memory: InertFormalMemoryReceiptV3::from_canonical_preimage(formal)
                    .map_err(|_| E::Mismatch("final FormalMemory receipt identity"))?,
                retained,
            };
            Ok((value, FinalReceiptStoragePolicy6V1(retained)))
        })
    }
    pub(crate) fn kernel_ir(&self) -> &InertKernelIrReceiptV3 {
        &self.kernel_ir
    }
    pub(crate) fn formal_memory(&self) -> &InertFormalMemoryReceiptV3 {
        &self.formal_memory
    }
    pub(crate) const fn retained_storage(&self) -> usize {
        self.retained
    }
}

/// Non-Clone witness tied to both exact borrowed owners, not to digests alone.
pub(crate) struct CheckedNativeFinalOutputReceiptsPolicy6V1<'n, 'r> {
    native: &'n PreparedNativeCheckedOutputWorkerHandoffPolicy6V1,
    receipts: &'r NativeFinalOutputReceiptsPolicy6V1,
}
impl CheckedNativeFinalOutputReceiptsPolicy6V1<'_, '_> {
    pub(crate) fn output(&self) -> &Graph {
        self.native.output()
    }
    pub(crate) fn receipts(&self) -> &NativeFinalOutputReceiptsPolicy6V1 {
        self.receipts
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
}

fn ranked(native: &PreparedNativeCheckedOutputWorkerHandoffPolicy6V1) -> &Ranked {
    match &native.stage {
        Stage6::Direct(stage) => stage.native_final_receipt_ranked_v1(),
        Stage6::Erased(stage) => stage.native_final_receipt_ranked_v1(),
    }
}

/// Replays the complete existing source/signed/optimization/native relation,
/// then binds the supplied final receipts. No source, target or policy selector.
pub(crate) fn check_native_final_output_receipts_policy6_v1<'n, 'r>(
    native: &'n PreparedNativeCheckedOutputWorkerHandoffPolicy6V1,
    receipts: &'r NativeFinalOutputReceiptsPolicy6V1,
    budget: &mut Budget<'_>,
) -> R<(
    CheckedNativeFinalOutputReceiptsPolicy6V1<'n, 'r>,
    FinalReceiptStoragePolicy6V1,
)> {
    scoped(budget, |budget| {
        budget.charge_work(3)?;
        let required = native
            .retained_floor
            .checked_add(receipts.retained)
            .ok_or(Resource::Arithmetic)?;
        if budget.storage() < required {
            return Err(Resource::Accounting.into());
        }
        native.verify_equivalence(budget)?;
        let inputs = native.stage.inputs();
        joins::check(
            inputs.output,
            ranked(native),
            receipts,
            &inputs.bindings.typed_descriptor_roots,
            budget,
        )?;
        let storage = size_of::<CheckedNativeFinalOutputReceiptsPolicy6V1<'_, '_>>();
        budget.reserve_storage(storage)?;
        Ok((
            CheckedNativeFinalOutputReceiptsPolicy6V1 { native, receipts },
            FinalReceiptStoragePolicy6V1(storage),
        ))
    })
}

/// Retains the sole native stage and exact inert receipt bytes. This is neither
/// a signed final refinement proof nor a protected publication capability.
pub(crate) struct PreparedNativeFinalOutputPolicy6V1 {
    native: PreparedNativeCheckedOutputWorkerHandoffPolicy6V1,
    receipts: NativeFinalOutputReceiptsPolicy6V1,
    retained_floor: usize,
}
impl PreparedNativeFinalOutputPolicy6V1 {
    pub(crate) fn output(&self) -> &Graph {
        self.native.output()
    }
    pub(crate) fn handoff(&self) -> &fe2o3_compiler_ffi::CompilerModuleHandoffV2 {
        self.native.handoff()
    }
    pub(crate) fn receipts(&self) -> &NativeFinalOutputReceiptsPolicy6V1 {
        &self.receipts
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> R<()> {
        if budget.storage() < self.retained_floor {
            return Err(Resource::Accounting.into());
        }
        check_native_final_output_receipts_policy6_v1(&self.native, &self.receipts, budget)?;
        Ok(())
    }
}
impl PreparedNativeCheckedOutputWorkerHandoffPolicy6V1 {
    /// Consumes already-reserved native and receipt owners once. All incoming
    /// reservations remain caller-owned on every exit. Success transfers only
    /// the additional wrapper delta, unreserved; no graph/history is copied.
    pub(crate) fn try_admit_final_output_receipts_v1(
        self,
        receipts: NativeFinalOutputReceiptsPolicy6V1,
        budget: &mut Budget<'_>,
    ) -> R<(
        PreparedNativeFinalOutputPolicy6V1,
        FinalReceiptStoragePolicy6V1,
    )> {
        scoped(budget, move |budget| {
            check_native_final_output_receipts_policy6_v1(&self, &receipts, budget)?;
            let delta = size_of::<PreparedNativeFinalOutputPolicy6V1>()
                .checked_sub(size_of::<Self>())
                .and_then(|n| n.checked_sub(size_of::<NativeFinalOutputReceiptsPolicy6V1>()))
                .ok_or(Resource::Arithmetic)?;
            budget.reserve_storage(delta)?;
            let retained_floor = self
                .retained_floor
                .checked_add(receipts.retained)
                .and_then(|n| n.checked_add(delta))
                .ok_or(Resource::Arithmetic)?;
            Ok((
                PreparedNativeFinalOutputPolicy6V1 {
                    native: self,
                    receipts,
                    retained_floor,
                },
                FinalReceiptStoragePolicy6V1(delta),
            ))
        })
    }
}
