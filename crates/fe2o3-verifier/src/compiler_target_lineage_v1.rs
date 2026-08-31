//! Independent validation of singleton target-side compiler lineage.

use std::{error::Error, fmt};

use fe2o3_amd_target::{
    PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1, PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
};
use fe2o3_compiler_lineage::{
    DataLayoutTranscriptV3, InertProductionSemanticCapsuleV3, ProductionTargetLineageErrorV3,
    SemanticToLlvmAssociationTranscriptV3, TargetBindingTranscriptV3, TargetLineageIdentityV3,
    derive_semantic_target_layout_identity_v1,
};
use fe2o3_kernel_ir::{
    ProductionSemanticDebugFragmentErrorV1, ProductionSemanticDebugReceiptExtensionV1,
};
use fe2o3_rustc_invocation::{ValidationError, encode_descriptor_v3};

use crate::{
    CompilerKirToLlvmReplayValidationErrorV1, ValidatedCompilerKirToLlvmReplayV1,
    ValidatedCompilerProofInputsV4, validate_compiler_kir_to_llvm_replay_v1,
};

/// Move-only ownership of independently decoded target lineage and exact KIR-to-LLVM replay.
///
/// The owner establishes exact content association and deterministic replay through the
/// pre-descriptor LLVM module. It does not prove semantic refinement, LLVM-to-machine refinement,
/// producer authenticity, publication authority, or runtime safety.
///
/// ```compile_fail
/// use fe2o3_verifier::ValidatedCompilerTargetLineageV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ValidatedCompilerTargetLineageV1>();
/// ```
#[derive(Debug)]
#[must_use = "dropping validated target lineage abandons exact target-side compiler custody"]
pub struct ValidatedCompilerTargetLineageV1 {
    target_binding: TargetBindingTranscriptV3,
    data_layout: DataLayoutTranscriptV3,
    semantic_to_llvm: SemanticToLlvmAssociationTranscriptV3,
    semantic_to_llvm_association_bytes: Box<[u8]>,
    replay: ValidatedCompilerKirToLlvmReplayV1,
    target_binding_receipt: TargetLineageIdentityV3,
    data_layout_receipt: TargetLineageIdentityV3,
    semantic_to_llvm_receipt: TargetLineageIdentityV3,
    final_llvm: TargetLineageIdentityV3,
    final_compiler_module_commitment: TargetLineageIdentityV3,
}

impl ValidatedCompilerTargetLineageV1 {
    /// Returns the strictly decoded singleton target-binding transcript.
    pub const fn target_binding(&self) -> &TargetBindingTranscriptV3 {
        &self.target_binding
    }

    /// Returns the strictly decoded target data-layout transcript.
    pub const fn data_layout(&self) -> &DataLayoutTranscriptV3 {
        &self.data_layout
    }

    /// Returns the strictly decoded semantic-to-LLVM association transcript.
    pub const fn semantic_to_llvm(&self) -> &SemanticToLlvmAssociationTranscriptV3 {
        &self.semantic_to_llvm
    }

    /// Returns the exact inner V3 association bytes, excluding any optional debug extension.
    pub fn semantic_to_llvm_association_bytes(&self) -> &[u8] {
        &self.semantic_to_llvm_association_bytes
    }

    /// Returns the independently replayed exact KIR-to-LLVM owner.
    pub const fn replay(&self) -> &ValidatedCompilerKirToLlvmReplayV1 {
        &self.replay
    }

    /// Returns the outer target-binding receipt coordinates checked by this owner.
    pub const fn target_binding_receipt_identity(&self) -> TargetLineageIdentityV3 {
        self.target_binding_receipt
    }

    /// Returns the outer data-layout receipt coordinates checked by this owner.
    pub const fn data_layout_receipt_identity(&self) -> TargetLineageIdentityV3 {
        self.data_layout_receipt
    }

    /// Returns the outer semantic-to-LLVM receipt coordinates checked by this owner.
    pub const fn semantic_to_llvm_receipt_identity(&self) -> TargetLineageIdentityV3 {
        self.semantic_to_llvm_receipt
    }

    /// Returns the exact final LLVM content coordinates associated by the compiler capsule.
    pub const fn final_llvm_identity(&self) -> TargetLineageIdentityV3 {
        self.final_llvm
    }

