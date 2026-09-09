//! Canonical, authority-free capability obligations and analysis results.

use alloc::vec::Vec;
mod codec;

use codec::validate_subject;

use crate::{
    ArtifactIdentityV1, DigestV1, EvidenceIdentityV1, ExactToolIdentityV1, StatementIdentityV1,
};

/// Exact wire version for inert capability-obligation sets.
pub const CAPABILITY_OBLIGATION_SET_VERSION_V1: u16 = 1;
/// Exact wire version for inert capability-result sets.
pub const CAPABILITY_RESULT_SET_VERSION_V1: u16 = 1;
/// Exact wire version that adds checked-analysis and refinement-receipt outcomes.
pub const CAPABILITY_RESULT_SET_VERSION_V2: u16 = 2;
/// Maximum obligations in one kernel-root capability contract.
pub const MAX_CAPABILITY_OBLIGATIONS_V1: usize = 128;
/// Maximum canonical bytes in one capability-obligation set.
pub const MAX_CAPABILITY_OBLIGATION_SET_BYTES_V1: usize = 64 * 1024;
/// Maximum bytes in one bounded rejection witness.
pub const MAX_CAPABILITY_REJECTED_WITNESS_BYTES_V1: usize = 4 * 1024;
/// Maximum aggregate witness bytes retained by one result set.
pub const MAX_CAPABILITY_WITNESS_BYTES_V1: usize = 256 * 1024;
/// Maximum canonical bytes in one capability-result set.
pub const MAX_CAPABILITY_RESULT_SET_BYTES_V1: usize = 1024 * 1024;

const OBLIGATION_MAGIC_V1: [u8; 8] = *b"FE2OCAPO";
const RESULT_MAGIC_V1: [u8; 8] = *b"FE2OCAPR";
const HEADER_BYTES_V1: usize = 16;
const SUBJECT_BYTES_V1: usize = 5 * 32 + 8;
const STABLE_ID_BYTES_V1: usize = 40;
const OBLIGATION_RECORD_BYTES_V1: usize = STABLE_ID_BYTES_V1 + 32 + 32;
const TERMINAL_IDENTITY_BYTES_V1: usize = 32;
const OBLIGATION_PREAMBLE_BYTES_V1: usize = HEADER_BYTES_V1 + SUBJECT_BYTES_V1 + 4;
const RESULT_PREAMBLE_BYTES_V1: usize = HEADER_BYTES_V1 + SUBJECT_BYTES_V1 + 32 + 4;
const MIN_RESULT_RECORD_BYTES_V1: usize = 32 + 32 + 4 + STABLE_ID_BYTES_V1 + 4 + 1;

const OBLIGATION_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/INERT-CAPABILITY-OBLIGATION/V1\0";
const OBLIGATION_SET_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/INERT-CAPABILITY-OBLIGATION-SET/V1\0";
const RESULT_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/INERT-CAPABILITY-RESULT/V1\0";
const RESULT_SET_IDENTITY_DOMAIN_V1: &[u8] = b"FE2O3/INERT-CAPABILITY-RESULT-SET/V1\0";
const RESULT_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/INERT-CAPABILITY-RESULT/V2\0";
const RESULT_SET_IDENTITY_DOMAIN_V2: &[u8] = b"FE2O3/INERT-CAPABILITY-RESULT-SET/V2\0";

const fn namespace_digest(bytes: &[u8]) -> DigestV1 {
    let mut digest = [0_u8; 32];
    let mut index = 0;
    while index < bytes.len() {
        digest[index] = bytes[index];
        index += 1;
    }
    DigestV1::from_untrusted_bytes(digest)
}

/// Namespace for the built-in V1 capability properties.
pub const CAPABILITY_PROPERTY_NAMESPACE_V1: DigestV1 =
    namespace_digest(b"FE2O3/CAPABILITY/PROPERTY/V1");
/// Namespace for the built-in V1 capability diagnostics.
pub const CAPABILITY_DIAGNOSTIC_NAMESPACE_V1: DigestV1 =
    namespace_digest(b"FE2O3/CAPABILITY/DIAG/V1");

