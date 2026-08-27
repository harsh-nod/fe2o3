//! Sealed semantic-preservation contracts for the production PLIRON pipeline.
//!
//! A declaration states what a pass is allowed to do. Only exact identity
//! comparison around the actual pass can certify that the declaration held.

use std::fmt;

use crate::KernelCheckPassKindV1;

pub const MAX_PLIRON_PASS_CONTRACTS_V1: usize = 8;
pub const MAX_PLIRON_PASS_IDENTITY_CHECKPOINTS_V1: usize = 17;
pub const MAX_PLIRON_PASS_IDENTITY_BYTES_V1: usize = 16 * 1024 * 1024;

const EFFECT_PRESERVE_EXACT_IDENTITY_V1: u8 = 1;

/// The only effect admitted for the existing analysis-only verifier stages.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlironPassAllowedEffectV1 {
    PreserveExactSemanticIdentity,
}

/// One sealed production declaration. There is deliberately no public
/// constructor: callers can inspect the fixed contracts but cannot extend the
/// production session with arbitrary passes or effects.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlironPassContractV1 {
    pass: KernelCheckPassKindV1,
    allowed_effect: PlironPassAllowedEffectV1,
}

impl PlironPassContractV1 {
    const fn identity(pass: KernelCheckPassKindV1) -> Self {
        Self {
            pass,
            allowed_effect: PlironPassAllowedEffectV1::PreserveExactSemanticIdentity,
        }
    }

    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        self.pass
    }

    pub const fn allowed_effect(&self) -> PlironPassAllowedEffectV1 {
        self.allowed_effect
    }
}

/// Exact contract order admitted by the V2 production verifier pipeline.
pub const PRODUCTION_PLIRON_PASS_CONTRACTS_V1: [PlironPassContractV1;
    MAX_PLIRON_PASS_CONTRACTS_V1] = [
    PlironPassContractV1::identity(KernelCheckPassKindV1::TensorLayout),
    PlironPassContractV1::identity(KernelCheckPassKindV1::MemoryBounds),
    PlironPassContractV1::identity(KernelCheckPassKindV1::AtomicLegality),
    PlironPassContractV1::identity(KernelCheckPassKindV1::RaceFreedom),
    PlironPassContractV1::identity(KernelCheckPassKindV1::HierarchicalOwnership),
    PlironPassContractV1::identity(KernelCheckPassKindV1::BarrierConvergence),
    PlironPassContractV1::identity(KernelCheckPassKindV1::WorkgroupMemory),
    PlironPassContractV1::identity(KernelCheckPassKindV1::SemanticRefinement),
];

/// Compact label retained after the provider has compared canonical bytes.
/// The label is evidence lineage only; the checker never accepts digest
/// equality as a substitute for the provider's exact comparison.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlironSemanticIdentityLabelV1 {
    sha256: [u8; 32],
    canonical_len: usize,
}

impl PlironSemanticIdentityLabelV1 {
    pub(crate) const fn new(sha256: [u8; 32], canonical_len: usize) -> Self {
        Self {
            sha256,
            canonical_len,
        }
    }

    pub const fn sha256(&self) -> &[u8; 32] {
        &self.sha256
    }

    pub const fn canonical_len(&self) -> usize {
        self.canonical_len
    }
}

/// Move-only evidence that one actual pass preserved exact semantic identity.
#[derive(Debug, Eq, PartialEq)]
pub struct PlironPassPreservationCertificateV1 {
    pass: KernelCheckPassKindV1,
    identity: PlironSemanticIdentityLabelV1,
}

impl PlironPassPreservationCertificateV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        self.pass
    }

    pub const fn identity(&self) -> PlironSemanticIdentityLabelV1 {
        self.identity
    }
}

/// Move-only report issued only after the complete fixed sequence is checked.
#[derive(Debug, Eq, PartialEq)]
pub struct PlironPassPreservationReportV1 {
    input_identity: PlironSemanticIdentityLabelV1,
    output_identity: PlironSemanticIdentityLabelV1,
    certificates: Vec<PlironPassPreservationCertificateV1>,
}

