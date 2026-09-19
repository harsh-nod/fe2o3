//! Five typed identities and exact existing signed-roster bytes. No new proof.
use super::super::{PreparedNativeCheckedOutputWorkerHandoffPolicy6V1, SourceProofV1, ranked};
use super::*;
use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3 as Identity, InertMiddleEndReceiptV3,
    InertMirToKirCorrespondenceReceiptV3, InertProofBindingAssociationV4 as Association,
    LineageErrorV3, MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3,
    MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V4, MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4,
};

#[cfg(test)]
#[path = "production_native_input_association_source_identity_v1_tests.rs"]
pub(super) mod source_identity_tests;

pub(super) fn check(
    native: &PreparedNativeCheckedOutputWorkerHandoffPolicy6V1,
    receipts: &NativeOriginalInputAssociationReceiptsPolicy6V1,
    budget: &mut Budget<'_>,
) -> R<()> {
    let inputs = native.stage.inputs();
    let kernel = match &native.stage {
        Stage6::Direct(stage) => stage.native_original_envelope_v1(),
        Stage6::Erased(stage) => stage.native_original_envelope_v1(),
    };
    let original_owner = inputs.output.owner;
    check_parts(
        inputs,
        original_owner,
        ranked(native),
        kernel,
        receipts,
        budget,
    )
}

/// The fixed J owner supplies original N through its sole retained Prefix6.
/// This is source evidence only; final-J admission uses the separate J entry.
pub(in crate::production_pipeline) fn check_policy7(
    native: &crate::production_pipeline::checked_output_policy7_v1::native::PreparedNativeCheckedOutputWorkerHandoffPolicy7V1,
    receipts: &NativeOriginalInputAssociationReceiptsPolicy6V1,
    budget: &mut Budget<'_>,
) -> R<()> {
    check_parts(
        native.inputs(),
        native.original_source_view(),
        native.ranked(),
        native.original_kernel_ir_preimage(),
        receipts,
        budget,
    )
}

fn check_parts(
    inputs: crate::production_pipeline::native_checked_output_handoff_v1::StageInputsV1<'_>,
    original_owner: crate::production_pipeline::native_checked_output_handoff_v1::OutputOwnerV1<'_>,
    ranked: &crate::production_ranked_projection_v1::AuthenticatedRankedVerificationRosterV1,
    kernel: &[u8],
    receipts: &NativeOriginalInputAssociationReceiptsPolicy6V1,
    budget: &mut Budget<'_>,
) -> R<()> {
    let source = inputs.output.owner.source(inputs.output.catalog)?;
    // Reuse the exact original-N analysis and two indexed name joins once.
    super::super::super::original_receipts::check_component(
        original_owner,
        inputs.output.catalog,
        ranked,
        &inputs.bindings.typed_descriptor_roots,
        kernel,
        receipts.formal_memory.canonical_preimage(),
        budget,
    )
    .map_err(|error| E::Original(Box::new(error)))?;
    let (middle, correspondence, verus) = match inputs.proof {
        SourceProofV1::Direct(proof) => (
            proof.middle_end_roster().canonical_bytes(),
            proof.correspondence_roster().canonical_bytes(),
            proof.verus_roster().canonical_bytes(),
        ),
        SourceProofV1::Erased(proof) => (
            proof.middle_end_roster().canonical_bytes(),
            proof.correspondence_roster().canonical_bytes(),
            proof.verus_roster().canonical_bytes(),
        ),
    };
    check_identities(
        Preimages {
            semantic: source.semantic.canonical_encoding(),
            middle,
            kernel,
            correspondence,
            verus,
        },
        receipts,
        budget,
    )
}

// Inert comparison inputs, never a public proof/owner constructor. Production
// derives every field above from the same retained native owner after replay.
#[derive(Clone, Copy)]
struct Preimages<'a> {
    semantic: &'a [u8],
    middle: &'a [u8],
    kernel: &'a [u8],
    correspondence: &'a [u8],
    verus: &'a [u8],
}