macro_rules! capability_identity {
    ($(#[$meta:meta])* $name:ident) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
        #[repr(transparent)]
        pub struct $name(DigestV1);

        impl $name {
            /// Constructs an opaque, unauthenticated identity for later validation.
            pub const fn from_untrusted_digest(digest: DigestV1) -> Self {
                Self(digest)
            }

            /// Returns the exact opaque digest.
            pub const fn digest(self) -> DigestV1 {
                self.0
            }

            const fn is_valid(self) -> bool {
                !self.0.is_zero()
            }
        }
    };
}

capability_identity!(
    /// Exact attributed kernel identity.
    KernelIdentityV1
);
capability_identity!(
    /// Exact physical kernel-root identity.
    KernelRootIdentityV1
);
capability_identity!(
    /// Exact executable canonical-KIR identity.
    ExecutableKirIdentityV1
);
capability_identity!(
    /// Exact selected target-model identity.
    TargetModelIdentityV1
);
capability_identity!(
    /// Exact static or concrete dynamic launch-contract identity.
    LaunchContractIdentityV1
);
capability_identity!(
    /// Domain-separated identity of one exact capability obligation.
    CapabilityObligationIdentityV1
);
capability_identity!(
    /// Domain-separated identity of one exact capability result.
    CapabilityResultIdentityV1
);
capability_identity!(
    /// Domain-separated identity of one canonical obligation set.
    InertCapabilityObligationSetIdentityV1
);
capability_identity!(
    /// Domain-separated identity of one canonical result set.
    InertCapabilityResultSetIdentityV1
);
capability_identity!(
    /// Exact identity of the checker that produced one checked-analysis result.
    CapabilityCheckerIdentityV1
);
capability_identity!(
    /// Exact identity of the complete analysis report containing one checked result.
    CapabilityAnalysisReportIdentityV1
);

/// The refinement boundary named by an exact receipt-backed result.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum CapabilityRefinementKindV1 {
    SourceMirToKir,
    Machine,
}

/// Exact domain-separated coordinates of a typed refinement receipt.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CapabilityRefinementReceiptIdentityV1 {
    digest: DigestV1,
    byte_len: u64,
}

impl CapabilityRefinementReceiptIdentityV1 {
    /// Constructs opaque receipt coordinates for validation by the owning sealed composer.
    pub const fn from_untrusted_parts(digest: DigestV1, byte_len: u64) -> Self {
        Self { digest, byte_len }
    }

    pub const fn digest(self) -> DigestV1 {
        self.digest
    }

    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    const fn is_valid(self) -> bool {
        !self.digest.is_zero() && self.byte_len != 0
    }
}

/// Exact immutable subject shared by an obligation set and its results.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub struct CapabilitySubjectV1 {
    kernel: KernelIdentityV1,
    root: KernelRootIdentityV1,
    executable_kir: ExecutableKirIdentityV1,
    executable_kir_epoch: u64,
    target_model: TargetModelIdentityV1,
    launch_contract: LaunchContractIdentityV1,
}

impl CapabilitySubjectV1 {
    /// Builds one exact subject. Epoch zero is reserved for an absent graph.
    pub fn new(
        kernel: KernelIdentityV1,
        root: KernelRootIdentityV1,
        executable_kir: ExecutableKirIdentityV1,
        executable_kir_epoch: u64,
        target_model: TargetModelIdentityV1,
        launch_contract: LaunchContractIdentityV1,
    ) -> Result<Self, CapabilityCodecErrorV1> {
        let subject = Self {
            kernel,
            root,
            executable_kir,
            executable_kir_epoch,
            target_model,
            launch_contract,
        };
        validate_subject(subject)?;
        Ok(subject)
    }

    pub const fn kernel(self) -> KernelIdentityV1 {
        self.kernel
    }

    pub const fn root(self) -> KernelRootIdentityV1 {
        self.root
    }

    pub const fn executable_kir(self) -> ExecutableKirIdentityV1 {
        self.executable_kir
    }

    pub const fn executable_kir_epoch(self) -> u64 {
        self.executable_kir_epoch
    }

    pub const fn target_model(self) -> TargetModelIdentityV1 {
        self.target_model
    }

    pub const fn launch_contract(self) -> LaunchContractIdentityV1 {
        self.launch_contract
    }
}

/// Stable, namespaced, versioned capability-property identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CapabilityPropertyIdV1 {
    namespace: DigestV1,
    schema_version: u16,
    code: u32,
}

