//! Verifier-side schedule and numerical evidence join for general GEMM.
//!
//! This crate cannot own the post-link machine token without introducing a
//! dependency cycle or a public attestation path. It therefore produces only
//! a linear schedule-plus-numerical token. hsaco-finalize owns the opaque exact
//! machine-inspection token. rustc-codegen must consume both tokens together
//! with its private frontend-correspondence capability in one transaction.

use core::fmt;

use sha2::{Digest as _, Sha256};

use crate::{
    AuthenticatedGeneralGemmNumericalPolicyV1, AuthenticatedGeneralGemmScheduleProofV1,
    GeneralGemmEvidenceIdentityV1, GeneralGemmNumericalPolicyRequestV1, GeneralGemmProofRequestV1,
    GeneralGemmProofScheduleV1, GeneralGemmPropertyEvidenceV1,
};

/// Number of descriptive identity domains compared at the eventual protected join.
pub const GENERAL_GEMM_FINAL_JOIN_IDENTITY_COUNT_V1: usize = 31;

const PROOF_NUMERICAL_DOMAIN_V1: &[u8] = b"fe2o3.general-gemm.proof-numerical-evidence.v1\0";

/// One exact identity domain compared across compile and protected-launch joins.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum GeneralGemmFinalJoinIdentityFieldV1 {
    /// Aggregate runtime-parameterized symbolic compilation.
    SymbolicCompilation = 0,
    /// Independently domain-separated schedule profile.
    Schedule = 1,
    /// Canonical symbolic host-plan schema.
    SymbolicPlan = 2,
    /// Canonical symbolic semantic-KIR template.
    SymbolicKir = 3,
    /// Compiler request.
    CompileRequest = 4,
    /// Required semantic obligation set.
    ObligationSet = 5,
    /// Compiler profile.
    CompilerProfile = 6,
    /// gfx942 target profile.
    TargetProfile = 7,
    /// Reviewed compiler toolchain route.
    ToolchainRoute = 8,
    /// Separate concrete plan/KIR/runtime-ABI launch instantiation.
    CheckedLaunchInstantiation = 9,
    /// Structural frontend binding later authenticated by rustc-codegen.
    FrontendSemanticBinding = 10,
    /// Shared BF16/FP32 numerical policy.
    NumericalPolicy = 11,
    /// Exact frontend ABI retained from the rustc receipt.
    FrontendAbi = 12,
    /// Owner-checked Pliron projection.
    PlironProjection = 13,
    /// Typed gfx942 Handoff V2 graph.
    HandoffV2 = 14,
    /// Deterministic LLVM assembly bytes.
    LlvmAssembly = 15,
    /// Inert compiler-module handoff.
    CompilerModuleHandoff = 16,
    /// Retained general-GEMM machine binding section.
    MachineBindingSection = 17,
    /// Compiler-owned kernel descriptor source.
    DescriptorSource = 18,
    /// Aggregate measured worker executable/build/LLVM commitment.
    MeasuredWorker = 19,
    /// First-build Worker V2 execution identity.
    WorkerExecution = 20,
    /// Sealed Worker V2 response.
    SealedWorkerResponse = 21,
    /// Raw worker-produced HSACO content.
    RawHsacoContent = 22,
    /// Raw post-worker inspection result.
    RawMachineInspection = 23,
    /// Exact machine-inspection policy.
    MachineInspectionPolicy = 24,
    /// Exact linked `.text` section.
    TextSection = 25,
    /// Finalized HSACO record.
    FinalizedHsaco = 26,
    /// Finalized HSACO content.
    FinalizedHsacoContent = 27,
    /// Finalized kernel descriptor.
    FinalizedDescriptor = 28,
    /// Aggregate target/resource/ISA inspection result.
    TargetResourceIsaInspection = 29,
    /// Opaque finalizer-owned aggregate machine inspection.
    AggregateMachineInspection = 30,
}

