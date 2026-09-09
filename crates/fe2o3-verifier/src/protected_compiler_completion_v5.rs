//! Protected re-admission of the exact #213 owner and #214 machine receipt.

use std::{error::Error, fmt};

use fe2o3_compiler_ffi::InertProductionCapabilityHandoffV5;
use fe2o3_compiler_lineage::{
    InertCapabilityRefinementReceiptKindV1, InertCapabilityRefinementReceiptV1,
    InertCompilerProofOwnerErrorV5, InertCompilerProofOwnerInputsV5, InertCompilerProofOwnerV5,
    InertLineageContentIdentityV3, InertMultiRootProofLineageV3,
    InertMultiRootStaticCapabilityEvidenceAssociationV1, InertProofBindingAssociationErrorV3,
    InertStaticCapabilityEvidenceAssociationErrorV1,
    InertStaticCapabilityEvidenceAssociationInputsV1, InertStaticCapabilityEvidenceAssociationV1,
    MultiRootProofLineageErrorV3, TargetMachineRefinementReceiptErrorV1,
    TargetMachineRefinementReceiptV1,
};
use fe2o3_proof_contracts::{
    CapabilityAnalysisReportIdentityV1, CapabilityCheckerIdentityV1, CapabilityOutcomeV1,
    CapabilityPropertyIdV1, CapabilityRefinementKindV1, CapabilityRefinementReceiptIdentityV1,
    CapabilityResultSpecV1, DigestV1, InertCapabilityResultSetV1,
};

use crate::compiler_capability_evidence_v1::{
    REQUIRED_CHECKED_PROPERTIES_V2, derive_checked_property_evidence_v2,
    validate_compiler_capability_evidence_v2,
};
use crate::{
    CompilerCapabilityEvidenceValidationErrorV1, CompilerProofInputValidationErrorV4,
    CompilerProofInputValidationErrorV5, ProtectedCompilerMultiRootProofInputsV5,
    validate_compiler_multi_root_proof_inputs_v5, validate_compiler_proof_inputs_v4,
};

const REQUIRED_REFINEMENT_PROPERTIES_V2: [CapabilityPropertyIdV1; 2] = [
    CapabilityPropertyIdV1::SOURCE_MIR_TO_KIR_REFINEMENT,
    CapabilityPropertyIdV1::MACHINE_REFINEMENT,
];