impl CapabilityPropertyIdV1 {
    pub const TYPING: Self = Self::builtin(1);
    pub const BOUNDS: Self = Self::builtin(2);
    pub const INITIALIZATION: Self = Self::builtin(3);
    pub const HIERARCHICAL_OWNERSHIP: Self = Self::builtin(4);
    pub const DATA_RACE_FREEDOM: Self = Self::builtin(5);
    pub const ATOMIC_LEGALITY: Self = Self::builtin(6);
    pub const UNIFORMITY: Self = Self::builtin(7);
    pub const BARRIER_CONVERGENCE: Self = Self::builtin(8);
    pub const WORKGROUP_MEMORY_EPOCHS: Self = Self::builtin(9);
    pub const TENSOR_LAYOUT: Self = Self::builtin(10);
    pub const EFFECTS: Self = Self::builtin(11);
    pub const RESOURCE_LEGALITY: Self = Self::builtin(12);
    pub const TARGET_CAPABILITY_CLOSURE: Self = Self::builtin(13);
    pub const SOURCE_MIR_TO_KIR_REFINEMENT: Self = Self::builtin(14);
    pub const MACHINE_REFINEMENT: Self = Self::builtin(15);
    pub const DYNAMIC_LAUNCH_PRECONDITIONS: Self = Self::builtin(16);

    pub const fn new(namespace: DigestV1, schema_version: u16, code: u32) -> Self {
        Self {
            namespace,
            schema_version,
            code,
        }
    }

    const fn builtin(code: u32) -> Self {
        Self::new(CAPABILITY_PROPERTY_NAMESPACE_V1, 1, code)
    }

    pub const fn namespace(self) -> DigestV1 {
        self.namespace
    }

    pub const fn schema_version(self) -> u16 {
        self.schema_version
    }

    pub const fn code(self) -> u32 {
        self.code
    }

    const fn is_valid(self) -> bool {
        !self.namespace.is_zero() && self.schema_version != 0 && self.code != 0
    }
}

/// Stable, namespaced, versioned capability diagnostic identifier.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct CapabilityDiagnosticIdV1 {
    namespace: DigestV1,
    schema_version: u16,
    code: u32,
}

impl CapabilityDiagnosticIdV1 {
    pub const REJECTED_WITH_WITNESS: Self = Self::builtin(1);
    pub const INCOMPLETE_ANALYSIS: Self = Self::builtin(2);
    pub const UNSUPPORTED_CAPABILITY: Self = Self::builtin(3);
    pub const UNREVIEWED_CAPABILITY: Self = Self::builtin(4);

    pub const fn new(namespace: DigestV1, schema_version: u16, code: u32) -> Self {
        Self {
            namespace,
            schema_version,
            code,
        }
    }

    const fn builtin(code: u32) -> Self {
        Self::new(CAPABILITY_DIAGNOSTIC_NAMESPACE_V1, 1, code)
    }

    pub const fn namespace(self) -> DigestV1 {
        self.namespace
    }

    pub const fn schema_version(self) -> u16 {
        self.schema_version
    }

    pub const fn code(self) -> u32 {
        self.code
    }

    const fn is_valid(self) -> bool {
        !self.namespace.is_zero() && self.schema_version != 0 && self.code != 0
    }
}

/// Caller-facing specification from which an exact obligation is derived.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityObligationSpecV1 {
    property: CapabilityPropertyIdV1,
    statement: StatementIdentityV1,
}

impl CapabilityObligationSpecV1 {
    pub const fn new(property: CapabilityPropertyIdV1, statement: StatementIdentityV1) -> Self {
        Self {
            property,
            statement,
        }
    }

    pub const fn property(self) -> CapabilityPropertyIdV1 {
        self.property
    }

    pub const fn statement(self) -> StatementIdentityV1 {
        self.statement
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityObligationV1 {
    identity: CapabilityObligationIdentityV1,
    property: CapabilityPropertyIdV1,
    statement: StatementIdentityV1,
}

impl CapabilityObligationV1 {
    pub const fn identity(self) -> CapabilityObligationIdentityV1 {
        self.identity
    }

    pub const fn property(self) -> CapabilityPropertyIdV1 {
        self.property
    }

    pub const fn statement(self) -> StatementIdentityV1 {
        self.statement
    }
}

/// Canonical, bounded capability obligations for one exact subject.
///
/// Construction and decoding establish internal consistency only. This type
/// does not authenticate the producer or grant any downstream authority.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCapabilityObligationSetV1 {
    subject: CapabilitySubjectV1,
    obligations: Vec<CapabilityObligationV1>,
    identity: InertCapabilityObligationSetIdentityV1,
    canonical_bytes: Vec<u8>,
}

/// Exact authority-free result classes carried from analysis and refinement stages.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityOutcomeKindV1 {
    Proven,
    Checked,
    RefinementReceipt,
    Rejected,
    Incomplete,
    Unsupported,
    Unreviewed,
}

