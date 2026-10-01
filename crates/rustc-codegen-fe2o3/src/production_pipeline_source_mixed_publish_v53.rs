//! The fixed source-owned production chain's exact managed-module producer.
use super::*;
use crate::kernel_ir_codegen::mixed_v53;
use fe2o3_compiler_ffi::{CodeObjectVersion, CompilerModuleHandoffV2};
use fe2o3_compiler_lineage::*;
use fe2o3_kernel_descriptor::mixed_conditional_v26::{
    MIXED_CONTRACT_CODEC_STORAGE_V26, MixedContractV26, decode_mixed_contract_v26,
};
use fe2o3_kernel_descriptor::{
    MIXED_DESCRIPTOR_READER_STORAGE_V53, MixedDescriptorErrorV53, MixedDescriptorTableV53,
    decode_mixed_descriptor_v53, encode_mixed_descriptor_v53, encoded_mixed_descriptor_v53_len,
};
use sha2::{Digest, Sha256};
use std::mem::size_of_val;

fn target_identity(digest: &[u8; 32], length: u64) -> Result<TargetLineageIdentityV3, Error> {
    TargetLineageIdentityV3::new(*digest, length).map_err(|_| mismatch("typed target identity"))
}

fn proof_identity(digest: &[u8; 32], length: u64) -> Result<InertLineageContentIdentityV3, Error> {
    InertLineageContentIdentityV3::new(*digest, length)
        .map_err(|_| mismatch("typed proof association identity"))
}

fn resource(source: &Source<'_>, error: Resource) -> Error {
    source.retain_query_resource_error_v18(error).into()
}

fn bytes(count: usize, budget: &mut Budget<'_>) -> Result<Vec<u8>, Error> {
    budget.reserve_storage(count)?;
    let mut value = Vec::new();
    value
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        value
            .capacity()
            .checked_sub(count)
            .ok_or(Resource::Accounting)?,
    )?;
    budget.charge_work(count)?;
    value.resize(count, 0);
    Ok(value)
}

fn wire_error(error: MixedDescriptorErrorV53<Resource>) -> Error {
    match error {
        MixedDescriptorErrorV53::Nominal(fe2o3_kernel_descriptor::DescriptorWireErrorV3::Work(
            e,
        )) => Error::Resource(e),
        MixedDescriptorErrorV53::Contract(
            fe2o3_kernel_descriptor::mixed_conditional_v26::MixedContractErrorV26::Resource(e),
        ) => Error::Resource(e),
        _ => mismatch("mandatory mixed descriptor transport"),
    }
}

fn module_error(error: mixed_v53::MixedModuleErrorV53) -> Error {
    match error {
        mixed_v53::MixedModuleErrorV53::Resource(error) => Error::Resource(error),
        mixed_v53::MixedModuleErrorV53::Descriptor(error) => wire_error(error),
        _ => mismatch("exact V18 mixed module retention/replay"),
    }
}

