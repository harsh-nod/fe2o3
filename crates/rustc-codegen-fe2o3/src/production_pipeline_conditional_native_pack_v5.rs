//! One growing carrier allocation becomes the outer handoff backing.
use super::*;
use fe2o3_compiler_ffi::{
    INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_SEAL_STORAGE_V5,
    InertFinalCompilerModuleCommitmentV3 as Commitment,
    InertSemanticCompilerModuleHandoffLayoutV5 as Outer,
    MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3, seal_inert_semantic_compiler_module_handoff_v5,
};
use fe2o3_rustc_invocation::{MAX_DESCRIPTOR_BYTES_V3, encode_descriptor_v3};
use sha2::{Digest, Sha256};

const SCRATCH: usize = NATIVE_CONDITIONAL_OUTPUT_WORKING_STORAGE_V1
    + NATIVE_CONDITIONAL_CARRIER_WORKING_STORAGE_V1
    + NATIVE_CONDITIONAL_METADATA_WORKING_STORAGE_V1
    + INERT_PRODUCTION_SEMANTIC_CAPSULE_WORKING_STORAGE_V5
    + INERT_SEMANTIC_COMPILER_MODULE_HANDOFF_SEAL_STORAGE_V5
    + size_of::<NativeLoweringAssociationInputsV1>()
    + size_of::<InertNativeLoweringAssociationV1>()
    + size_of::<InertNativeNeutralSubjectV1>()
    + size_of::<Sha256>()
    + size_of::<Error>()
    + 512;

