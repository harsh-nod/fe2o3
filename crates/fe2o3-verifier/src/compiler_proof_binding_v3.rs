//! Independent validation of compiler proof inputs carried by the frozen V3 capsule envelope.
//!
//! The legacy V3 association validates exact canonical content and structural relationships only.
//! The current V4 association additionally imports the exact signed aggregate
//! MIR-to-live-PLIRON Verus receipt under its embedded key. Neither path authenticates compiler
//! origin, establishes LLVM or machine refinement, or grants runtime authority.

use std::{error::Error, fmt};

use fe2o3_compiler_lineage::{
    InertCanonicalSemanticMirReceiptV3, InertFormalMemoryReceiptV3, InertKernelIrReceiptV3,
    InertLineageContentIdentityV3, InertMiddleEndReceiptV3, InertMirToKirCorrespondenceReceiptV3,
    InertProofBindingAssociationErrorV3, InertProofBindingAssociationErrorV4,
    InertProofBindingAssociationV3, InertProofBindingAssociationV4,
    InertProofBindingReceiptIdentityV3, InertProofBindingReceiptV3,
};
use fe2o3_kernel_ir::{Module, VerifiedCanonicalKernelIrErrorV5, VerifiedCanonicalKernelIrV5};
use fe2o3_lower_mir_kernel::{
    InertCanonicalFormalMemoryAdmissionEvidenceV3, InertCanonicalMirToKirCorrespondenceEvidenceV3,
    ProductionLineageEvidenceErrorV3,
};
use fe2o3_mir_model::semantic_mir_v1::{
    AdmittedInertSemanticMirV1, SemanticMirDecodeErrorV1, SemanticMirLimitsV1,
};
use fe2o3_pliron::{InertProductionMiddleEndEvidenceV5, ProductionMiddleEndEvidenceCodecErrorV5};

use crate::{
    CanonicalProductionMirPlironVerusExecutionEvidenceV1,
    ProductionMirPlironVerusExecutionEvidenceErrorV1,
};

/// Independently decoded and cross-checked V3 compiler proof inputs.
///
/// The value owns the exact semantic MIR, middle-end evidence, verified canonical Kernel IR,
/// MIR-to-KIR correspondence, formal-memory admission, and their outer association. It is
/// intentionally non-`Clone` so a later authority-bearing join can consume the one checked
/// occurrence without changing this type's current non-authoritative contract.
///
/// ```compile_fail
/// use fe2o3_verifier::ValidatedCompilerProofInputsV3;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ValidatedCompilerProofInputsV3>();
/// ```
#[derive(Debug)]
#[must_use = "dropping validated proof inputs abandons the exact decoded compiler evidence"]
pub struct ValidatedCompilerProofInputsV3 {
    association: InertProofBindingAssociationV3,
    receipt_identity: InertProofBindingReceiptIdentityV3,
    semantic_mir: AdmittedInertSemanticMirV1,
    middle_end: InertProductionMiddleEndEvidenceV5,
    kernel_ir: VerifiedCanonicalKernelIrV5,
    correspondence: InertCanonicalMirToKirCorrespondenceEvidenceV3,
    formal_memory: InertCanonicalFormalMemoryAdmissionEvidenceV3,
}

/// Independently decoded current compiler proof inputs, including the exact signed aggregate
/// MIR-to-live-PLIRON Verus receipt.
///
/// This value remains non-authoritative until a private host join consumes protected compiler
/// origin. It deliberately establishes no LLVM, ISA, machine, load, or launch claim.
///
/// ```compile_fail
/// use fe2o3_verifier::ValidatedCompilerProofInputsV4;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ValidatedCompilerProofInputsV4>();
/// ```
#[derive(Debug)]
#[must_use = "dropping validated V4 proof inputs abandons the exact signed compiler evidence"]
pub struct ValidatedCompilerProofInputsV4 {
    association: InertProofBindingAssociationV4,
    receipt_identity: InertProofBindingReceiptIdentityV3,
    semantic_mir: AdmittedInertSemanticMirV1,
    middle_end: InertProductionMiddleEndEvidenceV5,
    kernel_ir: VerifiedCanonicalKernelIrV5,
    correspondence: InertCanonicalMirToKirCorrespondenceEvidenceV3,
    formal_memory: InertCanonicalFormalMemoryAdmissionEvidenceV3,
    verus_execution: CanonicalProductionMirPlironVerusExecutionEvidenceV1,
}

