//! Inert identity reconciliation for monomorphization-dead observations.

use std::fmt;

use fe2o3_artifacts::DigestAlgorithm;
use fe2o3_rustc_front::{
    CONSTANT_FOLD_POLICY_VERSION_V1, DeadBranchContextV1, MonomorphizationDeadEvidenceIdentityV1,
    MonomorphizationDeadEvidenceV1,
};

use crate::Digest;

pub const MONOMORPHIZATION_DEAD_BINDING_VERSION_V1: u16 = 1;
pub const MONOMORPHIZATION_DEAD_BINDING_DOMAIN_V1: [u8; 8] = *b"FE2MDBB\0";

/// A caller-authored reference to one exact dead-branch observation.
///
/// This value is descriptive. It cannot authorize collection, panic filtering,
/// address-space filtering, lowering, loading, or launch.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MonomorphizationDeadClaimV1 {
    policy_version: u16,
    context: DeadBranchContextV1,
    evidence_identity: MonomorphizationDeadEvidenceIdentityV1,
}

impl MonomorphizationDeadClaimV1 {
    pub const fn new(
        policy_version: u16,
        context: DeadBranchContextV1,
        evidence_identity: MonomorphizationDeadEvidenceIdentityV1,
    ) -> Self {
        Self {
            policy_version,
            context,
            evidence_identity,
        }
    }

    pub fn from_evidence(evidence: &MonomorphizationDeadEvidenceV1) -> Self {
        Self::new(
            evidence.policy_version(),
            evidence.context(),
            evidence.identity(),
        )
    }

    pub const fn policy_version(self) -> u16 {
        self.policy_version
    }

    pub const fn context(self) -> DeadBranchContextV1 {
        self.context
    }

    pub const fn evidence_identity(self) -> MonomorphizationDeadEvidenceIdentityV1 {
        self.evidence_identity
    }

    pub const fn grants_compiler_authority(self) -> bool {
        false
    }
}

/// Canonical identity join between a compiler observation and an exact caller
/// reference to that observation.
///
/// The join detects substitution and drift. It remains evidence only and does
/// not turn either public input into compiler authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MonomorphizationDeadIdentityBindingV1 {
    policy_version: u16,
    context: DeadBranchContextV1,
    evidence_identity: MonomorphizationDeadEvidenceIdentityV1,
    evidence_byte_len: u32,
    binding_identity: Digest,
}

impl MonomorphizationDeadIdentityBindingV1 {
    pub const fn version(self) -> u16 {
        MONOMORPHIZATION_DEAD_BINDING_VERSION_V1
    }

    pub const fn policy_version(self) -> u16 {
        self.policy_version
    }

    pub const fn context(self) -> DeadBranchContextV1 {
        self.context
    }

    pub const fn evidence_identity(self) -> MonomorphizationDeadEvidenceIdentityV1 {
        self.evidence_identity
    }

    pub const fn evidence_byte_len(self) -> u32 {
        self.evidence_byte_len
    }

    pub const fn binding_identity(self) -> Digest {
        self.binding_identity
    }

    pub fn validate_against(
        &self,
        actual: &Self,
    ) -> Result<(), MonomorphizationDeadBindingErrorV1> {
        if self != actual {
            return Err(MonomorphizationDeadBindingErrorV1::BindingMismatch);
        }
        Ok(())
    }

    pub const fn grants_compiler_authority(self) -> bool {
        false
    }

    pub const fn grants_panic_exclusion_authority(self) -> bool {
        false
    }

    pub const fn grants_address_space_exclusion_authority(self) -> bool {
        false
    }

    pub const fn grants_load_authority(self) -> bool {
        false
    }

    pub const fn grants_launch_authority(self) -> bool {
        false
    }
}