impl PlironPassPreservationReportV1 {
    pub const fn input_identity(&self) -> PlironSemanticIdentityLabelV1 {
        self.input_identity
    }

    pub const fn output_identity(&self) -> PlironSemanticIdentityLabelV1 {
        self.output_identity
    }

    pub fn certificates(&self) -> &[PlironPassPreservationCertificateV1] {
        &self.certificates
    }

    pub fn is_exact_identity(&self) -> bool {
        self.input_identity == self.output_identity
            && self.certificates.len() == MAX_PLIRON_PASS_CONTRACTS_V1
    }
}

/// Stable fail-closed pass-manifest and session diagnostic. Codes below 020
/// are reserved for canonical snapshot and exact-comparison diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironPassPreservationErrorV1 {
    UnknownPassDeclaration {
        position: usize,
        declaration: u16,
    },
    UnknownEffectDeclaration {
        position: usize,
        declaration: u8,
    },
    PassOrderMismatch {
        position: usize,
        expected: KernelCheckPassKindV1,
        observed: KernelCheckPassKindV1,
    },
    DuplicatePassDeclaration {
        first_position: usize,
        duplicate_position: usize,
        pass: KernelCheckPassKindV1,
    },
    OmittedPassDeclaration {
        position: usize,
        pass: KernelCheckPassKindV1,
    },
    SemanticIdentityChanged {
        pass: KernelCheckPassKindV1,
        source_code: &'static str,
        detail: String,
    },
    StaleInputIdentity {
        pass: KernelCheckPassKindV1,
        source_code: &'static str,
        detail: String,
    },
    ResourceLimit {
        resource: &'static str,
        limit: usize,
        observed: usize,
    },
    IdentityUnavailable {
        detail: &'static str,
    },
    InvalidSessionState {
        detail: &'static str,
    },
}

impl PlironPassPreservationErrorV1 {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::UnknownPassDeclaration { .. } => "FE2O3-PRESERVE-020",
            Self::UnknownEffectDeclaration { .. } => "FE2O3-PRESERVE-021",
            Self::PassOrderMismatch { .. } => "FE2O3-PRESERVE-022",
            Self::DuplicatePassDeclaration { .. } => "FE2O3-PRESERVE-023",
            Self::OmittedPassDeclaration { .. } => "FE2O3-PRESERVE-024",
            Self::SemanticIdentityChanged { .. } => "FE2O3-PRESERVE-025",
            Self::StaleInputIdentity { .. } => "FE2O3-PRESERVE-026",
            Self::ResourceLimit { .. } => "FE2O3-PRESERVE-027",
            Self::IdentityUnavailable { .. } => "FE2O3-PRESERVE-028",
            Self::InvalidSessionState { .. } => "FE2O3-PRESERVE-029",
        }
    }
}

impl fmt::Display for PlironPassPreservationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error[{}]: ", self.code())?;
        match self {
            Self::UnknownPassDeclaration {
                position,
                declaration,
            } => write!(
                formatter,
                "unknown production pass declaration {declaration} at position {position}"
            ),
            Self::UnknownEffectDeclaration {
                position,
                declaration,
            } => write!(
                formatter,
                "unknown production pass effect {declaration} at position {position}"
            ),
            Self::PassOrderMismatch {
                position,
                expected,
                observed,
            } => write!(
                formatter,
                "production pass {observed:?} appears at position {position}; expected {expected:?}"
            ),
            Self::DuplicatePassDeclaration {
                first_position,
                duplicate_position,
                pass,
            } => write!(
                formatter,
                "production pass {pass:?} is duplicated at position {duplicate_position} (first declared at {first_position})"
            ),
            Self::OmittedPassDeclaration { position, pass } => write!(
                formatter,
                "production pass {pass:?} is omitted from required position {position}"
            ),
            Self::SemanticIdentityChanged {
                pass,
                source_code,
                detail,
            } => write!(
                formatter,
                "analysis-only pass {pass:?} changed semantic identity; error[{source_code}]: {detail}"
            ),
            Self::StaleInputIdentity {
                pass,
                source_code,
                detail,
            } => write!(
                formatter,
                "analysis-only pass {pass:?} received stale input; error[{source_code}]: {detail}"
            ),
            Self::ResourceLimit {
                resource,
                limit,
                observed,
            } => write!(
                formatter,
                "pass-preservation {resource} limit {limit} exceeded by observed value {observed}"
            ),
            Self::IdentityUnavailable { detail } => {
                write!(formatter, "semantic identity is unavailable: {detail}")
            }
            Self::InvalidSessionState { detail } => {
                write!(formatter, "invalid sealed pass session state: {detail}")
            }
        }
    }
}