    /// Returns the exact final-module commitment receipt coordinates in the association.
    pub const fn final_compiler_module_commitment_identity(&self) -> TargetLineageIdentityV3 {
        self.final_compiler_module_commitment
    }

    /// Reports that every target-side receipt coordinate was checked against the exact capsule.
    pub const fn has_exact_receipt_association(&self) -> bool {
        true
    }

    /// Reports deterministic reconstruction of target-bound KIR and pre-descriptor LLVM.
    pub const fn has_exact_kir_to_llvm_replay(&self) -> bool {
        self.replay.has_exact_target_binding_replay() && self.replay.has_exact_kir_to_llvm_replay()
    }

    /// Reports that association and deterministic replay are not a semantic-refinement proof.
    pub const fn establishes_semantic_refinement(&self) -> bool {
        false
    }

    /// Reports that this owner establishes no LLVM-to-machine refinement.
    pub const fn establishes_llvm_to_machine_refinement(&self) -> bool {
        false
    }

    /// Reports that this owner authenticates no compiler or verifier deployment.
    pub const fn authenticates_producer(&self) -> bool {
        false
    }

    /// Reports that this owner grants no publication, load, or launch authority.
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Independently decodes and cross-checks one singleton capsule's complete target-side lineage.
///
/// `proof_inputs` supplies the independently decoded semantic-MIR owner used to rederive the exact
/// semantic target-layout identity. The returned owner remains non-authoritative.
pub fn validate_compiler_target_lineage_v1(
    capsule: &InertProductionSemanticCapsuleV3,
    proof_inputs: &ValidatedCompilerProofInputsV4,
) -> Result<ValidatedCompilerTargetLineageV1, CompilerTargetLineageValidationErrorV1> {
    let receipts = capsule.receipts();
    if proof_inputs.receipt_identity() != receipts.proof_binding().identity()
        || proof_inputs.semantic_mir().canonical_encoding()
            != receipts.semantic_mir().canonical_preimage()
        || proof_inputs.kernel_ir().canonical_bytes() != receipts.kernel_ir().canonical_preimage()
    {
        return Err(CompilerTargetLineageValidationErrorV1::ProofInputMismatch);
    }

    let target_binding =
        TargetBindingTranscriptV3::decode(receipts.target_binding().canonical_preimage())
            .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let data_layout = DataLayoutTranscriptV3::decode(receipts.data_layout().canonical_preimage())
        .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let semantic_to_llvm_preimage = receipts.semantic_to_llvm().canonical_preimage();
    let (semantic_to_llvm, semantic_to_llvm_association_bytes) =
        match SemanticToLlvmAssociationTranscriptV3::decode(semantic_to_llvm_preimage) {
            Ok(association) => (
                association,
                semantic_to_llvm_preimage.to_vec().into_boxed_slice(),
            ),
            Err(_) => {
                let extension = ProductionSemanticDebugReceiptExtensionV1::from_canonical_bytes(
                    semantic_to_llvm_preimage,
                )
                .map_err(CompilerTargetLineageValidationErrorV1::SemanticDebugExtension)?;
                let association_bytes = extension.association_v3().to_vec().into_boxed_slice();
                let association = SemanticToLlvmAssociationTranscriptV3::decode(&association_bytes)
                    .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
                (association, association_bytes)
            }
        };
    let replay =
        validate_compiler_kir_to_llvm_replay_v1(receipts.kernel_ir(), receipts.amdgpu_lowering())
            .map_err(CompilerTargetLineageValidationErrorV1::Replay)?;

    let invocation_bytes = encode_descriptor_v3(capsule.invocation())
        .map_err(CompilerTargetLineageValidationErrorV1::Invocation)?;
    let protected_invocation = TargetLineageIdentityV3::new(
        capsule.invocation_digest().into_bytes(),
        u64::try_from(invocation_bytes.len())
            .map_err(|_| CompilerTargetLineageValidationErrorV1::LengthOverflow)?,
    )
    .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;

    let target_inputs = target_binding
        .inputs()
        .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let data_layout_inputs = data_layout
        .inputs()
        .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let association_inputs = semantic_to_llvm
        .inputs()
        .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;

    let semantic_mir = receipt_identity(
        receipts.semantic_mir().identity().sha256(),
        receipts.semantic_mir().identity().byte_len(),
    )?;
    let target_binding_receipt = receipt_identity(
        receipts.target_binding().identity().sha256(),
        receipts.target_binding().identity().byte_len(),
    )?;
    let data_layout_receipt = receipt_identity(
        receipts.data_layout().identity().sha256(),
        receipts.data_layout().identity().byte_len(),
    )?;
    let semantic_to_llvm_receipt = receipt_identity(
        receipts.semantic_to_llvm().identity().sha256(),
        receipts.semantic_to_llvm().identity().byte_len(),
    )?;

    let replay_evidence = replay.replay().evidence();
    let neutral_kir = TargetLineageIdentityV3::new(
        replay_evidence.neutral_kernel_ir_identity().sha256(),
        replay_evidence.neutral_kernel_ir_identity().byte_len(),
    )
    .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let target_kir = TargetLineageIdentityV3::new(
        replay_evidence.target_bound_kernel_ir_identity().sha256(),
        replay_evidence.target_bound_kernel_ir_identity().byte_len(),
    )
    .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    let configured_target = capsule.target().to_string();
    for (matches, field) in [
        (
            target_inputs.protected_rustc_invocation == protected_invocation,
            "protected rustc invocation",
        ),
        (target_inputs.semantic_mir == semantic_mir, "semantic MIR"),
        (
            target_inputs.target_neutral_kir == neutral_kir,
            "target-neutral Kernel IR",
        ),
        (
            target_inputs.target_bound_kir == target_kir,
            "target-bound Kernel IR",
        ),
        (
            target_inputs.configured_target == configured_target,
            "configured target",
        ),
        (
            replay_evidence.profile().device_target() == configured_target,
            "replayed target profile",
        ),
    ] {
        require_identity_match(matches, field)?;
    }

    let semantic_layout = derive_semantic_target_layout_identity_v1(
        target_inputs.rustc_llvm_target,
        data_layout_inputs.live_rustc_data_layout,
        data_layout_inputs.default_pointer_width_bits,
        target_inputs.target_cpu,
        target_inputs.target_features,
    )
    .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)?;
    for (matches, field) in [
        (
            data_layout_inputs.semantic_mir == semantic_mir,
            "data-layout semantic MIR",
        ),
        (
            data_layout_inputs.target_binding == target_binding_receipt,
            "data-layout target binding",
        ),
        (
            data_layout_inputs.semantic_layout == semantic_layout,
            "semantic target layout",
        ),
        (
            data_layout_inputs.live_rustc_data_layout == PRODUCTION_AMDHSA_RUSTC_DATA_LAYOUT_V1,
            "live rustc data layout",
        ),
        (
            data_layout_inputs.final_llvm_data_layout
                == PRODUCTION_AMDHSA_LLVM22_WORKER_DATA_LAYOUT_V1,
            "final LLVM Worker data layout",
        ),
        (
            proof_inputs
                .semantic_mir()
                .target_layout_identity()
                .as_bytes()
                == &semantic_layout.sha256(),
            "semantic-MIR target layout",
        ),
    ] {
        require_identity_match(matches, field)?;
    }

