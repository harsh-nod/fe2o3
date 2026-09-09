//! Fail-closed admission of issue #272 static capability evidence.
//!
//! This module is the crate-owned check boundary between caller-constructible inert records and the
//! sealed Worker V3 join. Its output remains authority-free because protected verifier policy,
//! authenticated refinement-receipt adapters, dynamic launch evidence, and external monotonic
//! currentness are separate dependencies.

use std::{error::Error, fmt};

use fe2o3_compiler_ffi::{
    InertProductionCapabilityResultIdentityV5, InertProductionCapabilityResultV5,
    InertProductionFinalGraphReportV5, InertProductionTargetCapabilityClosureV5,
    PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5,
};
use fe2o3_compiler_lineage::{
    InertCanonicalKernelIrV13ReceiptV5, InertCapabilityRefinementReceiptIdentityV1,
    InertCapabilityRefinementReceiptKindV1, InertCapabilityRefinementReceiptV1,
    InertLineageContentIdentityV3, InertProductionSemanticCapsuleV3,
    InertProofBindingAssociationErrorV4, InertProofBindingAssociationV4,
    InertStaticCapabilityEvidenceAssociationErrorV1, InertStaticCapabilityEvidenceAssociationV1,
};
use fe2o3_kernel_ir::{
    Module, OperationKind, Type, VerifiedCanonicalKernelIrErrorV13, VerifiedCanonicalKernelIrV13,
};
use fe2o3_proof_contracts::{
    CAPABILITY_RESULT_SET_VERSION_V2, CapabilityCodecErrorV1, CapabilityCompositionErrorV1,
    CapabilityOutcomeKindV1, CapabilityOutcomeV1, CapabilityPropertyIdV1,
    CapabilityRefinementKindV1, CapabilitySubjectFieldV1, CapabilitySubjectV1, DigestV1,
    EvidenceIdentityV1, InertCapabilityObligationSetIdentityV1, InertCapabilityObligationSetV1,
    InertCapabilityResultSetV1, validate_capability_composition_v1,
};
use sha2::{Digest as _, Sha256};

use crate::{ValidatedCompilerProofInputsV4, ValidatedCompilerProofInputsV5};

/// Move-only, independently decoded static capability evidence for one exact compiler subject.
///
/// Possession establishes canonical composition, exact frozen-capsule association, exact verified
/// KIR V13 identity, and the admitted result class for every obligation. It does not authenticate
/// the compiler or typed refinement-receipt issuers, establish publication/currentness, discharge
/// dynamic launch preconditions, or grant load/launch authority.
///
/// ```compile_fail
/// use fe2o3_verifier::ValidatedCompilerCapabilityEvidenceV1;
/// fn requires_clone<T: Clone>() {}
/// requires_clone::<ValidatedCompilerCapabilityEvidenceV1>();
/// ```
#[derive(Debug)]
#[must_use = "dropping validated capability evidence abandons exact authority-free custody"]
pub struct ValidatedCompilerCapabilityEvidenceV1 {
    association: InertStaticCapabilityEvidenceAssociationV1,
    obligations: InertCapabilityObligationSetV1,
    results: InertCapabilityResultSetV1,
    kernel_ir: VerifiedCanonicalKernelIrV13,
    source_refinement: Option<InertCapabilityRefinementReceiptV1>,
    machine_refinement: Option<InertCapabilityRefinementReceiptV1>,
}

/// Move-only capability evidence joined to native exact-canonical-KIR-V13 proof ownership.
///
/// This owner remains authority-free. It proves only that the capability sidecar names the exact
/// proof-binding receipt and semantic MIR retained by [`ValidatedCompilerProofInputsV4`] while its
/// V5 owner independently owns the exact executable KIR V13 graph and epoch. Protected compiler
/// provenance, currentness, publication, and runtime authority remain separate joins.
#[derive(Debug)]
#[must_use = "dropping the source join abandons exact capability/refinement custody"]
pub struct ValidatedCompilerCapabilitySourceOwnerV1 {
    proof_owner: ValidatedCompilerProofInputsV5,
    production_result_identity: InertProductionCapabilityResultIdentityV5,
}

impl ValidatedCompilerCapabilitySourceOwnerV1 {
    /// Returns the exact capability evidence retained by this source-owner join.
    pub const fn capability(&self) -> &ValidatedCompilerCapabilityEvidenceV1 {
        self.proof_owner.capability()
    }

    /// Returns the native exact-canonical-KIR-V13 proof owner.
    pub const fn proof_owner(&self) -> &ValidatedCompilerProofInputsV5 {
        &self.proof_owner
    }

    /// Returns the exact completed V5 result authenticated at this owner boundary.
    pub const fn production_result_identity(&self) -> InertProductionCapabilityResultIdentityV5 {
        self.production_result_identity
    }
}

/// Joins a native V13 proof owner to its exact completed V5 result and retained V4 source lineage.
pub fn validate_compiler_capability_source_owner_v1(
    proof_owner: ValidatedCompilerProofInputsV5,
    production_result: &InertProductionCapabilityResultV5,
    source_owner: &ValidatedCompilerProofInputsV4,
    compiler_policy: [u8; 32],
) -> Result<ValidatedCompilerCapabilitySourceOwnerV1, CompilerCapabilitySourceOwnerErrorV1> {
    let inputs = proof_owner.association().inputs();
    let proof_receipt = source_owner.receipt_identity();
    if inputs.legacy_proof_binding().sha256() != *proof_receipt.sha256()
        || inputs.legacy_proof_binding().byte_len() != proof_receipt.byte_len()
    {
        return Err(CompilerCapabilitySourceOwnerErrorV1::ProofBindingReceiptMismatch);
    }
    if inputs.semantic_mir_receipt() != source_owner.association().inputs().semantic_mir()
        || inputs.semantic_mir_identity()
            != *source_owner.semantic_mir().semantic_sha256().as_bytes()
    {
        return Err(CompilerCapabilitySourceOwnerErrorV1::SemanticMirMismatch);
    }
    if inputs.compiler_policy() != compiler_policy {
        return Err(CompilerCapabilitySourceOwnerErrorV1::CompilerPolicyMismatch);
    }
    let capability = proof_owner.capability();
    let source_refinement = capability.source_refinement().ok_or(
        CompilerCapabilitySourceOwnerErrorV1::ProductionResultMismatch("source refinement"),
    )?;
    let machine_refinement = capability.machine_refinement().ok_or(
        CompilerCapabilitySourceOwnerErrorV1::ProductionResultMismatch("machine refinement"),
    )?;
    let capability_identity = capability.association().identity();
    let selected_ordinal = usize::try_from(proof_owner.association().selected_subject_ordinal())
        .map_err(|_| {
            CompilerCapabilitySourceOwnerErrorV1::ProductionResultMismatch("subject ordinal")
        })?;
    if production_result.proof_owner().canonical_bytes()
        != proof_owner.association().canonical_bytes()
    {
        return Err(CompilerCapabilitySourceOwnerErrorV1::ProductionResultMismatch("proof owner"));
    }
    if inputs.capability_association() != production_result.capability_associations().identity()
        || production_result
            .capability_associations()
            .entries()
            .get(selected_ordinal)
            .is_none_or(|association| {
                association.identity() != capability_identity
                    || association.identity()
                        != proof_owner.association().selected_capability_association()
                    || association.subject() != capability.association().subject()
            })
    {
        return Err(
            CompilerCapabilitySourceOwnerErrorV1::ProductionResultMismatch(
                "capability association roster",
            ),
        );
    }
    if production_result.handoff().inputs().compiler_policy() != compiler_policy
        || production_result
            .handoff()
            .subjects()
            .get(selected_ordinal)
            .copied()
            != Some(capability.association().subject())
        || production_result.handoff().source_refinement().identity()
            != source_refinement.identity()
        || production_result.machine_refinement().identity() != machine_refinement.identity()
    {
        return Err(
            CompilerCapabilitySourceOwnerErrorV1::ProductionResultMismatch(
                "subject or refinement custody",
            ),
        );
    }
    Ok(ValidatedCompilerCapabilitySourceOwnerV1 {
        proof_owner,
        production_result_identity: production_result.identity(),
    })
}

