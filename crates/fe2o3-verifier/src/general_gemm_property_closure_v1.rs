//! Non-admitting closure requests for the general GEMM proof properties.
//!
//! The verifier can authenticate its own schedule and numerical evidence, but
//! it cannot authenticate rustc-codegen frontend correspondence or finalizer
//! machine inspection without reversing the dependency graph or accepting a
//! forgeable public bridge. This module therefore consumes verifier evidence
//! and reports the exact typed confirmations that the owning rustc transaction
//! must join. It never accepts a public identity registry and grants no proof,
//! artifact, publication, load, launch, or execution authority.

use core::fmt;

use crate::{
    GENERAL_GEMM_PROOF_PROPERTIES_V1, GeneralGemmEvidenceIdentityV1,
    GeneralGemmProofAndNumericalEvidenceV1, GeneralGemmProofPropertyV1, GeneralGemmProofRequestV1,
    GeneralGemmPropertyEvidenceV1,
};

/// Number of independently evaluated general GEMM property records.
pub const GENERAL_GEMM_PROPERTY_CLOSURE_COUNT_V1: usize = 12;

/// Exact frontend-owned confirmation needed to connect source/KIR to a model claim.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmSourcePropertyConfirmationKindV1 {
    /// Allocation extents, address spaces, borrow provenance, and access lifetimes.
    AllocationAndProvenance = 1,
    /// Every modeled global access corresponds to a guarded source/KIR access.
    GuardedGlobalAccesses = 2,
    /// Complete LDS writes and zero-fill dominate every staged read.
    LdsWriteReadInitialization = 3,
    /// Global and LDS effects exclude conflicting writers and read/write overlap.
    EffectConflictFreedom = 4,
    /// Imported control flow makes every required barrier dynamically convergent.
    ControlFlowBarrierConvergence = 5,
    /// Lane/tile ownership maps injectively to every live output element.
    OutputOwnership = 6,
    /// Publish, consume, and reuse operations correspond to the modeled LDS epochs.
    LdsLifecycle = 7,
    /// Imported accumulator updates correspond to every modeled K-prefix step.
    AccumulatorPhase = 8,
    /// M/N/K tails correspond to guarded loads, zero-fill, and suppressed stores.
    MaskedTail = 9,
    /// The source/KIR epilogue corresponds to alpha/beta and prior-C semantics.
    AlphaBetaEpilogue = 10,
    /// Source/KIR operation order corresponds to the declared numerical policy.
    NumericalOperationOrder = 11,
}

/// Exact finalizer-owned confirmation needed at the emitted-machine boundary.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmMachinePropertyConfirmationKindV1 {
    /// Post-link BF16 widening, FP32/MFMA rounding/order, and epilogue policy.
    Bf16Fp32MfmaAndEpiloguePolicy = 1,
    /// Exact post-link KIR-to-ISA, memory, control-flow, descriptor, and resource refinement.
    KirToGfx942MachineRefinement = 2,
}

/// One property-local open closure request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmPropertyClosureRequestV1 {
    evidence: GeneralGemmPropertyEvidenceV1,
    source_confirmation: Option<GeneralGemmSourcePropertyConfirmationKindV1>,
    machine_confirmation: Option<GeneralGemmMachinePropertyConfirmationKindV1>,
}

impl GeneralGemmPropertyClosureRequestV1 {
    /// Returns the exact verifier property evidence without promoting its status.
    pub const fn schedule_evidence(self) -> GeneralGemmPropertyEvidenceV1 {
        self.evidence
    }

    /// Returns the kind of opaque frontend confirmation rustc must consume.
    pub const fn source_confirmation(self) -> Option<GeneralGemmSourcePropertyConfirmationKindV1> {
        self.source_confirmation
    }

    /// Returns the kind of opaque finalizer confirmation rustc must consume.
    pub const fn machine_confirmation(
        self,
    ) -> Option<GeneralGemmMachinePropertyConfirmationKindV1> {
        self.machine_confirmation
    }

    /// A request names missing evidence and is never itself a closed property.
    pub const fn is_closed(self) -> bool {
        false
    }
}