    let expected_associations = [
        (
            semantic_mir,
            association_inputs.semantic_mir,
            "semantic MIR",
        ),
        (
            receipt_identity(
                receipts.middle_end().identity().sha256(),
                receipts.middle_end().identity().byte_len(),
            )?,
            association_inputs.middle_end,
            "middle end",
        ),
        (
            receipt_identity(
                receipts.kernel_ir().identity().sha256(),
                receipts.kernel_ir().identity().byte_len(),
            )?,
            association_inputs.kernel_ir,
            "Kernel IR",
        ),
        (
            receipt_identity(
                receipts.mir_to_kir_correspondence().identity().sha256(),
                receipts.mir_to_kir_correspondence().identity().byte_len(),
            )?,
            association_inputs.mir_to_kir_correspondence,
            "MIR-to-KIR correspondence",
        ),
        (
            receipt_identity(
                receipts.formal_memory().identity().sha256(),
                receipts.formal_memory().identity().byte_len(),
            )?,
            association_inputs.formal_memory,
            "formal memory",
        ),
        (
            receipt_identity(
                receipts.proof_binding().identity().sha256(),
                receipts.proof_binding().identity().byte_len(),
            )?,
            association_inputs.proof_binding,
            "proof binding",
        ),
        (
            target_binding_receipt,
            association_inputs.target_binding,
            "target binding",
        ),
        (
            data_layout_receipt,
            association_inputs.data_layout,
            "data layout",
        ),
        (
            receipt_identity(
                receipts.abi().identity().sha256(),
                receipts.abi().identity().byte_len(),
            )?,
            association_inputs.abi,
            "ABI",
        ),
        (
            receipt_identity(
                receipts.export_manifest().identity().sha256(),
                receipts.export_manifest().identity().byte_len(),
            )?,
            association_inputs.export_manifest,
            "export manifest",
        ),
        (
            receipt_identity(
                receipts.amdgpu_lowering().identity().sha256(),
                receipts.amdgpu_lowering().identity().byte_len(),
            )?,
            association_inputs.amdgpu_lowering,
            "AMDGPU lowering",
        ),
        (
            receipt_identity(
                receipts
                    .final_compiler_module_commitment()
                    .identity()
                    .sha256(),
                receipts
                    .final_compiler_module_commitment()
                    .identity()
                    .byte_len(),
            )?,
            association_inputs.final_compiler_module_commitment,
            "final compiler module commitment",
        ),
    ];
    for (actual, associated, field) in expected_associations {
        require_identity_match(actual == associated, field)?;
    }

