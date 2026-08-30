//! Private join from live production compiler owners to the inert V3 capsule.

use std::{collections::BTreeSet, error::Error, fmt};

use fe2o3_compiler_ffi::{
    CodeObjectVersion, CompilerDescriptorSourceV1, CompilerModuleHandoffV2,
    CompilerModuleSymbolManifestV1, CompilerModuleSymbolRoleV1,
    FinalCompilerModuleCommitmentErrorV3, InertFinalCompilerModuleCommitmentV3,
    InertSemanticCompilerModuleHandoffErrorV3, InertSemanticCompilerModuleHandoffV3,
};
use fe2o3_compiler_lineage::{
    InertAbiReceiptV3, InertAmdgpuLoweringReceiptV3, InertCanonicalSemanticMirReceiptV3,
    InertDataLayoutReceiptV3, InertExportManifestReceiptV3,
    InertFinalCompilerModuleCommitmentReceiptV3, InertFormalMemoryReceiptV3,
    InertKernelIrReceiptV3, InertLineageContentIdentityV3, InertMiddleEndReceiptV3,
    InertMirToKirCorrespondenceReceiptV3, InertProductionSemanticCapsuleV3,
    InertProofBindingAssociationErrorV3, InertProofBindingAssociationErrorV4,
    InertProofBindingAssociationInputsV4, InertProofBindingAssociationV4,
    InertProofBindingReceiptV3, InertRustcIdentityInventoryReceiptV3,
    InertRustcPreflightPlanReceiptV3, InertSemanticToLlvmAssociationV3,
    InertSemanticToLlvmReceiptV3, InertTargetBindingReceiptV3, LineageErrorV3,
    OrderedInertSemanticLineageReceiptsV3,
};
use fe2o3_kernel_ir::{
    FunctionRole, Module, ProductionSemanticDebugAvailabilityV1, ProductionSemanticDebugCarrierV1,
    ProductionSemanticDebugProducerGapV1, ProductionSemanticDebugReceiptExtensionV1,
    VerifiedCanonicalKernelIrErrorV8, VerifiedCanonicalKernelIrV8,
};
use fe2o3_lower_mir_kernel::{
    InertCanonicalFormalMemoryAdmissionEvidenceV4, InertCanonicalMirToKirCorrespondenceEvidenceV4,
    ProductionCanonicalKernelIrIdentityV1, ProductionCanonicalKernelIrVersionV1,
    ProductionCorrespondenceEvidenceErrorV4, ProductionFormalMemoryEvidenceErrorV4,
    ProductionFormalMemoryOwnerV1,
};
use fe2o3_rustc_invocation::{InvocationDigestV3, encode_descriptor_v3};
use fe2o3_verifier::{
    CanonicalProductionMirPlironVerusExecutionEvidenceV1, CompilerKirToLlvmReplayValidationErrorV1,
    ProductionMirPlironVerusExecutionEvidenceErrorV1,
};
use sha2::{Digest, Sha256};

use crate::production_ranked_projection_v1::AuthenticatedRankedVerificationV5;
use crate::production_target_lineage_v3::{
    DataLayoutTranscriptInputsV3, DataLayoutTranscriptV3, ProductionTargetLineageErrorV3,
    SemanticToLlvmAssociationInputsV3, SemanticToLlvmAssociationTranscriptV3,
    TargetBindingTranscriptInputsV3, TargetBindingTranscriptV3, TargetLineageIdentityV3,
};
use crate::production_target_v1::PRODUCTION_WORKER_DATA_LAYOUT_V1;
use crate::protected_rustc_invocation::{
    FinishedProtectedRustcInvocationV3, ProtectedRustcInvocationErrorV1,
};

const CODE_OBJECT_VERSION_V3: u16 = 6;
const WAVE_WIDTH_BITS_V3: u16 = 64;

fn validate_final_llvm_layout(llvm: &str) -> Result<(), ProductionSemanticLineageErrorV3> {
    let expected_header = format!(
        "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{PRODUCTION_WORKER_DATA_LAYOUT_V1}\"\n"
    );
    if !llvm.starts_with(&expected_header)
        || llvm.matches("target triple =").count() != 1
        || llvm.matches("target datalayout =").count() != 1
    {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "final LLVM does not retain the exact measured worker target layout",
        ));
    }
    Ok(())
}

/// Move-only canonical evidence prepared while the live semantic and formal
/// owners still exist. It grants no publication, load, or launch authority.
pub(crate) struct PreparedProductionSemanticLineageV3 {
    rustc_identity_inventory: InertRustcIdentityInventoryReceiptV3,
    rustc_preflight_plan: InertRustcPreflightPlanReceiptV3,
    semantic_mir: InertCanonicalSemanticMirReceiptV3,
    middle_end: InertMiddleEndReceiptV3,
    kernel_ir: InertKernelIrReceiptV3,
    mir_to_kir_correspondence: InertMirToKirCorrespondenceReceiptV3,
    formal_memory: InertFormalMemoryReceiptV3,
    verus_execution: CanonicalProductionMirPlironVerusExecutionEvidenceV1,
    amdgpu_lowering_replay: dialect_amdgcn::CanonicalProductionKirToLlvmReplayEvidenceV1,
    semantic_debug: crate::production_semantic_debug_v1::PreparedProductionSemanticDebugV1,
    neutral_kir_custody: ProductionCanonicalKernelIrIdentityV1,
    neutral_kir_identity: TargetLineageIdentityV3,
    bound_kir_identity: TargetLineageIdentityV3,
    semantic_layout_identity: TargetLineageIdentityV3,
    expected_exports: BTreeSet<(CompilerModuleSymbolRoleV1, String)>,
    rustc_layout: crate::semantic_layout_bridge::SemanticLayoutTargetV1,
    default_workgroup: [u32; 3],
}