/// Failure to join capability evidence to the existing source-refinement owner.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum CompilerCapabilitySourceOwnerErrorV1 {
    /// The sidecar names a different frozen proof-binding receipt.
    ProofBindingReceiptMismatch,
    /// The native owner names different source/MIR custody.
    SemanticMirMismatch,
    /// The native owner names a different protected compiler policy.
    CompilerPolicyMismatch,
    /// The completed V5 result does not retain this exact proof/capability owner.
    ProductionResultMismatch(&'static str),
}

impl fmt::Display for CompilerCapabilitySourceOwnerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ProofBindingReceiptMismatch => {
                formatter.write_str("capability evidence names a different proof-binding receipt")
            }
            Self::SemanticMirMismatch => {
                formatter.write_str("native V13 owner names different semantic MIR custody")
            }
            Self::CompilerPolicyMismatch => {
                formatter.write_str("native V13 owner names a different compiler policy")
            }
            Self::ProductionResultMismatch(field) => {
                write!(formatter, "completed V5 result substituted {field}")
            }
        }
    }
}

impl Error for CompilerCapabilitySourceOwnerErrorV1 {}

impl ValidatedCompilerCapabilityEvidenceV1 {
    /// Returns the exact side-by-side lineage association.
    pub const fn association(&self) -> &InertStaticCapabilityEvidenceAssociationV1 {
        &self.association
    }

    /// Returns the complete canonical obligation set matching the supplied policy identity.
    pub const fn obligations(&self) -> &InertCapabilityObligationSetV1 {
        &self.obligations
    }

    /// Returns the one-to-one complete result set with its exact admitted outcome classes.
    pub const fn results(&self) -> &InertCapabilityResultSetV1 {
        &self.results
    }

    /// Returns independently verified exact executable KIR V13 custody.
    pub const fn kernel_ir(&self) -> &VerifiedCanonicalKernelIrV13 {
        &self.kernel_ir
    }

    /// Returns the retained typed source/MIR-to-KIR receipt when that property is required.
    pub const fn source_refinement(&self) -> Option<&InertCapabilityRefinementReceiptV1> {
        self.source_refinement.as_ref()
    }

    /// Returns the retained typed final-machine receipt when that property is required.
    pub const fn machine_refinement(&self) -> Option<&InertCapabilityRefinementReceiptV1> {
        self.machine_refinement.as_ref()
    }

    pub(crate) fn into_machine_refinement(self) -> Option<InertCapabilityRefinementReceiptV1> {
        self.machine_refinement
    }
}

/// Strictly admits complete static capability evidence beside one frozen V3 capsule.
///
/// `expected_subject` and `expected_obligation_set` are the exact outputs that the sealed verifier
/// must reacquire from protected compiler policy and the final graph epoch. This function does not
/// authenticate their caller. Consequently, its move-only result is suitable as an owned input to
/// that join, but is not itself an authority token.
pub fn validate_compiler_capability_evidence_v1(
    association_bytes: &[u8],
    capsule: &InertProductionSemanticCapsuleV3,
    executable_kir_receipt: &InertCanonicalKernelIrV13ReceiptV5,
    expected_subject: CapabilitySubjectV1,
    expected_obligation_set: InertCapabilityObligationSetIdentityV1,
    source_refinement: Option<InertCapabilityRefinementReceiptV1>,
    machine_refinement: Option<InertCapabilityRefinementReceiptV1>,
) -> Result<ValidatedCompilerCapabilityEvidenceV1, CompilerCapabilityEvidenceValidationErrorV1> {
    validate_compiler_capability_evidence(
        association_bytes,
        capsule,
        executable_kir_receipt,
        expected_subject,
        expected_obligation_set,
        source_refinement,
        machine_refinement,
        CapabilityEvidenceAdmissionV1::LegacyAllProven,
    )
}

/// Admits only exact V2 checked-analysis and typed refinement-receipt outcomes.
///
/// This path is crate-private so only the sealed protected composer can supply the retained W4
/// report and target-closure custody. Neither clean W4 bytes nor this function alone grant
/// publication, load, or launch authority.
#[allow(clippy::too_many_arguments)]
pub(crate) fn validate_compiler_capability_evidence_v2(
    association_bytes: &[u8],
    capsule: &InertProductionSemanticCapsuleV3,
    executable_kir_receipt: &InertCanonicalKernelIrV13ReceiptV5,
    expected_subject: CapabilitySubjectV1,
    expected_obligation_set: InertCapabilityObligationSetIdentityV1,
    source_refinement: Option<InertCapabilityRefinementReceiptV1>,
    machine_refinement: Option<InertCapabilityRefinementReceiptV1>,
    report: &InertProductionFinalGraphReportV5,
    target_closure: &InertProductionTargetCapabilityClosureV5,
) -> Result<ValidatedCompilerCapabilityEvidenceV1, CompilerCapabilityEvidenceValidationErrorV1> {
    validate_compiler_capability_evidence(
        association_bytes,
        capsule,
        executable_kir_receipt,
        expected_subject,
        expected_obligation_set,
        source_refinement,
        machine_refinement,
        CapabilityEvidenceAdmissionV1::ExactCheckedV2 {
            report,
            target_closure,
        },
    )
}

enum CapabilityEvidenceAdmissionV1<'a> {
    LegacyAllProven,
    ExactCheckedV2 {
        report: &'a InertProductionFinalGraphReportV5,
        target_closure: &'a InertProductionTargetCapabilityClosureV5,
    },
}

