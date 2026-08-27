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
    InertProofBindingAssociationErrorV3, InertProofBindingAssociationInputsV3,
    InertProofBindingAssociationV3, InertProofBindingReceiptV3,
    InertRustcIdentityInventoryReceiptV3, InertRustcPreflightPlanReceiptV3,
    InertSemanticToLlvmReceiptV3, InertTargetBindingReceiptV3, LineageErrorV3,
    OrderedInertSemanticLineageReceiptsV3,
};
use fe2o3_kernel_ir::{FunctionRole, Module, VerifiedCanonicalKernelIrErrorV5};
use fe2o3_lower_mir_kernel::{
    InertCanonicalFormalMemoryAdmissionEvidenceV3, InertCanonicalMirToKirCorrespondenceEvidenceV3,
    ProductionFormalMemoryOwnerV1, ProductionLineageEvidenceErrorV3,
};
use fe2o3_rustc_invocation::{
    InvocationDigestV3, RustcInvocationDescriptorV3, encode_descriptor_v3,
};
use sha2::{Digest, Sha256};

use crate::production_ranked_projection_v1::AuthenticatedRankedVerificationV5;
use crate::production_target_lineage_v3::{
    AmdgpuLoweringTranscriptInputsV3, AmdgpuLoweringTranscriptV3, DataLayoutTranscriptInputsV3,
    DataLayoutTranscriptV3, ProductionTargetLineageErrorV3, SemanticToLlvmAssociationInputsV3,
    SemanticToLlvmAssociationTranscriptV3, TargetBindingTranscriptInputsV3,
    TargetBindingTranscriptV3, TargetLineageIdentityV3,
};

const CODE_OBJECT_VERSION_V3: u16 = 6;
const WAVE_WIDTH_BITS_V3: u16 = 64;

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
    neutral_kir_identity: TargetLineageIdentityV3,
    bound_kir_identity: TargetLineageIdentityV3,
    semantic_layout_identity: TargetLineageIdentityV3,
    expected_exports: BTreeSet<(CompilerModuleSymbolRoleV1, String)>,
    rustc_layout: crate::semantic_layout_bridge::SemanticLayoutTargetV1,
    default_workgroup: [u32; 3],
    pre_descriptor_llvm: Box<[u8]>,
}