impl PreparedProductionSemanticLineageV3 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_prepare(
        rustc_identity_inventory: &crate::collector::AuthenticatedRustcIdentityInventoryV3,
        rustc_preflight_plan: &crate::collector::AuthenticatedRustcPreflightPlanV3,
        rustc_target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
        ranked_verification: AuthenticatedRankedVerificationV5,
        admitted: &ProductionFormalMemoryOwnerV1,
        target_module: &Module,
        pre_descriptor_llvm: &str,
        semantic_debug_inputs: crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1,
    ) -> Result<Self, ProductionSemanticLineageErrorV3> {
        admitted
            .verify_equivalence()
            .map_err(|error| ProductionSemanticLineageErrorV3::LiveOwner(error.to_string()))?;

        let semantic = admitted.semantic_kir().semantic().semantic();
        let semantic_u32_induction = ranked_verification.semantic_u32_induction();
        let induction_function = semantic
            .functions()
            .get(semantic_u32_induction.function().index() as usize)
            .ok_or(ProductionSemanticLineageErrorV3::AxisMismatch(
                "semantic induction report names a function outside canonical semantic MIR",
            ))?;
        if semantic_u32_induction.semantic_mir_sha256() != semantic.semantic_sha256()
            || semantic_u32_induction.function_identity() != induction_function.identity()
            || semantic_u32_induction.grants_authority()
            || semantic_u32_induction.authorizes_compiler_transform()
        {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "semantic induction report is not an inert exact-custody fact",
            ));
        }
        let rustc_identity_inventory =
            InertRustcIdentityInventoryReceiptV3::from_canonical_preimage(
                rustc_identity_inventory.canonical_transcript(),
            )?;
        let rustc_preflight_plan = InertRustcPreflightPlanReceiptV3::from_canonical_preimage(
            rustc_preflight_plan.canonical_transcript(),
        )?;
        let semantic_mir = InertCanonicalSemanticMirReceiptV3::from_canonical_preimage(
            semantic.canonical_encoding(),
        )?;
        let middle_end = InertMiddleEndReceiptV3::from_canonical_preimage(
            ranked_verification.middle_end_evidence().canonical_bytes(),
        )?;

        let neutral_kir_custody = admitted.semantic_kir().canonical_kernel_ir_identity();
        if neutral_kir_custody.version() != ProductionCanonicalKernelIrVersionV1::V8 {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "gfx942 production lineage requires canonical Kernel IR V8",
            ));
        }
        let neutral_kir = admitted.semantic_kir().canonical_kernel_ir_v8();
        neutral_kir.revalidate()?;
        let bound_kir = VerifiedCanonicalKernelIrV8::from_module(target_module.clone())?;
        bound_kir.revalidate()?;
        let neutral_kir_identity = TargetLineageIdentityV3::new(
            *neutral_kir_custody.digest(),
            neutral_kir_custody.canonical_length(),
        )?;
        let bound_kir_identity = TargetLineageIdentityV3::new(
            *bound_kir.identity().digest(),
            bound_kir.canonical_bytes().len() as u64,
        )?;
        let kernel_ir =
            InertKernelIrReceiptV3::from_canonical_preimage(neutral_kir.canonical_bytes())?;
        let amdgpu_lowering_replay =
            dialect_amdgcn::CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs(
                neutral_kir.canonical_bytes(),
                target_module,
                rustc_target.profile(),
                pre_descriptor_llvm,
            )?;

        let correspondence = InertCanonicalMirToKirCorrespondenceEvidenceV4::from_live_owner(
            admitted.semantic_kir(),
            semantic_u32_induction,
        )?;
        if correspondence.canonical_kernel_ir_identity() != neutral_kir_custody {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "MIR-to-KIR correspondence names a different neutral KIR",
            ));
        }
        let mir_to_kir_correspondence =
            InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(
                correspondence.canonical_bytes(),
            )?;
        let semantic_debug = match semantic_debug_inputs {
            crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1::Unavailable(
                gap,
            ) => crate::production_semantic_debug_v1::PreparedProductionSemanticDebugV1::Unavailable(
                gap,
            ),
            crate::production_semantic_debug_v1::ProductionSemanticDebugInputsV1::Available {
                source_map,
                canonical_kir_v7,
            } => match crate::production_semantic_debug_v1::prepare_production_semantic_debug_v1(
                admitted.semantic_kir(),
                &correspondence,
                *source_map,
                &canonical_kir_v7,
            ) {
                Ok(prepared) => prepared,
                Err(error) => crate::production_semantic_debug_v1::PreparedProductionSemanticDebugV1::Unavailable(
                    semantic_debug_prepare_gap(&error),
                ),
            },
        };

        let formal = InertCanonicalFormalMemoryAdmissionEvidenceV4::from_live_owner(admitted)?;
        if formal.canonical_kernel_ir_identity() != neutral_kir_custody {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "formal-memory admission names a different neutral KIR",
            ));
        }
        let formal_memory =
            InertFormalMemoryReceiptV3::from_canonical_preimage(formal.canonical_bytes())?;

        if !ranked_verification.retained_functional_verification_is_coherent() {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "retained functional verification is incoherent",
            ));
        }
        let aggregate_verus = ranked_verification.aggregate_verus_execution().ok_or(
            ProductionSemanticLineageErrorV3::AxisMismatch(
                "production handoff requires an authenticated aggregate MIR-to-PLIRON Verus execution",
            ),
        )?;
        let verus_execution =
            CanonicalProductionMirPlironVerusExecutionEvidenceV1::from_execution(aggregate_verus)?;
        if verus_execution
            .claims()
            .pliron_evidence_identity()
            .as_bytes()
            != ranked_verification
                .middle_end_evidence()
                .identity()
                .sha256()
        {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "aggregate Verus execution names a different live middle-end PLIRON record",
            ));
        }

        let target_layout = crate::rustc_semantic_adapter_v1::canonical_target_layout_transcript_v1(
            rustc_target.rustc_layout(),
        );
        let target_layout_sha256: [u8; 32] = Sha256::digest(&target_layout).into();
        if semantic.target_layout_identity().as_bytes() != &target_layout_sha256 {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "semantic MIR target layout differs from the authenticated rustc layout",
            ));
        }
        let semantic_layout_identity =
            TargetLineageIdentityV3::new(target_layout_sha256, target_layout.len() as u64)?;

        validate_final_llvm_layout(pre_descriptor_llvm)?;

        let expected_exports = exact_source_and_kir_exports(semantic, target_module)?;
        let workgroup = target_module
            .kernels
            .first()
            .and_then(|kernel| kernel.workgroup_size)
            .ok_or(ProductionSemanticLineageErrorV3::AxisMismatch(
                "target-bound KIR has no exact workgroup size",
            ))?;

        Ok(Self {
            rustc_identity_inventory,
            rustc_preflight_plan,
            semantic_mir,
            middle_end,
            kernel_ir,
            mir_to_kir_correspondence,
            formal_memory,
            verus_execution,
            amdgpu_lowering_replay,
            semantic_debug,
            neutral_kir_custody,
            neutral_kir_identity,
            bound_kir_identity,
            semantic_layout_identity,
            expected_exports,
            rustc_layout: rustc_target.rustc_layout().clone(),
            default_workgroup: [workgroup.x, workgroup.y, workgroup.z],
        })
    }

    pub(crate) fn finish(
        self,
        invocation_custody: &FinishedProtectedRustcInvocationV3,
        target: fe2o3_compiler_ffi::DeviceTargetV1,
        descriptor_source: &CompilerDescriptorSourceV1,
        module_handoff: CompilerModuleHandoffV2,
    ) -> Result<InertSemanticCompilerModuleHandoffV3, ProductionSemanticLineageErrorV3> {
        invocation_custody
            .revalidate_for_publication()
            .map_err(ProductionSemanticLineageErrorV3::ProtectedRustcInvocation)?;
        let invocation = invocation_custody.descriptor().clone();
        if invocation.amd_target() != target.to_string()
            || descriptor_source.table().device_target() != target
            || module_handoff.target() != target
            || descriptor_source.table().code_object_version() != CodeObjectVersion::V6
            || module_handoff.code_object_version() != CodeObjectVersion::V6
        {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "invocation, descriptor, and module targets or code-object versions differ",
            ));
        }
        validate_final_exports(
            &self.expected_exports,
            descriptor_source,
            module_handoff.symbol_manifest(),
        )?;
        let final_llvm = std::str::from_utf8(module_handoff.module_bytes()).map_err(|_| {
            ProductionSemanticLineageErrorV3::AxisMismatch(
                "final compiler module is not canonical textual LLVM",
            )
        })?;
        validate_final_llvm_layout(final_llvm)?;
        let correspondence = InertCanonicalMirToKirCorrespondenceEvidenceV4::decode(
            self.mir_to_kir_correspondence.canonical_preimage(),
        )?;
        let formal = InertCanonicalFormalMemoryAdmissionEvidenceV4::decode(
            self.formal_memory.canonical_preimage(),
        )?;
        if correspondence
            .semantic_u32_induction()
            .semantic_mir_sha256()
            != self.semantic_mir.identity().sha256()
            || correspondence.canonical_kernel_ir_identity() != self.neutral_kir_custody
            || formal.canonical_kernel_ir_identity() != self.neutral_kir_custody
            || correspondence.grants_authority()
            || correspondence.semantic_u32_induction().grants_authority()
            || formal.grants_authority()
        {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "lossless semantic correspondence custody changed before final handoff",
            ));
        }

        let invocation_bytes = encode_descriptor_v3(&invocation)
            .map_err(|error| ProductionSemanticLineageErrorV3::Invocation(error.to_string()))?;
        let invocation_digest = InvocationDigestV3::calculate(&invocation)
            .map_err(|error| ProductionSemanticLineageErrorV3::Invocation(error.to_string()))?;
        let invocation_identity = TargetLineageIdentityV3::new(
            invocation_digest.into_bytes(),
            invocation_bytes.len() as u64,
        )?;

        let semantic_identity = receipt_identity(
            self.semantic_mir.identity().sha256(),
            self.semantic_mir.identity().byte_len(),
        )?;
        let middle_end_identity = receipt_identity(
            self.middle_end.identity().sha256(),
            self.middle_end.identity().byte_len(),
        )?;
        let kernel_ir_identity = receipt_identity(
            self.kernel_ir.identity().sha256(),
            self.kernel_ir.identity().byte_len(),
        )?;
        let correspondence_identity = receipt_identity(
            self.mir_to_kir_correspondence.identity().sha256(),
            self.mir_to_kir_correspondence.identity().byte_len(),
        )?;
        let formal_memory_identity = receipt_identity(
            self.formal_memory.identity().sha256(),
            self.formal_memory.identity().byte_len(),
        )?;

        let proof_binding = InertProofBindingAssociationV4::new(
            InertProofBindingAssociationInputsV4::new(
                proof_association_identity(
                    self.semantic_mir.identity().sha256(),
                    self.semantic_mir.identity().byte_len(),
                )?,
                proof_association_identity(
                    self.middle_end.identity().sha256(),
                    self.middle_end.identity().byte_len(),
                )?,
                proof_association_identity(
                    self.kernel_ir.identity().sha256(),
                    self.kernel_ir.identity().byte_len(),
                )?,
                proof_association_identity(
                    self.mir_to_kir_correspondence.identity().sha256(),
                    self.mir_to_kir_correspondence.identity().byte_len(),
                )?,
                proof_association_identity(
                    self.formal_memory.identity().sha256(),
                    self.formal_memory.identity().byte_len(),
                )?,
            ),
            self.verus_execution.canonical_bytes(),
        )?;
        let proof_binding =
            InertProofBindingReceiptV3::from_canonical_preimage(proof_binding.canonical_bytes())?;
        let proof_binding_identity = receipt_identity(
            proof_binding.identity().sha256(),
            proof_binding.identity().byte_len(),
        )?;

        let rustc_cpu = self.rustc_layout.active_cpu().ok_or(
            ProductionSemanticLineageErrorV3::AxisMismatch(
                "authenticated rustc target has no active CPU",
            ),
        )?;
        let rustc_features = self.rustc_layout.active_features().ok_or(
            ProductionSemanticLineageErrorV3::AxisMismatch(
                "authenticated rustc target has no active features",
            ),
        )?;
        let configured_target = target.to_string();
        let target_binding = TargetBindingTranscriptV3::new(TargetBindingTranscriptInputsV3 {
            protected_rustc_invocation: invocation_identity,
            semantic_mir: semantic_identity,
            target_neutral_kir: self.neutral_kir_identity,
            target_bound_kir: self.bound_kir_identity,
            configured_target: &configured_target,
            rustc_llvm_target: self.rustc_layout.llvm_target(),
            target_cpu: rustc_cpu,
            target_features: rustc_features,
            code_object_version: CODE_OBJECT_VERSION_V3,
            wave_width_bits: WAVE_WIDTH_BITS_V3,
            default_workgroup: self.default_workgroup,
        })?;
        let target_binding =
            InertTargetBindingReceiptV3::from_canonical_preimage(target_binding.canonical_bytes())?;
        let target_binding_identity = receipt_identity(
            target_binding.identity().sha256(),
            target_binding.identity().byte_len(),
        )?;

        let data_layout = DataLayoutTranscriptV3::new(DataLayoutTranscriptInputsV3 {
            semantic_mir: semantic_identity,
            target_binding: target_binding_identity,
            semantic_layout: self.semantic_layout_identity,
            rustc_llvm_target: self.rustc_layout.llvm_target(),
            live_rustc_data_layout: self.rustc_layout.data_layout(),
            final_llvm_target: self.rustc_layout.llvm_target(),
            final_llvm_data_layout: PRODUCTION_WORKER_DATA_LAYOUT_V1,
            default_pointer_width_bits: self.rustc_layout.default_pointer_width_bits(),
        })?;
        let data_layout =
            InertDataLayoutReceiptV3::from_canonical_preimage(data_layout.canonical_bytes())?;
        let data_layout_identity = receipt_identity(
            data_layout.identity().sha256(),
            data_layout.identity().byte_len(),
        )?;

        // The finalizer must be able to recover and strictly decode the exact
        // zero-digest descriptor source without knowing a backend-private codec.
        let abi = InertAbiReceiptV3::from_canonical_preimage(descriptor_source.canonical_bytes())?;
        let abi_identity = receipt_identity(abi.identity().sha256(), abi.identity().byte_len())?;

        let export_manifest = InertExportManifestReceiptV3::from_canonical_preimage(
            module_handoff.symbol_manifest().canonical_bytes(),
        )?;
        let export_manifest_identity = receipt_identity(
            export_manifest.identity().sha256(),
            export_manifest.identity().byte_len(),
        )?;

        let amdgpu_lowering = InertAmdgpuLoweringReceiptV3::from_canonical_preimage(
            self.amdgpu_lowering_replay.canonical_bytes(),
        )?;
        let validated_lowering = fe2o3_verifier::validate_compiler_kir_to_llvm_replay_v1(
            &self.kernel_ir,
            &amdgpu_lowering,
        )?;
        let replay_evidence = validated_lowering.replay().evidence();
        let replay_target_identity = TargetLineageIdentityV3::new(
            replay_evidence.target_bound_kernel_ir_identity().sha256(),
            replay_evidence.target_bound_kernel_ir_identity().byte_len(),
        )?;
        if replay_target_identity != self.bound_kir_identity
            || replay_evidence.profile().device_target() != configured_target
        {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "independently replayed AMDGPU lowering changed target-bound KIR or target profile",
            ));
        }
        let amdgpu_lowering_identity = receipt_identity(
            amdgpu_lowering.identity().sha256(),
            amdgpu_lowering.identity().byte_len(),
        )?;

        let final_commitment = InertFinalCompilerModuleCommitmentV3::from_handoff(&module_handoff)?;
        let final_compiler_module_commitment =
            InertFinalCompilerModuleCommitmentReceiptV3::from_canonical_preimage(
                final_commitment.canonical_bytes(),
            )?;
        let final_commitment_identity = receipt_identity(
            final_compiler_module_commitment.identity().sha256(),
            final_compiler_module_commitment.identity().byte_len(),
        )?;
        let module_identity = module_handoff.module_identity();
        let final_llvm_identity =
            TargetLineageIdentityV3::new(*module_identity.sha256(), module_identity.byte_len())?;

        let semantic_to_llvm_association =
            SemanticToLlvmAssociationTranscriptV3::new(SemanticToLlvmAssociationInputsV3 {
                semantic_mir: semantic_identity,
                middle_end: middle_end_identity,
                kernel_ir: kernel_ir_identity,
                mir_to_kir_correspondence: correspondence_identity,
                formal_memory: formal_memory_identity,
                proof_binding: proof_binding_identity,
                target_binding: target_binding_identity,
                data_layout: data_layout_identity,
                abi: abi_identity,
                export_manifest: export_manifest_identity,
                amdgpu_lowering: amdgpu_lowering_identity,
                final_llvm: final_llvm_identity,
                final_compiler_module_commitment: final_commitment_identity,
            })?;
        InertSemanticToLlvmAssociationV3::decode(semantic_to_llvm_association.canonical_bytes())
            .map_err(|_| {
                ProductionSemanticLineageErrorV3::AxisMismatch(
                    "compiler semantic-to-LLVM association is not the frozen canonical V3 schema",
                )
            })?;
        let semantic_debug = self.semantic_debug.finish(
            self.semantic_mir.canonical_preimage(),
            module_handoff.module_bytes(),
        );
        let semantic_to_llvm = attach_optional_semantic_debug_v1(
            semantic_to_llvm_association.canonical_bytes(),
            semantic_debug,
        )?;

        let receipts = OrderedInertSemanticLineageReceiptsV3::new(
            self.rustc_identity_inventory,
            self.rustc_preflight_plan,
            self.semantic_mir,
            self.middle_end,
            self.kernel_ir,
            self.mir_to_kir_correspondence,
            self.formal_memory,
            proof_binding,
            target_binding,
            data_layout,
            abi,
            export_manifest,
            amdgpu_lowering,
            semantic_to_llvm,
            final_compiler_module_commitment,
        );
        let capsule = InertProductionSemanticCapsuleV3::new(invocation, target, receipts)?;
        InertSemanticCompilerModuleHandoffV3::new(capsule, module_handoff).map_err(Into::into)
    }
}

