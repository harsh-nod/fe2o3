//! Same-process final authority join for the issue #138 general GEMM.
//!
//! This module is intentionally crate-private. The join consumes the rustc-owned
//! source correspondence, verifier-owned property closure, and finalizer-owned
//! machine observation together. Public identities are compared, but never
//! accepted in place of any owning input.

use core::fmt;

use fe2o3_general_gemm_compiler::{GeneralGemmScheduleV1, GeneralGemmSymbolicCompilationUnitV1};
use fe2o3_hsaco_finalize::{
    GeneralGemmBarrierRefinementV1, OpaqueGeneralGemmPostLinkMachineObservationV1,
};
use fe2o3_verifier::{
    GENERAL_GEMM_PROOF_PROPERTIES_V1, GENERAL_GEMM_PROPERTY_CLOSURE_COUNT_V1,
    GeneralGemmMachinePropertyConfirmationKindV1, GeneralGemmProofPropertyV1,
    GeneralGemmProofRequestV1, GeneralGemmProofScheduleV1, GeneralGemmPropertyClosureEvaluationV1,
    GeneralGemmPropertyEvidenceBasisV1, GeneralGemmPropertyEvidenceStatusV1,
    GeneralGemmSourcePropertyConfirmationKindV1,
};
use sha2::{Digest as _, Sha256};

use crate::collected_general_gemm_v1::{
    AuthenticatedGeneralGemmFrontendCorrespondenceV1, GeneralGemmSourcePropertyKindV1,
};

const FINAL_JOIN_DOMAIN_V1: &[u8] = b"FE2O3/GENERAL-GEMM/RUSTC-FINAL-JOIN/V1\0";
const MFMA_F32_16X16X16BF16_1K_OPCODE_V1: u32 = 0xd3e1_0002;

/// Exact owner-local property contract expected from the pinned verifier run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct GeneralGemmPropertyJoinContractV1 {
    property: GeneralGemmProofPropertyV1,
    status: GeneralGemmPropertyEvidenceStatusV1,
    basis: GeneralGemmPropertyEvidenceBasisV1,
    source: Option<GeneralGemmSourcePropertyConfirmationKindV1>,
    machine: Option<GeneralGemmMachinePropertyConfirmationKindV1>,
}

/// One checked property record retained by the private final qualification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct QualifiedGeneralGemmPropertyV1 {
    property: GeneralGemmProofPropertyV1,
    evidence_identity: [u8; 32],
    source_confirmation: Option<GeneralGemmSourcePropertyConfirmationKindV1>,
    machine_confirmation: Option<GeneralGemmMachinePropertyConfirmationKindV1>,
}

/// Private owning qualification produced only by the three-owner join.
///
/// It is deliberately neither `Clone` nor serializable. In particular, the
/// exact post-link observation stays alive so later rustc-private hardware and
/// publication phases do not reconstruct authority from the descriptive join
/// identity.
#[must_use = "the general GEMM qualification must remain in its owning rustc transaction"]
pub(crate) struct QualifiedGeneralGemmCompilationV1 {
    identity: [u8; 32],
    frontend: AuthenticatedGeneralGemmFrontendCorrespondenceV1,
    symbolic: GeneralGemmSymbolicCompilationUnitV1,
    proof: GeneralGemmPropertyClosureEvaluationV1,
    machine: OpaqueGeneralGemmPostLinkMachineObservationV1,
    properties: [QualifiedGeneralGemmPropertyV1; GENERAL_GEMM_PROPERTY_CLOSURE_COUNT_V1],
}

impl fmt::Debug for QualifiedGeneralGemmCompilationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedGeneralGemmCompilationV1")
            .field("identity", &self.identity)
            .field("symbolic", &self.symbolic.identity())
            .field("proof", &self.proof.proof_and_numerical_evidence_identity())
            .field("machine", &self.machine.identity())
            .field("properties", &self.properties)
            .finish_non_exhaustive()
    }
}

impl QualifiedGeneralGemmCompilationV1 {
    pub(crate) const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub(crate) const fn symbolic_unit(&self) -> &GeneralGemmSymbolicCompilationUnitV1 {
        &self.symbolic
    }

