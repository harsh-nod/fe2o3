//! Original-N proof receipts and actual native-F lowering, from one live owner.
use super::super::*;
use fe2o3_compiler_ffi::{
    InertFinalCompilerModuleCommitmentV3, MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3,
};
use fe2o3_compiler_lineage::*;
use fe2o3_rustc_invocation::{
    InvocationDigestV3, MAX_DESCRIPTOR_BYTES_V3, RustcInvocationDescriptorV3, encode_descriptor_v3,
};
use sha2::{Digest, Sha256};

fn coordinate(digest: [u8; 32], length: u64) -> R<TargetLineageIdentityV3> {
    TargetLineageIdentityV3::new(digest, length).map_err(E::Target)
}

// Existing opaque codecs have bounded logical working-set schedules, not
// instruction-exact metering. All allocation/copy/hash prepayments precede use.
fn transcript<T>(budget: &mut Budget<'_>) -> R<()> {
    codec::<T>(MAX_PRODUCTION_TARGET_LINEAGE_TRANSCRIPT_BYTES_V3, budget)
}

pub(super) fn build(
    live: &Live,
    carrier_bytes: &[u8],
    invocation: &RustcInvocationDescriptorV3,
    budget: &mut Budget<'_>,
) -> R<InertProductionSemanticCapsuleV3> {
    budget.reserve_storage(READ_STORAGE + CARRIER_STORAGE)?;
    let limit = budget.storage_limit();
    let carrier =
        read_native_refined_forwarding_carrier_v1(carrier_bytes, limit, |w| budget.charge_work(w))
            .map_err(E::Carrier)?;
    let frame =
        read_inert_refined_forwarding_output_v1(carrier.output(), limit, |w| budget.charge_work(w))
            .map_err(E::Framing)?;
    let bindings = &live.bindings;
    let target = &bindings.rustc_target;
    let layout = target.rustc_layout();
    if bindings
        .rustc_preflight_plan
        .rustc_identity_inventory_sha256()
        != bindings.rustc_identity_inventory.sha256()
        || invocation.amd_target() != target.profile().device_target()
        || target.profile() != live.native.profile
    {
        return Err(E::Mismatch(
            "native capsule live inventory/target/invocation",
        ));
    }

    macro_rules! receipt {
        ($ty:ty, $bytes:expr) => {{
            let bytes: &[u8] = $bytes;
            codec::<$ty>(bytes.len(), budget)?;
            budget.reserve_storage(HASH_STORAGE)?;
            budget.charge_work(bytes.len().checked_add(256).ok_or(Resource::Arithmetic)?)?;
            <$ty>::from_canonical_preimage(copied(bytes, budget)?).map_err(E::Lineage)?
        }};
    }
    macro_rules! identity {
        ($receipt:expr) => {{
            let id = $receipt.identity();
            coordinate(*id.sha256(), id.byte_len())?
        }};
    }
    let inventory = receipt!(
        InertRustcIdentityInventoryReceiptV3,
        bindings.rustc_identity_inventory.canonical_transcript()
    );
    let preflight = receipt!(
        InertRustcPreflightPlanReceiptV3,
        bindings.rustc_preflight_plan.canonical_transcript()
    );
    let semantic = receipt!(
        InertCanonicalSemanticMirReceiptV3,
        frame.field(Field::SemanticMir)
    );
    let middle_end = receipt!(
        InertMiddleEndReceiptV3,
        frame.field(Field::OriginalMiddleEnd)
    );
    let kernel_ir = receipt!(InertKernelIrReceiptV3, frame.field(Field::OriginalNative));
    let correspondence = receipt!(
        InertMirToKirCorrespondenceReceiptV3,
        frame.field(Field::OriginalCorrespondence)
    );
    let formal = receipt!(
        InertFormalMemoryReceiptV3,
        frame.field(Field::OriginalFormalMemory)
    );
    let proof = receipt!(
        InertProofBindingReceiptV3,
        frame.field(Field::OriginalInputV4)
    );

    // Pay descriptor clone, both nested encodings, digest and capsule encoding.
    codec::<RustcInvocationDescriptorV3>(4 * MAX_DESCRIPTOR_BYTES_V3, budget)?;
    let invocation_bytes = encode_descriptor_v3(invocation).map_err(E::Invocation)?;
    let invocation_digest =
        InvocationDigestV3::calculate(invocation).map_err(E::InvocationDigest)?;
    let invocation_id = coordinate(
        invocation_digest.into_bytes(),
        invocation_bytes.len() as u64,
    )?;
    drop(invocation_bytes);
    let cpu = layout
        .active_cpu()
        .ok_or(E::Mismatch("native capsule active CPU"))?;
    let features = layout
        .active_features()
        .ok_or(E::Mismatch("native capsule active features"))?;
    let roots = middle(live);
    budget.reserve_storage(size_of::<
        crate::production_pipeline::native_checked_output_handoff_v1::SourceInputsV1<'_>,
    >())?;
    let inputs = live.source.replay_inputs(budget).map_err(E::Live)?;
    let mut workgroups = vector(roots.root_count(), budget)?;
    budget.charge_work(roots.canonical_bytes().len())?;
    for i in 0..roots.root_count() {
        let root = roots
            .root(i)
            .ok_or(E::Mismatch("native capsule semantic root"))?;
        for kernel in &live.output().module().kernels {
            budget.charge_work(
                kernel
                    .id
                    .as_str()
                    .len()
                    .checked_add(root.export_symbol().len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Resource::Arithmetic)?,
            )?;
        }
        let kernel = live
            .output()
            .module()
            .kernels
            .iter()
            .find(|k| k.id.as_str() == root.export_symbol())
            .ok_or(E::Mismatch("native capsule exported F root"))?;
        let wg = kernel
            .workgroup_size
            .ok_or(E::Mismatch("native capsule F workgroup"))?;
        let workgroup = [wg.x, wg.y, wg.z];
        if root.workgroup() != workgroup
            || inputs
                .launch
                .roots()
                .get(i)
                .and_then(|r| r.source_launch().exact_workgroup())
                != Some(workgroup)
        {
            return Err(E::Mismatch("native capsule semantic-order workgroups"));
        }
        workgroups.push(MultiRootTargetWorkgroupInputV3 {
            kernel: root.export_symbol(),
            workgroup,
        });
    }
    transcript::<MultiRootTargetBindingTranscriptV3>(budget)?;
    // B is the actual target-binding endpoint; F has its own lowering subject.
    let bound = live
        .history_inputs()
        .prefix
        .prefix
        .prefix
        .prefix
        .input
        .canonical()
        .identity();
    let target_transcript =
        MultiRootTargetBindingTranscriptV3::new(MultiRootTargetBindingInputsV3 {
            protected_rustc_invocation: invocation_id,
            semantic_mir: identity!(semantic),
            native_neutral_subject: subject(inputs.original, inputs.catalog)?,
            target_bound_kir: coordinate(*bound.digest(), bound.canonical_length())?,
            configured_target: target.profile().device_target(),
            rustc_llvm_target: layout.llvm_target(),
            target_cpu: cpu,
            target_features: features,
            roster_identity: roots.roster_identity(),
            code_object_version: 6,
            wave_width_bits: 64,
            workgroups: &workgroups,
        })
        .map_err(E::Target)?;
    let target_binding = receipt!(
        InertTargetBindingReceiptV3,
        target_transcript.canonical_bytes()
    );
    drop(target_transcript);
    drop(workgroups);

    transcript::<TargetLineageIdentityV3>(budget)?;
    let semantic_layout = derive_semantic_target_layout_identity_v1(
        layout.llvm_target(),
        layout.data_layout(),
        layout.default_pointer_width_bits(),
        cpu,
        features,
    )
    .map_err(E::Target)?;
    if semantic_layout.sha256() != *inputs.semantic.target_layout_identity().as_bytes() {
        return Err(E::Mismatch("native capsule semantic target layout"));
    }
    transcript::<DataLayoutTranscriptV3>(budget)?;
    let layout_transcript = DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
        semantic_mir: identity!(semantic),
        target_binding: identity!(target_binding),
        semantic_layout,
        rustc_llvm_target: layout.llvm_target(),
        live_rustc_data_layout: layout.data_layout(),
        final_llvm_target: layout.llvm_target(),
        final_llvm_data_layout: crate::production_target_v1::PRODUCTION_WORKER_DATA_LAYOUT_V1,
        default_pointer_width_bits: layout.default_pointer_width_bits(),
    })
    .map_err(E::Target)?;
    let data_layout = receipt!(
        InertDataLayoutReceiptV3,
        layout_transcript.canonical_bytes()
    );
    drop(layout_transcript);

    let (module, descriptor, _) = live.native_output_parts();
    budget.charge_work(module.module_bytes().len())?;
    let llvm = std::str::from_utf8(module.module_bytes())
        .map_err(|_| E::Mismatch("native capsule final LLVM UTF-8"))?;
    let lowering = scoped(budget, |budget| {
        let relation = dialect_amdgcn::check_native_v12_text_descriptor_relation_v1(
            live.output(),
            inputs.catalog,
            live.output().canonical().canonical_bytes(),
            target.profile(),
            descriptor.table(),
            llvm,
            budget,
        )
        .map_err(E::Text)?;
        budget.reserve_storage(relation.storage().retained_storage())?;
        budget.reserve_storage(size_of::<InertNativeLoweringAssociationV1>() + HASH_STORAGE)?;
        budget.charge_work(NATIVE_LOWERING_ASSOCIATION_WORK_V1)?;
        budget.charge_work(relation.pre_descriptor_llvm().len())?;
        let association =
            InertNativeLoweringAssociationV1::new(NativeLoweringAssociationInputsV1 {
                final_native: subject(relation.output(), relation.catalog())?,
                carrier: coordinate(*carrier.identity().sha256(), carrier.identity().byte_len())?,
                descriptor: identity!(descriptor),
                pre_descriptor_llvm: coordinate(
                    Sha256::digest(relation.pre_descriptor_llvm().as_bytes()).into(),
                    relation.pre_descriptor_llvm().len() as u64,
                )?,
                final_llvm: coordinate(
                    *module.module_identity().sha256(),
                    module.module_identity().byte_len(),
                )?,
                module_handoff: identity!(module),
                profile: relation.profile(),
            });
        Ok(association)
    })?;
    budget.reserve_storage(size_of::<InertNativeLoweringAssociationV1>())?;
    let abi = receipt!(InertAbiReceiptV3, descriptor.canonical_bytes());
    let exports = receipt!(
        InertExportManifestReceiptV3,
        module.symbol_manifest().canonical_bytes()
    );
    let lowering = receipt!(InertAmdgpuLoweringReceiptV3, lowering.canonical_bytes());
    codec::<InertFinalCompilerModuleCommitmentV3>(
        MAX_FINAL_COMPILER_MODULE_COMMITMENT_BYTES_V3,
        budget,
    )?;
    for bytes in [
        module.module_bytes(),
        module.envelope().canonical_bytes(),
        module.symbol_manifest().canonical_bytes(),
        module.canonical_bytes(),
    ] {
        budget.charge_work(bytes.len())?;
    }
    let commitment =
        InertFinalCompilerModuleCommitmentV3::from_handoff(module).map_err(E::Commitment)?;
    let commitment = receipt!(
        InertFinalCompilerModuleCommitmentReceiptV3,
        commitment.canonical_bytes()
    );
    transcript::<SemanticToLlvmAssociationTranscriptV3>(budget)?;
    let semantic_to_llvm =
        SemanticToLlvmAssociationTranscriptV3::new(SemanticToLlvmAssociationInputsV3 {
            semantic_mir: identity!(semantic),
            middle_end: identity!(middle_end),
            kernel_ir: identity!(kernel_ir),
            mir_to_kir_correspondence: identity!(correspondence),
            formal_memory: identity!(formal),
            proof_binding: identity!(proof),
            target_binding: identity!(target_binding),
            data_layout: identity!(data_layout),
            abi: identity!(abi),
            export_manifest: identity!(exports),
            amdgpu_lowering: identity!(lowering),
            final_llvm: coordinate(
                *module.module_identity().sha256(),
                module.module_identity().byte_len(),
            )?,
            final_compiler_module_commitment: identity!(commitment),
        })
        .map_err(E::Target)?;
    let semantic_to_llvm = receipt!(
        InertSemanticToLlvmReceiptV3,
        semantic_to_llvm.canonical_bytes()
    );
    let mut extent = MAX_DESCRIPTOR_BYTES_V3 + 2048;
    for bytes in [
        inventory.canonical_preimage(),
        preflight.canonical_preimage(),
        semantic.canonical_preimage(),
        middle_end.canonical_preimage(),
        kernel_ir.canonical_preimage(),
        correspondence.canonical_preimage(),
        formal.canonical_preimage(),
        proof.canonical_preimage(),
        target_binding.canonical_preimage(),
        data_layout.canonical_preimage(),
        abi.canonical_preimage(),
        exports.canonical_preimage(),
        lowering.canonical_preimage(),
        semantic_to_llvm.canonical_preimage(),
        commitment.canonical_preimage(),
    ] {
        extent = extent
            .checked_add(bytes.len())
            .ok_or(Resource::Arithmetic)?;
    }
    codec::<InertProductionSemanticCapsuleV3>(
        extent.checked_mul(2).ok_or(Resource::Arithmetic)?,
        budget,
    )?;
    let receipts = OrderedInertSemanticLineageReceiptsV3::new(
        inventory,
        preflight,
        semantic,
        middle_end,
        kernel_ir,
        correspondence,
        formal,
        proof,
        target_binding,
        data_layout,
        abi,
        exports,
        lowering,
        semantic_to_llvm,
        commitment,
    );
    InertProductionSemanticCapsuleV3::new(invocation.clone(), module.target(), receipts)
        .map_err(E::Lineage)
}