fn semantic_debug_prepare_gap(
    error: &crate::production_pipeline::ProductionPipelineError,
) -> ProductionSemanticDebugProducerGapV1 {
    match error {
        crate::production_pipeline::ProductionPipelineError::SimulationProductionKirV9 => {
            ProductionSemanticDebugProducerGapV1::CanonicalKirV7ProjectionUnavailable
        }
        crate::production_pipeline::ProductionPipelineError::SemanticDebugMap(
            fe2o3_kernel_ir::SemanticDebugMapErrorV1::InvalidLength
            | fe2o3_kernel_ir::SemanticDebugMapErrorV1::Encoding
            | fe2o3_kernel_ir::SemanticDebugMapErrorV1::ResourceLimit
            | fe2o3_kernel_ir::SemanticDebugMapErrorV1::AllocationFailure,
        )
        | crate::production_pipeline::ProductionPipelineError::SemanticDebugFragment(
            fe2o3_kernel_ir::ProductionSemanticDebugFragmentErrorV1::ResourceLimit
            | fe2o3_kernel_ir::ProductionSemanticDebugFragmentErrorV1::AllocationFailure,
        ) => ProductionSemanticDebugProducerGapV1::ResourceLimit,
        crate::production_pipeline::ProductionPipelineError::SimulationDebugMapCorrespondence(
            _,
        ) => ProductionSemanticDebugProducerGapV1::CorrespondenceValidationUnavailable,
        crate::production_pipeline::ProductionPipelineError::SemanticDebugFragment(_) => {
            ProductionSemanticDebugProducerGapV1::FragmentConstructionUnavailable
        }
        crate::production_pipeline::ProductionPipelineError::SemanticDebugMap(_) => {
            ProductionSemanticDebugProducerGapV1::SemanticMapConstructionUnavailable
        }
        _ => ProductionSemanticDebugProducerGapV1::CorrespondenceValidationUnavailable,
    }
}

