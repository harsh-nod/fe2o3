//! Original-N V4 association on a distinct signed final-K owner. No new schema.
use super::*;
use super::final_receipts::{
    NativeFinalOutputReceiptsPolicy8V1, PreparedNativeFinalOutputPolicy8V1, finish_pair, require_pair,
};
use crate::production_pipeline::native_checked_output_handoff_v1::policy6::final_receipts::input_association::{
    NativeInputAssociationErrorPolicy6V1, NativeOriginalInputAssociationReceiptsPolicy6V1 as Payloads,
    joins,
};
fn input_error(value: NativeInputAssociationErrorPolicy6V1) -> ProductionPipelineError {
    error(CheckedOutputPolicy8StageErrorV1::InputAssociation(
        Box::new(value),
    ))
}

/// Opaque original-N formal and V4 receipt payloads, not signed custody.
pub(crate) struct NativeOriginalInputAssociationReceiptsPolicy8V1(Payloads);
impl NativeOriginalInputAssociationReceiptsPolicy8V1 {
    /// Copies bounded caller-owned/external backing. Reserve the full returned
    /// receipt before subsequent controlled work; drop before releasing it.
    pub(crate) fn try_from_preimages_v1(
        original_formal: &[u8],
        proof_binding_v4: &[u8],
        budget: &mut Budget<'_>,
    ) -> Result8<(Self, Policy8NativeStorageV1)> {
        scoped(0, budget, |budget| {
            let (payloads, receipt) =
                Payloads::try_from_preimages_v1(original_formal, proof_binding_v4, budget)
                    .map_err(input_error)?;
            let extra = size_of::<Self>().saturating_sub(size_of::<Payloads>());
            let retained = receipt
                .retained_storage()
                .checked_add(extra)
                .ok_or_else(|| resource(Resource::Arithmetic))?;
            budget.reserve_storage(retained).map_err(resource)?;
            Ok((Self(payloads), Policy8NativeStorageV1(retained)))
        })
    }
    pub(crate) fn formal_memory(&self) -> &fe2o3_compiler_lineage::InertFormalMemoryReceiptV3 {
        self.0.formal_memory()
    }
    pub(crate) fn proof_binding(&self) -> &fe2o3_compiler_lineage::InertProofBindingReceiptV3 {
        self.0.proof_binding()
    }
    pub(crate) fn retained_storage(&self) -> usize {
        self.0.retained_storage() + size_of::<Self>().saturating_sub(size_of::<Payloads>())
    }
}

fn check(
    final_output: &PreparedNativeFinalOutputPolicy8V1,
    receipts: &NativeOriginalInputAssociationReceiptsPolicy8V1,
    budget: &mut Budget<'_>,
) -> Result8<usize> {
    scoped(0, budget, |budget| {
        let incoming = require_pair(
            final_output.retained_storage_floor_v1(),
            receipts.retained_storage(),
            budget,
        )?;
        final_output.verify_equivalence(budget)?;
        joins::check_policy8(&final_output.native, &receipts.0, budget).map_err(input_error)?;
        Ok(incoming)
    })
}

/// Retains the one signed original-N owner, checked history, final-K receipts
/// and five-identity V4 association. No serialized heterogeneous proof is implied.
pub(crate) struct PreparedNativeAssociatedInputFinalOutputPolicy8V1 {
    final_output: PreparedNativeFinalOutputPolicy8V1,
    input_receipts: NativeOriginalInputAssociationReceiptsPolicy8V1,
    retained_floor: usize,
}
impl PreparedNativeFinalOutputPolicy8V1 {
    /// Consume both already-reserved owners. Return only added wrapper storage,
    /// unreserved; neither N backing nor prefix nor K artifacts are copied.
    pub(crate) fn try_admit_original_input_association_v1(
        self,
        receipts: NativeOriginalInputAssociationReceiptsPolicy8V1,
        budget: &mut Budget<'_>,
    ) -> Result8<(
        PreparedNativeAssociatedInputFinalOutputPolicy8V1,
        Policy8NativeStorageV1,
    )> {
        scoped(0, budget, move |budget| {
            let incoming = check(&self, &receipts, budget)?;
            let (retained_floor, receipt) = finish_pair::<
                PreparedNativeAssociatedInputFinalOutputPolicy8V1,
                Self,
                NativeOriginalInputAssociationReceiptsPolicy8V1,
            >(incoming, budget)?;
            Ok((
                PreparedNativeAssociatedInputFinalOutputPolicy8V1 {
                    final_output: self,
                    input_receipts: receipts,
                    retained_floor,
                },
                receipt,
            ))
        })
    }
}
impl PreparedNativeAssociatedInputFinalOutputPolicy8V1 {
    pub(crate) fn output(&self) -> &Graph {
        self.final_output.output()
    }
    pub(crate) fn original(&self) -> &Graph {
        self.final_output.original()
    }
    pub(crate) const fn policy_version(&self) -> u16 {
        8
    }
    pub(crate) fn prefix_execution(&self) -> &Policy7ExecutionWitnessV1 {
        self.final_output.prefix_execution()
    }
    pub(crate) fn handoff(&self) -> &fe2o3_compiler_ffi::CompilerModuleHandoffV2 {
        self.final_output.handoff()
    }
    pub(crate) fn original_kernel_ir_preimage(&self) -> &[u8] {
        self.final_output.original_kernel_ir_preimage()
    }
    pub(crate) fn input_receipts(&self) -> &NativeOriginalInputAssociationReceiptsPolicy8V1 {
        &self.input_receipts
    }
    pub(crate) fn final_receipts(&self) -> &NativeFinalOutputReceiptsPolicy8V1 {
        self.final_output.receipts()
    }
    pub(crate) const fn retained_storage_floor_v1(&self) -> usize {
        self.retained_floor
    }
    pub(crate) const fn grants_artifact_or_launch_authority(&self) -> bool {
        false
    }
    pub(crate) fn verify_equivalence(&self, budget: &mut Budget<'_>) -> Result8<()> {
        scoped(self.retained_floor, budget, |budget| {
            check(&self.final_output, &self.input_receipts, budget).map(|_| ())
        })
    }
}
