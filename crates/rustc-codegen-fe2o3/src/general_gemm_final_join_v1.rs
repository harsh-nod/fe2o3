//! Same-process final authority join for the issue #138 general GEMM.
//!
//! This module is intentionally crate-private. The join consumes the rustc-owned
//! source correspondence, verifier-owned property closure, and finalizer-owned
//! machine observation together. Public identities are compared, but never
//! accepted in place of any owning input.

#![allow(dead_code)] // The pipeline hook stays fail-closed until proof execution is authoritative.

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
const FINAL_PAIR_JOIN_DOMAIN_V1: &[u8] = b"FE2O3/GENERAL-GEMM/RUSTC-FINAL-PAIR-JOIN/V1\0";
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

/// One schedule-local owner retained inside the private pair qualification.
pub(crate) struct QualifiedGeneralGemmScheduleV1 {
    identity: [u8; 32],
    symbolic: GeneralGemmSymbolicCompilationUnitV1,
    proof: GeneralGemmPropertyClosureEvaluationV1,
    machine: OpaqueGeneralGemmPostLinkMachineObservationV1,
    properties: [QualifiedGeneralGemmPropertyV1; GENERAL_GEMM_PROPERTY_CLOSURE_COUNT_V1],
}

impl fmt::Debug for QualifiedGeneralGemmScheduleV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedGeneralGemmScheduleV1")
            .field("identity", &self.identity)
            .field("symbolic", &self.symbolic.identity())
            .field("proof", &self.proof.proof_and_numerical_evidence_identity())
            .field("machine", &self.machine.identity())
            .field("properties", &self.properties)
            .finish_non_exhaustive()
    }
}

impl QualifiedGeneralGemmScheduleV1 {
    pub(crate) const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub(crate) const fn symbolic_unit(&self) -> &GeneralGemmSymbolicCompilationUnitV1 {
        &self.symbolic
    }

    pub(crate) const fn proof_closure(&self) -> &GeneralGemmPropertyClosureEvaluationV1 {
        &self.proof
    }

    pub(crate) fn exact_finalized_bytes(&self) -> &[u8] {
        self.machine.exact_finalized_bytes()
    }

    pub(crate) const fn machine_observation(
        &self,
    ) -> &OpaqueGeneralGemmPostLinkMachineObservationV1 {
        &self.machine
    }

    pub(crate) const fn schedule(&self) -> GeneralGemmScheduleV1 {
        self.symbolic.schedule()
    }
}

/// Private seven-owner qualification for both production schedules.
///
/// This value is deliberately neither `Clone` nor serializable. One frontend
/// correspondence is consumed once and retained beside both exact symbolic,
/// verifier, and post-link machine owners. Borrowed schedule views can feed the
/// hardware executor without reconstructing authority from identities.
#[must_use = "the paired general GEMM qualification must remain in its owning rustc transaction"]
pub(crate) struct QualifiedGeneralGemmPairCompilationV1 {
    identity: [u8; 32],
    frontend: AuthenticatedGeneralGemmFrontendCorrespondenceV1,
    reference: QualifiedGeneralGemmScheduleV1,
    vectorized: QualifiedGeneralGemmScheduleV1,
}

impl fmt::Debug for QualifiedGeneralGemmPairCompilationV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QualifiedGeneralGemmPairCompilationV1")
            .field("identity", &self.identity)
            .field("frontend", &self.frontend.identity())
            .field("reference", &self.reference)
            .field("vectorized", &self.vectorized)
            .finish_non_exhaustive()
    }
}

impl QualifiedGeneralGemmPairCompilationV1 {
    pub(crate) const fn identity(&self) -> &[u8; 32] {
        &self.identity
    }

    pub(crate) const fn reference(&self) -> &QualifiedGeneralGemmScheduleV1 {
        &self.reference
    }

    pub(crate) const fn vectorized(&self) -> &QualifiedGeneralGemmScheduleV1 {
        &self.vectorized
    }
}

/// Exact reason the same-process three-owner join failed closed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum GeneralGemmFinalJoinErrorV1 {
    FrontendBindingSubstitution,
    FrontendRevalidation,
    PairRequestSubstitution,
    ScheduleOrder { index: usize },
    ProofRequestSubstitution,
    PropertyOrder { index: usize },
    PropertyStatus { index: usize },
    PropertyBasis { index: usize },
    PropertyEvidenceIdentity { index: usize },
    SourceRequestSubstitution { index: usize },
    MachineRequestSubstitution { index: usize },
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