/// Stable order of every eventual final-join identity field.
pub const GENERAL_GEMM_FINAL_JOIN_IDENTITY_FIELDS_V1: [GeneralGemmFinalJoinIdentityFieldV1;
    GENERAL_GEMM_FINAL_JOIN_IDENTITY_COUNT_V1] = [
    GeneralGemmFinalJoinIdentityFieldV1::SymbolicCompilation,
    GeneralGemmFinalJoinIdentityFieldV1::Schedule,
    GeneralGemmFinalJoinIdentityFieldV1::SymbolicPlan,
    GeneralGemmFinalJoinIdentityFieldV1::SymbolicKir,
    GeneralGemmFinalJoinIdentityFieldV1::CompileRequest,
    GeneralGemmFinalJoinIdentityFieldV1::ObligationSet,
    GeneralGemmFinalJoinIdentityFieldV1::CompilerProfile,
    GeneralGemmFinalJoinIdentityFieldV1::TargetProfile,
    GeneralGemmFinalJoinIdentityFieldV1::ToolchainRoute,
    GeneralGemmFinalJoinIdentityFieldV1::CheckedLaunchInstantiation,
    GeneralGemmFinalJoinIdentityFieldV1::FrontendSemanticBinding,
    GeneralGemmFinalJoinIdentityFieldV1::NumericalPolicy,
    GeneralGemmFinalJoinIdentityFieldV1::FrontendAbi,
    GeneralGemmFinalJoinIdentityFieldV1::PlironProjection,
    GeneralGemmFinalJoinIdentityFieldV1::HandoffV2,
    GeneralGemmFinalJoinIdentityFieldV1::LlvmAssembly,
    GeneralGemmFinalJoinIdentityFieldV1::CompilerModuleHandoff,
    GeneralGemmFinalJoinIdentityFieldV1::MachineBindingSection,
    GeneralGemmFinalJoinIdentityFieldV1::DescriptorSource,
    GeneralGemmFinalJoinIdentityFieldV1::MeasuredWorker,
    GeneralGemmFinalJoinIdentityFieldV1::WorkerExecution,
    GeneralGemmFinalJoinIdentityFieldV1::SealedWorkerResponse,
    GeneralGemmFinalJoinIdentityFieldV1::RawHsacoContent,
    GeneralGemmFinalJoinIdentityFieldV1::RawMachineInspection,
    GeneralGemmFinalJoinIdentityFieldV1::MachineInspectionPolicy,
    GeneralGemmFinalJoinIdentityFieldV1::TextSection,
    GeneralGemmFinalJoinIdentityFieldV1::FinalizedHsaco,
    GeneralGemmFinalJoinIdentityFieldV1::FinalizedHsacoContent,
    GeneralGemmFinalJoinIdentityFieldV1::FinalizedDescriptor,
    GeneralGemmFinalJoinIdentityFieldV1::TargetResourceIsaInspection,
    GeneralGemmFinalJoinIdentityFieldV1::AggregateMachineInspection,
];

impl GeneralGemmFinalJoinIdentityFieldV1 {
    const fn index(self) -> usize {
        self as usize
    }
}

/// Descriptive identity registry for the eventual three-owner final join.
///
/// This caller-constructible value only supports exact substitution checks. It
/// does not authenticate any producer and grants no proof, artifact,
/// publication, loading, or launch authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GeneralGemmFinalJoinIdentityRegistryV1 {
    schedule: GeneralGemmProofScheduleV1,
    identities: [GeneralGemmEvidenceIdentityV1; GENERAL_GEMM_FINAL_JOIN_IDENTITY_COUNT_V1],
}