fn attach_optional_semantic_debug_v1(
    association: &[u8],
    availability: ProductionSemanticDebugAvailabilityV1,
) -> Result<InertSemanticToLlvmReceiptV3, ProductionSemanticLineageErrorV3> {
    attach_optional_semantic_debug_with_fault_v1(
        association,
        availability,
        OptionalSemanticDebugAttachmentFaultV1::None,
    )
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[cfg_attr(not(test), allow(dead_code))]
enum OptionalSemanticDebugAttachmentFaultV1 {
    None,
    PrimaryCarrierResource,
    PrimaryCarrierStructural,
    FallbackCarrier,
    Extension,
    ExtensionReceipt,
}

fn attach_optional_semantic_debug_with_fault_v1(
    association: &[u8],
    availability: ProductionSemanticDebugAvailabilityV1,
    fault: OptionalSemanticDebugAttachmentFaultV1,
) -> Result<InertSemanticToLlvmReceiptV3, ProductionSemanticLineageErrorV3> {
    let primary = if matches!(
        fault,
        OptionalSemanticDebugAttachmentFaultV1::PrimaryCarrierResource
            | OptionalSemanticDebugAttachmentFaultV1::PrimaryCarrierStructural
            | OptionalSemanticDebugAttachmentFaultV1::FallbackCarrier
    ) {
        None
    } else {
        Some(ProductionSemanticDebugCarrierV1::new(
            association,
            availability,
        ))
    };
    let carrier = match primary {
        Some(Ok(carrier)) => carrier,
        Some(Err(error)) => {
            let gap = if matches!(
                error,
                fe2o3_kernel_ir::ProductionSemanticDebugFragmentErrorV1::ResourceLimit
                    | fe2o3_kernel_ir::ProductionSemanticDebugFragmentErrorV1::AllocationFailure
            ) {
                ProductionSemanticDebugProducerGapV1::ResourceLimit
            } else {
                ProductionSemanticDebugProducerGapV1::CarrierConstructionUnavailable
            };
            let Some(carrier) = fallback_debug_carrier_v1(association, gap, fault) else {
                return InertSemanticToLlvmReceiptV3::from_canonical_preimage(association)
                    .map_err(Into::into);
            };
            carrier
        }
        None => {
            let gap = if fault == OptionalSemanticDebugAttachmentFaultV1::PrimaryCarrierResource {
                ProductionSemanticDebugProducerGapV1::ResourceLimit
            } else {
                ProductionSemanticDebugProducerGapV1::CarrierConstructionUnavailable
            };
            let Some(carrier) = fallback_debug_carrier_v1(association, gap, fault) else {
                return InertSemanticToLlvmReceiptV3::from_canonical_preimage(association)
                    .map_err(Into::into);
            };
            carrier
        }
    };
    if fault == OptionalSemanticDebugAttachmentFaultV1::Extension {
        return InertSemanticToLlvmReceiptV3::from_canonical_preimage(association)
            .map_err(Into::into);
    }
    let Ok(extension) = ProductionSemanticDebugReceiptExtensionV1::new(association, carrier) else {
        return InertSemanticToLlvmReceiptV3::from_canonical_preimage(association)
            .map_err(Into::into);
    };
    if fault == OptionalSemanticDebugAttachmentFaultV1::ExtensionReceipt {
        return InertSemanticToLlvmReceiptV3::from_canonical_preimage(association)
            .map_err(Into::into);
    }
    match InertSemanticToLlvmReceiptV3::from_canonical_preimage(extension.canonical_bytes()) {
        Ok(receipt) => Ok(receipt),
        Err(_) => {
            InertSemanticToLlvmReceiptV3::from_canonical_preimage(association).map_err(Into::into)
        }
    }
}

fn fallback_debug_carrier_v1(
    association: &[u8],
    gap: ProductionSemanticDebugProducerGapV1,
    fault: OptionalSemanticDebugAttachmentFaultV1,
) -> Option<ProductionSemanticDebugCarrierV1> {
    if fault == OptionalSemanticDebugAttachmentFaultV1::FallbackCarrier {
        return None;
    }
    ProductionSemanticDebugCarrierV1::new(
        association,
        ProductionSemanticDebugAvailabilityV1::Unavailable(gap),
    )
    .ok()
}

fn receipt_identity(
    sha256: &[u8; 32],
    byte_len: u64,
) -> Result<TargetLineageIdentityV3, ProductionSemanticLineageErrorV3> {
    TargetLineageIdentityV3::new(*sha256, byte_len).map_err(Into::into)
}

fn proof_association_identity(
    sha256: &[u8; 32],
    byte_len: u64,
) -> Result<InertLineageContentIdentityV3, ProductionSemanticLineageErrorV3> {
    InertLineageContentIdentityV3::new(*sha256, byte_len).map_err(Into::into)
}

fn exact_source_and_kir_exports(
    semantic: &fe2o3_mir_model::semantic_mir_v1::AdmittedInertSemanticMirV1,
    target_module: &Module,
) -> Result<BTreeSet<(CompilerModuleSymbolRoleV1, String)>, ProductionSemanticLineageErrorV3> {
    use fe2o3_mir_model::semantic_mir_v1::SemanticFunctionExportV1;

    let source = semantic
        .functions()
        .iter()
        .filter_map(|function| match function.export()? {
            SemanticFunctionExportV1::Kernel(entry) => Some(
                semantic_link_symbol(entry.export_symbol())
                    .map(|symbol| (CompilerModuleSymbolRoleV1::KernelEntry, symbol)),
            ),
            SemanticFunctionExportV1::DeviceFfi { export_symbol } => Some(
                semantic_link_symbol(export_symbol)
                    .map(|symbol| (CompilerModuleSymbolRoleV1::DeviceFfiExport, symbol)),
            ),
        })
        .collect::<Result<BTreeSet<_>, _>>()?;
    let kir = target_module
        .functions
        .iter()
        .filter_map(|function| match function.role {
            FunctionRole::KernelEntry => Some((
                CompilerModuleSymbolRoleV1::KernelEntry,
                function.id.as_str().to_owned(),
            )),
            FunctionRole::DeviceFfiExport => Some((
                CompilerModuleSymbolRoleV1::DeviceFfiExport,
                function.id.as_str().to_owned(),
            )),
            FunctionRole::InternalHelper | FunctionRole::ExternalImport => None,
        })
        .collect::<BTreeSet<_>>();
    if source.is_empty() || source != kir {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "semantic and target-bound KIR export roles differ",
        ));
    }
    Ok(source)
}