pub(super) fn prepare(prefix: &Prefix, module: &Module, budget: &mut Budget<'_>) -> R<Vec<u8>> {
    budget.reserve_storage(SCRATCH)?;
    let content = &prefix.content;
    let output = NativeConditionalOutputLayoutV1::new(
        content.history.canonical_bytes().len(),
        content.catalog.canonical_bytes().len(),
        content.descriptor.canonical_bytes().len(),
    )
    .map_err(Error::Output)?;
    let carrier = NativeConditionalCarrierLayoutV1::new(
        output.encoded_len(),
        prefix.packet.source_packet().len(),
    )
    .map_err(Error::Carrier)?;
    let mut bytes = Vec::new();
    grow(&mut bytes, carrier.encoded_len(), budget)?;
    let output_bytes = &mut bytes[carrier.output_range()];
    copy(
        &mut output_bytes[output.history_range()],
        content.history.canonical_bytes(),
        budget,
    )?;
    copy(
        &mut output_bytes[output.catalog_range()],
        content.catalog.canonical_bytes(),
        budget,
    )?;
    copy(
        &mut output_bytes[output.descriptor_range()],
        content.descriptor.canonical_bytes(),
        budget,
    )?;
    let limit = budget.storage_limit();
    seal_native_conditional_output_v1(output, output_bytes, limit, |w| budget.charge_work(w))
        .map_err(Error::Output)?;
    copy(
        &mut bytes[carrier.source_range()],
        prefix.packet.source_packet(),
        budget,
    )?;
    let carrier_id =
        seal_native_conditional_carrier_v1(carrier, &mut bytes, limit, |w| budget.charge_work(w))
            .map_err(Error::Carrier)?;

    let bindings = &prefix.preparation.bindings;
    let target = &bindings.rustc_target;
    let invocation = invocation(&bindings.transaction.compiler_custody)?;
    if bindings
        .rustc_preflight_plan
        .rustc_identity_inventory_sha256()
        != bindings.rustc_identity_inventory.sha256()
        || invocation.amd_target() != target.profile().device_target()
        || module.target() != target.device_target()
    {
        return Err(Error::Mismatch(
            "conditional native invocation/inventory/target",
        ));
    }
    codec::<Vec<u8>>(MAX_DESCRIPTOR_BYTES_V3, budget)?;
    budget.charge_work(128 * MAX_DESCRIPTOR_BYTES_V3)?;
    let invocation = encode_descriptor_v3(invocation).map_err(Error::Invocation)?;
    let layout = target.rustc_layout();
    codec::<Box<[u8]>>(MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3, budget)?;
    let layout = canonical_semantic_target_layout_transcript_v1(
        layout.llvm_target(),
        layout.data_layout(),
        layout.default_pointer_width_bits(),
        layout
            .active_cpu()
            .ok_or(Error::Mismatch("conditional native CPU"))?,
        layout
            .active_features()
            .ok_or(Error::Mismatch("conditional native target features"))?,
    )
    .map_err(Error::Lineage)?;
    budget.charge_work(layout.len().checked_add(128).ok_or(Resource::Arithmetic)?)?;
    let expected_layout = prefix
        .preparation
        .ranked
        .materialized()
        .semantic_ssa()
        .source_semantic()
        .target_layout_identity();
    if Sha256::digest(&layout).as_slice() != expected_layout.as_bytes() {
        return Err(Error::Mismatch("conditional native semantic target layout"));
    }
    budget.charge_work(NATIVE_LOWERING_ASSOCIATION_WORK_V1 + 512)?;
    let final_id = prefix.chain.output().canonical().identity();
    let subject = InertNativeNeutralSubjectV1::new(
        *final_id.digest(),
        final_id.canonical_length(),
        *content.catalog.digest(),
        u64::try_from(content.catalog.canonical_bytes().len()).map_err(|_| Resource::Arithmetic)?,
    )
    .map_err(Error::Subject)?;
    budget.charge_work(
        content
            .descriptor
            .canonical_bytes()
            .len()
            .checked_add(128)
            .ok_or(Resource::Arithmetic)?,
    )?;
    let lowering = InertNativeLoweringAssociationV1::new(NativeLoweringAssociationInputsV1 {
        final_native: subject,
        carrier: coordinate(*carrier_id.sha256(), carrier_id.byte_len())?,
        descriptor: coordinate(
            Sha256::digest(content.descriptor.canonical_bytes()).into(),
            u64::try_from(content.descriptor.canonical_bytes().len())
                .map_err(|_| Resource::Arithmetic)?,
        )?,
        pre_descriptor_llvm: content.pre_descriptor_llvm,
        final_llvm: coordinate(
            *module.module_identity().sha256(),
            module.module_identity().byte_len(),
        )?,
        module_handoff: coordinate(*module.identity().sha256(), module.identity().byte_len())?,
        profile: target.profile(),
    });
    codec::<Commitment>(MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3, budget)?;
    for part in [
        module.module_bytes(),
        module.envelope().canonical_bytes(),
        module.symbol_manifest().canonical_bytes(),
        module.canonical_bytes(),
    ] {
        budget.charge_work(part.len())?;
    }
    let commitment = Commitment::from_handoff(module).map_err(Error::Commitment)?;
    let input = NativeConditionalMetadataInputV1 {
        invocation: &invocation,
        rustc_inventory: bindings.rustc_identity_inventory.canonical_transcript(),
        rustc_preflight: bindings.rustc_preflight_plan.canonical_transcript(),
        semantic_target_layout: &layout,
        native_lowering: lowering.canonical_bytes(),
        final_module_commitment: commitment.canonical_bytes(),
    };
    let metadata = NativeConditionalMetadataLayoutV1::new(input).map_err(Error::Metadata)?;
    let capsule = InertProductionSemanticCapsuleLayoutV5::new(metadata.encoded_len(), bytes.len())
        .map_err(Error::Capsule)?;
    let outer = Outer::new(capsule.encoded_len(), module.canonical_bytes().len())
        .map_err(Error::Handoff)?;
    let carrier_len = bytes.len();
    grow(&mut bytes, outer.encoded_len(), budget)?;
    budget.charge_work(carrier_len)?;
    bytes.copy_within(
        0..carrier_len,
        outer.capsule_range().start + capsule.carrier_range().start,
    );
    let metadata_bytes = &mut bytes[outer.capsule_range()][capsule.metadata_range()];
    for (range, field) in [
        (metadata.invocation_range(), input.invocation),
        (metadata.rustc_inventory_range(), input.rustc_inventory),
        (metadata.rustc_preflight_range(), input.rustc_preflight),
        (
            metadata.semantic_target_layout_range(),
            input.semantic_target_layout,
        ),
        (metadata.native_lowering_range(), input.native_lowering),
        (
            metadata.final_module_commitment_range(),
            input.final_module_commitment,
        ),
    ] {
        copy(&mut metadata_bytes[range], field, budget)?;
    }
    seal_native_conditional_metadata_v1(metadata, metadata_bytes, limit, |w| budget.charge_work(w))
        .map_err(Error::Metadata)?;
    copy(
        &mut bytes[outer.module_handoff_range()],
        module.canonical_bytes(),
        budget,
    )?;
    let capsule_id = seal_inert_production_semantic_capsule_v5(
        capsule,
        &mut bytes[outer.capsule_range()],
        limit,
        |w| budget.charge_work(w),
    )
    .map_err(Error::Capsule)?;
    seal_inert_semantic_compiler_module_handoff_v5(
        outer,
        &mut bytes,
        capsule_id,
        module.identity(),
        |w| budget.charge_work(w),
    )
    .map_err(Error::Seal)?;
    Ok(bytes)
}