impl GeneralGemmFinalJoinIdentityRegistryV1 {
    /// Checks nonzero, cross-domain-distinct identities in stable field order.
    pub fn checked(
        schedule: GeneralGemmProofScheduleV1,
        identities: [GeneralGemmEvidenceIdentityV1; GENERAL_GEMM_FINAL_JOIN_IDENTITY_COUNT_V1],
    ) -> Result<Self, GeneralGemmFinalJoinIdentityRegistryErrorV1> {
        if identities
            .iter()
            .any(|identity| identity.as_bytes() == &[0; 32])
        {
            return Err(GeneralGemmFinalJoinIdentityRegistryErrorV1::InvalidIdentity);
        }
        if identities
            .iter()
            .enumerate()
            .any(|(index, identity)| identities[..index].contains(identity))
        {
            return Err(GeneralGemmFinalJoinIdentityRegistryErrorV1::DuplicateIdentity);
        }
        Ok(Self {
            schedule,
            identities,
        })
    }

    /// Returns the separately identified schedule profile.
    pub const fn schedule(self) -> GeneralGemmProofScheduleV1 {
        self.schedule
    }

    /// Returns one raw identity without granting producer authority.
    pub const fn identity(
        self,
        field: GeneralGemmFinalJoinIdentityFieldV1,
    ) -> GeneralGemmEvidenceIdentityV1 {
        self.identities[field.index()]
    }

    /// Returns every raw identity in stable field order.
    pub const fn identities(
        self,
    ) -> [GeneralGemmEvidenceIdentityV1; GENERAL_GEMM_FINAL_JOIN_IDENTITY_COUNT_V1] {
        self.identities
    }

    /// Compares two descriptive registries without authenticating either one.
    pub fn require_exact(
        self,
        actual: Self,
    ) -> Result<(), GeneralGemmFinalJoinIdentityRegistryErrorV1> {
        if self.schedule != actual.schedule {
            return Err(GeneralGemmFinalJoinIdentityRegistryErrorV1::ScheduleSubstitution);
        }
        for field in GENERAL_GEMM_FINAL_JOIN_IDENTITY_FIELDS_V1 {
            if self.identity(field) != actual.identity(field) {
                return Err(
                    GeneralGemmFinalJoinIdentityRegistryErrorV1::IdentitySubstitution { field },
                );
            }
        }
        Ok(())
    }

    /// Descriptive registries never grant final-join authority.
    pub const fn grants_final_join_authority(self) -> bool {
        false
    }
}

/// Failure while constructing or comparing a descriptive final-join registry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmFinalJoinIdentityRegistryErrorV1 {
    /// A required raw identity was zero.
    InvalidIdentity,
    /// Two independently owned identity domains reused the same bytes.
    DuplicateIdentity,
    /// Two registries name different schedule profiles.
    ScheduleSubstitution,
    /// Two registries differ in one exact identity domain.
    IdentitySubstitution {
        /// Substituted domain.
        field: GeneralGemmFinalJoinIdentityFieldV1,
    },
}

impl fmt::Display for GeneralGemmFinalJoinIdentityRegistryErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "general GEMM final-join registry failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmFinalJoinIdentityRegistryErrorV1 {}

/// Linear, non-admitting verifier evidence for one schedule and numerical policy.
///
/// The retained property records include open, model-definition-only, and
/// weaker exact-real results. Consequently this value cannot enter a compiler
/// proof gate and must never be interpreted as discharged source, numerical
/// machine, or machine-refinement proof. Its private fields only authenticate
/// provenance of the evidence; they do not promote its authority level.
///
/// ```compile_fail
/// use fe2o3_verifier::{
///     GeneralGemmEvidenceIdentityV1, GeneralGemmProofAndNumericalEvidenceV1,
///     GeneralGemmProofRequestV1,
/// };
/// fn forge(request: GeneralGemmProofRequestV1) -> GeneralGemmProofAndNumericalEvidenceV1 {
///     GeneralGemmProofAndNumericalEvidenceV1 {
///         request,
///         schedule_proof_identity: GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([1; 32]),
///         numerical_policy_evidence_identity:
///             GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([2; 32]),
///         properties: todo!(),
///         identity: GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([3; 32]),
///     }
/// }
/// ```
#[derive(Debug)]
#[must_use = "proof/numerical evidence must be consumed by the three-owner final join"]
pub struct GeneralGemmProofAndNumericalEvidenceV1 {
    request: GeneralGemmProofRequestV1,
    schedule_proof_identity: GeneralGemmEvidenceIdentityV1,
    numerical_policy_evidence_identity: GeneralGemmEvidenceIdentityV1,
    properties: [GeneralGemmPropertyEvidenceV1; 12],
    identity: GeneralGemmEvidenceIdentityV1,
}