pub fn reconcile_monomorphization_dead_evidence_v1(
    observation: &MonomorphizationDeadEvidenceV1,
    claim: MonomorphizationDeadClaimV1,
) -> Result<MonomorphizationDeadIdentityBindingV1, MonomorphizationDeadBindingErrorV1> {
    if observation.policy_version() != CONSTANT_FOLD_POLICY_VERSION_V1 {
        return Err(
            MonomorphizationDeadBindingErrorV1::UnsupportedObservationPolicy {
                version: observation.policy_version(),
            },
        );
    }
    if claim.policy_version != observation.policy_version() {
        return Err(MonomorphizationDeadBindingErrorV1::PolicyVersionMismatch {
            observed: observation.policy_version(),
            claimed: claim.policy_version,
        });
    }

    let observed = observation.context();
    let claimed = claim.context;
    if observed.function_identity() != claimed.function_identity() {
        return Err(MonomorphizationDeadBindingErrorV1::FunctionIdentityMismatch);
    }
    if observed.cfg_identity() != claimed.cfg_identity() {
        return Err(MonomorphizationDeadBindingErrorV1::CfgIdentityMismatch);
    }
    if observed.source_identity() != claimed.source_identity() {
        return Err(MonomorphizationDeadBindingErrorV1::SourceIdentityMismatch);
    }
    if observation.identity() != claim.evidence_identity {
        return Err(MonomorphizationDeadBindingErrorV1::EvidenceIdentityMismatch);
    }

    let evidence_byte_len = u32::try_from(observation.canonical_bytes().len())
        .map_err(|_| MonomorphizationDeadBindingErrorV1::EvidenceTooLarge)?;
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&MONOMORPHIZATION_DEAD_BINDING_DOMAIN_V1);
    bytes.extend_from_slice(&MONOMORPHIZATION_DEAD_BINDING_VERSION_V1.to_le_bytes());
    bytes.extend_from_slice(&observation.policy_version().to_le_bytes());
    bytes.extend_from_slice(&observed.function_identity());
    bytes.extend_from_slice(&observed.cfg_identity());
    bytes.extend_from_slice(&observed.source_identity());
    bytes.extend_from_slice(&observation.identity().as_bytes());
    bytes.extend_from_slice(&evidence_byte_len.to_le_bytes());
    let binding_identity = sha256(&bytes);

    Ok(MonomorphizationDeadIdentityBindingV1 {
        policy_version: observation.policy_version(),
        context: observed,
        evidence_identity: observation.identity(),
        evidence_byte_len,
        binding_identity,
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MonomorphizationDeadBindingErrorV1 {
    UnsupportedObservationPolicy { version: u16 },
    PolicyVersionMismatch { observed: u16, claimed: u16 },
    FunctionIdentityMismatch,
    CfgIdentityMismatch,
    SourceIdentityMismatch,
    EvidenceIdentityMismatch,
    EvidenceTooLarge,
    BindingMismatch,
}

impl fmt::Display for MonomorphizationDeadBindingErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedObservationPolicy { version } => {
                write!(
                    formatter,
                    "compiler observation uses unsupported policy {version}"
                )
            }
            Self::PolicyVersionMismatch { observed, claimed } => write!(
                formatter,
                "dead-branch policy claim {claimed} disagrees with observation {observed}"
            ),
            Self::FunctionIdentityMismatch => {
                formatter.write_str("dead-branch function identity was substituted")
            }
            Self::CfgIdentityMismatch => {
                formatter.write_str("dead-branch CFG identity was substituted")
            }
            Self::SourceIdentityMismatch => {
                formatter.write_str("dead-branch source identity was substituted")
            }
            Self::EvidenceIdentityMismatch => {
                formatter.write_str("dead-branch decision evidence was substituted")
            }
            Self::EvidenceTooLarge => {
                formatter.write_str("dead-branch evidence length exceeds u32")
            }
            Self::BindingMismatch => {
                formatter.write_str("dead-branch identity binding was substituted")
            }
        }
    }
}

impl std::error::Error for MonomorphizationDeadBindingErrorV1 {}

fn sha256(bytes: &[u8]) -> Digest {
    let digest = DigestAlgorithm::Sha256.calculate(bytes);
    Digest::from_bytes(*digest.bytes().as_bytes())
}