impl ValidatedCompilerProofInputsV3 {
    /// Returns the independently decoded association.
    pub const fn association(&self) -> &InertProofBindingAssociationV3 {
        &self.association
    }

    /// Returns the exact outer lineage-receipt identity whose preimage was decoded.
    pub const fn receipt_identity(&self) -> InertProofBindingReceiptIdentityV3 {
        self.receipt_identity
    }

    /// Returns the independently decoded exact production semantic MIR.
    pub const fn semantic_mir(&self) -> &AdmittedInertSemanticMirV1 {
        &self.semantic_mir
    }

    /// Returns the independently decoded exact V5 middle-end evidence.
    pub const fn middle_end(&self) -> &InertProductionMiddleEndEvidenceV5 {
        &self.middle_end
    }

    /// Returns the independently decoded and semantically verified exact Kernel IR V5.
    pub const fn kernel_ir(&self) -> &VerifiedCanonicalKernelIrV5 {
        &self.kernel_ir
    }

    /// Returns the independently decoded exact MIR-to-KIR correspondence.
    pub const fn correspondence(&self) -> &InertCanonicalMirToKirCorrespondenceEvidenceV3 {
        &self.correspondence
    }

    /// Returns the independently decoded exact formal-memory admission.
    pub const fn formal_memory(&self) -> &InertCanonicalFormalMemoryAdmissionEvidenceV3 {
        &self.formal_memory
    }

    /// Reports that all five exact receipt preimages were independently decoded and associated.
    pub const fn has_exact_decoded_input_association(&self) -> bool {
        true
    }

    /// Reports that retained block locators and source statement counts match decoded MIR and KIR.
    pub const fn has_structural_mir_to_kir_correspondence(&self) -> bool {
        true
    }

    /// Reports that content association alone does not authenticate Verus execution.
    pub const fn authenticates_verus_execution(&self) -> bool {
        false
    }

    /// Reports that content association alone does not establish compiler refinement.
    pub const fn establishes_compiler_refinement(&self) -> bool {
        false
    }

    /// Reports that content association alone grants no load or launch authority.
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

impl ValidatedCompilerProofInputsV4 {
    /// Returns the independently decoded current association.
    pub const fn association(&self) -> &InertProofBindingAssociationV4 {
        &self.association
    }

    /// Returns the exact outer lineage-receipt identity whose preimage was decoded.
    pub const fn receipt_identity(&self) -> InertProofBindingReceiptIdentityV3 {
        self.receipt_identity
    }

    /// Returns the independently decoded exact production semantic MIR.
    pub const fn semantic_mir(&self) -> &AdmittedInertSemanticMirV1 {
        &self.semantic_mir
    }

    /// Returns the independently decoded exact V5 middle-end evidence.
    pub const fn middle_end(&self) -> &InertProductionMiddleEndEvidenceV5 {
        &self.middle_end
    }

    /// Returns the independently decoded and semantically verified exact Kernel IR V5.
    pub const fn kernel_ir(&self) -> &VerifiedCanonicalKernelIrV5 {
        &self.kernel_ir
    }

    /// Returns the independently decoded exact MIR-to-KIR correspondence.
    pub const fn correspondence(&self) -> &InertCanonicalMirToKirCorrespondenceEvidenceV3 {
        &self.correspondence
    }

    /// Returns the independently decoded exact formal-memory admission.
    pub const fn formal_memory(&self) -> &InertCanonicalFormalMemoryAdmissionEvidenceV3 {
        &self.formal_memory
    }