fn semantic_link_symbol(
    symbol: &fe2o3_mir_model::semantic_mir_v1::SemanticLinkSymbolV1,
) -> Result<String, ProductionSemanticLineageErrorV3> {
    std::str::from_utf8(symbol.as_bytes())
        .map(str::to_owned)
        .map_err(|_| {
            ProductionSemanticLineageErrorV3::AxisMismatch(
                "semantic export symbol is not valid UTF-8",
            )
        })
}

fn validate_final_exports(
    expected_exports: &BTreeSet<(CompilerModuleSymbolRoleV1, String)>,
    descriptor_source: &CompilerDescriptorSourceV1,
    manifest: &CompilerModuleSymbolManifestV1,
) -> Result<(), ProductionSemanticLineageErrorV3> {
    let observed_exports = manifest
        .entries()
        .filter(|(role, _)| {
            matches!(
                role,
                CompilerModuleSymbolRoleV1::KernelEntry
                    | CompilerModuleSymbolRoleV1::DeviceFfiExport
            )
        })
        .map(|(role, symbol)| (role, symbol.to_owned()))
        .collect::<BTreeSet<_>>();
    if &observed_exports != expected_exports {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "final compiler manifest export roles differ from semantic/KIR exports",
        ));
    }

    let expected_kernel_entries = expected_exports
        .iter()
        .filter(|(role, _)| *role == CompilerModuleSymbolRoleV1::KernelEntry)
        .map(|(_, symbol)| symbol.as_str())
        .collect::<BTreeSet<_>>();
    let descriptor_kernel_entries = descriptor_source
        .table()
        .kernels()
        .iter()
        .map(|kernel| kernel.entry_name().as_str())
        .collect::<BTreeSet<_>>();
    if expected_kernel_entries != descriptor_kernel_entries {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "compiler descriptor kernel entries differ from semantic/KIR entries",
        ));
    }

    let expected_descriptors = descriptor_source
        .table()
        .kernels()
        .iter()
        .map(|kernel| kernel.descriptor_symbol().as_str())
        .collect::<BTreeSet<_>>();
    let observed_descriptors = manifest
        .symbols(CompilerModuleSymbolRoleV1::KernelDescriptor)
        .collect::<BTreeSet<_>>();
    if expected_descriptors != observed_descriptors {
        return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
            "final compiler manifest descriptor symbols differ from descriptor source",
        ));
    }
    Ok(())
}