#[allow(clippy::too_many_arguments)]
fn validate_compiler_capability_evidence(
    association_bytes: &[u8],
    capsule: &InertProductionSemanticCapsuleV3,
    executable_kir_receipt: &InertCanonicalKernelIrV13ReceiptV5,
    expected_subject: CapabilitySubjectV1,
    expected_obligation_set: InertCapabilityObligationSetIdentityV1,
    source_refinement: Option<InertCapabilityRefinementReceiptV1>,
    machine_refinement: Option<InertCapabilityRefinementReceiptV1>,
    admission: CapabilityEvidenceAdmissionV1<'_>,
) -> Result<ValidatedCompilerCapabilityEvidenceV1, CompilerCapabilityEvidenceValidationErrorV1> {
    let association = InertStaticCapabilityEvidenceAssociationV1::decode(association_bytes)
        .map_err(CompilerCapabilityEvidenceValidationErrorV1::Association)?;
    let obligations =
        InertCapabilityObligationSetV1::decode_canonical(association.obligation_set_bytes())
            .map_err(CompilerCapabilityEvidenceValidationErrorV1::CapabilityCodec)?;
    let results = InertCapabilityResultSetV1::decode_canonical(association.result_set_bytes())
        .map_err(CompilerCapabilityEvidenceValidationErrorV1::CapabilityCodec)?;
    if association.subject() != expected_subject {
        return Err(
            CompilerCapabilityEvidenceValidationErrorV1::ExpectedSubjectMismatch(subject_mismatch(
                expected_subject,
                association.subject(),
            )),
        );
    }
    validate_capability_composition_v1(expected_subject, &obligations, &results)
        .map_err(CompilerCapabilityEvidenceValidationErrorV1::Composition)?;
    if obligations.identity() != expected_obligation_set {
        return Err(CompilerCapabilityEvidenceValidationErrorV1::ObligationSetMismatch);
    }

    validate_capsule_coordinates(&association, capsule, executable_kir_receipt)?;
    validate_proof_binding_coordinates(capsule)?;

    let (kernel_ir, module) = VerifiedCanonicalKernelIrV13::from_canonical_bytes_with_module(
        executable_kir_receipt.canonical_preimage().to_vec(),
    )
    .map_err(CompilerCapabilityEvidenceValidationErrorV1::KernelIr)?;
    if kernel_ir.identity().digest() != expected_subject.executable_kir().digest().as_bytes() {
        return Err(CompilerCapabilityEvidenceValidationErrorV1::ExecutableKirIdentityMismatch);
    }
    validate_context_subject(&module, expected_subject)?;

    let mut requires_source_refinement = false;
    let mut requires_machine_refinement = false;
    for (index, (obligation, result)) in obligations
        .obligations()
        .iter()
        .zip(results.results())
        .enumerate()
    {
        if matches!(&admission, CapabilityEvidenceAdmissionV1::LegacyAllProven) {
            let outcome = result.outcome().kind();
            if outcome != CapabilityOutcomeKindV1::Proven {
                return Err(
                    CompilerCapabilityEvidenceValidationErrorV1::OutcomeNotProven {
                        index,
                        property: obligation.property(),
                        outcome,
                    },
                );
            }
        }
        match obligation.property() {
            CapabilityPropertyIdV1::SOURCE_MIR_TO_KIR_REFINEMENT => {
                requires_source_refinement = true;
            }
            CapabilityPropertyIdV1::MACHINE_REFINEMENT => {
                requires_machine_refinement = true;
            }
            CapabilityPropertyIdV1::DYNAMIC_LAUNCH_PRECONDITIONS => {
                return Err(
                    CompilerCapabilityEvidenceValidationErrorV1::DynamicLaunchClaimInStaticEvidence,
                );
            }
            _ => {}
        }
    }

    let inputs = association.inputs();
    let source_refinement = validate_refinement_receipt(
        InertCapabilityRefinementReceiptKindV1::SourceMirToKir,
        requires_source_refinement,
        inputs.source_refinement(),
        source_refinement,
    )?;
    let machine_refinement = validate_refinement_receipt(
        InertCapabilityRefinementReceiptKindV1::Machine,
        requires_machine_refinement,
        inputs.machine_refinement(),
        machine_refinement,
    )?;
    match admission {
        CapabilityEvidenceAdmissionV1::LegacyAllProven => {
            validate_refinement_evidence_identity(
                &obligations,
                &results,
                CapabilityPropertyIdV1::SOURCE_MIR_TO_KIR_REFINEMENT,
                source_refinement.as_ref(),
            )?;
            validate_refinement_evidence_identity(
                &obligations,
                &results,
                CapabilityPropertyIdV1::MACHINE_REFINEMENT,
                machine_refinement.as_ref(),
            )?;
        }
        CapabilityEvidenceAdmissionV1::ExactCheckedV2 {
            report,
            target_closure,
        } => validate_exact_checked_outcomes_v2(
            expected_subject,
            &obligations,
            &results,
            report,
            target_closure,
            source_refinement.as_ref(),
            machine_refinement.as_ref(),
        )?,
    }

    Ok(ValidatedCompilerCapabilityEvidenceV1 {
        association,
        obligations,
        results,
        kernel_ir,
        source_refinement,
        machine_refinement,
    })
}

fn validate_capsule_coordinates(
    association: &InertStaticCapabilityEvidenceAssociationV1,
    capsule: &InertProductionSemanticCapsuleV3,
    executable_kir_receipt: &InertCanonicalKernelIrV13ReceiptV5,
) -> Result<(), CompilerCapabilityEvidenceValidationErrorV1> {
    let inputs = association.inputs();
    let receipts = capsule.receipts();
    let coordinates = [
        (
            inputs.capsule(),
            lineage_identity(*capsule.identity().sha256(), capsule.identity().byte_len())?,
            "frozen V3 capsule",
        ),
        (
            inputs.kernel_ir(),
            lineage_identity(
                executable_kir_receipt.identity().sha256(),
                executable_kir_receipt.identity().byte_len(),
            )?,
            "side-by-side optimized executable KIR V13",
        ),
        (
            inputs.proof_binding(),
            lineage_identity(
                *receipts.proof_binding().identity().sha256(),
                receipts.proof_binding().identity().byte_len(),
            )?,
            "V4 proof binding",
        ),
        (
            inputs.target_binding(),
            lineage_identity(
                *receipts.target_binding().identity().sha256(),
                receipts.target_binding().identity().byte_len(),
            )?,
            "target binding",
        ),
        (
            inputs.target_lowering(),
            lineage_identity(
                *receipts.amdgpu_lowering().identity().sha256(),
                receipts.amdgpu_lowering().identity().byte_len(),
            )?,
            "target lowering",
        ),
        (
            inputs.semantic_to_llvm(),
            lineage_identity(
                *receipts.semantic_to_llvm().identity().sha256(),
                receipts.semantic_to_llvm().identity().byte_len(),
            )?,
            "semantic to LLVM",
        ),
        (
            inputs.final_compiler_module(),
            lineage_identity(
                *receipts
                    .final_compiler_module_commitment()
                    .identity()
                    .sha256(),
                receipts
                    .final_compiler_module_commitment()
                    .identity()
                    .byte_len(),
            )?,
            "final compiler module",
        ),
    ];
    for (associated, actual, field) in coordinates {
        if associated != actual {
            return Err(
                CompilerCapabilityEvidenceValidationErrorV1::LineageReceiptMismatch { field },
            );
        }
    }
    Ok(())
}

const CHECKED_PROPERTY_EVIDENCE_DOMAIN_V2: &[u8] =
    b"FE2O3/CAPABILITY/CHECKED-PROPERTY-EVIDENCE/V2\0";