    /// Returns the exact canonical aggregate evidence and its imported signed receipt.
    pub const fn verus_execution(&self) -> &CanonicalProductionMirPlironVerusExecutionEvidenceV1 {
        &self.verus_execution
    }

    /// Reports that all exact stage bytes and the nested Verus evidence were associated.
    pub const fn has_exact_decoded_input_association(&self) -> bool {
        true
    }

    /// Reports that retained block locators and source statement counts match decoded MIR and KIR.
    pub const fn has_structural_mir_to_kir_correspondence(&self) -> bool {
        true
    }

    /// Reports that the exact signed receipt was independently imported under its embedded key.
    pub const fn authenticates_signed_verus_receipt_under_embedded_key(&self) -> bool {
        self.verus_execution
            .authenticates_signed_receipt_under_embedded_key()
    }

    /// Reports that protected compiler origin remains a separate required join.
    pub const fn authenticates_compiler_origin(&self) -> bool {
        false
    }

    /// Reports that source-side proof evidence establishes no LLVM or machine refinement.
    pub const fn establishes_llvm_or_machine_refinement(&self) -> bool {
        false
    }

    /// Reports that decoded proof inputs alone grant no load or launch authority.
    pub const fn grants_runtime_authority(&self) -> bool {
        false
    }
}

/// Strictly decodes and cross-checks the compiler proof association and all five input receipts.
pub fn validate_compiler_proof_inputs_v3(
    proof_binding: &InertProofBindingReceiptV3,
    semantic_mir: &InertCanonicalSemanticMirReceiptV3,
    middle_end: &InertMiddleEndReceiptV3,
    kernel_ir: &InertKernelIrReceiptV3,
    mir_to_kir_correspondence: &InertMirToKirCorrespondenceReceiptV3,
    formal_memory: &InertFormalMemoryReceiptV3,
) -> Result<ValidatedCompilerProofInputsV3, CompilerProofInputValidationErrorV3> {
    let association = InertProofBindingAssociationV3::decode(proof_binding.canonical_preimage())
        .map_err(CompilerProofInputValidationErrorV3::ProofBindingDecode)?;
    let inputs = association.inputs();
    for (actual, expected, field) in [
        (
            inputs.semantic_mir(),
            content_identity(
                semantic_mir.identity().sha256(),
                semantic_mir.identity().byte_len(),
            )?,
            "semantic MIR",
        ),
        (
            inputs.middle_end(),
            content_identity(
                middle_end.identity().sha256(),
                middle_end.identity().byte_len(),
            )?,
            "middle end",
        ),
        (
            inputs.kernel_ir(),
            content_identity(
                kernel_ir.identity().sha256(),
                kernel_ir.identity().byte_len(),
            )?,
            "Kernel IR",
        ),
        (
            inputs.mir_to_kir_correspondence(),
            content_identity(
                mir_to_kir_correspondence.identity().sha256(),
                mir_to_kir_correspondence.identity().byte_len(),
            )?,
            "MIR-to-KIR correspondence",
        ),
        (
            inputs.formal_memory(),
            content_identity(
                formal_memory.identity().sha256(),
                formal_memory.identity().byte_len(),
            )?,
            "formal memory",
        ),
    ] {
        if actual != expected {
            return Err(
                CompilerProofInputValidationErrorV3::ProofBindingIdentityMismatch { field },
            );
        }
    }

    let decoded = decode_and_cross_check_stages(
        semantic_mir,
        middle_end,
        kernel_ir,
        mir_to_kir_correspondence,
        formal_memory,
    )?;

    Ok(ValidatedCompilerProofInputsV3 {
        association,
        receipt_identity: proof_binding.identity(),
        semantic_mir: decoded.semantic_mir,
        middle_end: decoded.middle_end,
        kernel_ir: decoded.kernel_ir,
        correspondence: decoded.correspondence,
        formal_memory: decoded.formal_memory,
    })
}

/// Strictly decodes the current proof association, all five stage receipts, and the exact signed
/// aggregate MIR-to-live-PLIRON Verus execution.
pub fn validate_compiler_proof_inputs_v4(
    proof_binding: &InertProofBindingReceiptV3,
    semantic_mir: &InertCanonicalSemanticMirReceiptV3,
    middle_end: &InertMiddleEndReceiptV3,
    kernel_ir: &InertKernelIrReceiptV3,
    mir_to_kir_correspondence: &InertMirToKirCorrespondenceReceiptV3,
    formal_memory: &InertFormalMemoryReceiptV3,
) -> Result<ValidatedCompilerProofInputsV4, CompilerProofInputValidationErrorV4> {
    let association = InertProofBindingAssociationV4::decode(proof_binding.canonical_preimage())
        .map_err(CompilerProofInputValidationErrorV4::ProofBindingDecode)?;
    let inputs = association.inputs();
    for (actual, expected, field) in [
        (
            inputs.semantic_mir(),
            content_identity(
                semantic_mir.identity().sha256(),
                semantic_mir.identity().byte_len(),
            )
            .map_err(CompilerProofInputValidationErrorV4::Stage)?,
            "semantic MIR",
        ),
        (
            inputs.middle_end(),
            content_identity(
                middle_end.identity().sha256(),
                middle_end.identity().byte_len(),
            )
            .map_err(CompilerProofInputValidationErrorV4::Stage)?,
            "middle end",
        ),
        (
            inputs.kernel_ir(),
            content_identity(
                kernel_ir.identity().sha256(),
                kernel_ir.identity().byte_len(),
            )
            .map_err(CompilerProofInputValidationErrorV4::Stage)?,
            "Kernel IR",
        ),
        (
            inputs.mir_to_kir_correspondence(),
            content_identity(
                mir_to_kir_correspondence.identity().sha256(),
                mir_to_kir_correspondence.identity().byte_len(),
            )
            .map_err(CompilerProofInputValidationErrorV4::Stage)?,
            "MIR-to-KIR correspondence",
        ),
        (
            inputs.formal_memory(),
            content_identity(
                formal_memory.identity().sha256(),
                formal_memory.identity().byte_len(),
            )
            .map_err(CompilerProofInputValidationErrorV4::Stage)?,
            "formal memory",
        ),
    ] {
        if actual != expected {
            return Err(
                CompilerProofInputValidationErrorV4::ProofBindingIdentityMismatch { field },
            );
        }
    }
    let decoded = decode_and_cross_check_stages(
        semantic_mir,
        middle_end,
        kernel_ir,
        mir_to_kir_correspondence,
        formal_memory,
    )
    .map_err(CompilerProofInputValidationErrorV4::Stage)?;
    let verus_execution = CanonicalProductionMirPlironVerusExecutionEvidenceV1::decode(
        association.verus_execution_evidence(),
    )
    .map_err(CompilerProofInputValidationErrorV4::VerusEvidence)?;
    if verus_execution
        .claims()
        .pliron_evidence_identity()
        .as_bytes()
        != decoded.middle_end.identity().sha256()
    {
        return Err(CompilerProofInputValidationErrorV4::VerusMiddleEndMismatch);
    }

    Ok(ValidatedCompilerProofInputsV4 {
        association,
        receipt_identity: proof_binding.identity(),
        semantic_mir: decoded.semantic_mir,
        middle_end: decoded.middle_end,
        kernel_ir: decoded.kernel_ir,
        correspondence: decoded.correspondence,
        formal_memory: decoded.formal_memory,
        verus_execution,
    })
}

struct DecodedCompilerProofStagesV3 {
    semantic_mir: AdmittedInertSemanticMirV1,
    middle_end: InertProductionMiddleEndEvidenceV5,
    kernel_ir: VerifiedCanonicalKernelIrV5,
    correspondence: InertCanonicalMirToKirCorrespondenceEvidenceV3,
    formal_memory: InertCanonicalFormalMemoryAdmissionEvidenceV3,
}

fn decode_and_cross_check_stages(
    semantic_mir: &InertCanonicalSemanticMirReceiptV3,
    middle_end: &InertMiddleEndReceiptV3,
    kernel_ir: &InertKernelIrReceiptV3,
    mir_to_kir_correspondence: &InertMirToKirCorrespondenceReceiptV3,
    formal_memory: &InertFormalMemoryReceiptV3,
) -> Result<DecodedCompilerProofStagesV3, CompilerProofInputValidationErrorV3> {
    let decoded_semantic_mir = AdmittedInertSemanticMirV1::decode_current_production_canonical(
        semantic_mir.canonical_preimage(),
        SemanticMirLimitsV1::default(),
    )
    .map_err(CompilerProofInputValidationErrorV3::SemanticMirDecode)?;
    let decoded_middle_end =
        InertProductionMiddleEndEvidenceV5::decode(middle_end.canonical_preimage())
            .map_err(CompilerProofInputValidationErrorV3::MiddleEndDecode)?;
    let (decoded_kernel_ir, kernel_module) =
        VerifiedCanonicalKernelIrV5::from_canonical_bytes_with_module(
            kernel_ir.canonical_preimage().to_vec(),
        )
        .map_err(CompilerProofInputValidationErrorV3::KernelIr)?;
    let decoded_correspondence = InertCanonicalMirToKirCorrespondenceEvidenceV3::decode(
        mir_to_kir_correspondence.canonical_preimage(),
    )
    .map_err(CompilerProofInputValidationErrorV3::CorrespondenceDecode)?;
    let decoded_formal_memory =
        InertCanonicalFormalMemoryAdmissionEvidenceV3::decode(formal_memory.canonical_preimage())
            .map_err(CompilerProofInputValidationErrorV3::FormalMemoryDecode)?;

    let semantic_identity = decoded_semantic_mir.semantic_sha256();
    for (actual, field) in [
        (
            decoded_middle_end.source_semantic_identity(),
            "middle-end source semantic MIR",
        ),
        (
            decoded_correspondence.semantic_sha256(),
            "MIR-to-KIR correspondence semantic MIR",
        ),
    ] {
        if actual != semantic_identity.as_bytes() {
            return Err(CompilerProofInputValidationErrorV3::NestedIdentityMismatch { field });
        }
    }
    for (actual, field) in [
        (
            decoded_correspondence.canonical_kir_v5_identity(),
            "MIR-to-KIR correspondence Kernel IR",
        ),
        (
            decoded_formal_memory.canonical_kir_v5_identity(),
            "formal-memory admission Kernel IR",
        ),
    ] {
        if actual != decoded_kernel_ir.identity().digest() {
            return Err(CompilerProofInputValidationErrorV3::NestedIdentityMismatch { field });
        }
    }
    validate_structural_correspondence(
        &decoded_semantic_mir,
        &kernel_module,
        &decoded_correspondence,
    )?;
    Ok(DecodedCompilerProofStagesV3 {
        semantic_mir: decoded_semantic_mir,
        middle_end: decoded_middle_end,
        kernel_ir: decoded_kernel_ir,
        correspondence: decoded_correspondence,
        formal_memory: decoded_formal_memory,
    })
}

fn content_identity(
    sha256: &[u8; 32],
    byte_len: u64,
) -> Result<InertLineageContentIdentityV3, CompilerProofInputValidationErrorV3> {
    InertLineageContentIdentityV3::new(*sha256, byte_len)
        .map_err(CompilerProofInputValidationErrorV3::ProofBindingDecode)
}

fn validate_structural_correspondence(
    semantic_mir: &AdmittedInertSemanticMirV1,
    kernel_ir: &Module,
    correspondence: &InertCanonicalMirToKirCorrespondenceEvidenceV3,
) -> Result<(), CompilerProofInputValidationErrorV3> {
    let mut defined_kernel_functions = kernel_ir
        .functions
        .iter()
        .filter_map(|function| function.body.as_ref());
    let records = correspondence.blocks();
    let mut record_offset = 0_usize;
    let mut covered_functions = 0_usize;
    while let Some(first) = records.get(record_offset) {
        let semantic_function_index = usize::try_from(first.semantic_function()).map_err(|_| {
            CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                detail: "semantic function locator does not fit this host",
            }
        })?;
        let group_start = record_offset;
        while records
            .get(record_offset)
            .is_some_and(|record| record.semantic_function() == first.semantic_function())
        {
            record_offset += 1;
        }
        let function_records = &records[group_start..record_offset];
        let semantic_function = semantic_mir
            .functions()
            .get(semantic_function_index)
            .ok_or(
                CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                    detail: "correspondence names an absent semantic function",
                },
            )?;
        let kernel_body = defined_kernel_functions.next().ok_or(
            CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                detail: "defined Kernel IR function coverage differs from correspondence records",
            },
        )?;
        if semantic_function.blocks().len() != function_records.len() {
            return Err(
                CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                    detail: "semantic block coverage differs from correspondence records",
                },
            );
        }
        if kernel_body.blocks.len() != function_records.len() {
            return Err(
                CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                    detail: "Kernel IR block coverage differs from correspondence records",
                },
            );
        }
        for record in function_records {
            let semantic_block_index = usize::try_from(record.semantic_block()).map_err(|_| {
                CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                    detail: "semantic block locator does not fit this host",
                }
            })?;
            let semantic_block = semantic_function.blocks().get(semantic_block_index).ok_or(
                CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                    detail: "correspondence names an absent semantic block",
                },
            )?;
            if usize::try_from(record.source_statement_count())
                != Ok(semantic_block.statements().len())
            {
                return Err(
                    CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                        detail: "correspondence source statement count differs from semantic MIR",
                    },
                );
            }
            if !kernel_body
                .blocks
                .iter()
                .any(|block| block.id.0 == record.kernel_ir_block())
            {
                return Err(
                    CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                        detail: "correspondence names an absent Kernel IR block",
                    },
                );
            }
        }
        covered_functions += 1;
    }
    if usize::try_from(correspondence.function_count()) != Ok(covered_functions) {
        return Err(
            CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                detail: "declared function coverage differs from correspondence records",
            },
        );
    }
    if defined_kernel_functions.next().is_some() {
        return Err(
            CompilerProofInputValidationErrorV3::StructuralCorrespondence {
                detail: "defined Kernel IR function coverage differs from correspondence records",
            },
        );
    }
    Ok(())
}