impl GeneralGemmProofAndNumericalEvidenceV1 {
    /// Returns every exact compiler identity bound by the schedule proof.
    pub const fn request(&self) -> GeneralGemmProofRequestV1 {
        self.request
    }

    /// Returns the exact pinned schedule-proof evidence identity.
    pub const fn schedule_proof_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.schedule_proof_identity
    }

    /// Returns the exact shared numerical-policy evidence identity.
    pub const fn numerical_policy_evidence_identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.numerical_policy_evidence_identity
    }

    /// Returns every property record without changing its authority status.
    pub const fn properties(&self) -> &[GeneralGemmPropertyEvidenceV1; 12] {
        &self.properties
    }

    /// Returns the aggregate verifier evidence identity.
    pub const fn identity(&self) -> GeneralGemmEvidenceIdentityV1 {
        self.identity
    }

    /// Open and weaker property records prohibit compiler proof-gate entry.
    pub const fn can_enter_compiler_proof_gate(&self) -> bool {
        false
    }

    /// Evidence alone grants no artifact, publication, load, or launch authority.
    pub const fn grants_artifact_or_runtime_authority(&self) -> bool {
        false
    }
}

/// Failure while joining typed schedule and numerical verifier evidence.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GeneralGemmProofAndNumericalEvidenceErrorV1 {
    /// Numerical evidence names another exact compiler identity.
    IdentitySubstitution {
        /// Substituted domain.
        field: GeneralGemmFinalJoinIdentityFieldV1,
    },
}

impl fmt::Display for GeneralGemmProofAndNumericalEvidenceErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            formatter,
            "general GEMM proof/numerical evidence failed: {self:?}"
        )
    }
}

impl std::error::Error for GeneralGemmProofAndNumericalEvidenceErrorV1 {}

/// Joins one pinned schedule result and one shared numerical-policy witness.
///
/// This operation preserves every open/weaker property status and produces
/// non-admitting evidence. It does not close any frontend or machine boundary.
pub fn join_general_gemm_proof_and_numerical_evidence_v1(
    schedule_proof: AuthenticatedGeneralGemmScheduleProofV1,
    numerical_policy: AuthenticatedGeneralGemmNumericalPolicyV1,
) -> Result<GeneralGemmProofAndNumericalEvidenceV1, GeneralGemmProofAndNumericalEvidenceErrorV1> {
    let request = schedule_proof.request();
    validate_numerical_request(request, numerical_policy.request())?;
    let schedule_proof_identity = schedule_proof.identity();
    let properties = *schedule_proof.properties();
    let numerical_policy_evidence_identity = numerical_policy.identity();
    let mut hasher = Sha256::new();
    hasher.update(PROOF_NUMERICAL_DOMAIN_V1);
    hasher.update([schedule_tag(request.schedule())]);
    for identity in schedule_request_identities(request) {
        hasher.update(identity.as_bytes());
    }
    hasher.update(schedule_proof_identity.as_bytes());
    hasher.update(numerical_policy_evidence_identity.as_bytes());
    for property in properties {
        hasher.update(property.identity().as_bytes());
    }
    Ok(GeneralGemmProofAndNumericalEvidenceV1 {
        request,
        schedule_proof_identity,
        numerical_policy_evidence_identity,
        properties,
        identity: GeneralGemmEvidenceIdentityV1::from_untrusted_bytes(hasher.finalize().into()),
    })
}