#[derive(Debug)]
pub(crate) enum ProductionSemanticLineageErrorV3 {
    AxisMismatch(&'static str),
    Invocation(String),
    ProtectedRustcInvocation(ProtectedRustcInvocationErrorV1),
    LiveOwner(String),
    CanonicalKir(VerifiedCanonicalKernelIrErrorV8),
    Correspondence(ProductionCorrespondenceEvidenceErrorV4),
    FormalMemory(ProductionFormalMemoryEvidenceErrorV4),
    VerusEvidence(ProductionMirPlironVerusExecutionEvidenceErrorV1),
    KirToLlvmReplay(dialect_amdgcn::ProductionKirToLlvmReplayErrorV1),
    KirToLlvmReplayValidation(CompilerKirToLlvmReplayValidationErrorV1),
    Receipt(LineageErrorV3),
    ProofIdentity(InertProofBindingAssociationErrorV3),
    ProofBinding(InertProofBindingAssociationErrorV4),
    Transcript(ProductionTargetLineageErrorV3),
    FinalCommitment(FinalCompilerModuleCommitmentErrorV3),
    Capsule(InertSemanticCompilerModuleHandoffErrorV3),
}

impl fmt::Display for ProductionSemanticLineageErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AxisMismatch(detail) => {
                write!(formatter, "production V3 lineage mismatch: {detail}")
            }
            Self::Invocation(detail) => {
                write!(formatter, "production V3 invocation failed: {detail}")
            }
            Self::ProtectedRustcInvocation(error) => write!(
                formatter,
                "production V3 protected rustc custody failed: {error}"
            ),
            Self::LiveOwner(detail) => {
                write!(formatter, "production V3 live owner failed: {detail}")
            }
            Self::CanonicalKir(error) => {
                write!(formatter, "production V3 canonical KIR failed: {error}")
            }
            Self::Correspondence(error) => {
                write!(
                    formatter,
                    "production lossless correspondence failed: {error}"
                )
            }
            Self::FormalMemory(error) => {
                write!(
                    formatter,
                    "production formal-memory evidence failed: {error}"
                )
            }
            Self::VerusEvidence(error) => {
                write!(
                    formatter,
                    "production V3 aggregate Verus evidence failed: {error}"
                )
            }
            Self::KirToLlvmReplay(error) => {
                write!(formatter, "production KIR-to-LLVM replay failed: {error}")
            }
            Self::KirToLlvmReplayValidation(error) => write!(
                formatter,
                "production independent KIR-to-LLVM validation failed: {error}"
            ),
            Self::Receipt(error) => write!(formatter, "production V3 receipt failed: {error}"),
            Self::ProofIdentity(error) => {
                write!(formatter, "production V3 proof identity failed: {error}")
            }
            Self::ProofBinding(error) => {
                write!(formatter, "production V3 proof binding failed: {error}")
            }
            Self::Transcript(error) => {
                write!(formatter, "production V3 transcript failed: {error}")
            }
            Self::FinalCommitment(error) => {
                write!(formatter, "production V3 final commitment failed: {error}")
            }
            Self::Capsule(error) => write!(formatter, "production V3 capsule failed: {error}"),
        }
    }
}