    pub(crate) fn exact_finalized_bytes(&self) -> &[u8] {
        self.machine.exact_finalized_bytes()
    }

    pub(crate) const fn machine_observation(
        &self,
    ) -> &OpaqueGeneralGemmPostLinkMachineObservationV1 {
        &self.machine
    }

    pub(crate) fn into_owners(
        self,
    ) -> (
        AuthenticatedGeneralGemmFrontendCorrespondenceV1,
        GeneralGemmSymbolicCompilationUnitV1,
        GeneralGemmPropertyClosureEvaluationV1,
        OpaqueGeneralGemmPostLinkMachineObservationV1,
    ) {
        (self.frontend, self.symbolic, self.proof, self.machine)
    }
}

/// Exact reason the same-process three-owner join failed closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GeneralGemmFinalJoinErrorV1 {
    FrontendBindingSubstitution,
    FrontendIdentity,
    ProofRequestSubstitution,
    PropertyOrder { index: usize },
    PropertyStatus { index: usize },
    PropertyBasis { index: usize },
    PropertyEvidenceIdentity { index: usize },
    SourceRequestSubstitution { index: usize },
    MachineRequestSubstitution { index: usize },
    SourceReceiptOrder { index: usize },
    SourceReceiptIdentity { index: usize },
    MissingSourceConfirmation { index: usize },
    MachineCompilationSubstitution,
    MachineScheduleSubstitution,
    MachineArtifactIdentity,
    MachineNumericalRefinement,
    MachineRefinement,
}

impl fmt::Display for GeneralGemmFinalJoinErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "general GEMM final join failed: {self:?}")
    }
}

impl std::error::Error for GeneralGemmFinalJoinErrorV1 {}

/// Consumes the exact source, verifier, and machine owners in one rustc process.
pub(crate) fn qualify_general_gemm_compilation_v1(
    frontend: AuthenticatedGeneralGemmFrontendCorrespondenceV1,
    symbolic: GeneralGemmSymbolicCompilationUnitV1,
    proof: GeneralGemmPropertyClosureEvaluationV1,
    machine: OpaqueGeneralGemmPostLinkMachineObservationV1,
) -> Result<QualifiedGeneralGemmCompilationV1, GeneralGemmFinalJoinErrorV1> {
    if frontend.binding() != symbolic.frontend_semantics() {
        return Err(GeneralGemmFinalJoinErrorV1::FrontendBindingSubstitution);
    }
    if frontend.identity().as_bytes() == &[0; 32] {
        return Err(GeneralGemmFinalJoinErrorV1::FrontendIdentity);
    }

    let expected_request = symbolic
        .symbolic_schedule_proof_request()
        .map_err(|_| GeneralGemmFinalJoinErrorV1::ProofRequestSubstitution)?;
    require_exact_proof_request(expected_request, proof.proof_request())?;

    let schedule = proof.proof_request().schedule();
    let contracts = property_join_contracts(schedule);
    let source_receipts = frontend.source_properties();
    validate_source_receipts(source_receipts)?;
    let machine_confirmations = validate_machine_observation(&symbolic, &machine)?;

    let mut properties = [QualifiedGeneralGemmPropertyV1 {
        property: GeneralGemmProofPropertyV1::MemorySafe,
        evidence_identity: [0; 32],
        source_confirmation: None,
        machine_confirmation: None,
    }; GENERAL_GEMM_PROPERTY_CLOSURE_COUNT_V1];
    for (index, ((request, contract), qualified)) in proof
        .requests()
        .iter()
        .zip(contracts)
        .zip(properties.iter_mut())
        .enumerate()
    {
        let evidence = request.schedule_evidence();
        if evidence.property() != GENERAL_GEMM_PROOF_PROPERTIES_V1[index]
            || evidence.property() != contract.property
        {
            return Err(GeneralGemmFinalJoinErrorV1::PropertyOrder { index });
        }
        if evidence.status() != contract.status {
            return Err(GeneralGemmFinalJoinErrorV1::PropertyStatus { index });
        }
        if evidence.basis() != contract.basis {
            return Err(GeneralGemmFinalJoinErrorV1::PropertyBasis { index });
        }
        if evidence.identity().as_bytes() == &[0; 32] {
            return Err(GeneralGemmFinalJoinErrorV1::PropertyEvidenceIdentity { index });
        }
        if request.source_confirmation() != contract.source {
            return Err(GeneralGemmFinalJoinErrorV1::SourceRequestSubstitution { index });
        }
        if request.machine_confirmation() != contract.machine {
            return Err(GeneralGemmFinalJoinErrorV1::MachineRequestSubstitution { index });
        }
        if let Some(required) = contract.source {
            if !source_receipts
                .iter()
                .any(|receipt| source_kind_to_confirmation(receipt.kind()) == required)
            {
                return Err(GeneralGemmFinalJoinErrorV1::MissingSourceConfirmation { index });
            }
        }
        if let Some(required) = contract.machine {
            let present = match required {
                GeneralGemmMachinePropertyConfirmationKindV1::Bf16Fp32MfmaAndEpiloguePolicy => {
                    machine_confirmations.numerical
                }
                GeneralGemmMachinePropertyConfirmationKindV1::KirToGfx942MachineRefinement => {
                    machine_confirmations.machine
                }
            };
            if !present {
                return Err(GeneralGemmFinalJoinErrorV1::MachineRefinement);
            }
        }
        *qualified = QualifiedGeneralGemmPropertyV1 {
            property: evidence.property(),
            evidence_identity: *evidence.identity().as_bytes(),
            source_confirmation: contract.source,
            machine_confirmation: contract.machine,
        };
    }

    let identity = calculate_final_join_identity(&frontend, &symbolic, &proof, &machine);
    Ok(QualifiedGeneralGemmCompilationV1 {
        identity,
        frontend,
        symbolic,
        proof,
        machine,
        properties,
    })
}

