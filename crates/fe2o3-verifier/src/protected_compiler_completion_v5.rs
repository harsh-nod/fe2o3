//! Protected re-admission of the exact #213 owner and #214 machine receipt.

use std::{error::Error, fmt};

use fe2o3_compiler_ffi::InertProductionCapabilityHandoffV5;
use fe2o3_compiler_lineage::{
    InertCapabilityRefinementReceiptKindV1, InertCapabilityRefinementReceiptV1,
    InertCompilerProofOwnerErrorV5, InertCompilerProofOwnerV5, InertMultiRootProofLineageV3,
    InertMultiRootStaticCapabilityEvidenceAssociationV1,
    InertStaticCapabilityEvidenceAssociationErrorV1, MultiRootProofLineageErrorV3,
};

use crate::{
    CompilerCapabilityEvidenceValidationErrorV1, CompilerProofInputValidationErrorV4,
    CompilerProofInputValidationErrorV5, ValidatedCompilerMultiRootProofInputsV5,
    validate_compiler_capability_evidence_v1, validate_compiler_multi_root_proof_inputs_v5,
    validate_compiler_proof_inputs_v4,
};

/// Reconstructs one complete move-only #213 owner from immutable canonical request evidence.
///
/// `machine_refinement_bytes` must be the exact canonical evidence emitted by the successful
/// #214 checker. This function proves that every subject association and the selected V5 owner
/// bind that receipt; it does not treat the transport peer as its issuer.
pub fn validate_protected_compiler_completion_inputs_v5(
    handoff: &InertProductionCapabilityHandoffV5,
    proof_owner_bytes: &[u8],
    capability_association_bytes: &[u8],
    machine_refinement_bytes: &[u8],
) -> Result<ValidatedCompilerMultiRootProofInputsV5, ProtectedCompilerCompletionInputErrorV5> {
    let owner = InertCompilerProofOwnerV5::decode(proof_owner_bytes)
        .map_err(ProtectedCompilerCompletionInputErrorV5::ProofOwner)?;
    let selected_ordinal = usize::try_from(owner.selected_subject_ordinal())
        .map_err(|_| ProtectedCompilerCompletionInputErrorV5::SelectedSubjectOutOfRange)?;
    let associations =
        InertMultiRootStaticCapabilityEvidenceAssociationV1::decode(capability_association_bytes)
            .map_err(ProtectedCompilerCompletionInputErrorV5::Lineage)?;
    let selected_association = associations
        .entries()
        .get(selected_ordinal)
        .ok_or(ProtectedCompilerCompletionInputErrorV5::SelectedSubjectOutOfRange)?;
    let subject = handoff
        .subjects()
        .get(selected_ordinal)
        .copied()
        .ok_or(ProtectedCompilerCompletionInputErrorV5::SelectedSubjectOutOfRange)?;
    let obligations = handoff
        .obligation_roster()
        .get(selected_ordinal)
        .ok_or(ProtectedCompilerCompletionInputErrorV5::SelectedSubjectOutOfRange)?;

    if associations.entries().len() != handoff.subjects().len()
        || handoff.subjects().len() != handoff.obligation_roster().len()
    {
        return Err(ProtectedCompilerCompletionInputErrorV5::RosterMismatch);
    }

    let capsule = handoff.legacy_handoff().capsule();
    let receipts = capsule.receipts();
    let source_owner = validate_compiler_proof_inputs_v4(
        receipts.proof_binding(),
        receipts.semantic_mir(),
        receipts.middle_end(),
        receipts.kernel_ir(),
        receipts.mir_to_kir_correspondence(),
        receipts.formal_memory(),
    )
    .map_err(ProtectedCompilerCompletionInputErrorV5::SourceOwner)?;
    let source_refinement = InertCapabilityRefinementReceiptV1::from_canonical_preimage(
        InertCapabilityRefinementReceiptKindV1::SourceMirToKir,
        handoff.source_refinement().canonical_preimage().to_vec(),
    )
    .map_err(ProtectedCompilerCompletionInputErrorV5::Lineage)?;
    let machine_refinement = InertCapabilityRefinementReceiptV1::from_canonical_preimage(
        InertCapabilityRefinementReceiptKindV1::Machine,
        machine_refinement_bytes.to_vec(),
    )
    .map_err(ProtectedCompilerCompletionInputErrorV5::Lineage)?;
    let capability = validate_compiler_capability_evidence_v1(
        selected_association.canonical_bytes(),
        capsule,
        handoff.executable_kir(),
        subject,
        obligations.identity(),
        Some(source_refinement),
        Some(machine_refinement),
    )
    .map_err(ProtectedCompilerCompletionInputErrorV5::Capability)?;
    let proof_lineage =
        InertMultiRootProofLineageV3::decode(handoff.proof_lineage().canonical_bytes())
            .map_err(ProtectedCompilerCompletionInputErrorV5::ProofLineage)?;

    validate_compiler_multi_root_proof_inputs_v5(
        proof_owner_bytes,
        &source_owner,
        handoff.executable_kir(),
        capability,
        handoff.inputs().compiler_policy(),
        proof_lineage,
        associations,
        u32::try_from(selected_ordinal)
            .map_err(|_| ProtectedCompilerCompletionInputErrorV5::SelectedSubjectOutOfRange)?,
    )
    .map_err(ProtectedCompilerCompletionInputErrorV5::NativeOwner)
}

/// Deterministic rejection from protected #213/#214 completion-input re-admission.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedCompilerCompletionInputErrorV5 {
    ProofOwner(InertCompilerProofOwnerErrorV5),
    Lineage(InertStaticCapabilityEvidenceAssociationErrorV1),
    ProofLineage(MultiRootProofLineageErrorV3),
    SourceOwner(CompilerProofInputValidationErrorV4),
    Capability(CompilerCapabilityEvidenceValidationErrorV1),
    NativeOwner(CompilerProofInputValidationErrorV5),
    SelectedSubjectOutOfRange,
    RosterMismatch,
}

impl fmt::Display for ProtectedCompilerCompletionInputErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "protected compiler completion input rejected: {self:?}"
        )
    }
}

impl Error for ProtectedCompilerCompletionInputErrorV5 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::ProofOwner(source) => Some(source),
            Self::Lineage(source) => Some(source),
            Self::ProofLineage(source) => Some(source),
            Self::SourceOwner(source) => Some(source),
            Self::Capability(source) => Some(source),
            Self::NativeOwner(source) => Some(source),
            Self::SelectedSubjectOutOfRange | Self::RosterMismatch => None,
        }
    }
}