impl std::error::Error for PlironPassPreservationErrorV1 {}

#[derive(Clone, Copy)]
struct RawPassContractV1 {
    pass: u16,
    effect: u8,
}

const fn pass_code(pass: KernelCheckPassKindV1) -> u16 {
    match pass {
        KernelCheckPassKindV1::TensorLayout => 1,
        KernelCheckPassKindV1::MemoryBounds => 2,
        KernelCheckPassKindV1::AtomicLegality => 3,
        KernelCheckPassKindV1::RaceFreedom => 4,
        KernelCheckPassKindV1::HierarchicalOwnership => 5,
        KernelCheckPassKindV1::BarrierConvergence => 6,
        KernelCheckPassKindV1::WorkgroupMemory => 7,
        KernelCheckPassKindV1::SemanticRefinement => 8,
        KernelCheckPassKindV1::Structural | KernelCheckPassKindV1::ControlFlow => 0,
    }
}

const PRODUCTION_RAW_PASS_CONTRACTS_V1: [RawPassContractV1; MAX_PLIRON_PASS_CONTRACTS_V1] = [
    RawPassContractV1 {
        pass: pass_code(KernelCheckPassKindV1::TensorLayout),
        effect: EFFECT_PRESERVE_EXACT_IDENTITY_V1,
    },
    RawPassContractV1 {
        pass: pass_code(KernelCheckPassKindV1::MemoryBounds),
        effect: EFFECT_PRESERVE_EXACT_IDENTITY_V1,
    },
    RawPassContractV1 {
        pass: pass_code(KernelCheckPassKindV1::AtomicLegality),
        effect: EFFECT_PRESERVE_EXACT_IDENTITY_V1,
    },
    RawPassContractV1 {
        pass: pass_code(KernelCheckPassKindV1::RaceFreedom),
        effect: EFFECT_PRESERVE_EXACT_IDENTITY_V1,
    },
    RawPassContractV1 {
        pass: pass_code(KernelCheckPassKindV1::HierarchicalOwnership),
        effect: EFFECT_PRESERVE_EXACT_IDENTITY_V1,
    },
    RawPassContractV1 {
        pass: pass_code(KernelCheckPassKindV1::BarrierConvergence),
        effect: EFFECT_PRESERVE_EXACT_IDENTITY_V1,
    },
    RawPassContractV1 {
        pass: pass_code(KernelCheckPassKindV1::WorkgroupMemory),
        effect: EFFECT_PRESERVE_EXACT_IDENTITY_V1,
    },
    RawPassContractV1 {
        pass: pass_code(KernelCheckPassKindV1::SemanticRefinement),
        effect: EFFECT_PRESERVE_EXACT_IDENTITY_V1,
    },
];

fn decode_pass(code: u16) -> Option<KernelCheckPassKindV1> {
    PRODUCTION_PLIRON_PASS_CONTRACTS_V1
        .iter()
        .map(PlironPassContractV1::pass)
        .find(|pass| pass_code(*pass) == code)
}