fn descriptor_wire(
    candidate: &PreparedMixedPublicationV28<'_, '_, '_>,
    budget: &mut Budget<'_>,
) -> Result<Vec<u8>, Error> {
    let worker = candidate.worker(budget)?;
    let nominal = worker.descriptor(budget)?;
    let count = worker.root_count(budget)?;
    if count != nominal.kernel_count() {
        return Err(mismatch("mixed descriptor root census"));
    }
    let row_bytes = count
        .checked_mul(size_of::<MixedContractV26<'_>>())
        .ok_or(Resource::Arithmetic)?;
    budget.reserve_storage(
        row_bytes + MIXED_DESCRIPTOR_READER_STORAGE_V53 + MIXED_CONTRACT_CODEC_STORAGE_V26,
    )?;
    let mut contracts = Vec::new();
    contracts
        .try_reserve_exact(count)
        .map_err(|_| Resource::Allocation)?;
    budget.reserve_storage(
        contracts
            .capacity()
            .checked_sub(count)
            .and_then(|n| n.checked_mul(size_of::<MixedContractV26<'_>>()))
            .ok_or(Resource::Accounting)?,
    )?;
    for original in 0..count {
        contracts.push(
            decode_mixed_contract_v26(worker.contract(original, budget)?, &mut |n| {
                budget.charge_work(n)
            })
            .map_err(|error| match error {
                fe2o3_kernel_descriptor::mixed_conditional_v26::MixedContractErrorV26::Resource(
                    e,
                ) => Error::Resource(e),
                _ => mismatch("retained source contract changed"),
            })?,
        );
    }
    // Deterministic in-place sorting. Exact descriptor join below rejects every
    // duplicate, missing or foreign kernel; original source-root ordinals remain.
    let depth = usize::BITS as usize - count.leading_zeros() as usize;
    let sort_work = count
        .checked_mul(depth.checked_add(1).ok_or(Resource::Arithmetic)?)
        .and_then(|n| n.checked_mul(128))
        .ok_or(Resource::Arithmetic)?;
    budget.charge_work(sort_work)?;
    contracts.sort_unstable_by_key(|row| row.subjects().kernel_id);
    let length =
        encoded_mixed_descriptor_v53_len(nominal.canonical_bytes(), &contracts, &mut |n| {
            budget.charge_work(n)
        })
        .map_err(wire_error)?;
    let mut wire = bytes(length, budget)?;
    encode_mixed_descriptor_v53(nominal.canonical_bytes(), &contracts, &mut wire, &mut |n| {
        budget.charge_work(n)
    })
    .map_err(wire_error)?;
    let storage = contracts
        .capacity()
        .checked_mul(size_of::<MixedContractV26<'_>>())
        .ok_or(Resource::Arithmetic)?;
    drop(contracts);
    budget.release_storage(
        storage + MIXED_DESCRIPTOR_READER_STORAGE_V53 + MIXED_CONTRACT_CODEC_STORAGE_V26,
    )?;
    Ok(wire)
}

#[cfg(test)]
#[path = "production_pipeline_source_mixed_publish_v53_tests.rs"]
mod tests;

/// All fields are borrowed from this exact execution and current native owner.
/// The generic protocol codecs below remain in their existing bounded allocation
/// domain; source generation, descriptor buffers and retained module text remain
/// charged to this source account. No codec alone authenticates these inputs.
impl ExecutedProtectedMixedPublicationV29<'_, '_, '_, '_, '_> {
    fn target_receipts_v53(
        &self,
        descriptor: &[u8],
        semantic: &InertCanonicalSemanticMirReceiptV3,
        budget: &mut Budget<'_>,
    ) -> Result<(InertTargetBindingReceiptV3, InertDataLayoutReceiptV3), Error> {
        let prepared = &self.candidate.prepared;
        let bindings = prepared.bindings(budget)?;
        let owner = prepared.native(budget)?.output(budget)?;
        let invocation = self.candidate.invocation(budget)?;
        let invocation_bytes = fe2o3_rustc_invocation::encode_descriptor_v3(invocation)
            .map_err(|_| mismatch("protected invocation encoding"))?;
        let invocation_digest = fe2o3_rustc_invocation::InvocationDigestV3::calculate(invocation)
            .map_err(|_| mismatch("protected invocation digest"))?;
        let layout = bindings.rustc_target.rustc_layout();
        let cpu = layout
            .active_cpu()
            .ok_or_else(|| mismatch("authenticated target CPU"))?;
        let features = layout
            .active_features()
            .ok_or_else(|| mismatch("authenticated target features"))?;
        let target_name = bindings.rustc_target.device_target().to_string();
        let row_size = size_of::<MultiRootTargetWorkgroupInputV2<'_>>();
        let count = owner.module().kernels.len();
        budget.reserve_storage(count.checked_mul(row_size).ok_or(Resource::Arithmetic)?)?;
        let mut workgroups = Vec::new();
        workgroups
            .try_reserve_exact(count)
            .map_err(|_| Resource::Allocation)?;
        budget.reserve_storage(
            workgroups
                .capacity()
                .checked_sub(count)
                .and_then(|n| n.checked_mul(row_size))
                .ok_or(Resource::Accounting)?,
        )?;
        for kernel in &owner.module().kernels {
            budget.charge_work(4)?;
            let group = kernel
                .workgroup_size
                .ok_or_else(|| mismatch("exact mixed workgroup"))?;
            workgroups.push(MultiRootTargetWorkgroupInputV2 {
                kernel: kernel.id.as_str(),
                workgroup: [group.x, group.y, group.z],
            });
        }
        let target_transcript =
            MultiRootTargetBindingTranscriptV2::new(MultiRootTargetBindingInputsV2 {
                protected_rustc_invocation: target_identity(
                    invocation_digest.as_bytes(),
                    invocation_bytes.len() as u64,
                )?,
                semantic_mir: target_identity(
                    semantic.identity().sha256(),
                    semantic.identity().byte_len(),
                )?,
                target_neutral_kir: target_identity(
                    owner.identity().digest(),
                    owner.canonical_bytes().len() as u64,
                )?,
                target_bound_kir: target_identity(
                    owner.identity().digest(),
                    owner.canonical_bytes().len() as u64,
                )?,
                configured_target: &target_name,
                rustc_llvm_target: layout.llvm_target(),
                target_cpu: cpu,
                target_features: features,
                roster_identity: Sha256::digest(descriptor).into(),
                code_object_version: 6,
                wave_width_bits: 64,
                workgroups: &workgroups,
            })
            .map_err(|_| mismatch("exact mixed target transcript"))?;
        let retained = workgroups
            .capacity()
            .checked_mul(row_size)
            .ok_or(Resource::Arithmetic)?;
        drop(workgroups);
        budget.release_storage(retained)?;
        let target = InertTargetBindingReceiptV3::from_canonical_preimage(
            target_transcript.canonical_bytes(),
        )
        .map_err(|_| mismatch("mixed target transcript encoding"))?;
        let layout_transcript = DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
            semantic_mir: target_identity(
                semantic.identity().sha256(),
                semantic.identity().byte_len(),
            )?,
            target_binding: target_identity(
                target.identity().sha256(),
                target.identity().byte_len(),
            )?,
            semantic_layout: derive_semantic_target_layout_identity_v1(
                layout.llvm_target(),
                layout.data_layout(),
                layout.default_pointer_width_bits(),
                cpu,
                features,
            )
            .map_err(|_| mismatch("original semantic target layout"))?,
            rustc_llvm_target: layout.llvm_target(),
            live_rustc_data_layout: layout.data_layout(),
            final_llvm_target: layout.llvm_target(),
            final_llvm_data_layout: crate::production_target_v1::PRODUCTION_WORKER_DATA_LAYOUT_V1,
            default_pointer_width_bits: layout.default_pointer_width_bits(),
        })
        .map_err(|_| mismatch("mixed data-layout transcript"))?;
        let data_layout =
            InertDataLayoutReceiptV3::from_canonical_preimage(layout_transcript.canonical_bytes())
                .map_err(|_| mismatch("mixed data-layout encoding"))?;
        Ok((target, data_layout))
    }

    fn with_module_v53<R>(
        &self,
        budget: &mut Budget<'_>,
        consume: impl FnOnce(
            &CompilerModuleHandoffV2,
            &MixedDescriptorTableV53<'_>,
            &mut Budget<'_>,
        ) -> Result<R, Error>,
    ) -> Result<R, Error> {
        self.candidate.revalidate(budget)?;
        self.executed
            .replay_signed_receipt(budget)
            .map_err(Error::MixedRelocationExpressions)?;
        let prepared = &self.candidate.prepared;
        let source = prepared.inputs.source;
        let floor = budget.storage();
        let header = size_of::<MixedDescriptorTableV53<'_>>()
            + MIXED_DESCRIPTOR_READER_STORAGE_V53
            + size_of::<Vec<u8>>()
            + size_of::<CompilerModuleHandoffV2>()
            + size_of::<R>()
            + size_of::<Result<R, Error>>()
            + size_of_val(&consume);
        budget
            .with_prepaid_scope(floor, 0, 0, header, |budget| {
                let wire = descriptor_wire(prepared, budget)?;
                let table = decode_mixed_descriptor_v53(&wire, &mut |n| budget.charge_work(n))
                    .map_err(wire_error)?;
                let native = prepared.native(budget)?;
                let owner = native.output(budget)?;
                let worker = prepared.worker(budget)?;
                let (module, storage) =
                    mixed_v53::retain_text(owner, worker.llvm_ir(budget)?, &table, budget)
                        .map_err(module_error)?;
                budget.reserve_storage(storage)?;
                let bindings = prepared.bindings(budget)?;
                let target = bindings.rustc_target.device_target();
                let envelope =
                    crate::production_worker_handoff::derive_production_compiler_ffi_envelope(
                        target,
                        owner.module(),
                        &module,
                        bindings.transaction.compiler_ffi_envelope.clone(),
                        *owner.identity().digest(),
                    )
                    .map_err(|e| {
                        Error::Pipeline(Box::new(ProductionPipelineError::WorkerHandoff(e)))
                    })?;
                crate::compiler_module_contract::validate_exact_target_binding(
                    target,
                    owner.module(),
                )
                .map_err(|_| mismatch("exact mixed target"))?;
                crate::compiler_module_contract::validate_envelope_module_roles(&envelope, &module)
                    .map_err(|_| mismatch("exact mixed symbol roles"))?;
                let manifest = crate::compiler_module_contract::construct_symbol_manifest(&module)
                    .map_err(|_| mismatch("exact mixed symbol manifest"))?;
                let handoff = CompilerModuleHandoffV2::new(
                    CompilerModuleKindV1::LlvmTextIr,
                    target,
                    CodeObjectVersion::V6,
                    envelope,
                    manifest,
                    module.llvm_ir().as_bytes(),
                )
                .map_err(|_| mismatch("mixed module handoff encoding"))?;
                mixed_v53::check_metadata(owner, &module, &table, budget).map_err(module_error)?;
                let result = consume(&handoff, &table, budget)?;
                self.candidate.revalidate(budget)?;
                drop(handoff);
                drop(module);
                budget.release_storage(storage)?;
                drop(table);
                let retained = wire.capacity();
                drop(wire);
                budget.release_storage(retained)?;
                Ok(result)
            })
            .map_err(|e| match e {
                Error::Resource(e) => resource(source, e),
                other => other,
            })
    }

    /// Assemble every capsule field from the retained exact source/native proof
    /// chain and publish only inside this lexical custody. The V50 middle-end
    /// field is the format discriminator; no historical proof field is accepted.
    pub(crate) fn with_strict_handoff_v53<R>(
        &self,
        budget: &mut Budget<'_>,
        consume: impl FnOnce(&Handoff, &mut Budget<'_>) -> Result<R, Error>,
    ) -> Result<R, Error> {
        self.with_module_v53(budget, |module, descriptor, budget| {
            let prepared = &self.candidate.prepared;
            let source = prepared.inputs.source;
            let native = prepared.native(budget)?;
            let owner = native.output(budget)?;
            let bindings = prepared.bindings(budget)?;
            let semantic_bytes = source.source_semantic(budget)?.canonical_encoding();
            let generated = prepared.generated_source(budget)?;
            let receipt = self
                .executed
                .signed_receipt(budget)
                .map_err(Error::MixedRelocationExpressions)?;
            let length = self.layout(budget)?.encoded_len();
            let mut middle = bytes(length, budget)?;
            self.encode(&mut middle, budget)?;

            let inventory = InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(
                bindings.rustc_identity_inventory.canonical_transcript(),
            )
            .map_err(|_| mismatch("mixed identity inventory encoding"))?;
            let preflight = InertRustcPreflightPlanReceiptV3::from_canonical_preimage(
                bindings.rustc_preflight_plan.canonical_transcript(),
            )
            .map_err(|_| mismatch("mixed preflight encoding"))?;
            let semantic =
                InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(semantic_bytes)
                    .map_err(|_| mismatch("mixed semantic encoding"))?;
            let middle_end = InertMiddleEndReceiptV3::from_canonical_preimage(middle.as_slice())
                .map_err(|_| mismatch("typed middle-end encoding"))?;
            let kir = InertKernelIrReceiptV3::from_canonical_preimage(owner.canonical_bytes())
                .map_err(|_| mismatch("final mixed KIR encoding"))?;
            let correspondence =
                InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(generated)
                    .map_err(|_| mismatch("typed source statement encoding"))?;
            let memory =
                InertFormalMemoryReceiptV3::from_canonical_preimage(descriptor.canonical_bytes())
                    .map_err(|_| mismatch("conditional memory premises encoding"))?;
            let association = InertProofBindingAssociationV4::new(
                InertProofBindingAssociationInputsV4::new(
                    proof_identity(semantic.identity().sha256(), semantic.identity().byte_len())?,
                    proof_identity(
                        middle_end.identity().sha256(),
                        middle_end.identity().byte_len(),
                    )?,
                    proof_identity(kir.identity().sha256(), kir.identity().byte_len())?,
                    proof_identity(
                        correspondence.identity().sha256(),
                        correspondence.identity().byte_len(),
                    )?,
                    proof_identity(memory.identity().sha256(), memory.identity().byte_len())?,
                ),
                receipt,
            )
            .map_err(|_| mismatch("typed proof association"))?;
            let proof =
                InertProofBindingReceiptV3::from_canonical_preimage(association.canonical_bytes())
                    .map_err(|_| mismatch("typed proof association encoding"))?;

            let invocation = self.candidate.invocation(budget)?;
            let target = bindings.rustc_target.device_target();
            let (target_receipt, data_layout) =
                self.target_receipts_v53(descriptor.canonical_bytes(), &semantic, budget)?;
            let abi = InertAbiReceiptV3::from_canonical_preimage(descriptor.canonical_bytes())
                .map_err(|_| mismatch("mixed ABI encoding"))?;
            let exports = InertExportManifestReceiptV3::from_canonical_preimage(
                module.symbol_manifest().canonical_bytes(),
            )
            .map_err(|_| mismatch("mixed export encoding"))?;
            let lowering = InertAmdgpuLoweringReceiptV3::from_canonical_preimage(
                prepared.worker(budget)?.llvm_ir(budget)?.as_bytes(),
            )
            .map_err(|_| mismatch("typed native text encoding"))?;
            let commitment =
                fe2o3_compiler_ffi::InertFinalCompilerModuleCommitmentV3::from_handoff(module)
                    .map_err(|_| mismatch("mixed final module commitment"))?;
            let final_module =
                InertFinalCompilerModuleCommitmentReceiptV3::from_canonical_preimage(
                    commitment.canonical_bytes(),
                )
                .map_err(|_| mismatch("mixed final module encoding"))?;
            let llvm_association =
                SemanticToLlvmAssociationTranscriptV3::new(SemanticToLlvmAssociationInputsV3 {
                    semantic_mir: target_identity(
                        semantic.identity().sha256(),
                        semantic.identity().byte_len(),
                    )?,
                    middle_end: target_identity(
                        middle_end.identity().sha256(),
                        middle_end.identity().byte_len(),
                    )?,
                    kernel_ir: target_identity(kir.identity().sha256(), kir.identity().byte_len())?,
                    mir_to_kir_correspondence: target_identity(
                        correspondence.identity().sha256(),
                        correspondence.identity().byte_len(),
                    )?,
                    formal_memory: target_identity(
                        memory.identity().sha256(),
                        memory.identity().byte_len(),
                    )?,
                    proof_binding: target_identity(
                        proof.identity().sha256(),
                        proof.identity().byte_len(),
                    )?,
                    target_binding: target_identity(
                        target_receipt.identity().sha256(),
                        target_receipt.identity().byte_len(),
                    )?,
                    data_layout: target_identity(
                        data_layout.identity().sha256(),
                        data_layout.identity().byte_len(),
                    )?,
                    abi: target_identity(abi.identity().sha256(), abi.identity().byte_len())?,
                    export_manifest: target_identity(
                        exports.identity().sha256(),
                        exports.identity().byte_len(),
                    )?,
                    amdgpu_lowering: target_identity(
                        lowering.identity().sha256(),
                        lowering.identity().byte_len(),
                    )?,
                    final_llvm: target_identity(
                        module.module_identity().sha256(),
                        module.module_identity().byte_len(),
                    )?,
                    final_compiler_module_commitment: target_identity(
                        final_module.identity().sha256(),
                        final_module.identity().byte_len(),
                    )?,
                })
                .map_err(|_| mismatch("mixed semantic/native association"))?;
            let association = InertSemanticToLlvmReceiptV3::from_canonical_preimage(
                llvm_association.canonical_bytes(),
            )
            .map_err(|_| mismatch("mixed semantic/native encoding"))?;
            let receipts = OrderedInertSemanticLineageReceiptsV3::new(
                inventory,
                preflight,
                semantic,
                middle_end,
                kir,
                correspondence,
                memory,
                proof,
                target_receipt,
                data_layout,
                abi,
                exports,
                lowering,
                association,
                final_module,
            );
            self.executed
                .replay_lineage_capsule_v50(&receipts, budget)
                .map_err(Error::MixedRelocationExpressions)?;
            let capsule =
                InertProductionSemanticCapsuleV3::new(invocation.clone(), target, receipts)
                    .map_err(|_| mismatch("complete typed semantic capsule"))?;
            let handoff = Handoff::new(capsule, module.clone())
                .map_err(|_| mismatch("complete typed module handoff"))?;
            self.check_strict_handoff_v53(&handoff, budget)?;
            let result = consume(&handoff, budget)?;
            drop(handoff);
            let retained = middle.capacity();
            drop(middle);
            budget.release_storage(retained)?;
            Ok(result)
        })
    }

    /// Live-owner replay of the V50-format capsule before managed publication.
    /// This does not accept an older proof preimage based on matching digests.
    pub(crate) fn check_strict_handoff_v53(
        &self,
        handoff: &Handoff,
        budget: &mut Budget<'_>,
    ) -> Result<(), Error> {
        self.candidate.revalidate(budget)?;
        self.executed
            .replay_signed_receipt(budget)
            .map_err(Error::MixedRelocationExpressions)?;
        let capsule = handoff.capsule();
        let receipts = capsule.receipts();
        let prepared = &self.candidate.prepared;
        self.executed
            .replay_lineage_capsule_v50(receipts, budget)
            .map_err(Error::MixedRelocationExpressions)?;
        if capsule.invocation() != self.candidate.invocation(budget)? {
            return Err(mismatch("exact original protected invocation"));
        }
        let bindings = prepared.bindings(budget)?;
        let module = handoff.module_handoff();
        if module.kind() != CompilerModuleKindV1::LlvmTextIr
            || module.target() != bindings.rustc_target.device_target()
        {
            return Err(mismatch("exact typed module target/kind"));
        }
        for (actual, expected) in [
            (
                receipts.rustc_identity_inventory().canonical_preimage(),
                bindings.rustc_identity_inventory.canonical_transcript(),
            ),
            (
                receipts.rustc_preflight_plan().canonical_preimage(),
                bindings.rustc_preflight_plan.canonical_transcript(),
            ),
            (
                receipts.mir_to_kir_correspondence().canonical_preimage(),
                prepared.generated_source(budget)?,
            ),
            (
                receipts.amdgpu_lowering().canonical_preimage(),
                prepared.worker(budget)?.llvm_ir(budget)?.as_bytes(),
            ),
            (
                receipts.formal_memory().canonical_preimage(),
                receipts.abi().canonical_preimage(),
            ),
            (
                receipts.export_manifest().canonical_preimage(),
                module.symbol_manifest().canonical_bytes(),
            ),
        ] {
            equal(actual, expected, budget)?;
        }
        let descriptor = descriptor_wire(prepared, budget)?;
        let selected = equal(receipts.abi().canonical_preimage(), &descriptor, budget);
        let retained = descriptor.capacity();
        drop(descriptor);
        budget.release_storage(retained)?;
        selected?;
        let proof =
            InertProofBindingAssociationV4::decode(receipts.proof_binding().canonical_preimage())
                .map_err(|_| mismatch("typed proof association framing"))?;
        let expected_proof = InertProofBindingAssociationInputsV4::new(
            proof_identity(
                receipts.semantic_mir().identity().sha256(),
                receipts.semantic_mir().identity().byte_len(),
            )?,
            proof_identity(
                receipts.middle_end().identity().sha256(),
                receipts.middle_end().identity().byte_len(),
            )?,
            proof_identity(
                receipts.kernel_ir().identity().sha256(),
                receipts.kernel_ir().identity().byte_len(),
            )?,
            proof_identity(
                receipts.mir_to_kir_correspondence().identity().sha256(),
                receipts.mir_to_kir_correspondence().identity().byte_len(),
            )?,
            proof_identity(
                receipts.formal_memory().identity().sha256(),
                receipts.formal_memory().identity().byte_len(),
            )?,
        );
        if proof.inputs() != expected_proof {
            return Err(mismatch("typed proof association fields"));
        }
        equal(
            proof.verus_execution_evidence(),
            self.executed
                .signed_receipt(budget)
                .map_err(Error::MixedRelocationExpressions)?,
            budget,
        )?;
        let (target, layout) = self.target_receipts_v53(
            receipts.abi().canonical_preimage(),
            receipts.semantic_mir(),
            budget,
        )?;
        equal(
            receipts.target_binding().canonical_preimage(),
            target.canonical_preimage(),
            budget,
        )?;
        equal(
            receipts.data_layout().canonical_preimage(),
            layout.canonical_preimage(),
            budget,
        )?;
        let associated =
            SemanticToLlvmAssociationTranscriptV3::new(SemanticToLlvmAssociationInputsV3 {
                semantic_mir: target_identity(
                    receipts.semantic_mir().identity().sha256(),
                    receipts.semantic_mir().identity().byte_len(),
                )?,
                middle_end: target_identity(
                    receipts.middle_end().identity().sha256(),
                    receipts.middle_end().identity().byte_len(),
                )?,
                kernel_ir: target_identity(
                    receipts.kernel_ir().identity().sha256(),
                    receipts.kernel_ir().identity().byte_len(),
                )?,
                mir_to_kir_correspondence: target_identity(
                    receipts.mir_to_kir_correspondence().identity().sha256(),
                    receipts.mir_to_kir_correspondence().identity().byte_len(),
                )?,
                formal_memory: target_identity(
                    receipts.formal_memory().identity().sha256(),
                    receipts.formal_memory().identity().byte_len(),
                )?,
                proof_binding: target_identity(
                    receipts.proof_binding().identity().sha256(),
                    receipts.proof_binding().identity().byte_len(),
                )?,
                target_binding: target_identity(
                    receipts.target_binding().identity().sha256(),
                    receipts.target_binding().identity().byte_len(),
                )?,
                data_layout: target_identity(
                    receipts.data_layout().identity().sha256(),
                    receipts.data_layout().identity().byte_len(),
                )?,
                abi: target_identity(
                    receipts.abi().identity().sha256(),
                    receipts.abi().identity().byte_len(),
                )?,
                export_manifest: target_identity(
                    receipts.export_manifest().identity().sha256(),
                    receipts.export_manifest().identity().byte_len(),
                )?,
                amdgpu_lowering: target_identity(
                    receipts.amdgpu_lowering().identity().sha256(),
                    receipts.amdgpu_lowering().identity().byte_len(),
                )?,
                final_llvm: target_identity(
                    module.module_identity().sha256(),
                    module.module_identity().byte_len(),
                )?,
                final_compiler_module_commitment: target_identity(
                    receipts
                        .final_compiler_module_commitment()
                        .identity()
                        .sha256(),
                    receipts
                        .final_compiler_module_commitment()
                        .identity()
                        .byte_len(),
                )?,
            })
            .map_err(|_| mismatch("typed source/native receipt association"))?;
        equal(
            receipts.semantic_to_llvm().canonical_preimage(),
            associated.canonical_bytes(),
            budget,
        )?;
        // Rebuild the concrete module from the same retained native owner. This
        // binds code, descriptor section, exact FFI envelope and symbol manifest;
        // caller-supplied module bytes never become the native replay input.
        self.with_module_v53(budget, |expected, _, budget| {
            equal(module.canonical_bytes(), expected.canonical_bytes(), budget)
        })?;
        Ok(())
    }
}

impl<'tcx> ProductionCompilation<'tcx, CollectedRustStage<'tcx>> {
    /// Ordinary compilation has one source-owned route. The runtime lease and
    /// protected rustc capability stay alive through exact managed publication;
    /// failure at any stage never selects a historical lowering/proof path.
    pub(crate) fn publish_mixed_worker_handoff_v53(
        self,
        budget: &mut Budget<'_>,
        compiler_execution: crate::protected_compiler_execution::AdmittedProtectedCompilerExecutionV1,
    ) -> Result<fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1, ProductionPipelineError>
    {
        let runtime = fe2o3_verifier::FunctionalRefinementVerusRuntimeLeaseV1::open(
            "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
        )
        .map_err(ProductionPipelineError::MixedRuntime)?;
        self.with_original_source_mixed_publication_on_account_v28(budget, |prepared, budget| {
            let protected = prepared.into_protected(budget)?;
            protected.with_executed_composition_v29(&runtime, 120, budget, |executed, budget| {
                executed.with_strict_handoff_v53(budget, |handoff, budget| {
                    executed.check_strict_handoff_v53(handoff, budget)?;
                    protected.revalidate(budget)?;
                    let bindings = protected.prepared.bindings(budget)?;
                    let transaction = &bindings.transaction;
                    let receipt = fe2o3_artifact_transaction::publish_compiler_module_handoff_v3(
                        &transaction.output_dir, &transaction.producer,
                        protected.attempt(budget)?, handoff,
                    ).map_err(|e| Error::Pipeline(Box::new(ProductionPipelineError::StrictV3Publication(e))))?;
                    let subject = fe2o3_artifact_transaction::InertCompilerExecutionSubjectV1::from_publication(receipt, handoff)
                        .map_err(|e| Error::Pipeline(Box::new(ProductionPipelineError::CompilerExecutionSubject(e))))?;
                    let carriage = compiler_execution.acquire(subject.clone())
                        .map_err(|e| Error::Pipeline(Box::new(ProductionPipelineError::ProtectedCompilerExecution(e))))?;
                    let transport = fe2o3_artifact_transaction::publish_compiler_execution_receipt_transport_v1(
                        &transaction.output_dir, &transaction.producer, &subject, carriage.canonical_bytes(),
                    ).map_err(|e| Error::Pipeline(Box::new(ProductionPipelineError::CompilerExecutionReceiptTransport(e))))?;
                    if transport.subject() != subject.identity() || transport.length() != carriage.canonical_bytes().len() {
                        return Err(Error::Pipeline(Box::new(ProductionPipelineError::CompilerExecutionReceiptTransportBindingMismatch)));
                    }
                    protected.revalidate(budget)?;
                    Ok(subject)
                })
            })
        }).map(SourceOwnedCompilationContinuationV29::into_observation)
          .map_err(|error| ProductionPipelineError::MixedSourcePublication(Box::new(error)))
    }
}