impl Error for ProductionSemanticLineageErrorV3 {}

impl From<VerifiedCanonicalKernelIrErrorV8> for ProductionSemanticLineageErrorV3 {
    fn from(error: VerifiedCanonicalKernelIrErrorV8) -> Self {
        Self::CanonicalKir(error)
    }
}

impl From<ProductionCorrespondenceEvidenceErrorV4> for ProductionSemanticLineageErrorV3 {
    fn from(error: ProductionCorrespondenceEvidenceErrorV4) -> Self {
        Self::Correspondence(error)
    }
}

impl From<ProductionFormalMemoryEvidenceErrorV4> for ProductionSemanticLineageErrorV3 {
    fn from(error: ProductionFormalMemoryEvidenceErrorV4) -> Self {
        Self::FormalMemory(error)
    }
}

impl From<LineageErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: LineageErrorV3) -> Self {
        Self::Receipt(error)
    }
}

impl From<ProductionMirPlironVerusExecutionEvidenceErrorV1> for ProductionSemanticLineageErrorV3 {
    fn from(error: ProductionMirPlironVerusExecutionEvidenceErrorV1) -> Self {
        Self::VerusEvidence(error)
    }
}

impl From<dialect_amdgcn::ProductionKirToLlvmReplayErrorV1> for ProductionSemanticLineageErrorV3 {
    fn from(error: dialect_amdgcn::ProductionKirToLlvmReplayErrorV1) -> Self {
        Self::KirToLlvmReplay(error)
    }
}

