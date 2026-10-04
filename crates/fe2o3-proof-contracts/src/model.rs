use alloc::vec::Vec;

use crate::{
    ArtifactIdentityV1, CorrespondenceIdentityV1, DigestV1, EvidenceIdentityV1,
    ExactInputIdentityV1, ExactModelIdentityV1, ExactToolIdentityV1, ObligationIdentityV1,
    PropertyIdentityV1, StatementIdentityV1, TcbEntryIdentityV1,
};

/// Independent property classifications. No variant implies any other variant.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PropertyKindV1 {
    MemorySafety,
    DataRaceFreedom,
    SynchronizationSafety,
    BarrierConvergence,
    AtomicSafety,
    FunctionalCorrectness,
    Termination,
    Progress,
    ResourceBounds,
    QueueSafety,
    LeaseSafety,
    GenerationSafety,
    ProofErasureCorrespondence,
    Extension { namespace: DigestV1, code: u16 },
}

/// Exact reported authority for one property.
///
/// This enum deliberately has no ordering operation. For example, Proved is
/// not accepted where Validated or Checked was requested.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PropertyStatusV1 {
    Proved,
    Validated,
    Contracted,
    Checked,
    Unsupported,
}

/// Common identity binding carried by every status-specific evidence record.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct EvidenceBindingV1 {
    pub identity: EvidenceIdentityV1,
    pub property: PropertyIdentityV1,
    pub statement: StatementIdentityV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProvedEvidenceV1 {
    pub binding: EvidenceBindingV1,
    pub input: ExactInputIdentityV1,
    pub model: ExactModelIdentityV1,
    pub tool: ExactToolIdentityV1,
    pub proof_artifact: ArtifactIdentityV1,
    pub correspondence: CorrespondenceIdentityV1,
    pub trusted_computing_base: Vec<TcbEntryIdentityV1>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ValidatedEvidenceV1 {
    pub binding: EvidenceBindingV1,
    pub input: ExactInputIdentityV1,
    pub model: ExactModelIdentityV1,
    pub tool: ExactToolIdentityV1,
    pub validation_artifact: ArtifactIdentityV1,
    pub trusted_computing_base: Vec<TcbEntryIdentityV1>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ContractedEvidenceV1 {
    pub binding: EvidenceBindingV1,
    pub contract_artifact: ArtifactIdentityV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CheckedEvidenceV1 {
    pub binding: EvidenceBindingV1,
    pub input: ExactInputIdentityV1,
    pub tool: ExactToolIdentityV1,
    pub check_artifact: ArtifactIdentityV1,
    pub trusted_computing_base: Vec<TcbEntryIdentityV1>,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum UnsupportedReasonV1 {
    NoModel,
    NoTool,
    UnsupportedConstruct,
    UnresolvedCorrespondence,
    OutsideDeclaredScope,
    Extension { namespace: DigestV1, code: u16 },
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct UnsupportedEvidenceV1 {
    pub binding: EvidenceBindingV1,
    pub reason: UnsupportedReasonV1,
    pub rationale_artifact: ArtifactIdentityV1,
}

/// Evidence variants are one-to-one with property statuses.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PropertyEvidenceV1 {
    Proved(ProvedEvidenceV1),
    Validated(ValidatedEvidenceV1),
    Contracted(ContractedEvidenceV1),
    Checked(CheckedEvidenceV1),
    Unsupported(UnsupportedEvidenceV1),
}

impl PropertyEvidenceV1 {
    pub const fn status(&self) -> PropertyStatusV1 {
        match self {
            Self::Proved(_) => PropertyStatusV1::Proved,
            Self::Validated(_) => PropertyStatusV1::Validated,
            Self::Contracted(_) => PropertyStatusV1::Contracted,
            Self::Checked(_) => PropertyStatusV1::Checked,
            Self::Unsupported(_) => PropertyStatusV1::Unsupported,
        }
    }

    pub const fn binding(&self) -> EvidenceBindingV1 {
        match self {
            Self::Proved(record) => record.binding,
            Self::Validated(record) => record.binding,
            Self::Contracted(record) => record.binding,
            Self::Checked(record) => record.binding,
            Self::Unsupported(record) => record.binding,
        }
    }

    pub fn trusted_computing_base(&self) -> &[TcbEntryIdentityV1] {
        match self {
            Self::Proved(record) => &record.trusted_computing_base,
            Self::Validated(record) => &record.trusted_computing_base,
            Self::Checked(record) => &record.trusted_computing_base,
            Self::Contracted(_) | Self::Unsupported(_) => &[],
        }
    }

    pub const fn exact_tool(&self) -> Option<ExactToolIdentityV1> {
        match self {
            Self::Proved(record) => Some(record.tool),
            Self::Validated(record) => Some(record.tool),
            Self::Checked(record) => Some(record.tool),
            Self::Contracted(_) | Self::Unsupported(_) => None,
        }
    }
}

/// One independently evidenced property.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PropertyRecordV1 {
    pub identity: PropertyIdentityV1,
    pub kind: PropertyKindV1,
    pub statement: StatementIdentityV1,
    pub status: PropertyStatusV1,
    pub evidence: PropertyEvidenceV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TcbEntryKindV1 {
    Tool,
    ModelAssumption,
    CompilerAssumption,
    RuntimeAssumption,
    HardwareAssumption,
    HumanReview,
    Extension { namespace: DigestV1, code: u16 },
}

/// One explicit trusted-computing-base dependency.
///
/// Tool entries carry the exact tool identity they represent. Other entries
/// must leave exact_tool empty.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct TcbEntryV1 {
    pub identity: TcbEntryIdentityV1,
    pub kind: TcbEntryKindV1,
    pub component: ArtifactIdentityV1,
    pub exact_tool: Option<ExactToolIdentityV1>,
    pub rationale: ArtifactIdentityV1,
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CorrespondenceKindV1 {
    ProofErasure,
    SourceToModel,
    ModelToExecutable,
    SourceToExecutable,
    Extension { namespace: DigestV1, code: u16 },
}

/// An exact, property-local relation between two input representations.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CorrespondenceReferenceV1 {
    pub identity: CorrespondenceIdentityV1,
    pub kind: CorrespondenceKindV1,
    pub property: PropertyIdentityV1,
    pub statement: StatementIdentityV1,
    pub from: ExactInputIdentityV1,
    pub to: ExactInputIdentityV1,
    pub witness_artifact: ArtifactIdentityV1,
}

/// Exact evidence cited to satisfy one obligation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ObligationSatisfactionV1 {
    pub evidence: EvidenceIdentityV1,
    pub property: PropertyIdentityV1,
    pub statement: StatementIdentityV1,
    pub status: PropertyStatusV1,
}

/// A bounded-set member describing one required property status.
///
/// Open obligations are structurally valid, but validate_closed rejects them.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct ObligationRecordV1 {
    pub identity: ObligationIdentityV1,
    pub property: PropertyIdentityV1,
    pub statement: StatementIdentityV1,
    pub required_status: PropertyStatusV1,
    pub satisfaction: Option<ObligationSatisfactionV1>,
}

/// Canonically ordered, bounded records for one contract set.
///
/// These vectors are untrusted wire-model inputs. Validation checks their
/// bounds and canonical order before a consumer relies on their structure.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ContractSetV1 {
    pub properties: Vec<PropertyRecordV1>,
    pub obligations: Vec<ObligationRecordV1>,
    pub trusted_computing_base: Vec<TcbEntryV1>,
    pub correspondences: Vec<CorrespondenceReferenceV1>,
}