fn check_identities(
    expected: Preimages<'_>,
    receipts: &NativeOriginalInputAssociationReceiptsPolicy6V1,
    budget: &mut Budget<'_>,
) -> R<()> {
    scoped(budget, |budget| {
        budget.charge_work(6)?;
        if budget.storage() < receipts.retained {
            return Err(Resource::Accounting.into());
        }
        if expected.verus.is_empty()
            || expected.verus.len() > MAX_INERT_PROOF_BINDING_VERUS_EVIDENCE_BYTES_V4
        {
            return Err(E::Mismatch("complete native Verus evidence extent"));
        }
        let wire = receipts.proof_binding.canonical_preimage();
        if wire.is_empty() || wire.len() > MAX_INERT_PROOF_BINDING_ASSOCIATION_BYTES_V4 {
            return Err(E::Mismatch("original V4 association extent"));
        }
        // The existing codec reconstructs the canonical envelope. Its capped
        // codec allocations remain a separate domain; charge visible scratch
        // plus conservative canonical Vec-to-Box overlap here.
        let decoded = wire
            .len()
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(size_of::<Association>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(decoded)?;
        budget.charge_work(wire.len().checked_mul(2).ok_or(Resource::Arithmetic)?)?;
        let association = Association::decode(wire)
            .map_err(|_| E::Mismatch("original V4 association encoding"))?;
        let inputs = association.inputs();
        let semantic = typed_identity(
            expected.semantic,
            MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3,
            budget,
            InertCanonicalSemanticMirReceiptV3::from_canonical_preimage,
            |receipt| (*receipt.identity().sha256(), receipt.identity().byte_len()),
        )?;
        exact_identity(
            inputs.semantic_mir(),
            semantic,
            "original semantic receipt identity",
            budget,
        )?;
        let middle = typed_identity(
            expected.middle,
            MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
            budget,
            InertMiddleEndReceiptV3::from_canonical_preimage,
            |receipt| (*receipt.identity().sha256(), receipt.identity().byte_len()),
        )?;
        exact_identity(
            inputs.middle_end(),
            middle,
            "original Middle receipt identity",
            budget,
        )?;
        let kernel = typed_identity(
            expected.kernel,
            MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
            budget,
            InertKernelIrReceiptV3::from_canonical_preimage,
            |receipt| (*receipt.identity().sha256(), receipt.identity().byte_len()),
        )?;
        exact_identity(
            inputs.kernel_ir(),
            kernel,
            "original KernelIr receipt identity",
            budget,
        )?;
        let correspondence = typed_identity(
            expected.correspondence,
            MAX_LINEAGE_RECEIPT_PREIMAGE_BYTES_V3,
            budget,
            InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage,
            |receipt| (*receipt.identity().sha256(), receipt.identity().byte_len()),
        )?;
        exact_identity(
            inputs.mir_to_kir_correspondence(),
            correspondence,
            "original correspondence receipt identity",
            budget,
        )?;
        let formal = receipts.formal_memory.identity();
        exact_identity(
            inputs.formal_memory(),
            (*formal.sha256(), formal.byte_len()),
            "original FormalMemory receipt identity",
            budget,
        )?;
        let actual_verus = association.verus_execution_evidence();
        budget.charge_work(
            actual_verus
                .len()
                .checked_add(expected.verus.len())
                .and_then(|bytes| bytes.checked_add(1))
                .ok_or(Resource::Arithmetic)?,
        )?;
        if actual_verus != expected.verus {
            return Err(E::Mismatch("exact retained native Verus roster"));
        }
        Ok(())
    })
}

fn exact_identity(
    actual: Identity,
    expected: ([u8; 32], u64),
    field: &'static str,
    budget: &mut Budget<'_>,
) -> R<()> {
    budget.charge_work(80)?;
    if actual.sha256() != expected.0 || actual.byte_len() != expected.1 {
        return Err(E::Mismatch(field));
    }
    Ok(())
}

fn typed_identity<T>(
    bytes: &[u8],
    limit: usize,
    budget: &mut Budget<'_>,
    construct: impl FnOnce(Vec<u8>) -> Result<T, LineageErrorV3>,
    identity: impl FnOnce(&T) -> ([u8; 32], u64),
) -> R<([u8; 32], u64)> {
    scoped(budget, |budget| {
        budget.charge_work(2)?;
        if bytes.is_empty() || bytes.len() > limit {
            return Err(E::Mismatch("original typed identity preimage extent"));
        }
        let storage = bytes
            .len()
            .checked_mul(2)
            .and_then(|bytes| bytes.checked_add(size_of::<T>()))
            .ok_or(Resource::Arithmetic)?;
        budget.reserve_storage(storage)?;
        budget.charge_work(bytes.len().checked_mul(2).ok_or(Resource::Arithmetic)?)?;
        let temporary = construct(copy_prepaid(bytes, budget)?)
            .map_err(|_| E::Mismatch("original typed receipt identity"))?;
        let result = identity(&temporary);
        drop(temporary);
        Ok(result)
    })
}

#[cfg(test)]
pub(super) mod test_support {
    use super::*;

    // These test-only views exercise inert identity comparisons, not source
    // admission or signed-native custody. They cannot construct either owner.
    pub(in super::super) fn check(
        expected: [&[u8]; 5],
        receipts: &NativeOriginalInputAssociationReceiptsPolicy6V1,
        budget: &mut Budget<'_>,
    ) -> R<()> {
        check_identities(
            Preimages {
                semantic: expected[0],
                middle: expected[1],
                kernel: expected[2],
                correspondence: expected[3],
                verus: expected[4],
            },
            receipts,
            budget,
        )
    }

    pub(in super::super) fn semantic_identity(
        bytes: &[u8],
        budget: &mut Budget<'_>,
    ) -> R<([u8; 32], u64)> {
        typed_identity(
            bytes,
            MAX_CANONICAL_SEMANTIC_MIR_BYTES_V3,
            budget,
            InertCanonicalSemanticMirReceiptV3::from_canonical_preimage,
            |receipt| (*receipt.identity().sha256(), receipt.identity().byte_len()),
        )
    }
}
