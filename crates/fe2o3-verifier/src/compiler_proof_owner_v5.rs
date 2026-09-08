//! Native exact-canonical-KIR-V13 proof ownership composed beside frozen V4 source custody.

use std::{error::Error, fmt};

use fe2o3_compiler_lineage::{
    InertCanonicalKernelIrV13ReceiptV5, InertCompilerProofOwnerErrorV5, InertCompilerProofOwnerV5,
    InertMultiRootProofLineageV3, InertMultiRootStaticCapabilityEvidenceAssociationV1,
    MultiRootProofRosterKindV3,
};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrErrorV13;

use crate::{ValidatedCompilerCapabilityEvidenceV1, ValidatedCompilerProofInputsV4};

/// Move-only owner of exact source lineage and one native final canonical-KIR-V13 graph.
///
/// The frozen V4 owner remains a separate source-custody object. This V5 owner binds that exact
/// source identity to a separately decoded V13 graph, its optimization epoch, execution subject,
/// compiler policy, capability evidence, and source/machine refinement receipts. It cannot be
/// constructed by projecting the V4 owner's V8 graph and grants no runtime authority.
#[derive(Debug)]
#[must_use = "dropping the V5 owner abandons exact V13 proof custody"]
pub struct ValidatedCompilerProofInputsV5 {
    association: InertCompilerProofOwnerV5,
    capability: ValidatedCompilerCapabilityEvidenceV1,
}

/// Move-only V5 owner for one selected subject that retains the complete multi-root proof roster.
///
/// The selected subject is identified by its canonical descriptor-order ordinal and exact
/// [`fe2o3_proof_contracts::CapabilitySubjectV1`] coordinates, not by a workload name. All four
/// proof rosters remain retained so downstream protected verification cannot project a singleton
/// owner and discard other roots.
#[derive(Debug)]
#[must_use = "dropping the multi-root V5 owner abandons the complete proof roster"]
pub struct ValidatedCompilerMultiRootProofInputsV5 {
    selected: ValidatedCompilerProofInputsV5,
    proof_lineage: InertMultiRootProofLineageV3,
    capability_associations: InertMultiRootStaticCapabilityEvidenceAssociationV1,
}

impl ValidatedCompilerMultiRootProofInputsV5 {
    /// Returns the selected subject's exact V5 owner and capability evidence.
    pub const fn selected(&self) -> &ValidatedCompilerProofInputsV5 {
        &self.selected
    }

    /// Returns the complete, mutually consistent four-roster proof lineage.
    pub const fn proof_lineage(&self) -> &InertMultiRootProofLineageV3 {
        &self.proof_lineage
    }

    /// Returns the complete canonical capability-association roster.
    pub const fn capability_associations(
        &self,
    ) -> &InertMultiRootStaticCapabilityEvidenceAssociationV1 {
        &self.capability_associations
    }

    /// Returns the selected subject's canonical descriptor-order ordinal.
    pub const fn selected_subject_ordinal(&self) -> u32 {
        self.selected.association().selected_subject_ordinal()
    }

    /// Consumes the validated owner into the only three inert records accepted by the sealed
    /// completion response. No singleton association projection is exposed.
    pub fn into_completion_parts(
        self,
    ) -> Result<
        (
            InertCompilerProofOwnerV5,
            InertMultiRootStaticCapabilityEvidenceAssociationV1,
            fe2o3_compiler_lineage::InertCapabilityRefinementReceiptV1,
        ),
        CompilerProofInputValidationErrorV5,
    > {
        let Self {
            selected,
            capability_associations,
            ..
        } = self;
        let ValidatedCompilerProofInputsV5 {
            association,
            capability,
        } = selected;
        let machine_refinement = capability
            .into_machine_refinement()
            .ok_or(CompilerProofInputValidationErrorV5::MissingMachineRefinement)?;
        Ok((association, capability_associations, machine_refinement))
    }
}

impl ValidatedCompilerProofInputsV5 {
    /// Returns the exact canonical V5 association.
    pub const fn association(&self) -> &InertCompilerProofOwnerV5 {
        &self.association
    }

    /// Returns the exact independently verified V13 capability/graph owner.
    pub const fn capability(&self) -> &ValidatedCompilerCapabilityEvidenceV1 {
        &self.capability
    }
}