fn validate_declarations(
    declarations: &[RawPassContractV1],
) -> Result<(), PlironPassPreservationErrorV1> {
    if declarations.len() > MAX_PLIRON_PASS_CONTRACTS_V1 {
        return Err(PlironPassPreservationErrorV1::ResourceLimit {
            resource: "declaration count",
            limit: MAX_PLIRON_PASS_CONTRACTS_V1,
            observed: declarations.len(),
        });
    }
    let mut decoded = Vec::with_capacity(declarations.len());
    for (position, declaration) in declarations.iter().enumerate() {
        let pass = decode_pass(declaration.pass).ok_or(
            PlironPassPreservationErrorV1::UnknownPassDeclaration {
                position,
                declaration: declaration.pass,
            },
        )?;
        if declaration.effect != EFFECT_PRESERVE_EXACT_IDENTITY_V1 {
            return Err(PlironPassPreservationErrorV1::UnknownEffectDeclaration {
                position,
                declaration: declaration.effect,
            });
        }
        if let Some(first_position) = decoded.iter().position(|seen| *seen == pass) {
            return Err(PlironPassPreservationErrorV1::DuplicatePassDeclaration {
                first_position,
                duplicate_position: position,
                pass,
            });
        }
        decoded.push(pass);
    }
    for (position, contract) in PRODUCTION_PLIRON_PASS_CONTRACTS_V1.iter().enumerate() {
        if !decoded.contains(&contract.pass()) {
            return Err(PlironPassPreservationErrorV1::OmittedPassDeclaration {
                position,
                pass: contract.pass(),
            });
        }
    }
    for (position, (observed, expected)) in decoded
        .iter()
        .zip(PRODUCTION_PLIRON_PASS_CONTRACTS_V1.iter())
        .enumerate()
    {
        if *observed != expected.pass() {
            return Err(PlironPassPreservationErrorV1::PassOrderMismatch {
                position,
                expected: expected.pass(),
                observed: *observed,
            });
        }
    }
    Ok(())
}

pub(crate) enum IdentityCaptureFailureV1 {
    ResourceLimit { observed: usize },
    Unavailable(&'static str),
}

pub(crate) struct IdentityComparisonFailureV1 {
    code: &'static str,
    detail: String,
}

impl IdentityComparisonFailureV1 {
    pub(crate) fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}

/// Crate-private provider seam. Only compiler-owned modules can bind an exact
/// canonical snapshot implementation; public callers cannot inject callbacks.
pub(crate) trait PlironSemanticIdentityProviderV1 {
    type Snapshot;

    fn capture(&mut self) -> Result<Self::Snapshot, IdentityCaptureFailureV1>;
    fn label(&self, snapshot: &Self::Snapshot) -> PlironSemanticIdentityLabelV1;
    fn require_exact_identity(
        &self,
        expected: &Self::Snapshot,
        observed: &Self::Snapshot,
    ) -> Result<(), IdentityComparisonFailureV1>;
}

fn identity_error(error: IdentityCaptureFailureV1) -> PlironPassPreservationErrorV1 {
    match error {
        IdentityCaptureFailureV1::ResourceLimit { observed } => {
            PlironPassPreservationErrorV1::ResourceLimit {
                resource: "semantic identity bytes",
                limit: MAX_PLIRON_PASS_IDENTITY_BYTES_V1,
                observed,
            }
        }
        IdentityCaptureFailureV1::Unavailable(detail) => {
            PlironPassPreservationErrorV1::IdentityUnavailable { detail }
        }
    }
}

struct PendingPassV1<S> {
    pass: KernelCheckPassKindV1,
    before: S,
}

pub(crate) struct PlironPassContractSessionV1<'d, P: PlironSemanticIdentityProviderV1> {
    provider: P,
    declarations: &'d [RawPassContractV1],
    input_identity: PlironSemanticIdentityLabelV1,
    lineage: Option<P::Snapshot>,
    lineage_identity: PlironSemanticIdentityLabelV1,
    next: usize,
    checkpoints: usize,
    pending: Option<PendingPassV1<P::Snapshot>>,
    certificates: Vec<PlironPassPreservationCertificateV1>,
}

impl<'d, P: PlironSemanticIdentityProviderV1> PlironPassContractSessionV1<'d, P> {
    fn new(
        mut provider: P,
        declarations: &'d [RawPassContractV1],
    ) -> Result<Self, PlironPassPreservationErrorV1> {
        validate_declarations(declarations)?;
        let input = provider.capture().map_err(identity_error)?;
        let input_identity = provider.label(&input);
        Ok(Self {
            provider,
            declarations,
            input_identity,
            lineage: Some(input),
            lineage_identity: input_identity,
            next: 0,
            checkpoints: 1,
            pending: None,
            certificates: Vec::with_capacity(MAX_PLIRON_PASS_CONTRACTS_V1),
        })
    }