    Ok(ValidatedCompilerTargetLineageV1 {
        target_binding,
        data_layout,
        semantic_to_llvm,
        semantic_to_llvm_association_bytes,
        replay,
        target_binding_receipt,
        data_layout_receipt,
        semantic_to_llvm_receipt,
        final_llvm: association_inputs.final_llvm,
        final_compiler_module_commitment: association_inputs.final_compiler_module_commitment,
    })
}

fn receipt_identity(
    sha256: &[u8; 32],
    byte_len: u64,
) -> Result<TargetLineageIdentityV3, CompilerTargetLineageValidationErrorV1> {
    TargetLineageIdentityV3::new(*sha256, byte_len)
        .map_err(CompilerTargetLineageValidationErrorV1::TargetLineage)
}

fn require_identity_match(
    matches: bool,
    field: &'static str,
) -> Result<(), CompilerTargetLineageValidationErrorV1> {
    if matches {
        Ok(())
    } else {
        Err(CompilerTargetLineageValidationErrorV1::IdentityMismatch { field })
    }
}

/// Failure while independently reconstructing target-side compiler lineage.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerTargetLineageValidationErrorV1 {
    /// A canonical target-lineage record failed strict construction or decoding.
    TargetLineage(ProductionTargetLineageErrorV3),
    /// The optional semantic-debug receipt extension was malformed.
    SemanticDebugExtension(ProductionSemanticDebugFragmentErrorV1),
    /// Exact target-bound KIR or pre-descriptor LLVM replay failed.
    Replay(CompilerKirToLlvmReplayValidationErrorV1),
    /// The retained rustc invocation could not be canonically re-encoded.
    Invocation(ValidationError),
    /// A host length could not be represented by the canonical wire coordinate.
    LengthOverflow,
    /// The independently decoded proof owner does not belong to this capsule.
    ProofInputMismatch,
    /// An association names content other than the exact independently retained input.
    IdentityMismatch {
        /// Human-readable association field.
        field: &'static str,
    },
}

impl fmt::Display for CompilerTargetLineageValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TargetLineage(error) => {
                write!(formatter, "target-lineage decoding failed: {error}")
            }
            Self::SemanticDebugExtension(error) => {
                write!(
                    formatter,
                    "semantic-debug receipt extension decoding failed: {error}"
                )
            }
            Self::Replay(error) => write!(formatter, "target-lineage replay failed: {error}"),
            Self::Invocation(error) => {
                write!(
                    formatter,
                    "target-lineage invocation encoding failed: {error}"
                )
            }
            Self::LengthOverflow => formatter.write_str("target-lineage length overflow"),
            Self::ProofInputMismatch => formatter
                .write_str("target-lineage proof owner does not match the exact compiler capsule"),
            Self::IdentityMismatch { field } => {
                write!(formatter, "target-lineage {field} identity mismatch")
            }
        }
    }
}

impl Error for CompilerTargetLineageValidationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::TargetLineage(error) => Some(error),
            Self::SemanticDebugExtension(error) => Some(error),
            Self::Replay(error) => Some(error),
            Self::Invocation(error) => Some(error),
            Self::LengthOverflow | Self::ProofInputMismatch | Self::IdentityMismatch { .. } => None,
        }
    }
}