/// Failure to decode or exactly match compiler proof inputs.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerProofInputValidationErrorV3 {
    /// The proof-binding preimage is not the exact canonical frozen format.
    ProofBindingDecode(InertProofBindingAssociationErrorV3),
    /// A named association input differs from its independently retained receipt.
    ProofBindingIdentityMismatch {
        /// The mismatched semantic stage.
        field: &'static str,
    },
    /// Exact production semantic MIR could not be decoded and admitted.
    SemanticMirDecode(SemanticMirDecodeErrorV1),
    /// Exact V5 middle-end evidence could not be decoded.
    MiddleEndDecode(ProductionMiddleEndEvidenceCodecErrorV5),
    /// Exact canonical Kernel IR V5 could not be decoded or semantically verified.
    KernelIr(VerifiedCanonicalKernelIrErrorV5),
    /// Exact MIR-to-KIR correspondence evidence could not be decoded.
    CorrespondenceDecode(ProductionLineageEvidenceErrorV3),
    /// Exact formal-memory admission evidence could not be decoded.
    FormalMemoryDecode(ProductionLineageEvidenceErrorV3),
    /// A nested semantic or Kernel IR identity differs from its decoded owner.
    NestedIdentityMismatch {
        /// The mismatched nested identity.
        field: &'static str,
    },
    /// Structurally decoded correspondence does not match the decoded MIR and KIR.
    StructuralCorrespondence {
        /// Stable fail-closed mismatch description.
        detail: &'static str,
    },
}