pub(crate) const REQUIRED_CHECKED_PROPERTIES_V2: [CapabilityPropertyIdV1; 13] = [
    CapabilityPropertyIdV1::TYPING,
    CapabilityPropertyIdV1::BOUNDS,
    CapabilityPropertyIdV1::INITIALIZATION,
    CapabilityPropertyIdV1::HIERARCHICAL_OWNERSHIP,
    CapabilityPropertyIdV1::DATA_RACE_FREEDOM,
    CapabilityPropertyIdV1::ATOMIC_LEGALITY,
    CapabilityPropertyIdV1::UNIFORMITY,
    CapabilityPropertyIdV1::BARRIER_CONVERGENCE,
    CapabilityPropertyIdV1::WORKGROUP_MEMORY_EPOCHS,
    CapabilityPropertyIdV1::TENSOR_LAYOUT,
    CapabilityPropertyIdV1::EFFECTS,
    CapabilityPropertyIdV1::RESOURCE_LEGALITY,
    CapabilityPropertyIdV1::TARGET_CAPABILITY_CLOSURE,
];

const CHECKED_EVIDENCE_INDEXES_V2: [&[usize]; 13] = [
    &[0],
    &[5],
    &[13],
    &[9],
    &[7, 8, 14],
    &[6],
    &[3, 16],
    &[10, 11],
    &[15],
    &[4],
    &[17, 18],
    &[2, 12],
    &[1],
];

const _: () = {
    let mut counts = [0_u8; PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5];
    let mut property = 0;
    while property < CHECKED_EVIDENCE_INDEXES_V2.len() {
        let indexes = CHECKED_EVIDENCE_INDEXES_V2[property];
        let mut index = 0;
        while index < indexes.len() {
            let evidence = indexes[index];
            assert!(evidence < PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5);
            counts[evidence] += 1;
            index += 1;
        }
        property += 1;
    }
    let mut evidence = 0;
    while evidence < counts.len() {
        assert!(counts[evidence] == 1);
        evidence += 1;
    }
};

#[allow(clippy::too_many_arguments)]
fn validate_exact_checked_outcomes_v2(
    subject: CapabilitySubjectV1,
    obligations: &InertCapabilityObligationSetV1,
    results: &InertCapabilityResultSetV1,
    report: &InertProductionFinalGraphReportV5,
    target_closure: &InertProductionTargetCapabilityClosureV5,
    source_refinement: Option<&InertCapabilityRefinementReceiptV1>,
    machine_refinement: Option<&InertCapabilityRefinementReceiptV1>,
) -> Result<(), CompilerCapabilityEvidenceValidationErrorV1> {
    if results.schema_version() != CAPABILITY_RESULT_SET_VERSION_V2 {
        return Err(
            CompilerCapabilityEvidenceValidationErrorV1::ResultSchemaDowngrade {
                observed: results.schema_version(),
            },
        );
    }
    if report.final_graph() != *subject.executable_kir().digest().as_bytes()
        || report.final_epoch() != subject.executable_kir_epoch()
        || target_closure.neutral_graph() != report.final_graph()
        || target_closure.neutral_graph_bytes() != report.final_graph_bytes()
        || target_closure.neutral_epoch() != report.final_epoch()
        || target_closure.target_model() != subject.target_model()
        || report.checked_evidence().len() != PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5
    {
        return Err(CompilerCapabilityEvidenceValidationErrorV1::CheckedAnalysisContextMismatch);
    }

    for (index, (obligation, result)) in obligations
        .obligations()
        .iter()
        .zip(results.results())
        .enumerate()
    {
        let property = obligation.property();
        match property {
            CapabilityPropertyIdV1::SOURCE_MIR_TO_KIR_REFINEMENT => {
                validate_exact_refinement_outcome_v2(
                    index,
                    property,
                    result.outcome(),
                    CapabilityRefinementKindV1::SourceMirToKir,
                    source_refinement,
                )?;
            }
            CapabilityPropertyIdV1::MACHINE_REFINEMENT => {
                validate_exact_refinement_outcome_v2(
                    index,
                    property,
                    result.outcome(),
                    CapabilityRefinementKindV1::Machine,
                    machine_refinement,
                )?;
            }
            CapabilityPropertyIdV1::DYNAMIC_LAUNCH_PRECONDITIONS => {
                return Err(
                    CompilerCapabilityEvidenceValidationErrorV1::DynamicLaunchClaimInStaticEvidence,
                );
            }
            _ => {
                let CapabilityOutcomeV1::Checked {
                    evidence,
                    checker,
                    report: result_report,
                    executable_kir,
                    executable_kir_epoch,
                    analysis_epoch,
                } = result.outcome()
                else {
                    return Err(
                        CompilerCapabilityEvidenceValidationErrorV1::WrongOutcomeClass {
                            index,
                            property,
                            expected: CapabilityOutcomeKindV1::Checked,
                            actual: result.outcome().kind(),
                        },
                    );
                };
                let expected =
                    derive_checked_property_evidence_v2(property, report, target_closure)?;
                if *evidence != expected
                    || checker.digest().as_bytes() != &report.checker_identity()
                    || result_report.digest().as_bytes() != &report.report_identity()
                    || *executable_kir != subject.executable_kir()
                    || *executable_kir_epoch != subject.executable_kir_epoch()
                    || *analysis_epoch != report.analysis_epoch()
                {
                    return Err(
                        CompilerCapabilityEvidenceValidationErrorV1::CheckedEvidenceMismatch {
                            index,
                            property,
                        },
                    );
                }
            }
        }
    }
    Ok(())
}

fn validate_exact_refinement_outcome_v2(
    index: usize,
    property: CapabilityPropertyIdV1,
    outcome: &CapabilityOutcomeV1,
    expected_kind: CapabilityRefinementKindV1,
    receipt: Option<&InertCapabilityRefinementReceiptV1>,
) -> Result<(), CompilerCapabilityEvidenceValidationErrorV1> {
    let CapabilityOutcomeV1::RefinementReceipt {
        kind,
        receipt: claimed,
    } = outcome
    else {
        return Err(
            CompilerCapabilityEvidenceValidationErrorV1::WrongOutcomeClass {
                index,
                property,
                expected: CapabilityOutcomeKindV1::RefinementReceipt,
                actual: outcome.kind(),
            },
        );
    };
    let receipt = receipt.ok_or(
        CompilerCapabilityEvidenceValidationErrorV1::MissingRefinementReceipt {
            kind: match expected_kind {
                CapabilityRefinementKindV1::SourceMirToKir => {
                    InertCapabilityRefinementReceiptKindV1::SourceMirToKir
                }
                CapabilityRefinementKindV1::Machine => {
                    InertCapabilityRefinementReceiptKindV1::Machine
                }
            },
        },
    )?;
    let identity = receipt.identity();
    if *kind != expected_kind
        || claimed.digest().as_bytes() != &identity.sha256()
        || claimed.byte_len() != identity.byte_len()
    {
        return Err(
            CompilerCapabilityEvidenceValidationErrorV1::RefinementOutcomeMismatch {
                index,
                property,
            },
        );
    }
    Ok(())
}