impl From<CompilerKirToLlvmReplayValidationErrorV1> for ProductionSemanticLineageErrorV3 {
    fn from(error: CompilerKirToLlvmReplayValidationErrorV1) -> Self {
        Self::KirToLlvmReplayValidation(error)
    }
}

impl From<InertProofBindingAssociationErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: InertProofBindingAssociationErrorV3) -> Self {
        Self::ProofIdentity(error)
    }
}

impl From<InertProofBindingAssociationErrorV4> for ProductionSemanticLineageErrorV3 {
    fn from(error: InertProofBindingAssociationErrorV4) -> Self {
        Self::ProofBinding(error)
    }
}

impl From<ProductionTargetLineageErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: ProductionTargetLineageErrorV3) -> Self {
        Self::Transcript(error)
    }
}

impl From<FinalCompilerModuleCommitmentErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: FinalCompilerModuleCommitmentErrorV3) -> Self {
        Self::FinalCommitment(error)
    }
}

impl From<InertSemanticCompilerModuleHandoffErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: InertSemanticCompilerModuleHandoffErrorV3) -> Self {
        Self::Capsule(error)
    }
}

#[cfg(test)]
mod layout_tests {
    use super::*;

    fn llvm_with_layout(layout: &str) -> String {
        format!(
            "target triple = \"amdgcn-amd-amdhsa\"\ntarget datalayout = \"{layout}\"\n\ndefine void @body() {{ ret void }}\n"
        )
    }

    #[test]
    fn final_llvm_requires_one_exact_measured_worker_layout() {
        let exact = llvm_with_layout(PRODUCTION_WORKER_DATA_LAYOUT_V1);
        validate_final_llvm_layout(&exact).unwrap();

        let stale_layout = format!(
            "e-{}",
            PRODUCTION_WORKER_DATA_LAYOUT_V1
                .strip_prefix("e-m:e-")
                .expect("canonical production layout retains ELF mangling")
        );
        assert!(validate_final_llvm_layout(&llvm_with_layout(&stale_layout)).is_err());
        assert!(
            validate_final_llvm_layout(&format!(
                "{exact}target datalayout = \"{PRODUCTION_WORKER_DATA_LAYOUT_V1}\"\n"
            ))
            .is_err()
        );
    }

    #[test]
    fn production_capsule_requires_shared_independent_kir_to_llvm_replay() {
        let source = include_str!("production_semantic_lineage_v3.rs");
        assert!(source.contains("CanonicalProductionKirToLlvmReplayEvidenceV1::from_live_inputs"));
        assert!(source.contains("validate_compiler_kir_to_llvm_replay_v1"));
        assert!(!source.contains(concat!("AmdgpuLoweringTranscript", "V3::new")));
    }

    fn test_association() -> Vec<u8> {
        let identity =
            fe2o3_compiler_lineage::InertSemanticToLlvmContentIdentityV3::new([0x41; 32], 1)
                .unwrap();
        fe2o3_compiler_lineage::InertSemanticToLlvmAssociationV3::new(
            fe2o3_compiler_lineage::InertSemanticToLlvmAssociationInputsV3::new(
                identity, identity, identity, identity, identity, identity, identity, identity,
                identity, identity, identity, identity, identity,
            ),
        )
        .unwrap()
        .canonical_bytes()
        .to_vec()
    }

    #[test]
    fn every_debug_attachment_failure_preserves_the_frozen_core_association() {
        let association = test_association();
        let availability = ProductionSemanticDebugAvailabilityV1::Unavailable(
            ProductionSemanticDebugProducerGapV1::SourceMapUnavailable,
        );
        for (fault, expected_gap) in [
            (
                OptionalSemanticDebugAttachmentFaultV1::PrimaryCarrierResource,
                ProductionSemanticDebugProducerGapV1::ResourceLimit,
            ),
            (
                OptionalSemanticDebugAttachmentFaultV1::PrimaryCarrierStructural,
                ProductionSemanticDebugProducerGapV1::CarrierConstructionUnavailable,
            ),
        ] {
            let receipt = attach_optional_semantic_debug_with_fault_v1(
                &association,
                availability.clone(),
                fault,
            )
            .unwrap();
            let extension = ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(
                receipt.canonical_preimage(),
            )
            .unwrap();
            assert_eq!(extension.association_v3(), association);
            assert!(matches!(
                extension.carrier_v1().availability(),
                ProductionSemanticDebugAvailabilityV1::Unavailable(gap) if *gap == expected_gap
            ));
        }

        for fault in [
            OptionalSemanticDebugAttachmentFaultV1::FallbackCarrier,
            OptionalSemanticDebugAttachmentFaultV1::Extension,
            OptionalSemanticDebugAttachmentFaultV1::ExtensionReceipt,
        ] {
            let receipt = attach_optional_semantic_debug_with_fault_v1(
                &association,
                availability.clone(),
                fault,
            )
            .unwrap();
            assert_eq!(receipt.canonical_preimage(), association);
            assert!(
                fe2o3_compiler_lineage::InertSemanticToLlvmAssociationV3::decode(
                    receipt.canonical_preimage(),
                )
                .is_ok()
            );
        }
    }
}