impl fmt::Display for CompilerProofInputValidationErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProofBindingDecode(error) => {
                write!(formatter, "cannot decode compiler proof binding: {error}")
            }
            Self::ProofBindingIdentityMismatch { field } => {
                write!(
                    formatter,
                    "compiler proof binding has substituted {field} identity"
                )
            }
            Self::SemanticMirDecode(error) => {
                write!(formatter, "cannot decode compiler semantic MIR: {error}")
            }
            Self::MiddleEndDecode(error) => {
                write!(
                    formatter,
                    "cannot decode compiler middle-end evidence: {error}"
                )
            }
            Self::KernelIr(error) => {
                write!(formatter, "cannot validate compiler Kernel IR: {error}")
            }
            Self::CorrespondenceDecode(error) => {
                write!(
                    formatter,
                    "cannot decode compiler MIR-to-KIR evidence: {error}"
                )
            }
            Self::FormalMemoryDecode(error) => {
                write!(
                    formatter,
                    "cannot decode compiler formal-memory evidence: {error}"
                )
            }
            Self::NestedIdentityMismatch { field } => {
                write!(
                    formatter,
                    "compiler proof inputs have substituted {field} identity"
                )
            }
            Self::StructuralCorrespondence { detail } => {
                write!(
                    formatter,
                    "compiler proof inputs have invalid structural correspondence: {detail}"
                )
            }
        }
    }
}