/// Authority-free evidence or failure detail for one obligation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CapabilityOutcomeV1 {
    Proven {
        evidence: EvidenceIdentityV1,
        tool: ExactToolIdentityV1,
        proof_artifact: ArtifactIdentityV1,
    },
    /// A static checker discharged its documented obligation on one exact graph epoch.
    ///
    /// This is not proof evidence and never grants authority by itself.
    Checked {
        evidence: EvidenceIdentityV1,
        checker: CapabilityCheckerIdentityV1,
        report: CapabilityAnalysisReportIdentityV1,
        executable_kir: ExecutableKirIdentityV1,
        executable_kir_epoch: u64,
        analysis_epoch: u64,
    },
    /// An exact typed receipt crosses one independently owned refinement boundary.
    ///
    /// The sealed composer must decode and validate the receipt bytes; these coordinates alone
    /// are inert and are not a `Proven` result.
    RefinementReceipt {
        kind: CapabilityRefinementKindV1,
        receipt: CapabilityRefinementReceiptIdentityV1,
    },
    Rejected {
        diagnostic: CapabilityDiagnosticIdV1,
        witness: Vec<u8>,
    },
    Incomplete {
        diagnostic: CapabilityDiagnosticIdV1,
        detail: ArtifactIdentityV1,
    },
    Unsupported {
        diagnostic: CapabilityDiagnosticIdV1,
        detail: ArtifactIdentityV1,
    },
    Unreviewed {
        diagnostic: CapabilityDiagnosticIdV1,
        detail: ArtifactIdentityV1,
    },
}

impl CapabilityOutcomeV1 {
    pub const fn kind(&self) -> CapabilityOutcomeKindV1 {
        match self {
            Self::Proven { .. } => CapabilityOutcomeKindV1::Proven,
            Self::Checked { .. } => CapabilityOutcomeKindV1::Checked,
            Self::RefinementReceipt { .. } => CapabilityOutcomeKindV1::RefinementReceipt,
            Self::Rejected { .. } => CapabilityOutcomeKindV1::Rejected,
            Self::Incomplete { .. } => CapabilityOutcomeKindV1::Incomplete,
            Self::Unsupported { .. } => CapabilityOutcomeKindV1::Unsupported,
            Self::Unreviewed { .. } => CapabilityOutcomeKindV1::Unreviewed,
        }
    }
}

/// Caller-facing result paired with one exact obligation identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityResultSpecV1 {
    obligation: CapabilityObligationIdentityV1,
    outcome: CapabilityOutcomeV1,
}