pub(crate) fn derive_checked_property_evidence_v2(
    property: CapabilityPropertyIdV1,
    report: &InertProductionFinalGraphReportV5,
    target_closure: &InertProductionTargetCapabilityClosureV5,
) -> Result<EvidenceIdentityV1, CompilerCapabilityEvidenceValidationErrorV1> {
    let indexes = checked_evidence_indexes_v2(property).ok_or(
        CompilerCapabilityEvidenceValidationErrorV1::PropertyNotDischargeableByCheckedAnalysis {
            property,
        },
    )?;
    if report.checked_evidence().len() != PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5 {
        return Err(CompilerCapabilityEvidenceValidationErrorV1::CheckedAnalysisContextMismatch);
    }
    let mut digest = Sha256::new();
    digest.update(CHECKED_PROPERTY_EVIDENCE_DOMAIN_V2);
    digest.update(property.namespace().as_bytes());
    digest.update(property.schema_version().to_le_bytes());
    digest.update(property.code().to_le_bytes());
    digest.update(report.checker_identity());
    digest.update(report.report_identity());
    digest.update(report.final_graph());
    digest.update(report.final_graph_bytes().to_le_bytes());
    digest.update(report.final_epoch().to_le_bytes());
    digest.update(report.analysis_epoch().to_le_bytes());
    digest.update(report.schedule_identity());
    digest.update(target_closure.closure_identity());
    digest.update((indexes.len() as u64).to_le_bytes());
    for index in indexes {
        digest.update(report.checked_evidence()[*index]);
    }
    Ok(EvidenceIdentityV1::from_untrusted_digest(
        DigestV1::from_untrusted_bytes(digest.finalize().into()),
    ))
}

const fn checked_evidence_indexes_v2(property: CapabilityPropertyIdV1) -> Option<&'static [usize]> {
    match property {
        CapabilityPropertyIdV1::TYPING => Some(CHECKED_EVIDENCE_INDEXES_V2[0]),
        CapabilityPropertyIdV1::BOUNDS => Some(CHECKED_EVIDENCE_INDEXES_V2[1]),
        CapabilityPropertyIdV1::INITIALIZATION => Some(CHECKED_EVIDENCE_INDEXES_V2[2]),
        CapabilityPropertyIdV1::HIERARCHICAL_OWNERSHIP => Some(CHECKED_EVIDENCE_INDEXES_V2[3]),
        CapabilityPropertyIdV1::DATA_RACE_FREEDOM => Some(CHECKED_EVIDENCE_INDEXES_V2[4]),
        CapabilityPropertyIdV1::ATOMIC_LEGALITY => Some(CHECKED_EVIDENCE_INDEXES_V2[5]),
        CapabilityPropertyIdV1::UNIFORMITY => Some(CHECKED_EVIDENCE_INDEXES_V2[6]),
        CapabilityPropertyIdV1::BARRIER_CONVERGENCE => Some(CHECKED_EVIDENCE_INDEXES_V2[7]),
        CapabilityPropertyIdV1::WORKGROUP_MEMORY_EPOCHS => Some(CHECKED_EVIDENCE_INDEXES_V2[8]),
        CapabilityPropertyIdV1::TENSOR_LAYOUT => Some(CHECKED_EVIDENCE_INDEXES_V2[9]),
        CapabilityPropertyIdV1::EFFECTS => Some(CHECKED_EVIDENCE_INDEXES_V2[10]),
        CapabilityPropertyIdV1::RESOURCE_LEGALITY => Some(CHECKED_EVIDENCE_INDEXES_V2[11]),
        CapabilityPropertyIdV1::TARGET_CAPABILITY_CLOSURE => Some(CHECKED_EVIDENCE_INDEXES_V2[12]),
        _ => None,
    }
}

#[cfg(test)]
mod checked_evidence_mapping_tests {
    use super::*;
    use fe2o3_compiler_ffi::{
        InertProductionFinalGraphReportV5, InertProductionTargetCapabilityClosureV5,
    };
    use fe2o3_proof_contracts::{
        ArtifactIdentityV1, CapabilityAnalysisReportIdentityV1, CapabilityCheckerIdentityV1,
        CapabilityObligationSpecV1, CapabilityResultSpecV1, ExactToolIdentityV1,
        ExecutableKirIdentityV1, KernelIdentityV1, KernelRootIdentityV1, LaunchContractIdentityV1,
        StatementIdentityV1, TargetModelIdentityV1,
    };

    const GRAPH: [u8; 32] = [41; 32];
    const FINAL_EPOCH: u64 = 7;
    const ANALYSIS_EPOCH: u64 = 11;

    fn digest(seed: u8) -> DigestV1 {
        DigestV1::from_untrusted_bytes([seed; 32])
    }

    fn w4_witness() -> Vec<u8> {
        const DOMAIN: &[u8] = b"FE2O3/PRODUCTION-W4-FINAL-GRAPH-CAPABILITY-WITNESS/V1\0";
        const CHECKSUM_DOMAIN: &[u8] = b"FE2O3/PRODUCTION-W4-WITNESS-CHECKSUM/V1\0";
        let mut bytes = DOMAIN.to_vec();
        bytes.extend_from_slice(&1_u16.to_le_bytes());
        bytes.extend_from_slice(b"checked-evidence-mapping-test");
        let mut checksum = Sha256::new();
        checksum.update(CHECKSUM_DOMAIN);
        checksum.update(&bytes);
        bytes.extend_from_slice(&checksum.finalize());
        bytes
    }

    fn checked_evidence() -> Vec<[u8; 32]> {
        (1..=PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5)
            .map(|index| [index as u8; 32])
            .collect()
    }

    fn make_report(
        graph: [u8; 32],
        final_epoch: u64,
        analysis_epoch: u64,
        evidence: Vec<[u8; 32]>,
    ) -> InertProductionFinalGraphReportV5 {
        InertProductionFinalGraphReportV5::new(
            graph,
            4096,
            final_epoch,
            analysis_epoch,
            [51; 32],
            [52; 32],
            evidence,
            w4_witness(),
        )
        .unwrap()
    }

    fn subject(target: u8) -> CapabilitySubjectV1 {
        CapabilitySubjectV1::new(
            KernelIdentityV1::from_untrusted_digest(digest(61)),
            KernelRootIdentityV1::from_untrusted_digest(digest(62)),
            ExecutableKirIdentityV1::from_untrusted_digest(DigestV1::from_untrusted_bytes(GRAPH)),
            FINAL_EPOCH,
            TargetModelIdentityV1::from_untrusted_digest(digest(target)),
            LaunchContractIdentityV1::from_untrusted_digest(digest(64)),
        )
        .unwrap()
    }

    fn closure(target: u8) -> InertProductionTargetCapabilityClosureV5 {
        InertProductionTargetCapabilityClosureV5::new(
            [71; 32],
            GRAPH,
            4096,
            FINAL_EPOCH,
            TargetModelIdentityV1::from_untrusted_digest(digest(target)),
            LaunchContractIdentityV1::from_untrusted_digest(digest(72)),
            b"exact target decisions".to_vec(),
        )
        .unwrap()
    }