impl Error for CompilerProofInputValidationErrorV3 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProofBindingDecode(error) => Some(error),
            Self::SemanticMirDecode(error) => Some(error),
            Self::MiddleEndDecode(error) => Some(error),
            Self::KernelIr(error) => Some(error),
            Self::CorrespondenceDecode(error) | Self::FormalMemoryDecode(error) => Some(error),
            Self::ProofBindingIdentityMismatch { .. }
            | Self::NestedIdentityMismatch { .. }
            | Self::StructuralCorrespondence { .. } => None,
        }
    }
}

/// Failure to decode or exactly match the current signed compiler proof inputs.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerProofInputValidationErrorV4 {
    /// The proof-binding preimage is not the exact canonical V4 format.
    ProofBindingDecode(InertProofBindingAssociationErrorV4),
    /// A named V4 association input differs from its independently retained receipt.
    ProofBindingIdentityMismatch {
        /// The mismatched semantic stage.
        field: &'static str,
    },
    /// One of the five shared compiler stages failed strict decoding or cross-checking.
    Stage(CompilerProofInputValidationErrorV3),
    /// The nested aggregate Verus execution failed canonical decode or signed receipt import.
    VerusEvidence(ProductionMirPlironVerusExecutionEvidenceErrorV1),
    /// The signed aggregate receipt names a different live PLIRON middle-end record.
    VerusMiddleEndMismatch,
}

impl fmt::Display for CompilerProofInputValidationErrorV4 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProofBindingDecode(error) => {
                write!(
                    formatter,
                    "cannot decode current compiler proof binding: {error}"
                )
            }
            Self::ProofBindingIdentityMismatch { field } => write!(
                formatter,
                "current compiler proof binding has substituted {field} identity"
            ),
            Self::Stage(error) => write!(formatter, "current compiler proof stage failed: {error}"),
            Self::VerusEvidence(error) => write!(
                formatter,
                "cannot validate current aggregate Verus execution: {error}"
            ),
            Self::VerusMiddleEndMismatch => formatter.write_str(
                "current aggregate Verus execution names a different middle-end PLIRON record",
            ),
        }
    }
}

impl Error for CompilerProofInputValidationErrorV4 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProofBindingDecode(error) => Some(error),
            Self::Stage(error) => Some(error),
            Self::VerusEvidence(error) => Some(error),
            Self::ProofBindingIdentityMismatch { .. } | Self::VerusMiddleEndMismatch => None,
        }
    }
}