/// Consumes one source owner and both ordered schedule-owner triples.
pub(crate) fn qualify_general_gemm_pair_compilation_v1(
    frontend: AuthenticatedGeneralGemmFrontendCorrespondenceV1,
    reference_symbolic: GeneralGemmSymbolicCompilationUnitV1,
    reference_proof: GeneralGemmPropertyClosureEvaluationV1,
    reference_machine: OpaqueGeneralGemmPostLinkMachineObservationV1,
    vectorized_symbolic: GeneralGemmSymbolicCompilationUnitV1,
    vectorized_proof: GeneralGemmPropertyClosureEvaluationV1,
    vectorized_machine: OpaqueGeneralGemmPostLinkMachineObservationV1,
) -> Result<QualifiedGeneralGemmPairCompilationV1, GeneralGemmFinalJoinErrorV1> {
    if !frontend.revalidate() {
        return Err(GeneralGemmFinalJoinErrorV1::FrontendRevalidation);
    }
    require_pair_request_consistency(&reference_symbolic, &vectorized_symbolic)?;
    let reference = qualify_general_gemm_schedule_v1(
        &frontend,
        GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
        0,
        reference_symbolic,
        reference_proof,
        reference_machine,
    )?;
    let vectorized = qualify_general_gemm_schedule_v1(
        &frontend,
        GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
        1,
        vectorized_symbolic,
        vectorized_proof,
        vectorized_machine,
    )?;
    let identity = calculate_final_pair_join_identity(&frontend, &reference, &vectorized);
    Ok(QualifiedGeneralGemmPairCompilationV1 {
        identity,
        frontend,
        reference,
        vectorized,
    })
}

fn require_pair_request_consistency(
    reference: &GeneralGemmSymbolicCompilationUnitV1,
    vectorized: &GeneralGemmSymbolicCompilationUnitV1,
) -> Result<(), GeneralGemmFinalJoinErrorV1> {
    let reference_request = reference.request();
    let vectorized_request = vectorized.request();
    if reference.frontend_semantics() != vectorized.frontend_semantics()
        || reference_request.identity() != vectorized_request.identity()
        || reference_request.kernel_instance_identity()
            != vectorized_request.kernel_instance_identity()
        || reference_request.input() != vectorized_request.input()
        || reference_request.input_obligations_identity()
            != vectorized_request.input_obligations_identity()
        || reference_request.compiler_profile_identity()
            != vectorized_request.compiler_profile_identity()
        || reference_request.target_profile_identity()
            != vectorized_request.target_profile_identity()
        || reference_request.selector() != vectorized_request.selector()
        || reference_request.limits() != vectorized_request.limits()
        || reference.toolchain_route_identity() != vectorized.toolchain_route_identity()
        || reference.limits() != vectorized.limits()
    {
        return Err(GeneralGemmFinalJoinErrorV1::PairRequestSubstitution);
    }
    Ok(())
}