    pub(crate) fn begin_pass(
        &mut self,
        pass: KernelCheckPassKindV1,
    ) -> Result<(), PlironPassPreservationErrorV1> {
        if self.pending.is_some() {
            return Err(PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "a pass is already active",
            });
        }
        let declaration = self.declarations.get(self.next).ok_or(
            PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "pass executed after the fixed sequence",
            },
        )?;
        let declared = decode_pass(declaration.pass).ok_or(
            PlironPassPreservationErrorV1::UnknownPassDeclaration {
                position: self.next,
                declaration: declaration.pass,
            },
        )?;
        if declared != pass {
            return Err(PlironPassPreservationErrorV1::PassOrderMismatch {
                position: self.next,
                expected: declared,
                observed: pass,
            });
        }
        let before = self.capture()?;
        let expected =
            self.lineage
                .take()
                .ok_or(PlironPassPreservationErrorV1::InvalidSessionState {
                    detail: "the prior identity snapshot is absent",
                })?;
        if let Err(mismatch) = self.provider.require_exact_identity(&expected, &before) {
            return Err(PlironPassPreservationErrorV1::StaleInputIdentity {
                pass,
                source_code: mismatch.code,
                detail: mismatch.detail,
            });
        }
        self.pending = Some(PendingPassV1 { pass, before });
        Ok(())
    }

    pub(crate) fn end_pass(
        &mut self,
        pass: KernelCheckPassKindV1,
    ) -> Result<(), PlironPassPreservationErrorV1> {
        let pending =
            self.pending
                .take()
                .ok_or(PlironPassPreservationErrorV1::InvalidSessionState {
                    detail: "no pass is active",
                })?;
        if pending.pass != pass {
            return Err(PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "the completed pass differs from the active pass",
            });
        }
        let after = self.capture()?;
        if let Err(mismatch) = self
            .provider
            .require_exact_identity(&pending.before, &after)
        {
            return Err(PlironPassPreservationErrorV1::SemanticIdentityChanged {
                pass,
                source_code: mismatch.code,
                detail: mismatch.detail,
            });
        }
        if self.certificates.len() == MAX_PLIRON_PASS_CONTRACTS_V1 {
            return Err(PlironPassPreservationErrorV1::ResourceLimit {
                resource: "preservation certificate count",
                limit: MAX_PLIRON_PASS_CONTRACTS_V1,
                observed: self.certificates.len().saturating_add(1),
            });
        }
        let identity = self.provider.label(&after);
        self.certificates
            .push(PlironPassPreservationCertificateV1 { pass, identity });
        self.lineage = Some(after);
        self.lineage_identity = identity;
        self.next = self.next.saturating_add(1);
        Ok(())
    }

    fn capture(&mut self) -> Result<P::Snapshot, PlironPassPreservationErrorV1> {
        if self.checkpoints == MAX_PLIRON_PASS_IDENTITY_CHECKPOINTS_V1 {
            return Err(PlironPassPreservationErrorV1::ResourceLimit {
                resource: "semantic identity checkpoint count",
                limit: MAX_PLIRON_PASS_IDENTITY_CHECKPOINTS_V1,
                observed: self.checkpoints.saturating_add(1),
            });
        }
        let identity = self.provider.capture().map_err(identity_error)?;
        self.checkpoints = self.checkpoints.saturating_add(1);
        Ok(identity)
    }

    pub(crate) fn finish(
        self,
    ) -> Result<PlironPassPreservationReportV1, PlironPassPreservationErrorV1> {
        if self.pending.is_some() {
            return Err(PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "the final pass has not completed",
            });
        }
        if self.next != MAX_PLIRON_PASS_CONTRACTS_V1 {
            let contract = PRODUCTION_PLIRON_PASS_CONTRACTS_V1.get(self.next).ok_or(
                PlironPassPreservationErrorV1::InvalidSessionState {
                    detail: "completed pass count exceeds the fixed sequence",
                },
            )?;
            return Err(PlironPassPreservationErrorV1::OmittedPassDeclaration {
                position: self.next,
                pass: contract.pass(),
            });
        }
        Ok(PlironPassPreservationReportV1 {
            input_identity: self.input_identity,
            output_identity: self.lineage_identity,
            certificates: self.certificates,
        })
    }
}