impl CapabilityResultSpecV1 {
    pub const fn new(
        obligation: CapabilityObligationIdentityV1,
        outcome: CapabilityOutcomeV1,
    ) -> Self {
        Self {
            obligation,
            outcome,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CapabilityResultV1 {
    identity: CapabilityResultIdentityV1,
    obligation: CapabilityObligationIdentityV1,
    outcome: CapabilityOutcomeV1,
}

impl CapabilityResultV1 {
    pub const fn identity(&self) -> CapabilityResultIdentityV1 {
        self.identity
    }

    pub const fn obligation(&self) -> CapabilityObligationIdentityV1 {
        self.obligation
    }

    pub const fn outcome(&self) -> &CapabilityOutcomeV1 {
        &self.outcome
    }
}

/// Canonical, bounded results for one exact obligation set and subject.
///
/// Even an all-`Proven` set remains inert. Only the independently sealed
/// verifier owned by issue #213 may consume authenticated evidence and decide
/// whether another authority-bearing transition is justified.
#[derive(Debug, Eq, PartialEq)]
pub struct InertCapabilityResultSetV1 {
    schema_version: u16,
    subject: CapabilitySubjectV1,
    obligation_set: InertCapabilityObligationSetIdentityV1,
    results: Vec<CapabilityResultV1>,
    identity: InertCapabilityResultSetIdentityV1,
    canonical_bytes: Vec<u8>,
}

/// Checks exact subject and one-to-one obligation/result composition.
///
/// Success is structural consistency, not proof authentication or authority.
pub fn validate_capability_composition_v1(
    expected_subject: CapabilitySubjectV1,
    obligations: &InertCapabilityObligationSetV1,
    results: &InertCapabilityResultSetV1,
) -> Result<(), CapabilityCompositionErrorV1> {
    if let Some(field) = subject_mismatch(expected_subject, obligations.subject) {
        return Err(CapabilityCompositionErrorV1::ObligationSubjectMismatch(
            field,
        ));
    }
    if let Some(field) = subject_mismatch(expected_subject, results.subject) {
        return Err(CapabilityCompositionErrorV1::ResultSubjectMismatch(field));
    }
    if obligations.identity != results.obligation_set {
        return Err(CapabilityCompositionErrorV1::ObligationSetIdentityMismatch);
    }
    if obligations.obligations.len() != results.results.len() {
        return Err(CapabilityCompositionErrorV1::ResultCountMismatch {
            expected: obligations.obligations.len(),
            actual: results.results.len(),
        });
    }
    for (index, (obligation, result)) in obligations
        .obligations
        .iter()
        .zip(&results.results)
        .enumerate()
    {
        if obligation.identity != result.obligation {
            return Err(CapabilityCompositionErrorV1::ObligationResultMismatch { index });
        }
    }
    Ok(())
}

fn subject_mismatch(
    expected: CapabilitySubjectV1,
    actual: CapabilitySubjectV1,
) -> Option<CapabilitySubjectFieldV1> {
    if expected.kernel != actual.kernel {
        Some(CapabilitySubjectFieldV1::Kernel)
    } else if expected.root != actual.root {
        Some(CapabilitySubjectFieldV1::Root)
    } else if expected.executable_kir != actual.executable_kir {
        Some(CapabilitySubjectFieldV1::ExecutableKir)
    } else if expected.executable_kir_epoch != actual.executable_kir_epoch {
        Some(CapabilitySubjectFieldV1::ExecutableKirEpoch)
    } else if expected.target_model != actual.target_model {
        Some(CapabilitySubjectFieldV1::TargetModel)
    } else if expected.launch_contract != actual.launch_contract {
        Some(CapabilitySubjectFieldV1::LaunchContract)
    } else {
        None
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityRecordKindV1 {
    ObligationSet,
    Obligation,
    ResultSet,
    Result,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityResourceV1 {
    Obligations,
    Results,
    ObligationSetBytes,
    ResultSetBytes,
    RejectedWitnessBytes,
    AggregateWitnessBytes,
    ResultIdentityPreimageBytes,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityIdentityFieldV1 {
    Kernel,
    Root,
    ExecutableKir,
    TargetModel,
    LaunchContract,
    Property,
    Statement,
    Diagnostic,
    Obligation,
    ObligationSet,
    Result,
    ResultSet,
    Evidence,
    Tool,
    Artifact,
    Checker,
    Report,
    Receipt,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilitySubjectFieldV1 {
    Kernel,
    Root,
    ExecutableKir,
    ExecutableKirEpoch,
    TargetModel,
    LaunchContract,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityCodecErrorV1 {
    LimitExceeded {
        resource: CapabilityResourceV1,
        actual: usize,
        maximum: usize,
    },
    AllocationFailure {
        resource: CapabilityResourceV1,
    },
    InvalidMagic {
        record: CapabilityRecordKindV1,
    },
    UnsupportedVersion {
        record: CapabilityRecordKindV1,
        version: u16,
    },
    UnsupportedFlags {
        record: CapabilityRecordKindV1,
        flags: u16,
    },
    NonzeroReserved {
        record: CapabilityRecordKindV1,
        index: Option<usize>,
    },
    Truncated,
    TrailingBytes,
    LengthOverflow,
    EmptyObligations,
    EmptyWitness {
        index: usize,
    },
    InvalidKirEpoch,
    InvalidIdentity {
        field: CapabilityIdentityFieldV1,
        index: Option<usize>,
    },
    InvalidStableIdentifier {
        field: CapabilityIdentityFieldV1,
        index: usize,
    },
    UnknownOutcome {
        index: usize,
        tag: u8,
    },
    DuplicateProperty {
        index: usize,
    },
    DuplicateObligation {
        index: usize,
    },
    DuplicateResult {
        index: usize,
    },
    NonCanonicalOrder {
        record: CapabilityRecordKindV1,
        index: usize,
    },
    IdentityMismatch {
        record: CapabilityRecordKindV1,
        index: Option<usize>,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CapabilityCompositionErrorV1 {
    ObligationSubjectMismatch(CapabilitySubjectFieldV1),
    ResultSubjectMismatch(CapabilitySubjectFieldV1),
    ObligationSetIdentityMismatch,
    ResultCountMismatch { expected: usize, actual: usize },
    ObligationResultMismatch { index: usize },
}