fn validate_numerical_request(
    schedule: GeneralGemmProofRequestV1,
    numerical: GeneralGemmNumericalPolicyRequestV1,
) -> Result<(), GeneralGemmProofAndNumericalEvidenceErrorV1> {
    for (field, expected, actual) in [
        (
            GeneralGemmFinalJoinIdentityFieldV1::SymbolicCompilation,
            schedule.symbolic_compilation_identity(),
            numerical.symbolic_compilation_identity(),
        ),
        (
            GeneralGemmFinalJoinIdentityFieldV1::SymbolicPlan,
            schedule.symbolic_plan_identity(),
            numerical.symbolic_plan_identity(),
        ),
        (
            GeneralGemmFinalJoinIdentityFieldV1::SymbolicKir,
            schedule.symbolic_kir_identity(),
            numerical.symbolic_kir_identity(),
        ),
        (
            GeneralGemmFinalJoinIdentityFieldV1::NumericalPolicy,
            schedule.numerical_policy_identity(),
            numerical.numerical_policy_identity(),
        ),
    ] {
        if expected != actual {
            return Err(
                GeneralGemmProofAndNumericalEvidenceErrorV1::IdentitySubstitution { field },
            );
        }
    }
    Ok(())
}

const fn schedule_request_identities(
    request: GeneralGemmProofRequestV1,
) -> [GeneralGemmEvidenceIdentityV1; 11] {
    [
        request.schedule_identity(),
        request.symbolic_plan_identity(),
        request.symbolic_kir_identity(),
        request.symbolic_compilation_identity(),
        request.compile_request_identity(),
        request.obligation_set_identity(),
        request.compiler_identity(),
        request.target_identity(),
        request.toolchain_identity(),
        request.source_template_identity(),
        request.numerical_policy_identity(),
    ]
}