pub(crate) fn begin_production_pliron_pass_contract_session_v1<P>(
    provider: P,
) -> Result<PlironPassContractSessionV1<'static, P>, PlironPassPreservationErrorV1>
where
    P: PlironSemanticIdentityProviderV1,
{
    PlironPassContractSessionV1::new(provider, &PRODUCTION_RAW_PASS_CONTRACTS_V1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::VecDeque;

    struct ScriptedIdentityProviderV1 {
        snapshots: VecDeque<Result<Vec<u8>, IdentityCaptureFailureV1>>,
    }

    impl PlironSemanticIdentityProviderV1 for ScriptedIdentityProviderV1 {
        type Snapshot = Vec<u8>;

        fn capture(&mut self) -> Result<Self::Snapshot, IdentityCaptureFailureV1> {
            self.snapshots
                .pop_front()
                .unwrap_or(Err(IdentityCaptureFailureV1::Unavailable(
                    "script exhausted",
                )))
        }

        fn label(&self, snapshot: &Self::Snapshot) -> PlironSemanticIdentityLabelV1 {
            let mut label = [0_u8; 32];
            for (index, byte) in snapshot.iter().copied().enumerate() {
                label[index % label.len()] ^= byte;
            }
            PlironSemanticIdentityLabelV1::new(label, snapshot.len())
        }

        fn require_exact_identity(
            &self,
            expected: &Self::Snapshot,
            observed: &Self::Snapshot,
        ) -> Result<(), IdentityComparisonFailureV1> {
            if expected == observed {
                return Ok(());
            }
            let difference = expected
                .iter()
                .zip(observed)
                .position(|(expected, observed)| expected != observed)
                .unwrap_or(expected.len().min(observed.len()));
            Err(IdentityComparisonFailureV1::new(
                "FE2O3-PRESERVE-010",
                format!("first changed component at canonical byte {difference}"),
            ))
        }
    }

    fn provider(values: &[u8]) -> ScriptedIdentityProviderV1 {
        ScriptedIdentityProviderV1 {
            snapshots: values.iter().map(|value| Ok(vec![*value; 4])).collect(),
        }
    }

    fn raw_contracts() -> Vec<RawPassContractV1> {
        PRODUCTION_RAW_PASS_CONTRACTS_V1.to_vec()
    }

    #[test]
    fn fixed_contract_order_is_exact_and_identity_only() {
        assert_eq!(PRODUCTION_PLIRON_PASS_CONTRACTS_V1.len(), 8);
        assert_eq!(
            PRODUCTION_PLIRON_PASS_CONTRACTS_V1.map(|contract| contract.pass()),
            crate::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
        );
        assert!(PRODUCTION_PLIRON_PASS_CONTRACTS_V1.iter().all(|contract| {
            contract.allowed_effect() == PlironPassAllowedEffectV1::PreserveExactSemanticIdentity
        }));
    }

    #[test]
    fn declarations_fail_closed_for_order_omission_duplication_and_unknowns() {
        let mut reordered = raw_contracts();
        reordered.swap(0, 1);
        assert!(matches!(
            validate_declarations(&reordered),
            Err(PlironPassPreservationErrorV1::PassOrderMismatch { .. })
        ));

        let mut omitted = raw_contracts();
        omitted.remove(3);
        assert!(matches!(
            validate_declarations(&omitted),
            Err(PlironPassPreservationErrorV1::OmittedPassDeclaration {
                pass: KernelCheckPassKindV1::RaceFreedom,
                ..
            })
        ));

        let mut duplicate = raw_contracts();
        duplicate[1] = duplicate[0];
        assert!(matches!(
            validate_declarations(&duplicate),
            Err(PlironPassPreservationErrorV1::DuplicatePassDeclaration { .. })
        ));

        let mut unknown_pass = raw_contracts();
        unknown_pass[0].pass = u16::MAX;
        assert_eq!(
            validate_declarations(&unknown_pass).unwrap_err().code(),
            "FE2O3-PRESERVE-020"
        );

        let mut unknown_effect = raw_contracts();
        unknown_effect[0].effect = u8::MAX;
        assert_eq!(
            validate_declarations(&unknown_effect).unwrap_err().code(),
            "FE2O3-PRESERVE-021"
        );
    }

    #[test]
    fn changed_identity_and_stale_input_wrap_the_exact_mismatch() {
        let mut changed =
            begin_production_pliron_pass_contract_session_v1(provider(&[1, 1, 2])).unwrap();
        changed
            .begin_pass(KernelCheckPassKindV1::TensorLayout)
            .unwrap();
        let error = changed
            .end_pass(KernelCheckPassKindV1::TensorLayout)
            .unwrap_err();
        assert_eq!(error.code(), "FE2O3-PRESERVE-025");
        assert!(error.to_string().contains("FE2O3-PRESERVE-010"));

        let mut stale =
            begin_production_pliron_pass_contract_session_v1(provider(&[1, 2])).unwrap();
        let error = stale
            .begin_pass(KernelCheckPassKindV1::TensorLayout)
            .unwrap_err();
        assert_eq!(error.code(), "FE2O3-PRESERVE-026");
        assert!(error.to_string().contains("FE2O3-PRESERVE-010"));
    }

    #[test]
    fn clean_fixed_pipeline_returns_one_compact_certificate_per_pass() {
        let values = vec![7; MAX_PLIRON_PASS_IDENTITY_CHECKPOINTS_V1];
        let mut session =
            begin_production_pliron_pass_contract_session_v1(provider(&values)).unwrap();
        for contract in PRODUCTION_PLIRON_PASS_CONTRACTS_V1 {
            session.begin_pass(contract.pass()).unwrap();
            session.end_pass(contract.pass()).unwrap();
        }
        let report = session.finish().unwrap();
        assert!(report.is_exact_identity());
        assert_eq!(report.certificates().len(), 8);
        assert_eq!(report.input_identity().canonical_len(), 4);
        assert_eq!(
            report
                .certificates()
                .iter()
                .map(PlironPassPreservationCertificateV1::pass)
                .collect::<Vec<_>>(),
            PRODUCTION_PLIRON_PASS_CONTRACTS_V1
                .iter()
                .map(PlironPassContractV1::pass)
                .collect::<Vec<_>>()
        );
    }

    #[test]
    fn omitted_execution_and_resource_limits_fail_closed() {
        let values = vec![3; MAX_PLIRON_PASS_IDENTITY_CHECKPOINTS_V1];
        let session = begin_production_pliron_pass_contract_session_v1(provider(&values)).unwrap();
        assert!(matches!(
            session.finish(),
            Err(PlironPassPreservationErrorV1::OmittedPassDeclaration {
                pass: KernelCheckPassKindV1::TensorLayout,
                ..
            })
        ));

        let oversized = vec![PRODUCTION_RAW_PASS_CONTRACTS_V1[0]; 9];
        assert!(matches!(
            validate_declarations(&oversized),
            Err(PlironPassPreservationErrorV1::ResourceLimit {
                resource: "declaration count",
                ..
            })
        ));
        let provider = ScriptedIdentityProviderV1 {
            snapshots: VecDeque::from([Err(IdentityCaptureFailureV1::ResourceLimit {
                observed: MAX_PLIRON_PASS_IDENTITY_BYTES_V1 + 1,
            })]),
        };
        let error = begin_production_pliron_pass_contract_session_v1(provider)
            .err()
            .expect("resource failure must reject the session");
        assert_eq!(error.code(), "FE2O3-PRESERVE-027");
    }
}