/// Constructs and validates the sole complete #213 owner from exact compiler and finalizer custody.
///
/// Checked W4 results remain inert inputs. Authority-bearing proof ownership is created only after
/// this composition has reacquired source ownership, target closure, both refinement receipts,
/// compiler policy, complete root rosters, and exact final-graph evidence.
pub fn compose_protected_compiler_completion_inputs_v5(
    handoff: &InertProductionCapabilityHandoffV5,
    machine_refinement_bytes: &[u8],
) -> Result<ProtectedCompilerMultiRootProofInputsV5, ProtectedCompilerCompletionInputErrorV5> {
    if handoff.subjects().len() != handoff.obligation_roster().len()
        || handoff.subjects().is_empty()
    {
        return Err(ProtectedCompilerCompletionInputErrorV5::RosterMismatch);
    }
    for obligations in handoff.obligation_roster() {
        validate_required_properties_v2(obligations)?;
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
    source_refinement
        .validate_source_subject_roster_v1(handoff.proof_lineage(), handoff.subjects())
        .map_err(ProtectedCompilerCompletionInputErrorV5::Lineage)?;
    let _typed_machine_refinement =
        TargetMachineRefinementReceiptV1::decode(machine_refinement_bytes)
            .map_err(ProtectedCompilerCompletionInputErrorV5::MachineRefinement)?;
    let machine_refinement = InertCapabilityRefinementReceiptV1::from_canonical_preimage(
        InertCapabilityRefinementReceiptKindV1::Machine,
        machine_refinement_bytes.to_vec(),
    )
    .map_err(ProtectedCompilerCompletionInputErrorV5::Lineage)?;
    let source_identity = source_refinement.identity();
    let machine_identity = machine_refinement.identity();
    let association_inputs = association_inputs_v2(handoff, machine_identity)?;
    let report = handoff.final_graph_report();
    let closure = handoff.target_closure();

    let mut associations = Vec::new();
    associations
        .try_reserve_exact(handoff.subjects().len())
        .map_err(|_| ProtectedCompilerCompletionInputErrorV5::AllocationFailed)?;
    for obligations in handoff.obligation_roster() {
        let subject = obligations.subject();
        let results = InertCapabilityResultSetV1::from_specs_v2(
            subject,
            obligations.identity(),
            obligations
                .obligations()
                .iter()
                .map(|obligation| {
                    let property = obligation.property();
                    let outcome = match property {
                        CapabilityPropertyIdV1::SOURCE_MIR_TO_KIR_REFINEMENT => {
                            refinement_outcome_v2(
                                CapabilityRefinementKindV1::SourceMirToKir,
                                source_identity,
                            )
                        }
                        CapabilityPropertyIdV1::MACHINE_REFINEMENT => refinement_outcome_v2(
                            CapabilityRefinementKindV1::Machine,
                            machine_identity,
                        ),
                        _ => CapabilityOutcomeV1::Checked {
                            evidence: derive_checked_property_evidence_v2(
                                property, report, closure,
                            )?,
                            checker: CapabilityCheckerIdentityV1::from_untrusted_digest(
                                DigestV1::from_untrusted_bytes(report.checker_identity()),
                            ),
                            report: CapabilityAnalysisReportIdentityV1::from_untrusted_digest(
                                DigestV1::from_untrusted_bytes(report.report_identity()),
                            ),
                            executable_kir: subject.executable_kir(),
                            executable_kir_epoch: subject.executable_kir_epoch(),
                            analysis_epoch: report.analysis_epoch(),
                        },
                    };
                    Ok(CapabilityResultSpecV1::new(obligation.identity(), outcome))
                })
                .collect::<Result<Vec<_>, CompilerCapabilityEvidenceValidationErrorV1>>()
                .map_err(ProtectedCompilerCompletionInputErrorV5::Capability)?,
        )
        .map_err(|error| {
            ProtectedCompilerCompletionInputErrorV5::Capability(
                CompilerCapabilityEvidenceValidationErrorV1::CapabilityCodec(error),
            )
        })?;
        associations.push(
            InertStaticCapabilityEvidenceAssociationV1::new(
                association_inputs,
                obligations,
                &results,
            )
            .map_err(ProtectedCompilerCompletionInputErrorV5::Lineage)?,
        );
    }
    let associations = InertMultiRootStaticCapabilityEvidenceAssociationV1::new(associations)
        .map_err(ProtectedCompilerCompletionInputErrorV5::Lineage)?;
    let selected_ordinal = 0_u32;
    let selected_association = associations
        .entries()
        .first()
        .ok_or(ProtectedCompilerCompletionInputErrorV5::RosterMismatch)?;
    let selected_subject = handoff.subjects()[0];

    let mut selected_capability = None;
    for (ordinal, ((subject, obligations), association)) in handoff
        .subjects()
        .iter()
        .copied()
        .zip(handoff.obligation_roster())
        .zip(associations.entries())
        .enumerate()
    {
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
        let capability = validate_compiler_capability_evidence_v2(
            association.canonical_bytes(),
            capsule,
            handoff.executable_kir(),
            subject,
            obligations.identity(),
            Some(source_refinement),
            Some(machine_refinement),
            report,
            closure,
        )
        .map_err(ProtectedCompilerCompletionInputErrorV5::Capability)?;
        if ordinal == selected_ordinal as usize {
            selected_capability = Some(capability);
        }
    }

    let owner_inputs = InertCompilerProofOwnerInputsV5::new(
        lineage(
            *source_owner.receipt_identity().sha256(),
            source_owner.receipt_identity().byte_len(),
        )?,
        source_owner.association().inputs().semantic_mir(),
        handoff.inputs().semantic_mir_identity(),
        handoff.executable_kir().identity(),
        handoff.executable_kir().identity().byte_len(),
        selected_subject,
        handoff.inputs().compiler_policy(),
        source_identity,
        machine_identity,
        associations.identity(),
    )
    .map_err(ProtectedCompilerCompletionInputErrorV5::ProofOwner)?;
    let owner = InertCompilerProofOwnerV5::new_multi_root_for_association_at_ordinal(
        owner_inputs,
        handoff.proof_lineage(),
        handoff.subjects().to_vec(),
        selected_ordinal,
        selected_association.identity(),
    )
    .map_err(ProtectedCompilerCompletionInputErrorV5::ProofOwner)?;
    let capability =
        selected_capability.ok_or(ProtectedCompilerCompletionInputErrorV5::RosterMismatch)?;
    let proof_lineage =
        InertMultiRootProofLineageV3::decode(handoff.proof_lineage().canonical_bytes())
            .map_err(ProtectedCompilerCompletionInputErrorV5::ProofLineage)?;
    validate_compiler_multi_root_proof_inputs_v5(
        owner.canonical_bytes(),
        &source_owner,
        handoff.executable_kir(),
        capability,
        handoff.inputs().compiler_policy(),
        proof_lineage,
        associations,
        selected_ordinal,
    )
    .map(|validated| validated.into_protected_compiler_completion_v5())
    .map_err(ProtectedCompilerCompletionInputErrorV5::NativeOwner)
}

fn validate_required_properties_v2(
    obligations: &fe2o3_proof_contracts::InertCapabilityObligationSetV1,
) -> Result<(), ProtectedCompilerCompletionInputErrorV5> {
    for property in REQUIRED_CHECKED_PROPERTIES_V2
        .into_iter()
        .chain(REQUIRED_REFINEMENT_PROPERTIES_V2)
    {
        if !obligations
            .obligations()
            .iter()
            .any(|obligation| obligation.property() == property)
        {
            return Err(ProtectedCompilerCompletionInputErrorV5::MissingRequiredProperty(property));
        }
    }
    if obligations.obligations().len()
        != REQUIRED_CHECKED_PROPERTIES_V2.len() + REQUIRED_REFINEMENT_PROPERTIES_V2.len()
    {
        return Err(ProtectedCompilerCompletionInputErrorV5::UnexpectedProperty);
    }
    Ok(())
}

fn refinement_outcome_v2(
    kind: CapabilityRefinementKindV1,
    identity: fe2o3_compiler_lineage::InertCapabilityRefinementReceiptIdentityV1,
) -> CapabilityOutcomeV1 {
    CapabilityOutcomeV1::RefinementReceipt {
        kind,
        receipt: CapabilityRefinementReceiptIdentityV1::from_untrusted_parts(
            DigestV1::from_untrusted_bytes(identity.sha256()),
            identity.byte_len(),
        ),
    }
}

fn association_inputs_v2(
    handoff: &InertProductionCapabilityHandoffV5,
    machine_identity: fe2o3_compiler_lineage::InertCapabilityRefinementReceiptIdentityV1,
) -> Result<InertStaticCapabilityEvidenceAssociationInputsV1, ProtectedCompilerCompletionInputErrorV5>
{
    let capsule = handoff.legacy_handoff().capsule();
    let receipts = capsule.receipts();
    Ok(InertStaticCapabilityEvidenceAssociationInputsV1::new(
        lineage(*capsule.identity().sha256(), capsule.identity().byte_len())?,
        lineage(
            handoff.executable_kir().identity().sha256(),
            handoff.executable_kir().identity().byte_len(),
        )?,
        lineage(
            *receipts.proof_binding().identity().sha256(),
            receipts.proof_binding().identity().byte_len(),
        )?,
        lineage(
            *receipts.target_binding().identity().sha256(),
            receipts.target_binding().identity().byte_len(),
        )?,
        lineage(
            *receipts.amdgpu_lowering().identity().sha256(),
            receipts.amdgpu_lowering().identity().byte_len(),
        )?,
        lineage(
            *receipts.semantic_to_llvm().identity().sha256(),
            receipts.semantic_to_llvm().identity().byte_len(),
        )?,
        lineage(
            *receipts
                .final_compiler_module_commitment()
                .identity()
                .sha256(),
            receipts
                .final_compiler_module_commitment()
                .identity()
                .byte_len(),
        )?,
        Some(handoff.source_refinement().identity()),
        Some(machine_identity),
    ))
}

fn lineage(
    sha256: [u8; 32],
    byte_len: u64,
) -> Result<InertLineageContentIdentityV3, ProtectedCompilerCompletionInputErrorV5> {
    InertLineageContentIdentityV3::new(sha256, byte_len)
        .map_err(ProtectedCompilerCompletionInputErrorV5::LineageIdentity)
}

/// Deterministic rejection from protected #213/#214 completion-input re-admission.
#[derive(Debug)]
#[non_exhaustive]
pub enum ProtectedCompilerCompletionInputErrorV5 {
    ProofOwner(InertCompilerProofOwnerErrorV5),
    Lineage(InertStaticCapabilityEvidenceAssociationErrorV1),
    LineageIdentity(InertProofBindingAssociationErrorV3),
    ProofLineage(MultiRootProofLineageErrorV3),
    MachineRefinement(TargetMachineRefinementReceiptErrorV1),
    SourceOwner(CompilerProofInputValidationErrorV4),
    Capability(CompilerCapabilityEvidenceValidationErrorV1),
    NativeOwner(CompilerProofInputValidationErrorV5),
    RosterMismatch,
    MissingRequiredProperty(CapabilityPropertyIdV1),
    UnexpectedProperty,
    AllocationFailed,
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
            Self::LineageIdentity(source) => Some(source),
            Self::ProofLineage(source) => Some(source),
            Self::MachineRefinement(source) => Some(source),
            Self::SourceOwner(source) => Some(source),
            Self::Capability(source) => Some(source),
            Self::NativeOwner(source) => Some(source),
            Self::RosterMismatch
            | Self::MissingRequiredProperty(_)
            | Self::UnexpectedProperty
            | Self::AllocationFailed => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_proof_contracts::{
        CapabilityObligationSpecV1, CapabilitySubjectV1, ExecutableKirIdentityV1,
        InertCapabilityObligationSetV1, KernelIdentityV1, KernelRootIdentityV1,
        LaunchContractIdentityV1, StatementIdentityV1, TargetModelIdentityV1,
    };

    fn digest(seed: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([seed; 32])
    }

    fn obligations(
        properties: impl IntoIterator<Item = CapabilityPropertyIdV1>,
    ) -> InertCapabilityObligationSetV1 {
        let subject = CapabilitySubjectV1::new(
            KernelIdentityV1::from_untrusted_digest(digest(1)),
            KernelRootIdentityV1::from_untrusted_digest(digest(2)),
            ExecutableKirIdentityV1::from_untrusted_digest(digest(3)),
            4,
            TargetModelIdentityV1::from_untrusted_digest(digest(5)),
            LaunchContractIdentityV1::from_untrusted_digest(digest(6)),
        )
        .unwrap();
        InertCapabilityObligationSetV1::from_specs(
            subject,
            properties
                .into_iter()
                .enumerate()
                .map(|(index, property)| {
                    CapabilityObligationSpecV1::new(
                        property,
                        StatementIdentityV1::from_untrusted_digest(digest(20 + index as u8)),
                    )
                })
                .collect(),
        )
        .unwrap()
    }

    fn required_properties() -> Vec<CapabilityPropertyIdV1> {
        REQUIRED_CHECKED_PROPERTIES_V2
            .into_iter()
            .chain(REQUIRED_REFINEMENT_PROPERTIES_V2)
            .collect()
    }

    #[test]
    fn exact_v2_property_roster_rejects_omission_and_substitution() {
        assert!(validate_required_properties_v2(&obligations(required_properties())).is_ok());

        let mut omitted = required_properties();
        omitted.retain(|property| *property != CapabilityPropertyIdV1::BOUNDS);
        assert!(matches!(
            validate_required_properties_v2(&obligations(omitted)),
            Err(
                ProtectedCompilerCompletionInputErrorV5::MissingRequiredProperty(
                    CapabilityPropertyIdV1::BOUNDS
                )
            )
        ));

        let mut substituted = required_properties();
        substituted[0] = CapabilityPropertyIdV1::DYNAMIC_LAUNCH_PRECONDITIONS;
        assert!(matches!(
            validate_required_properties_v2(&obligations(substituted)),
            Err(ProtectedCompilerCompletionInputErrorV5::MissingRequiredProperty(_))
        ));
    }
}