const fn schedule_tag(schedule: GeneralGemmProofScheduleV1) -> u8 {
    match schedule {
        GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1 => 1,
        GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1 => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(seed: u8) -> GeneralGemmEvidenceIdentityV1 {
        GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([seed; 32])
    }

    fn registry_identities()
    -> [GeneralGemmEvidenceIdentityV1; GENERAL_GEMM_FINAL_JOIN_IDENTITY_COUNT_V1] {
        core::array::from_fn(|index| identity((index + 1) as u8))
    }

    fn registry() -> GeneralGemmFinalJoinIdentityRegistryV1 {
        GeneralGemmFinalJoinIdentityRegistryV1::checked(
            GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
            registry_identities(),
        )
        .unwrap()
    }

    fn proof_request(offset: u8) -> GeneralGemmProofRequestV1 {
        GeneralGemmProofRequestV1::checked(
            GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
            identity(offset + 1),
            identity(offset + 2),
            identity(offset + 3),
            identity(offset + 4),
            identity(offset + 5),
            identity(offset + 6),
            identity(offset + 7),
            identity(offset + 8),
            identity(offset + 9),
            identity(offset + 10),
            identity(offset + 11),
        )
        .unwrap()
    }

    fn numerical_request(proof: GeneralGemmProofRequestV1) -> GeneralGemmNumericalPolicyRequestV1 {
        GeneralGemmNumericalPolicyRequestV1::checked(
            proof.symbolic_compilation_identity(),
            proof.symbolic_plan_identity(),
            proof.symbolic_kir_identity(),
            proof.numerical_policy_identity(),
        )
        .unwrap()
    }

    #[test]
    fn raw_registry_rejects_zero_duplicate_and_grants_no_authority() {
        let mut raw = registry_identities();
        raw[7] = GeneralGemmEvidenceIdentityV1::from_untrusted_bytes([0; 32]);
        assert_eq!(
            GeneralGemmFinalJoinIdentityRegistryV1::checked(
                GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
                raw,
            ),
            Err(GeneralGemmFinalJoinIdentityRegistryErrorV1::InvalidIdentity)
        );
        let mut raw = registry_identities();
        raw[30] = raw[4];
        assert_eq!(
            GeneralGemmFinalJoinIdentityRegistryV1::checked(
                GeneralGemmProofScheduleV1::ReferenceWave64Xor4V1,
                raw,
            ),
            Err(GeneralGemmFinalJoinIdentityRegistryErrorV1::DuplicateIdentity)
        );
        assert!(!registry().grants_final_join_authority());
    }

    #[test]
    fn every_final_join_identity_substitution_is_detected() {
        let expected = registry();
        expected.require_exact(expected).unwrap();
        for field in GENERAL_GEMM_FINAL_JOIN_IDENTITY_FIELDS_V1 {
            let mut substituted = expected.identities();
            substituted[field.index()] = identity(100 + field.index() as u8);
            let actual =
                GeneralGemmFinalJoinIdentityRegistryV1::checked(expected.schedule(), substituted)
                    .unwrap();
            assert_eq!(
                expected.require_exact(actual),
                Err(GeneralGemmFinalJoinIdentityRegistryErrorV1::IdentitySubstitution { field }),
                "accepted field {field:?}"
            );
        }
        let changed_schedule = GeneralGemmFinalJoinIdentityRegistryV1::checked(
            GeneralGemmProofScheduleV1::VectorizedAOnlyBf16GlobalTransferV1,
            expected.identities(),
        )
        .unwrap();
        assert_eq!(
            expected.require_exact(changed_schedule),
            Err(GeneralGemmFinalJoinIdentityRegistryErrorV1::ScheduleSubstitution)
        );
    }

    #[test]
    fn every_numerical_identity_substitution_is_detected() {
        let proof = proof_request(0);
        let numerical = numerical_request(proof);
        validate_numerical_request(proof, numerical).unwrap();
        for field in [
            GeneralGemmFinalJoinIdentityFieldV1::SymbolicCompilation,
            GeneralGemmFinalJoinIdentityFieldV1::SymbolicPlan,
            GeneralGemmFinalJoinIdentityFieldV1::SymbolicKir,
            GeneralGemmFinalJoinIdentityFieldV1::NumericalPolicy,
        ] {
            let changed_proof = proof_request(32);
            let changed = match field {
                GeneralGemmFinalJoinIdentityFieldV1::SymbolicCompilation => {
                    GeneralGemmNumericalPolicyRequestV1::checked(
                        changed_proof.symbolic_compilation_identity(),
                        numerical.symbolic_plan_identity(),
                        numerical.symbolic_kir_identity(),
                        numerical.numerical_policy_identity(),
                    )
                }
                GeneralGemmFinalJoinIdentityFieldV1::SymbolicPlan => {
                    GeneralGemmNumericalPolicyRequestV1::checked(
                        numerical.symbolic_compilation_identity(),
                        changed_proof.symbolic_plan_identity(),
                        numerical.symbolic_kir_identity(),
                        numerical.numerical_policy_identity(),
                    )
                }
                GeneralGemmFinalJoinIdentityFieldV1::SymbolicKir => {
                    GeneralGemmNumericalPolicyRequestV1::checked(
                        numerical.symbolic_compilation_identity(),
                        numerical.symbolic_plan_identity(),
                        changed_proof.symbolic_kir_identity(),
                        numerical.numerical_policy_identity(),
                    )
                }
                GeneralGemmFinalJoinIdentityFieldV1::NumericalPolicy => {
                    GeneralGemmNumericalPolicyRequestV1::checked(
                        numerical.symbolic_compilation_identity(),
                        numerical.symbolic_plan_identity(),
                        numerical.symbolic_kir_identity(),
                        changed_proof.numerical_policy_identity(),
                    )
                }
                _ => unreachable!(),
            }
            .unwrap();
            assert_eq!(
                validate_numerical_request(proof, changed),
                Err(GeneralGemmProofAndNumericalEvidenceErrorV1::IdentitySubstitution { field }),
                "accepted field {field:?}"
            );
        }
    }
}