/// Consumed verifier evidence evaluated into twelve exact open-property requests.
///
/// This value is deliberately not `Clone`. Its private fields preserve the
/// provenance of the verifier evidence while making no claim about source or
/// machine producers. Only rustc-codegen can later consume the owning opaque
/// capabilities and decide whether every request was satisfied in one compile
/// transaction.
///
/// ```compile_fail
/// use fe2o3_verifier::{
///     GeneralGemmPropertyClosureEvaluationV1, GeneralGemmProofRequestV1,
/// };
/// fn forge(proof_request: GeneralGemmProofRequestV1) -> GeneralGemmPropertyClosureEvaluationV1 {
///     GeneralGemmPropertyClosureEvaluationV1 { proof_request, ..todo!() }
/// }
/// ```
///
/// ```compile_fail
/// use fe2o3_verifier::GeneralGemmPropertyClosureEvaluationV1;
/// fn duplicate(evaluation: &GeneralGemmPropertyClosureEvaluationV1) {
///     let _forged_second_owner = (*evaluation).clone();
/// }
/// ```
#[derive(Debug)]
#[must_use = "property closure requests must be joined to opaque source and machine confirmations"]
pub struct GeneralGemmPropertyClosureEvaluationV1 {
    proof_request: GeneralGemmProofRequestV1,
    proof_and_numerical_evidence_identity: GeneralGemmEvidenceIdentityV1,
    schedule_proof_identity: GeneralGemmEvidenceIdentityV1,
    numerical_policy_evidence_identity: GeneralGemmEvidenceIdentityV1,
    requests: [GeneralGemmPropertyClosureRequestV1; GENERAL_GEMM_PROPERTY_CLOSURE_COUNT_V1],
}

impl GeneralGemmPropertyClosureEvaluationV1 {
    /// Returns the exact symbolic compilation and schedule proof request.
    pub const fn proof_request(&self) -> GeneralGemmProofRequestV1 {
        self.proof_request
    }

    /// Returns the consumed verifier proof/numerical aggregate identity.
    pub const fn proof_and_numerical_evidence_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.proof_and_numerical_evidence_identity
    }

    /// Returns the independently executed schedule-proof identity.
    pub const fn schedule_proof_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.schedule_proof_identity
    }

    /// Returns the independently evaluated numerical-policy identity.
    pub const fn numerical_policy_evidence_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.numerical_policy_evidence_identity
    }

    /// Returns all twelve property-local closure requests in canonical order.
    pub const fn requests(
        &self,
    ) -> &[GeneralGemmPropertyClosureRequestV1; GENERAL_GEMM_PROPERTY_CLOSURE_COUNT_V1] {
        &self.requests
    }

    /// Missing owner-authenticated confirmations prohibit proof-gate entry.
    pub const fn can_enter_compiler_proof_gate(&self) -> bool {
        false
    }

    /// A closure checklist grants no artifact or execution authority.
    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Verifier evidence did not contain the canonical twelve-property sequence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmPropertyClosureEvaluationErrorV1 {
    index: usize,
    expected: GeneralGemmProofPropertyV1,
    actual: GeneralGemmProofPropertyV1,
}

impl GeneralGemmPropertyClosureEvaluationErrorV1 {
    /// Returns the rejected record index.
    pub const fn index(self) -> usize {
        self.index
    }

    /// Returns the expected canonical property.
    pub const fn expected(self) -> GeneralGemmProofPropertyV1 {
        self.expected
    }

    /// Returns the substituted property.
    pub const fn actual(self) -> GeneralGemmProofPropertyV1 {
        self.actual
    }
}

impl fmt::Display for GeneralGemmPropertyClosureEvaluationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "general GEMM property {} was {:?}, expected {:?}",
            self.index, self.actual, self.expected
        )
    }
}

impl std::error::Error for GeneralGemmPropertyClosureEvaluationErrorV1 {}

/// Consumes verifier-owned evidence and enumerates every still-open owner input.
///
/// The returned requests do not accept or authenticate source/finalizer data.
/// The eventual rustc-codegen join must consume those owners' opaque tokens,
/// compare the exact symbolic compilation and artifact identities, and satisfy
/// each request independently.
pub fn evaluate_general_gemm_property_closure_v1(
    evidence: GeneralGemmProofAndNumericalEvidenceV1,
) -> Result<GeneralGemmPropertyClosureEvaluationV1, GeneralGemmPropertyClosureEvaluationErrorV1> {
    for (index, (actual, expected)) in evidence
        .properties()
        .iter()
        .zip(GENERAL_GEMM_PROOF_PROPERTIES_V1)
        .enumerate()
    {
        if actual.property() != expected {
            return Err(GeneralGemmPropertyClosureEvaluationErrorV1 {
                index,
                expected,
                actual: actual.property(),
            });
        }
    }
    let requests =
        core::array::from_fn(|index| property_closure_request(evidence.properties()[index]));
    Ok(GeneralGemmPropertyClosureEvaluationV1 {
        proof_request: evidence.request(),
        proof_and_numerical_evidence_identity: evidence.identity(),
        schedule_proof_identity: evidence.schedule_proof_identity(),
        numerical_policy_evidence_identity: evidence.numerical_policy_evidence_identity(),
        requests,
    })
}