impl PreparedProductionSemanticLineageV3 {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn try_prepare(
        rustc_identity_inventory: &crate::collector::AuthenticatedRustcIdentityInventoryV3,
        rustc_preflight_plan: &crate::collector::AuthenticatedRustcPreflightPlanV3,
        rustc_target: &crate::production_target_v1::AuthenticatedProductionTargetV1,
        ranked_verification: &AuthenticatedRankedVerificationV5,
        admitted: &ProductionFormalMemoryOwnerV1,
        target_module: &Module,
        pre_descriptor_llvm: &str,
    ) -> Result<Self, ProductionSemanticLineageErrorV3> {
        admitted
            .verify_equivalence()
            .map_err(|error| ProductionSemanticLineageErrorV3::LiveOwner(error.to_string()))?;

        let semantic = admitted.semantic_kir().semantic().semantic();
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

        let neutral_kir = fe2o3_kernel_ir::VerifiedCanonicalKernelIrV5::from_module(
            admitted.semantic_kir().module().clone(),
        )?;
        neutral_kir.revalidate()?;
        let bound_kir =
            fe2o3_kernel_ir::VerifiedCanonicalKernelIrV5::from_module(target_module.clone())?;
        bound_kir.revalidate()?;
        let neutral_kir_identity = TargetLineageIdentityV3::new(
            *neutral_kir.identity().digest(),
            neutral_kir.canonical_bytes().len() as u64,
        )?;
        let bound_kir_identity = TargetLineageIdentityV3::new(
            *bound_kir.identity().digest(),
            bound_kir.canonical_bytes().len() as u64,
        )?;
        let kernel_ir =
            InertKernelIrReceiptV3::from_canonical_preimage(neutral_kir.canonical_bytes())?;

        let correspondence = InertCanonicalMirToKirCorrespondenceEvidenceV3::from_live_owner(
            admitted.semantic_kir(),
        )?;
        if correspondence.canonical_kir_v5_identity() != neutral_kir.identity().digest() {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "MIR-to-KIR correspondence names a different neutral KIR",
            ));
        }
        let mir_to_kir_correspondence =
            InertMirToKirCorrespondenceReceiptV3::from_canonical_preimage(
                correspondence.canonical_bytes(),
            )?;

        let formal = InertCanonicalFormalMemoryAdmissionEvidenceV3::from_live_owner(admitted)?;
        if formal.canonical_kir_v5_identity() != neutral_kir.identity().digest() {
            return Err(ProductionSemanticLineageErrorV3::AxisMismatch(
                "formal-memory admission names a different neutral KIR",
            ));
        }
        let formal_memory =
            InertFormalMemoryReceiptV3::from_canonical_preimage(formal.canonical_bytes())?;

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
            neutral_kir_identity,
            bound_kir_identity,
            semantic_layout_identity,
            expected_exports,
            rustc_layout: rustc_target.rustc_layout().clone(),
            default_workgroup: [workgroup.x, workgroup.y, workgroup.z],
            pre_descriptor_llvm: pre_descriptor_llvm.as_bytes().into(),
        })
    }

    pub(crate) fn finish(
        self,
        invocation: RustcInvocationDescriptorV3,
        target: fe2o3_compiler_ffi::DeviceTargetV1,
        descriptor_source: &CompilerDescriptorSourceV1,
        module_handoff: CompilerModuleHandoffV2,
    ) -> Result<InertSemanticCompilerModuleHandoffV3, ProductionSemanticLineageErrorV3> {
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

        let proof_binding =
            InertProofBindingAssociationV3::new(InertProofBindingAssociationInputsV3::new(
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
            ))?;
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
            final_llvm_data_layout: self.rustc_layout.data_layout(),
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

        let amdgpu_lowering = AmdgpuLoweringTranscriptV3::new(AmdgpuLoweringTranscriptInputsV3 {
            target_binding: target_binding_identity,
            data_layout: data_layout_identity,
            target_bound_kir: self.bound_kir_identity,
            configured_target: &configured_target,
            pre_descriptor_llvm: &self.pre_descriptor_llvm,
        })?;
        let amdgpu_lowering = InertAmdgpuLoweringReceiptV3::from_canonical_preimage(
            amdgpu_lowering.canonical_bytes(),
        )?;
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

        let semantic_to_llvm =
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
        let semantic_to_llvm = InertSemanticToLlvmReceiptV3::from_canonical_preimage(
            semantic_to_llvm.canonical_bytes(),
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
    LiveOwner(String),
    CanonicalKir(VerifiedCanonicalKernelIrErrorV5),
    Evidence(ProductionLineageEvidenceErrorV3),
    Receipt(LineageErrorV3),
    ProofBinding(InertProofBindingAssociationErrorV3),
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
            Self::LiveOwner(detail) => {
                write!(formatter, "production V3 live owner failed: {detail}")
            }
            Self::CanonicalKir(error) => {
                write!(formatter, "production V3 canonical KIR failed: {error}")
            }
            Self::Evidence(error) => write!(formatter, "production V3 evidence failed: {error}"),
            Self::Receipt(error) => write!(formatter, "production V3 receipt failed: {error}"),
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

impl From<VerifiedCanonicalKernelIrErrorV5> for ProductionSemanticLineageErrorV3 {
    fn from(error: VerifiedCanonicalKernelIrErrorV5) -> Self {
        Self::CanonicalKir(error)
    }
}

impl From<ProductionLineageEvidenceErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: ProductionLineageEvidenceErrorV3) -> Self {
        Self::Evidence(error)
    }
}

impl From<LineageErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: LineageErrorV3) -> Self {
        Self::Receipt(error)
    }
}

impl From<InertProofBindingAssociationErrorV3> for ProductionSemanticLineageErrorV3 {
    fn from(error: InertProofBindingAssociationErrorV3) -> Self {
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