fn require_exact_proof_request(
    expected: GeneralGemmProofRequestV1,
    actual: GeneralGemmProofRequestV1,
) -> Result<(), GeneralGemmFinalJoinErrorV1> {
    if actual != expected {
        return Err(GeneralGemmFinalJoinErrorV1::ProofRequestSubstitution);
    }
    Ok(())
}

fn validate_source_receipts(
    receipts: &[crate::collected_general_gemm_v1::GeneralGemmSourcePropertyReceiptV1; 11],
) -> Result<(), GeneralGemmFinalJoinErrorV1> {
    validate_source_receipt_snapshot(&receipts.each_ref().map(|receipt| SourceReceiptSnapshotV1 {
        kind: receipt.kind(),
        evidence_identity: *receipt.evidence_identity(),
    }))
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct SourceReceiptSnapshotV1 {
    kind: GeneralGemmSourcePropertyKindV1,
    evidence_identity: [u8; 32],
}

fn validate_source_receipt_snapshot(
    receipts: &[SourceReceiptSnapshotV1; 11],
) -> Result<(), GeneralGemmFinalJoinErrorV1> {
    for (index, (receipt, expected)) in receipts
        .iter()
        .zip(canonical_source_property_kinds())
        .enumerate()
    {
        if receipt.kind != expected {
            return Err(GeneralGemmFinalJoinErrorV1::SourceReceiptOrder { index });
        }
        if receipt.evidence_identity == [0; 32] {
            return Err(GeneralGemmFinalJoinErrorV1::SourceReceiptIdentity { index });
        }
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MachineConfirmationsV1 {
    numerical: bool,
    machine: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct MachineObservationFactsV1 {
    compilation_matches: bool,
    schedule_matches: bool,
    artifact_identity_valid: bool,
    numerical_refinement_matches: bool,
    machine_refinement_matches: bool,
}

fn validate_machine_observation_facts(
    facts: MachineObservationFactsV1,
) -> Result<MachineConfirmationsV1, GeneralGemmFinalJoinErrorV1> {
    if !facts.compilation_matches {
        return Err(GeneralGemmFinalJoinErrorV1::MachineCompilationSubstitution);
    }
    if !facts.schedule_matches {
        return Err(GeneralGemmFinalJoinErrorV1::MachineScheduleSubstitution);
    }
    if !facts.artifact_identity_valid {
        return Err(GeneralGemmFinalJoinErrorV1::MachineArtifactIdentity);
    }
    if !facts.numerical_refinement_matches {
        return Err(GeneralGemmFinalJoinErrorV1::MachineNumericalRefinement);
    }
    if !facts.machine_refinement_matches {
        return Err(GeneralGemmFinalJoinErrorV1::MachineRefinement);
    }
    Ok(MachineConfirmationsV1 {
        numerical: true,
        machine: true,
    })
}

fn validate_machine_observation(
    symbolic: &GeneralGemmSymbolicCompilationUnitV1,
    machine: &OpaqueGeneralGemmPostLinkMachineObservationV1,
) -> Result<MachineConfirmationsV1, GeneralGemmFinalJoinErrorV1> {
    let numerical = machine.mfma_numerical_refinement();
    let expected_vector_loads = match symbolic.schedule() {
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1 => 0,
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => 1,
    };
    validate_machine_observation_facts(MachineObservationFactsV1 {
        compilation_matches: machine.symbolic_compilation_identity() == symbolic.identity(),
        schedule_matches: machine.schedule() == symbolic.schedule()
            && machine.schedule_identity() == symbolic.schedule_identity(),
        artifact_identity_valid: machine.symbolic_artifact_identity().as_bytes() != &[0; 32]
            && machine.identity().as_bytes() != &[0; 32]
            && machine.finalized_output_identity().sha256() != &[0; 32]
            && machine
                .finalized_output_identity()
                .matches(machine.exact_finalized_bytes()),
        numerical_refinement_matches: numerical.identity().as_bytes() != &[0; 32]
            && numerical.llvm_assembly_identity() == machine.llvm_assembly_identity()
            && numerical.kernel_symbol_sha256() == machine.kernel_symbol_sha256()
            && numerical.opcode() == MFMA_F32_16X16X16BF16_1K_OPCODE_V1
            && numerical.count() == 1
            && numerical.fp_contract_is_off(),
        machine_refinement_matches: machine.barriers_ir() == 2
            && machine.barriers_isa() == 0
            && machine.barrier_refinement() == GeneralGemmBarrierRefinementV1::SingleWaveElision
            && machine.vector_global_load_count() == expected_vector_loads,
    })
}

fn calculate_final_join_identity(
    frontend: &AuthenticatedGeneralGemmFrontendCorrespondenceV1,
    symbolic: &GeneralGemmSymbolicCompilationUnitV1,
    proof: &GeneralGemmPropertyClosureEvaluationV1,
    machine: &OpaqueGeneralGemmPostLinkMachineObservationV1,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(FINAL_JOIN_DOMAIN_V1);
    digest.update(frontend.identity().as_bytes());
    digest.update(symbolic.identity().as_bytes());
    digest.update(proof.proof_and_numerical_evidence_identity().as_bytes());
    digest.update(proof.schedule_proof_identity().as_bytes());
    digest.update(proof.numerical_policy_evidence_identity().as_bytes());
    digest.update(machine.identity().as_bytes());
    digest.update(machine.finalized_output_identity().sha256());
    for receipt in frontend.source_properties() {
        digest.update([receipt.kind() as u8]);
        digest.update(receipt.evidence_identity());
    }
    for request in proof.requests() {
        digest.update(request.schedule_evidence().identity().as_bytes());
    }
    digest.finalize().into()
}

fn property_join_contracts(
    schedule: GeneralGemmProofScheduleV1,
) -> [GeneralGemmPropertyJoinContractV1; GENERAL_GEMM_PROPERTY_CLOSURE_COUNT_V1] {
    use GeneralGemmMachinePropertyConfirmationKindV1::{
        Bf16Fp32MfmaAndEpiloguePolicy, KirToGfx942MachineRefinement,
    };
    use GeneralGemmProofPropertyV1::*;
    use GeneralGemmPropertyEvidenceBasisV1::{ModelDefinition, OpenObligation, VerifiedTheorem};
    use GeneralGemmPropertyEvidenceStatusV1::{
        ModelDefinitionOnly, OpenArtifactRequired, OpenCorrespondenceRequired,
        ScheduleModelTheoremVerified, WeakerExactRealTheoremVerified,
    };
    use GeneralGemmSourcePropertyConfirmationKindV1::*;

    let schedule_theorem = |reference, vectorized| match schedule {
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1 => reference,
        GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => vectorized,
    };
    [
        GeneralGemmPropertyJoinContractV1 {
            property: MemorySafe,
            status: OpenCorrespondenceRequired,
            basis: OpenObligation("allocation_provenance_from_authenticated_kir_v1"),
            source: Some(AllocationAndProvenance),
            machine: None,
        },
        GeneralGemmPropertyJoinContractV1 {
            property: BoundsSafe,
            status: ScheduleModelTheoremVerified,
            basis: VerifiedTheorem(schedule_theorem(
                "reference_modeled_global_accesses_are_bounded_v1",
                "vectorized_a_only_modeled_global_accesses_are_bounded_v1",
            )),
            source: Some(GuardedGlobalAccesses),
            machine: None,
        },
        GeneralGemmPropertyJoinContractV1 {
            property: Initialized,
            status: OpenCorrespondenceRequired,
            basis: OpenObligation("lds_write_read_initialization_from_authenticated_kir_v1"),
            source: Some(LdsWriteReadInitialization),
            machine: None,
        },
        GeneralGemmPropertyJoinContractV1 {
            property: RaceFree,
            status: OpenCorrespondenceRequired,
            basis: OpenObligation(
                "global_and_lds_effect_conflict_freedom_from_authenticated_kir_v1",
            ),
            source: Some(EffectConflictFreedom),
            machine: None,
        },
        GeneralGemmPropertyJoinContractV1 {
            property: BarrierConvergent,
            status: ModelDefinitionOnly,
            basis: ModelDefinition("lane_reaches_barrier_v1"),
            source: Some(ControlFlowBarrierConvergence),
            machine: None,
        },
        GeneralGemmPropertyJoinContractV1 {
            property: OutputRegionInjective,
            status: ScheduleModelTheoremVerified,
            basis: VerifiedTheorem(schedule_theorem(
                "reference_output_region_is_injective_v1",
                "vectorized_a_only_output_region_is_injective_v1",
            )),
            source: Some(OutputOwnership),
            machine: None,
        },
        GeneralGemmPropertyJoinContractV1 {
            property: LdsEpochCorrect,
            status: ModelDefinitionOnly,
            basis: ModelDefinition("schedule_lds_epoch_correct_v1"),
            source: Some(LdsLifecycle),
            machine: None,
        },
        GeneralGemmPropertyJoinContractV1 {
            property: AccumulatorPhaseRefinement,
            status: ScheduleModelTheoremVerified,
            basis: VerifiedTheorem(schedule_theorem(
                "reference_accumulator_refines_contract_v1",
                "vectorized_accumulator_refines_contract_v1",
            )),
            source: Some(AccumulatorPhase),
            machine: None,
        },
        GeneralGemmPropertyJoinContractV1 {
            property: TailRefinement,
            status: ScheduleModelTheoremVerified,
            basis: VerifiedTheorem(schedule_theorem(
                "reference_scalar_tail_zero_fills_v1",
                "vectorized_full_transfer_and_scalar_tail_refine_v1",
            )),
            source: Some(MaskedTail),
            machine: None,
        },
        GeneralGemmPropertyJoinContractV1 {
            property: EpilogueRefinement,
            status: ScheduleModelTheoremVerified,
            basis: VerifiedTheorem(schedule_theorem(
                "reference_epilogue_refines_exact_real_contract_v1",
                "vectorized_a_only_epilogue_refines_exact_real_contract_v1",
            )),
            source: Some(AlphaBetaEpilogue),
            machine: None,
        },
        GeneralGemmPropertyJoinContractV1 {
            property: NumericalContract,
            status: WeakerExactRealTheoremVerified,
            basis: VerifiedTheorem(schedule_theorem(
                "reference_numerical_contract_v1",
                "vectorized_numerical_contract_v1",
            )),
            source: Some(NumericalOperationOrder),
            machine: Some(Bf16Fp32MfmaAndEpiloguePolicy),
        },
        GeneralGemmPropertyJoinContractV1 {
            property: MachineRefinementBoundary,
            status: OpenArtifactRequired,
            basis: OpenObligation("emitted_gfx942_machine_refinement_v1"),
            source: None,
            machine: Some(KirToGfx942MachineRefinement),
        },
    ]
}

const fn canonical_source_property_kinds() -> [GeneralGemmSourcePropertyKindV1; 11] {
    use GeneralGemmSourcePropertyKindV1::*;
    [
        AllocationAndProvenance,
        GuardedGlobalAccesses,
        LdsWriteReadInitialization,
        EffectConflictFreedom,
        ControlFlowBarrierConvergence,
        OutputOwnership,
        LdsLifecycle,
        AccumulatorPhase,
        MaskedTail,
        AlphaBetaEpilogue,
        NumericalOperationOrder,
    ]
}

const fn source_kind_to_confirmation(
    kind: GeneralGemmSourcePropertyKindV1,
) -> GeneralGemmSourcePropertyConfirmationKindV1 {
    match kind {
        GeneralGemmSourcePropertyKindV1::AllocationAndProvenance => {
            GeneralGemmSourcePropertyConfirmationKindV1::AllocationAndProvenance
        }
        GeneralGemmSourcePropertyKindV1::GuardedGlobalAccesses => {
            GeneralGemmSourcePropertyConfirmationKindV1::GuardedGlobalAccesses
        }
        GeneralGemmSourcePropertyKindV1::LdsWriteReadInitialization => {
            GeneralGemmSourcePropertyConfirmationKindV1::LdsWriteReadInitialization
        }
        GeneralGemmSourcePropertyKindV1::EffectConflictFreedom => {
            GeneralGemmSourcePropertyConfirmationKindV1::EffectConflictFreedom
        }
        GeneralGemmSourcePropertyKindV1::ControlFlowBarrierConvergence => {
            GeneralGemmSourcePropertyConfirmationKindV1::ControlFlowBarrierConvergence
        }
        GeneralGemmSourcePropertyKindV1::OutputOwnership => {
            GeneralGemmSourcePropertyConfirmationKindV1::OutputOwnership
        }
        GeneralGemmSourcePropertyKindV1::LdsLifecycle => {
            GeneralGemmSourcePropertyConfirmationKindV1::LdsLifecycle
        }
        GeneralGemmSourcePropertyKindV1::AccumulatorPhase => {
            GeneralGemmSourcePropertyConfirmationKindV1::AccumulatorPhase
        }
        GeneralGemmSourcePropertyKindV1::MaskedTail => {
            GeneralGemmSourcePropertyConfirmationKindV1::MaskedTail
        }
        GeneralGemmSourcePropertyKindV1::AlphaBetaEpilogue => {
            GeneralGemmSourcePropertyConfirmationKindV1::AlphaBetaEpilogue
        }
        GeneralGemmSourcePropertyKindV1::NumericalOperationOrder => {
            GeneralGemmSourcePropertyConfirmationKindV1::NumericalOperationOrder
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fe2o3_verifier::GeneralGemmEvidenceIdentityV1;

    #[derive(Clone, Copy)]
    struct ObservedPropertyV1 {
        property: GeneralGemmProofPropertyV1,
        status: GeneralGemmPropertyEvidenceStatusV1,
        basis: GeneralGemmPropertyEvidenceBasisV1,
        source: Option<GeneralGemmSourcePropertyConfirmationKindV1>,
        machine: Option<GeneralGemmMachinePropertyConfirmationKindV1>,
    }

    fn validate_observed_properties(
        schedule: GeneralGemmProofScheduleV1,
        observed: &[ObservedPropertyV1; 12],
    ) -> Result<(), GeneralGemmFinalJoinErrorV1> {
        for (index, (actual, expected)) in observed
            .iter()
            .zip(property_join_contracts(schedule))
            .enumerate()
        {
            if actual.property != expected.property {
                return Err(GeneralGemmFinalJoinErrorV1::PropertyOrder { index });
            }
            if actual.status != expected.status {
                return Err(GeneralGemmFinalJoinErrorV1::PropertyStatus { index });
            }
            if actual.basis != expected.basis {
                return Err(GeneralGemmFinalJoinErrorV1::PropertyBasis { index });
            }
            if actual.source != expected.source {
                return Err(GeneralGemmFinalJoinErrorV1::SourceRequestSubstitution { index });
            }
            if actual.machine != expected.machine {
                return Err(GeneralGemmFinalJoinErrorV1::MachineRequestSubstitution { index });
            }
        }
        Ok(())
    }

    fn canonical_observed(schedule: GeneralGemmProofScheduleV1) -> [ObservedPropertyV1; 12] {
        property_join_contracts(schedule).map(|contract| ObservedPropertyV1 {
            property: contract.property,
            status: contract.status,
            basis: contract.basis,
            source: contract.source,
            machine: contract.machine,
        })
    }

    fn proof_request(
        schedule: GeneralGemmProofScheduleV1,
        identities: [u8; 11],
    ) -> GeneralGemmProofRequestV1 {
        let identity = |index: usize| {
            GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([identities[index]; 32])
        };
        GeneralGemmProofRequestV1::checked(
            schedule,
            identity(0),
            identity(1),
            identity(2),
            identity(3),
            identity(4),
            identity(5),
            identity(6),
            identity(7),
            identity(8),
            identity(9),
            identity(10),
        )
        .unwrap()
    }

    #[test]
    fn both_schedules_require_the_exact_twelve_property_contracts() {
        for schedule in [
            GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
            GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
        ] {
            validate_observed_properties(schedule, &canonical_observed(schedule)).unwrap();
        }
    }

    #[test]
    fn every_symbolic_proof_request_domain_rejects_substitution() {
        let schedule = GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1;
        let identities = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11];
        let expected = proof_request(schedule, identities);
        require_exact_proof_request(expected, expected).unwrap();
        for index in 0..identities.len() {
            let mut changed = identities;
            changed[index] = 32 + index as u8;
            assert_eq!(
                require_exact_proof_request(expected, proof_request(schedule, changed)),
                Err(GeneralGemmFinalJoinErrorV1::ProofRequestSubstitution)
            );
        }
        assert_eq!(
            require_exact_proof_request(
                expected,
                proof_request(
                    GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                    identities,
                )
            ),
            Err(GeneralGemmFinalJoinErrorV1::ProofRequestSubstitution)
        );
    }

    #[test]
    fn property_order_status_and_basis_substitutions_fail_at_each_index() {
        let schedule = GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1;
        for index in 0..12 {
            let mut changed = canonical_observed(schedule);
            changed[index].property = GENERAL_GEMM_PROOF_PROPERTIES_V1[(index + 1) % 12];
            assert_eq!(
                validate_observed_properties(schedule, &changed),
                Err(GeneralGemmFinalJoinErrorV1::PropertyOrder { index })
            );

            let mut changed = canonical_observed(schedule);
            changed[index].status = GeneralGemmPropertyEvidenceStatusV1::OpenArtifactRequired;
            if changed[index].status == property_join_contracts(schedule)[index].status {
                changed[index].status =
                    GeneralGemmPropertyEvidenceStatusV1::OpenCorrespondenceRequired;
            }
            assert_eq!(
                validate_observed_properties(schedule, &changed),
                Err(GeneralGemmFinalJoinErrorV1::PropertyStatus { index })
            );

            let mut changed = canonical_observed(schedule);
            changed[index].basis = GeneralGemmPropertyEvidenceBasisV1::OpenObligation("hostile");
            assert_eq!(
                validate_observed_properties(schedule, &changed),
                Err(GeneralGemmFinalJoinErrorV1::PropertyBasis { index })
            );
        }
    }

    #[test]
    fn every_missing_or_substituted_confirmation_fails_closed() {
        let schedule = GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1;
        let canonical = canonical_observed(schedule);
        for index in 0..12 {
            if canonical[index].source.is_some() {
                let mut changed = canonical;
                changed[index].source = None;
                assert_eq!(
                    validate_observed_properties(schedule, &changed),
                    Err(GeneralGemmFinalJoinErrorV1::SourceRequestSubstitution { index })
                );
            }
            if canonical[index].machine.is_some() {
                let mut changed = canonical;
                changed[index].machine = None;
                assert_eq!(
                    validate_observed_properties(schedule, &changed),
                    Err(GeneralGemmFinalJoinErrorV1::MachineRequestSubstitution { index })
                );
            }
        }
    }

    #[test]
    fn every_missing_or_reordered_source_receipt_fails_closed() {
        let canonical = canonical_source_property_kinds().map(|kind| SourceReceiptSnapshotV1 {
            kind,
            evidence_identity: [kind as u8; 32],
        });
        validate_source_receipt_snapshot(&canonical).unwrap();
        for index in 0..11 {
            let mut missing = canonical;
            missing[index].evidence_identity = [0; 32];
            assert_eq!(
                validate_source_receipt_snapshot(&missing),
                Err(GeneralGemmFinalJoinErrorV1::SourceReceiptIdentity { index })
            );

            let mut reordered = canonical;
            reordered[index].kind = canonical[(index + 1) % 11].kind;
            assert_eq!(
                validate_source_receipt_snapshot(&reordered),
                Err(GeneralGemmFinalJoinErrorV1::SourceReceiptOrder { index })
            );
        }
    }

    #[test]
    fn source_receipt_order_is_a_bijection_with_verifier_confirmation_kinds() {
        let kinds = canonical_source_property_kinds();
        for (index, kind) in kinds.into_iter().enumerate() {
            assert_eq!(source_kind_to_confirmation(kind) as usize, index + 1);
        }
        let contracts = property_join_contracts(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1);
        for required in kinds.map(source_kind_to_confirmation) {
            assert_eq!(
                contracts
                    .iter()
                    .filter(|contract| contract.source == Some(required))
                    .count(),
                1
            );
        }
    }

    #[test]
    fn machine_confirmation_is_required_only_at_numerical_and_machine_boundaries() {
        let contracts = property_join_contracts(GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1);
        assert_eq!(
            contracts
                .iter()
                .filter_map(|contract| contract.machine)
                .collect::<Vec<_>>(),
            vec![
                GeneralGemmMachinePropertyConfirmationKindV1::Bf16Fp32MfmaAndEpiloguePolicy,
                GeneralGemmMachinePropertyConfirmationKindV1::KirToGfx942MachineRefinement,
            ]
        );
    }

    #[test]
    fn each_hostile_machine_observation_fact_fails_before_qualification() {
        let canonical = MachineObservationFactsV1 {
            compilation_matches: true,
            schedule_matches: true,
            artifact_identity_valid: true,
            numerical_refinement_matches: true,
            machine_refinement_matches: true,
        };
        assert_eq!(
            validate_machine_observation_facts(canonical),
            Ok(MachineConfirmationsV1 {
                numerical: true,
                machine: true,
            })
        );

        let mutations: [(
            fn(&mut MachineObservationFactsV1),
            GeneralGemmFinalJoinErrorV1,
        ); 5] = [
            (
                |facts| facts.compilation_matches = false,
                GeneralGemmFinalJoinErrorV1::MachineCompilationSubstitution,
            ),
            (
                |facts| facts.schedule_matches = false,
                GeneralGemmFinalJoinErrorV1::MachineScheduleSubstitution,
            ),
            (
                |facts| facts.artifact_identity_valid = false,
                GeneralGemmFinalJoinErrorV1::MachineArtifactIdentity,
            ),
            (
                |facts| facts.numerical_refinement_matches = false,
                GeneralGemmFinalJoinErrorV1::MachineNumericalRefinement,
            ),
            (
                |facts| facts.machine_refinement_matches = false,
                GeneralGemmFinalJoinErrorV1::MachineRefinement,
            ),
        ];
        for (mutate, expected) in mutations {
            let mut changed = canonical;
            mutate(&mut changed);
            assert_eq!(validate_machine_observation_facts(changed), Err(expected));
        }
    }
}