const fn property_closure_request(
    evidence: GeneralGemmPropertyEvidenceV1,
) -> GeneralGemmPropertyClosureRequestV1 {
    let (source_confirmation, machine_confirmation) =
        property_confirmation_requirements(evidence.property());
    GeneralGemmPropertyClosureRequestV1 {
        evidence,
        source_confirmation,
        machine_confirmation,
    }
}

const fn property_confirmation_requirements(
    property: GeneralGemmProofPropertyV1,
) -> (
    Option<GeneralGemmSourcePropertyConfirmationKindV1>,
    Option<GeneralGemmMachinePropertyConfirmationKindV1>,
) {
    use GeneralGemmMachinePropertyConfirmationKindV1::{
        Bf16Fp32MfmaAndEpiloguePolicy, KirToGfx942MachineRefinement,
    };
    use GeneralGemmProofPropertyV1::*;
    use GeneralGemmSourcePropertyConfirmationKindV1::*;

    match property {
        MemorySafe => (Some(AllocationAndProvenance), None),
        BoundsSafe => (Some(GuardedGlobalAccesses), None),
        Initialized => (Some(LdsWriteReadInitialization), None),
        RaceFree => (Some(EffectConflictFreedom), None),
        BarrierConvergent => (Some(ControlFlowBarrierConvergence), None),
        OutputRegionInjective => (Some(OutputOwnership), None),
        LdsEpochCorrect => (Some(LdsLifecycle), None),
        AccumulatorPhaseRefinement => (Some(AccumulatorPhase), None),
        TailRefinement => (Some(MaskedTail), None),
        EpilogueRefinement => (Some(AlphaBetaEpilogue), None),
        NumericalContract => (
            Some(NumericalOperationOrder),
            Some(Bf16Fp32MfmaAndEpiloguePolicy),
        ),
        MachineRefinementBoundary => (None, Some(KirToGfx942MachineRefinement)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_property_has_an_explicit_owner_confirmation() {
        use GeneralGemmMachinePropertyConfirmationKindV1::{
            Bf16Fp32MfmaAndEpiloguePolicy, KirToGfx942MachineRefinement,
        };
        use GeneralGemmSourcePropertyConfirmationKindV1::*;

        let requirements = GENERAL_GEMM_PROOF_PROPERTIES_V1.map(property_confirmation_requirements);
        assert_eq!(GENERAL_GEMM_PROPERTY_CLOSURE_COUNT_V1, 12);
        assert_eq!(requirements.len(), GENERAL_GEMM_PROPERTY_CLOSURE_COUNT_V1);
        assert_eq!(
            requirements,
            [
                (Some(AllocationAndProvenance), None),
                (Some(GuardedGlobalAccesses), None),
                (Some(LdsWriteReadInitialization), None),
                (Some(EffectConflictFreedom), None),
                (Some(ControlFlowBarrierConvergence), None),
                (Some(OutputOwnership), None),
                (Some(LdsLifecycle), None),
                (Some(AccumulatorPhase), None),
                (Some(MaskedTail), None),
                (Some(AlphaBetaEpilogue), None),
                (
                    Some(NumericalOperationOrder),
                    Some(Bf16Fp32MfmaAndEpiloguePolicy),
                ),
                (None, Some(KirToGfx942MachineRefinement)),
            ]
        );
    }

    #[test]
    fn schedule_theorem_properties_still_require_source_correspondence() {
        assert_eq!(
            property_confirmation_requirements(GeneralGemmProofPropertyV1::BoundsSafe),
            (
                Some(GeneralGemmSourcePropertyConfirmationKindV1::GuardedGlobalAccesses),
                None,
            )
        );
    }
}