/// Validates a native V13 owner against exact V4 source custody without treating V8 as V13.
pub fn validate_compiler_proof_inputs_v5(
    association_bytes: &[u8],
    source_owner: &ValidatedCompilerProofInputsV4,
    executable_kir_receipt: &InertCanonicalKernelIrV13ReceiptV5,
    capability: ValidatedCompilerCapabilityEvidenceV1,
    compiler_policy: [u8; 32],
) -> Result<ValidatedCompilerProofInputsV5, CompilerProofInputValidationErrorV5> {
    if compiler_policy == [0; 32] {
        return Err(CompilerProofInputValidationErrorV5::ZeroCompilerPolicy);
    }
    let association = InertCompilerProofOwnerV5::decode(association_bytes)
        .map_err(CompilerProofInputValidationErrorV5::Association)?;
    let inputs = association.inputs();
    let source_receipt = capability
        .source_refinement()
        .ok_or(CompilerProofInputValidationErrorV5::MissingSourceRefinement)?;
    let machine_receipt = capability
        .machine_refinement()
        .ok_or(CompilerProofInputValidationErrorV5::MissingMachineRefinement)?;
    let capability_association = capability.association().identity();
    let verified_kir = capability.kernel_ir();
    verified_kir
        .revalidate()
        .map_err(CompilerProofInputValidationErrorV5::KernelIr)?;

    let proof_receipt = source_owner.receipt_identity();
    require_match(
        inputs.legacy_proof_binding().sha256() == *proof_receipt.sha256()
            && inputs.legacy_proof_binding().byte_len() == proof_receipt.byte_len(),
        "legacy V4 proof binding",
    )?;
    require_match(
        inputs.semantic_mir_receipt() == source_owner.association().inputs().semantic_mir(),
        "semantic MIR receipt",
    )?;
    require_match(
        inputs.semantic_mir_identity() == *source_owner.semantic_mir().semantic_sha256().as_bytes(),
        "semantic MIR identity",
    )?;
    require_match(
        inputs.executable_kir_receipt().sha256() == executable_kir_receipt.identity().sha256()
            && inputs.executable_kir_receipt().byte_len()
                == executable_kir_receipt.identity().byte_len(),
        "canonical KIR V13 receipt",
    )?;
    require_match(
        executable_kir_receipt.canonical_preimage() == verified_kir.canonical_bytes(),
        "canonical KIR V13 receipt preimage",
    )?;
    require_match(
        inputs.subject() == capability.association().subject(),
        "V13 graph subject",
    )?;
    require_match(
        inputs.subject().executable_kir().digest().as_bytes() == verified_kir.identity().digest()
            && inputs.executable_kir_bytes() == verified_kir.identity().canonical_length(),
        "verified canonical KIR V13 identity",
    )?;
    require_match(
        inputs.compiler_policy() == compiler_policy,
        "compiler policy",
    )?;
    require_match(
        inputs.source_refinement() == source_receipt.identity(),
        "source refinement receipt",
    )?;
    require_match(
        inputs.machine_refinement() == machine_receipt.identity(),
        "machine refinement receipt",
    )?;
    require_match(
        association.selected_capability_association() == capability_association,
        "capability association",
    )?;

    Ok(ValidatedCompilerProofInputsV5 {
        association,
        capability,
    })
}

/// Validates any selected subject while retaining and checking the complete multi-root roster.
///
/// `expected_subject_ordinal` must come from the protected caller's descriptor-order selection.
/// The canonical V5 owner independently binds the same ordinal. The complete V3 lineage is
/// consumed and retained; no singleton projection or workload-name lookup is accepted.
#[allow(clippy::too_many_arguments)]
pub fn validate_compiler_multi_root_proof_inputs_v5(
    association_bytes: &[u8],
    source_owner: &ValidatedCompilerProofInputsV4,
    executable_kir_receipt: &InertCanonicalKernelIrV13ReceiptV5,
    capability: ValidatedCompilerCapabilityEvidenceV1,
    compiler_policy: [u8; 32],
    proof_lineage: InertMultiRootProofLineageV3,
    capability_associations: InertMultiRootStaticCapabilityEvidenceAssociationV1,
    expected_subject_ordinal: u32,
) -> Result<ValidatedCompilerMultiRootProofInputsV5, CompilerProofInputValidationErrorV5> {
    let selected = validate_compiler_proof_inputs_v5(
        association_bytes,
        source_owner,
        executable_kir_receipt,
        capability,
        compiler_policy,
    )?;
    let association = selected.association();
    require_match(
        association.proof_lineage() == Some(proof_lineage.identity()),
        "multi-root proof lineage",
    )?;
    if association.selected_subject_ordinal() != expected_subject_ordinal {
        return Err(
            CompilerProofInputValidationErrorV5::SelectedSubjectOrdinalMismatch {
                expected: expected_subject_ordinal,
                actual: association.selected_subject_ordinal(),
            },
        );
    }
    validate_complete_proof_roster_v5(association, &proof_lineage, &capability_associations)?;
    Ok(ValidatedCompilerMultiRootProofInputsV5 {
        selected,
        proof_lineage,
        capability_associations,
    })
}