    fn obligations(subject: CapabilitySubjectV1) -> InertCapabilityObligationSetV1 {
        InertCapabilityObligationSetV1::from_specs(
            subject,
            REQUIRED_CHECKED_PROPERTIES_V2
                .into_iter()
                .enumerate()
                .map(|(index, property)| {
                    CapabilityObligationSpecV1::new(
                        property,
                        StatementIdentityV1::from_untrusted_digest(digest(80 + index as u8)),
                    )
                })
                .collect(),
        )
        .unwrap()
    }

    fn checked_specs(
        subject: CapabilitySubjectV1,
        obligations: &InertCapabilityObligationSetV1,
        report: &InertProductionFinalGraphReportV5,
        closure: &InertProductionTargetCapabilityClosureV5,
    ) -> Vec<CapabilityResultSpecV1> {
        obligations
            .obligations()
            .iter()
            .map(|obligation| {
                CapabilityResultSpecV1::new(
                    obligation.identity(),
                    CapabilityOutcomeV1::Checked {
                        evidence: derive_checked_property_evidence_v2(
                            obligation.property(),
                            report,
                            closure,
                        )
                        .unwrap(),
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
                )
            })
            .collect()
    }

    #[test]
    fn all_w4_evidence_has_one_checked_property_owner() {
        assert_eq!(REQUIRED_CHECKED_PROPERTIES_V2.len(), 13);
        let mut seen = [0_u8; PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5];
        for property in REQUIRED_CHECKED_PROPERTIES_V2 {
            for index in checked_evidence_indexes_v2(property).unwrap() {
                seen[*index] += 1;
            }
        }
        assert_eq!(seen, [1; PRODUCTION_W4_CHECKED_EVIDENCE_COUNT_V5]);
    }

    #[test]
    fn checked_is_not_proven_and_wrong_evidence_is_rejected() {
        let subject = subject(63);
        let report = make_report(GRAPH, FINAL_EPOCH, ANALYSIS_EPOCH, checked_evidence());
        let closure = closure(63);
        let obligations = obligations(subject);
        let mut specs = checked_specs(subject, &obligations, &report, &closure);
        specs[0] = CapabilityResultSpecV1::new(
            obligations.obligations()[0].identity(),
            CapabilityOutcomeV1::Proven {
                evidence: EvidenceIdentityV1::from_untrusted_digest(digest(101)),
                tool: ExactToolIdentityV1::new(digest(102), digest(103)),
                proof_artifact: ArtifactIdentityV1::new(digest(104), digest(105)),
            },
        );
        let proven =
            InertCapabilityResultSetV1::from_specs_v2(subject, obligations.identity(), specs)
                .unwrap();
        assert!(matches!(
            validate_exact_checked_outcomes_v2(
                subject,
                &obligations,
                &proven,
                &report,
                &closure,
                None,
                None,
            ),
            Err(
                CompilerCapabilityEvidenceValidationErrorV1::WrongOutcomeClass {
                    expected: CapabilityOutcomeKindV1::Checked,
                    actual: CapabilityOutcomeKindV1::Proven,
                    ..
                }
            )
        ));

        let mut specs = checked_specs(subject, &obligations, &report, &closure);
        let property = obligations.obligations()[0].property();
        specs[0] = CapabilityResultSpecV1::new(
            obligations.obligations()[0].identity(),
            CapabilityOutcomeV1::Checked {
                evidence: EvidenceIdentityV1::from_untrusted_digest(digest(106)),
                checker: CapabilityCheckerIdentityV1::from_untrusted_digest(digest(51)),
                report: CapabilityAnalysisReportIdentityV1::from_untrusted_digest(
                    DigestV1::from_untrusted_bytes(report.report_identity()),
                ),
                executable_kir: subject.executable_kir(),
                executable_kir_epoch: subject.executable_kir_epoch(),
                analysis_epoch: report.analysis_epoch(),
            },
        );
        let wrong =
            InertCapabilityResultSetV1::from_specs_v2(subject, obligations.identity(), specs)
                .unwrap();
        assert!(matches!(
            validate_exact_checked_outcomes_v2(
                subject,
                &obligations,
                &wrong,
                &report,
                &closure,
                None,
                None,
            ),
            Err(CompilerCapabilityEvidenceValidationErrorV1::CheckedEvidenceMismatch {
                property: actual,
                ..
            }) if actual == property
        ));
    }

    #[test]
    fn graph_epochs_evidence_order_and_target_are_exact() {
        let subject = subject(63);
        let report = make_report(GRAPH, FINAL_EPOCH, ANALYSIS_EPOCH, checked_evidence());
        let closure = closure(63);
        let obligations = obligations(subject);
        let results = InertCapabilityResultSetV1::from_specs_v2(
            subject,
            obligations.identity(),
            checked_specs(subject, &obligations, &report, &closure),
        )
        .unwrap();
        assert!(
            validate_exact_checked_outcomes_v2(
                subject,
                &obligations,
                &results,
                &report,
                &closure,
                None,
                None,
            )
            .is_ok()
        );

        let mut reordered = checked_evidence();
        reordered.swap(0, 1);
        for hostile in [
            make_report([42; 32], FINAL_EPOCH, ANALYSIS_EPOCH, checked_evidence()),
            make_report(GRAPH, FINAL_EPOCH + 1, ANALYSIS_EPOCH, checked_evidence()),
        ] {
            assert!(matches!(
                validate_exact_checked_outcomes_v2(
                    subject,
                    &obligations,
                    &results,
                    &hostile,
                    &closure,
                    None,
                    None,
                ),
                Err(CompilerCapabilityEvidenceValidationErrorV1::CheckedAnalysisContextMismatch)
            ));
        }
        for hostile in [
            make_report(GRAPH, FINAL_EPOCH, ANALYSIS_EPOCH + 1, checked_evidence()),
            make_report(GRAPH, FINAL_EPOCH, ANALYSIS_EPOCH, reordered),
        ] {
            assert!(matches!(
                validate_exact_checked_outcomes_v2(
                    subject,
                    &obligations,
                    &results,
                    &hostile,
                    &closure,
                    None,
                    None,
                ),
                Err(CompilerCapabilityEvidenceValidationErrorV1::CheckedEvidenceMismatch { .. })
            ));
        }
        assert!(matches!(
            validate_exact_checked_outcomes_v2(
                subject,
                &obligations,
                &results,
                &report,
                &self::closure(65),
                None,
                None,
            ),
            Err(CompilerCapabilityEvidenceValidationErrorV1::CheckedAnalysisContextMismatch)
        ));
    }
}

fn validate_proof_binding_coordinates(
    capsule: &InertProductionSemanticCapsuleV3,
) -> Result<(), CompilerCapabilityEvidenceValidationErrorV1> {
    let proof = InertProofBindingAssociationV4::decode(
        capsule.receipts().proof_binding().canonical_preimage(),
    )
    .map_err(CompilerCapabilityEvidenceValidationErrorV1::ProofBinding)?;
    let capsule_kir = capsule.receipts().kernel_ir().identity();
    if proof.inputs().kernel_ir()
        != lineage_identity(*capsule_kir.sha256(), capsule_kir.byte_len())?
    {
        return Err(CompilerCapabilityEvidenceValidationErrorV1::ProofBindingKirMismatch);
    }
    Ok(())
}

fn validate_context_subject(
    module: &Module,
    subject: CapabilitySubjectV1,
) -> Result<(), CompilerCapabilityEvidenceValidationErrorV1> {
    let kernel = subject.kernel().digest();
    let root = subject.root().digest();
    let target = subject.target_model().digest();
    let launch = subject.launch_contract().digest();
    let mut issuances = 0_usize;
    let mut kernel_matches = 0_usize;
    let mut root_matches = 0_usize;
    let mut target_matches = 0_usize;
    let mut launch_matches = 0_usize;
    for function in &module.functions {
        let Some(body) = &function.body else {
            continue;
        };
        for block in &body.blocks {
            for operation in &block.operations {
                let OperationKind::KernelContextIssue(issue) = &operation.kind else {
                    continue;
                };
                let Some(Type::KernelContext(context)) =
                    operation.results.first().map(|value| &value.ty)
                else {
                    continue;
                };
                issuances = issuances
                    .checked_add(1)
                    .ok_or(CompilerCapabilityEvidenceValidationErrorV1::ContextMultiplicity)?;
                if context.kernel_marker() != kernel.as_bytes() {
                    continue;
                }
                kernel_matches = kernel_matches
                    .checked_add(1)
                    .ok_or(CompilerCapabilityEvidenceValidationErrorV1::ContextMultiplicity)?;
                // The importer carries the authenticated physical kernel-binding identity in the
                // context contract coordinate; KIR separately verifies that `context.root()` is
                // the function that issued this value.
                if &issue.source().contract() != root.as_bytes() {
                    continue;
                }
                root_matches = root_matches
                    .checked_add(1)
                    .ok_or(CompilerCapabilityEvidenceValidationErrorV1::ContextMultiplicity)?;
                if context.target() != target.as_bytes() {
                    continue;
                }
                target_matches = target_matches
                    .checked_add(1)
                    .ok_or(CompilerCapabilityEvidenceValidationErrorV1::ContextMultiplicity)?;
                if context.launch() == launch.as_bytes() {
                    launch_matches = launch_matches
                        .checked_add(1)
                        .ok_or(CompilerCapabilityEvidenceValidationErrorV1::ContextMultiplicity)?;
                }
            }
        }
    }
    if issuances == 0 {
        return Err(CompilerCapabilityEvidenceValidationErrorV1::ContextMultiplicity);
    }
    for (matches, field) in [
        (kernel_matches, CapabilitySubjectFieldV1::Kernel),
        (root_matches, CapabilitySubjectFieldV1::Root),
        (target_matches, CapabilitySubjectFieldV1::TargetModel),
        (launch_matches, CapabilitySubjectFieldV1::LaunchContract),
    ] {
        match matches {
            0 => {
                return Err(
                    CompilerCapabilityEvidenceValidationErrorV1::ContextSubjectMismatch(field),
                );
            }
            1 => {}
            _ => {
                return Err(CompilerCapabilityEvidenceValidationErrorV1::ContextMultiplicity);
            }
        }
    }
    Ok(())
}

fn validate_refinement_receipt(
    kind: InertCapabilityRefinementReceiptKindV1,
    required: bool,
    associated: Option<InertCapabilityRefinementReceiptIdentityV1>,
    supplied: Option<InertCapabilityRefinementReceiptV1>,
) -> Result<Option<InertCapabilityRefinementReceiptV1>, CompilerCapabilityEvidenceValidationErrorV1>
{
    match (required, associated, supplied) {
        (false, None, None) => Ok(None),
        (false, _, _) => {
            Err(CompilerCapabilityEvidenceValidationErrorV1::UnexpectedRefinementReceipt { kind })
        }
        (true, None, _) | (true, _, None) => {
            Err(CompilerCapabilityEvidenceValidationErrorV1::MissingRefinementReceipt { kind })
        }
        (true, Some(associated), Some(receipt)) => {
            if receipt.kind() != kind {
                return Err(
                    CompilerCapabilityEvidenceValidationErrorV1::WrongRefinementReceiptKind {
                        expected: kind,
                        actual: receipt.kind(),
                    },
                );
            }
            if receipt.identity() != associated {
                return Err(
                    CompilerCapabilityEvidenceValidationErrorV1::RefinementReceiptMismatch { kind },
                );
            }
            Ok(Some(receipt))
        }
    }
}

fn validate_refinement_evidence_identity(
    obligations: &InertCapabilityObligationSetV1,
    results: &InertCapabilityResultSetV1,
    property: CapabilityPropertyIdV1,
    receipt: Option<&InertCapabilityRefinementReceiptV1>,
) -> Result<(), CompilerCapabilityEvidenceValidationErrorV1> {
    let Some(receipt) = receipt else {
        return Ok(());
    };
    let Some((_, result)) = obligations
        .obligations()
        .iter()
        .zip(results.results())
        .find(|(obligation, _)| obligation.property() == property)
    else {
        return Err(
            CompilerCapabilityEvidenceValidationErrorV1::UnexpectedRefinementReceipt {
                kind: receipt.kind(),
            },
        );
    };
    let CapabilityOutcomeV1::Proven { evidence, .. } = result.outcome() else {
        unreachable!("non-Proven capability outcomes were rejected before receipt binding")
    };
    if evidence.digest().as_bytes() != &receipt.identity().sha256() {
        return Err(
            CompilerCapabilityEvidenceValidationErrorV1::RefinementEvidenceIdentityMismatch {
                kind: receipt.kind(),
            },
        );
    }
    Ok(())
}

fn subject_mismatch(
    expected: CapabilitySubjectV1,
    actual: CapabilitySubjectV1,
) -> CapabilitySubjectFieldV1 {
    if expected.kernel() != actual.kernel() {
        CapabilitySubjectFieldV1::Kernel
    } else if expected.root() != actual.root() {
        CapabilitySubjectFieldV1::Root
    } else if expected.executable_kir() != actual.executable_kir() {
        CapabilitySubjectFieldV1::ExecutableKir
    } else if expected.executable_kir_epoch() != actual.executable_kir_epoch() {
        CapabilitySubjectFieldV1::ExecutableKirEpoch
    } else if expected.target_model() != actual.target_model() {
        CapabilitySubjectFieldV1::TargetModel
    } else {
        CapabilitySubjectFieldV1::LaunchContract
    }
}

fn lineage_identity(
    sha256: [u8; 32],
    byte_len: u64,
) -> Result<InertLineageContentIdentityV3, CompilerCapabilityEvidenceValidationErrorV1> {
    InertLineageContentIdentityV3::new(sha256, byte_len)
        .map_err(|_| CompilerCapabilityEvidenceValidationErrorV1::InvalidLineageIdentity)
}

/// Fail-closed static capability-evidence validation failures.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerCapabilityEvidenceValidationErrorV1 {
    /// The side-by-side association is malformed or noncanonical.
    Association(InertStaticCapabilityEvidenceAssociationErrorV1),
    /// A nested capability set is malformed or noncanonical.
    CapabilityCodec(CapabilityCodecErrorV1),
    /// The complete obligation/result pair does not match protected expectations.
    Composition(CapabilityCompositionErrorV1),
    /// The explicit association subject differs from protected expectations.
    ExpectedSubjectMismatch(CapabilitySubjectFieldV1),
    /// The expected complete obligation-set identity differs.
    ObligationSetMismatch,
    /// A frozen capsule or receipt coordinate was substituted.
    LineageReceiptMismatch { field: &'static str },
    /// A capsule identity could not be represented as exact lineage coordinates.
    InvalidLineageIdentity,
    /// The frozen proof-binding receipt is not exact V4.
    ProofBinding(InertProofBindingAssociationErrorV4),
    /// The V4 proof association names a different KIR receipt.
    ProofBindingKirMismatch,
    /// The executable KIR is not exact verified V13.
    KernelIr(VerifiedCanonicalKernelIrErrorV13),
    /// The capability subject names a different executable KIR identity.
    ExecutableKirIdentityMismatch,
    /// Compiler-issued KIR context provenance differs from one subject coordinate.
    ContextSubjectMismatch(CapabilitySubjectFieldV1),
    /// The exact KIR does not issue precisely one context matching the complete subject.
    ContextMultiplicity,
    /// One required obligation has a nonfinal outcome.
    OutcomeNotProven {
        index: usize,
        property: CapabilityPropertyIdV1,
        outcome: CapabilityOutcomeKindV1,
    },
    /// The protected checked-evidence route received a legacy result schema.
    ResultSchemaDowngrade { observed: u16 },
    /// A property used an evidence class not permitted by sealed policy.
    WrongOutcomeClass {
        index: usize,
        property: CapabilityPropertyIdV1,
        expected: CapabilityOutcomeKindV1,
        actual: CapabilityOutcomeKindV1,
    },
    /// Checked evidence names a stale or cross-spliced checker, report, graph, or epoch.
    CheckedEvidenceMismatch {
        index: usize,
        property: CapabilityPropertyIdV1,
    },
    /// The retained W4 report and target closure do not describe one exact analysis occurrence.
    CheckedAnalysisContextMismatch,
    /// Sealed policy does not permit static checked analysis to discharge this property.
    PropertyNotDischargeableByCheckedAnalysis { property: CapabilityPropertyIdV1 },
    /// A source or machine property names a different receipt class or exact receipt.
    RefinementOutcomeMismatch {
        index: usize,
        property: CapabilityPropertyIdV1,
    },
    /// Per-dispatch launch evidence was incorrectly represented as static proof.
    DynamicLaunchClaimInStaticEvidence,
    /// A required typed refinement receipt is absent.
    MissingRefinementReceipt {
        kind: InertCapabilityRefinementReceiptKindV1,
    },
    /// An unrequired typed refinement receipt was injected.
    UnexpectedRefinementReceipt {
        kind: InertCapabilityRefinementReceiptKindV1,
    },
    /// A typed receipt was supplied for the wrong refinement boundary.
    WrongRefinementReceiptKind {
        expected: InertCapabilityRefinementReceiptKindV1,
        actual: InertCapabilityRefinementReceiptKindV1,
    },
    /// Opaque refinement-receipt bytes differ from the exact association.
    RefinementReceiptMismatch {
        kind: InertCapabilityRefinementReceiptKindV1,
    },
    /// A `Proven` result names evidence other than the retained exact refinement receipt.
    RefinementEvidenceIdentityMismatch {
        kind: InertCapabilityRefinementReceiptKindV1,
    },
}

impl fmt::Display for CompilerCapabilityEvidenceValidationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Association(error) => write!(formatter, "capability association failed: {error}"),
            Self::CapabilityCodec(error) => write!(formatter, "capability set failed: {error:?}"),
            Self::Composition(error) => {
                write!(formatter, "capability composition failed: {error:?}")
            }
            Self::ExpectedSubjectMismatch(field) => {
                write!(formatter, "capability subject mismatch: {field:?}")
            }
            Self::ObligationSetMismatch => {
                formatter.write_str("complete capability obligation-set identity mismatch")
            }
            Self::LineageReceiptMismatch { field } => {
                write!(formatter, "capability association substituted {field}")
            }
            Self::InvalidLineageIdentity => {
                formatter.write_str("invalid capability lineage identity")
            }
            Self::ProofBinding(error) => {
                write!(formatter, "capability proof binding failed: {error}")
            }
            Self::ProofBindingKirMismatch => {
                formatter.write_str("capability proof binding names a different KIR receipt")
            }
            Self::KernelIr(error) => write!(formatter, "capability executable KIR failed: {error}"),
            Self::ExecutableKirIdentityMismatch => {
                formatter.write_str("capability subject names a different executable KIR identity")
            }
            Self::ContextSubjectMismatch(field) => {
                write!(formatter, "compiler-issued KIR context mismatch: {field:?}")
            }
            Self::ContextMultiplicity => formatter.write_str(
                "capability subject does not select exactly one compiler-issued KIR context",
            ),
            Self::OutcomeNotProven {
                index,
                property,
                outcome,
            } => write!(
                formatter,
                "capability result {index} for {property:?} is {outcome:?}, not Proven"
            ),
            Self::ResultSchemaDowngrade { observed } => write!(
                formatter,
                "checked capability composition requires result schema V2, observed V{observed}"
            ),
            Self::WrongOutcomeClass {
                index,
                property,
                expected,
                actual,
            } => write!(
                formatter,
                "capability result {index} for {property:?} is {actual:?}, expected {expected:?}"
            ),
            Self::CheckedEvidenceMismatch { index, property } => write!(
                formatter,
                "checked capability result {index} for {property:?} substituted its evidence occurrence"
            ),
            Self::CheckedAnalysisContextMismatch => formatter.write_str(
                "checked capability evidence names a stale or cross-spliced W4 analysis context",
            ),
            Self::PropertyNotDischargeableByCheckedAnalysis { property } => write!(
                formatter,
                "sealed capability policy does not discharge {property:?} with checked analysis"
            ),
            Self::RefinementOutcomeMismatch { index, property } => write!(
                formatter,
                "refinement result {index} for {property:?} names a different receipt boundary or identity"
            ),
            Self::DynamicLaunchClaimInStaticEvidence => formatter.write_str(
                "dynamic launch preconditions cannot be discharged by static capability evidence",
            ),
            Self::MissingRefinementReceipt { kind } => {
                write!(formatter, "missing required {kind:?} refinement receipt")
            }
            Self::UnexpectedRefinementReceipt { kind } => {
                write!(formatter, "unexpected {kind:?} refinement receipt")
            }
            Self::WrongRefinementReceiptKind { expected, actual } => write!(
                formatter,
                "wrong refinement receipt kind: expected {expected:?}, found {actual:?}"
            ),
            Self::RefinementReceiptMismatch { kind } => {
                write!(formatter, "substituted {kind:?} refinement receipt")
            }
            Self::RefinementEvidenceIdentityMismatch { kind } => write!(
                formatter,
                "{kind:?} Proven result names different refinement evidence"
            ),
        }
    }
}

impl Error for CompilerCapabilityEvidenceValidationErrorV1 {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Association(error) => Some(error),
            Self::ProofBinding(error) => Some(error),
            Self::KernelIr(error) => Some(error),
            _ => None,
        }
    }
}