fn qualify_general_gemm_schedule_v1(
    frontend: &AuthenticatedGeneralGemmFrontendCorrespondenceV1,
    expected_schedule: GeneralGemmScheduleV1,
    schedule_index: usize,
    symbolic: GeneralGemmSymbolicCompilationUnitV1,
    proof: GeneralGemmPropertyClosureEvaluationV1,
    machine: OpaqueGeneralGemmPostLinkMachineObservationV1,
) -> Result<QualifiedGeneralGemmScheduleV1, GeneralGemmFinalJoinErrorV1> {
    require_schedule_order(symbolic.schedule(), expected_schedule, schedule_index)?;
    if frontend.binding() != symbolic.frontend_semantics() {
        return Err(GeneralGemmFinalJoinErrorV1::FrontendBindingSubstitution);
    }

    let expected_request = symbolic
        .symbolic_schedule_proof_request()
        .map_err(|_| GeneralGemmFinalJoinErrorV1::ProofRequestSubstitution)?;
    require_exact_proof_request(expected_request, proof.proof_request())?;

    let schedule = proof.proof_request().schedule();
    let contracts = property_join_contracts(schedule);
    let source_receipts = frontend.source_properties();
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
        if let Some(required) = contract.source
            && !source_receipts
                .iter()
                .any(|receipt| source_kind_to_confirmation(receipt.kind()) == required)
        {
            return Err(GeneralGemmFinalJoinErrorV1::MissingSourceConfirmation { index });
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

    let identity = calculate_final_join_identity(frontend, &symbolic, &proof, &machine);
    Ok(QualifiedGeneralGemmScheduleV1 {
        identity,
        symbolic,
        proof,
        machine,
        properties,
    })
}

fn require_schedule_order(
    actual: GeneralGemmScheduleV1,
    expected: GeneralGemmScheduleV1,
    index: usize,
) -> Result<(), GeneralGemmFinalJoinErrorV1> {
    if actual != expected {
        return Err(GeneralGemmFinalJoinErrorV1::ScheduleOrder { index });
    }
    Ok(())
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

fn calculate_final_pair_join_identity(
    frontend: &AuthenticatedGeneralGemmFrontendCorrespondenceV1,
    reference: &QualifiedGeneralGemmScheduleV1,
    vectorized: &QualifiedGeneralGemmScheduleV1,
) -> [u8; 32] {
    let mut digest = Sha256::new();
    digest.update(FINAL_PAIR_JOIN_DOMAIN_V1);
    digest.update(frontend.identity().as_bytes());
    digest.update(reference.identity());
    digest.update(reference.symbolic.identity().as_bytes());
    digest.update(
        reference
            .proof
            .proof_and_numerical_evidence_identity()
            .as_bytes(),
    );
    digest.update(reference.machine.identity().as_bytes());
    digest.update(vectorized.identity());
    digest.update(vectorized.symbolic.identity().as_bytes());
    digest.update(
        vectorized
            .proof
            .proof_and_numerical_evidence_identity()
            .as_bytes(),
    );
    digest.update(vectorized.machine.identity().as_bytes());
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
    use fe2o3_compiler_api::{
        CompileLimitsV1, CompileRequestV1, CompilerProfileIdentityV1, CompilerStageV1,
        KernelInstanceIdentityV1, PipelineSelectorV1, RequestIdentityV1, SnapshotFormatIdentityV1,
        SnapshotIdentityV1, StageSnapshotV1, TargetProfileIdentityV1,
    };
    use fe2o3_general_gemm_compiler::{
        GeneralGemmFrontendSemanticBindingV1, GeneralGemmLoweringLimitsV1,
        GeneralGemmSymbolicKirV1, GeneralGemmSymbolicPlanV1,
        general_gemm_symbolic_obligation_set_identity_v1,
        general_gemm_symbolic_pipeline_configuration_identity_v1,
    };
    use fe2o3_verifier::GeneralGemmEvidenceIdentityV1;

    #[derive(Clone, Copy)]
    struct ObservedPropertyV1 {
        property: GeneralGemmProofPropertyV1,
        status: GeneralGemmPropertyEvidenceStatusV1,
        basis: GeneralGemmPropertyEvidenceBasisV1,
        source: Option<GeneralGemmSourcePropertyConfirmationKindV1>,
        machine: Option<GeneralGemmMachinePropertyConfirmationKindV1>,
    }

    type MachineFactMutationV1 = (
        fn(&mut MachineObservationFactsV1),
        GeneralGemmFinalJoinErrorV1,
    );

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

    fn identity(byte: u8) -> [u8; 32] {
        [byte; 32]
    }

    fn frontend_binding(kernel_byte: u8, source_byte: u8) -> GeneralGemmFrontendSemanticBindingV1 {
        GeneralGemmFrontendSemanticBindingV1::from_consumed_frontend_receipt_observation(
            identity(kernel_byte),
            identity(source_byte),
            identity(0x42),
            identity(0x43),
            GeneralGemmSymbolicPlanV1::canonical(),
            GeneralGemmSymbolicKirV1::canonical(),
        )
        .unwrap()
    }

    #[derive(Clone, Copy)]
    struct SymbolicUnitFixtureV1 {
        request_byte: u8,
        input_byte: u8,
        compiler_byte: u8,
        target_byte: u8,
        compile_limits: CompileLimitsV1,
        lowering_limits: GeneralGemmLoweringLimitsV1,
    }

    fn canonical_symbolic_fixture() -> SymbolicUnitFixtureV1 {
        SymbolicUnitFixtureV1 {
            request_byte: 0x11,
            input_byte: 0x17,
            compiler_byte: 0x13,
            target_byte: 0x14,
            compile_limits: CompileLimitsV1::new(16, 16, 16, 4096, 16_384, 4096).unwrap(),
            lowering_limits: GeneralGemmLoweringLimitsV1::default(),
        }
    }

    fn symbolic_unit_for(
        schedule: GeneralGemmScheduleV1,
        frontend: GeneralGemmFrontendSemanticBindingV1,
        fixture: SymbolicUnitFixtureV1,
    ) -> GeneralGemmSymbolicCompilationUnitV1 {
        let input = StageSnapshotV1::new(
            CompilerStageV1::FrontendInput,
            SnapshotIdentityV1::from_untrusted_bytes(identity(fixture.input_byte)),
            SnapshotFormatIdentityV1::from_untrusted_bytes(identity(0x18)),
            vec![fixture.input_byte],
        )
        .unwrap();
        let obligations = general_gemm_symbolic_obligation_set_identity_v1(&input, &frontend);
        let request = CompileRequestV1::new(
            RequestIdentityV1::from_untrusted_bytes(identity(fixture.request_byte)),
            KernelInstanceIdentityV1::from_untrusted_bytes(*frontend.kernel_instance_identity()),
            CompilerProfileIdentityV1::from_untrusted_bytes(identity(fixture.compiler_byte)),
            TargetProfileIdentityV1::from_untrusted_bytes(identity(fixture.target_byte)),
            general_gemm_symbolic_pipeline_configuration_identity_v1(schedule),
            obligations,
            PipelineSelectorV1::PlironV1,
            input,
            fixture.compile_limits,
        )
        .unwrap();
        GeneralGemmSymbolicCompilationUnitV1::checked(
            &request,
            frontend,
            schedule,
            fixture.lowering_limits,
        )
        .unwrap()
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
    fn pair_join_requires_reference_then_vectorized_owner_order() {
        let reference = GeneralGemmScheduleV1::ReferenceWave64Xor4V1;
        let vectorized = GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1;
        require_schedule_order(reference, reference, 0).unwrap();
        require_schedule_order(vectorized, vectorized, 1).unwrap();
        assert_eq!(
            require_schedule_order(vectorized, reference, 0),
            Err(GeneralGemmFinalJoinErrorV1::ScheduleOrder { index: 0 })
        );
        assert_eq!(
            require_schedule_order(reference, vectorized, 1),
            Err(GeneralGemmFinalJoinErrorV1::ScheduleOrder { index: 1 })
        );
    }

    #[test]
    fn pair_join_rejects_every_caller_variable_schedule_invariant() {
        let reference = symbolic_unit_for(
            GeneralGemmScheduleV1::ReferenceWave64Xor4V1,
            frontend_binding(0x12, 0x41),
            canonical_symbolic_fixture(),
        );
        let canonical_vectorized = || {
            symbolic_unit_for(
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                frontend_binding(0x12, 0x41),
                canonical_symbolic_fixture(),
            )
        };
        require_pair_request_consistency(&reference, &canonical_vectorized()).unwrap();

        let substitutions = [
            symbolic_unit_for(
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                frontend_binding(0x12, 0x51),
                canonical_symbolic_fixture(),
            ),
            symbolic_unit_for(
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                frontend_binding(0x12, 0x41),
                SymbolicUnitFixtureV1 {
                    request_byte: 0x21,
                    ..canonical_symbolic_fixture()
                },
            ),
            symbolic_unit_for(
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                frontend_binding(0x12, 0x41),
                SymbolicUnitFixtureV1 {
                    input_byte: 0x27,
                    ..canonical_symbolic_fixture()
                },
            ),
            symbolic_unit_for(
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                frontend_binding(0x12, 0x41),
                SymbolicUnitFixtureV1 {
                    compiler_byte: 0x23,
                    ..canonical_symbolic_fixture()
                },
            ),
            symbolic_unit_for(
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                frontend_binding(0x12, 0x41),
                SymbolicUnitFixtureV1 {
                    target_byte: 0x24,
                    ..canonical_symbolic_fixture()
                },
            ),
            symbolic_unit_for(
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                frontend_binding(0x12, 0x41),
                SymbolicUnitFixtureV1 {
                    compile_limits: CompileLimitsV1::new(15, 16, 16, 4096, 16_384, 4096).unwrap(),
                    ..canonical_symbolic_fixture()
                },
            ),
            symbolic_unit_for(
                GeneralGemmScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
                frontend_binding(0x12, 0x41),
                SymbolicUnitFixtureV1 {
                    lowering_limits: GeneralGemmLoweringLimitsV1::new(4095, 32).unwrap(),
                    ..canonical_symbolic_fixture()
                },
            ),
        ];
        for vectorized in substitutions {
            assert_eq!(
                require_pair_request_consistency(&reference, &vectorized),
                Err(GeneralGemmFinalJoinErrorV1::PairRequestSubstitution)
            );
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

        let mutations: [MachineFactMutationV1; 5] = [
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