fn validate_complete_proof_roster_v5(
    association: &InertCompilerProofOwnerV5,
    proof_lineage: &InertMultiRootProofLineageV3,
    capability_associations: &InertMultiRootStaticCapabilityEvidenceAssociationV1,
) -> Result<(), CompilerProofInputValidationErrorV5> {
    let roster = proof_lineage.roster(MultiRootProofRosterKindV3::MiddleEnd);
    let subjects = association.subjects();
    require_match(
        subjects.len() == roster.root_count(),
        "multi-root subject count",
    )?;
    require_match(
        capability_associations.entries().len() == subjects.len(),
        "capability association roster count",
    )?;
    let inputs = association.inputs();
    require_match(
        inputs.capability_association() == capability_associations.identity(),
        "capability association roster",
    )?;
    require_match(
        inputs.semantic_mir_identity() == roster.semantic_mir_sha256(),
        "multi-root semantic MIR",
    )?;
    require_match(
        inputs.executable_kir_bytes() == roster.neutral_kir().canonical_length()
            && inputs.subject().executable_kir().digest().as_bytes()
                == &roster.neutral_kir().digest()
            && inputs.subject().executable_kir_epoch() == roster.neutral_kir().graph_epoch(),
        "multi-root canonical KIR",
    )?;
    for (subject_ordinal, root_ordinal) in roster.canonical_kernel_order().iter().enumerate() {
        let subject = subjects.get(subject_ordinal).ok_or(
            CompilerProofInputValidationErrorV5::IdentityMismatch("multi-root subject roster"),
        )?;
        let root = roster.root(*root_ordinal as usize).ok_or(
            CompilerProofInputValidationErrorV5::IdentityMismatch("multi-root proof roster"),
        )?;
        let capability_association = capability_associations
            .entries()
            .get(subject_ordinal)
            .ok_or(CompilerProofInputValidationErrorV5::IdentityMismatch(
                "capability association roster",
            ))?;
        require_match(
            subject.kernel().digest().as_bytes() == &root.kernel_binding()
                && subject.root().digest().as_bytes() == &root.semantic_root_identity()
                && subject.executable_kir().digest().as_bytes() == &roster.neutral_kir().digest()
                && subject.executable_kir_epoch() == roster.neutral_kir().graph_epoch()
                && subject.target_model() == inputs.subject().target_model()
                && capability_association.subject() == *subject,
            "multi-root proof subject",
        )?;
    }
    let selected_ordinal = usize::try_from(association.selected_subject_ordinal())
        .map_err(|_| CompilerProofInputValidationErrorV5::IdentityMismatch("subject ordinal"))?;
    require_match(
        subjects.get(selected_ordinal) == Some(&inputs.subject())
            && capability_associations
                .entries()
                .get(selected_ordinal)
                .is_some_and(|entry| {
                    entry.identity() == association.selected_capability_association()
                }),
        "selected multi-root subject",
    )
}

fn require_match(
    matches: bool,
    field: &'static str,
) -> Result<(), CompilerProofInputValidationErrorV5> {
    if matches {
        Ok(())
    } else {
        Err(CompilerProofInputValidationErrorV5::IdentityMismatch(field))
    }
}

/// Failure to validate one native exact-canonical-KIR-V13 proof owner.
#[derive(Debug)]
#[non_exhaustive]
pub enum CompilerProofInputValidationErrorV5 {
    /// The canonical V5 association is malformed or downgraded.
    Association(InertCompilerProofOwnerErrorV5),
    /// Compiler policy identity is absent.
    ZeroCompilerPolicy,
    /// Capability policy requires source refinement.
    MissingSourceRefinement,
    /// Capability policy requires machine refinement.
    MissingMachineRefinement,
    /// Exact V13 bytes no longer pass semantic verification.
    KernelIr(VerifiedCanonicalKernelIrErrorV13),
    /// One exact owner coordinate differs.
    IdentityMismatch(&'static str),
    /// The protected selection and canonical owner name different descriptor-order subjects.
    SelectedSubjectOrdinalMismatch {
        /// Ordinal selected by protected policy.
        expected: u32,
        /// Ordinal retained in the canonical V5 owner.
        actual: u32,
    },
}

impl fmt::Display for CompilerProofInputValidationErrorV5 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Association(error) => write!(formatter, "V5 owner association failed: {error}"),
            Self::ZeroCompilerPolicy => formatter.write_str("compiler policy identity is zero"),
            Self::MissingSourceRefinement => {
                formatter.write_str("native V13 owner is missing source refinement")
            }
            Self::MissingMachineRefinement => {
                formatter.write_str("native V13 owner is missing machine refinement")
            }
            Self::KernelIr(error) => write!(formatter, "canonical KIR V13 failed: {error}"),
            Self::IdentityMismatch(field) => {
                write!(formatter, "native V13 owner substituted {field}")
            }
            Self::SelectedSubjectOrdinalMismatch { expected, actual } => write!(
                formatter,
                "native V13 owner selected subject ordinal {actual}, expected {expected}"
            ),
        }
    }
}

impl Error for CompilerProofInputValidationErrorV5 {}
