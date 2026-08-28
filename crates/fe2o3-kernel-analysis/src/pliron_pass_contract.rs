//! Sealed structural-preservation contracts for the production PLIRON pipeline.
//!
//! A declaration states what a pass is allowed to do. Only exact identity
//! comparison plus PLIRON's monotonic context mutation-attempt epoch around the
//! actual pass can certify that the declaration held. The epoch detects
//! transient mutate-then-restore attempts while snapshots retain precise diffs.
//!
//! The production session is crate-private, admits only the fixed nine stages,
//! and operates on the identity module's closed operation/type/attribute set.
//! Unsafe code is denied in this crate; safe PLIRON mutation routes are owned by
//! the pinned fork and epoch-instrumented before mutable access or insertion.
//! This enforces IR immutability but does not prove an analysis result sound.

use std::{
    fmt,
    panic::{AssertUnwindSafe, catch_unwind},
    sync::Arc,
};

use crate::KernelCheckPassKindV1;

pub const MAX_PLIRON_PASS_CONTRACTS_V1: usize = 9;

/// The only effect admitted for the existing analysis-only verifier stages.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum PlironPassAllowedEffectV1 {
    PreserveExactStructuralIdentity,
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
            allowed_effect: PlironPassAllowedEffectV1::PreserveExactStructuralIdentity,
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
    PlironPassContractV1::identity(KernelCheckPassKindV1::PipelineProtocol),
    PlironPassContractV1::identity(KernelCheckPassKindV1::WorkgroupMemory),
    PlironPassContractV1::identity(KernelCheckPassKindV1::SemanticRefinement),
];

/// Compact label retained after the provider has compared canonical bytes.
/// The label is evidence lineage only; the checker never accepts digest
/// equality as a substitute for the provider's exact comparison.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct PlironStructuralIdentityLabelV1 {
    sha256: [u8; 32],
    canonical_len: usize,
}

impl PlironStructuralIdentityLabelV1 {
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

/// Immutable evidence that one actual pass preserved exact structural identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironPassPreservationCertificateV1 {
    pass: KernelCheckPassKindV1,
    identity: PlironStructuralIdentityLabelV1,
    mutation_epoch: u64,
}

impl PlironPassPreservationCertificateV1 {
    pub const fn pass(&self) -> KernelCheckPassKindV1 {
        self.pass
    }

    pub const fn identity(&self) -> PlironStructuralIdentityLabelV1 {
        self.identity
    }

    pub const fn mutation_epoch(&self) -> u64 {
        self.mutation_epoch
    }
}

/// Crate-private custody handle shared with the report-validation session.
/// Public labels cannot construct it because acceptance also requires pointer
/// identity of the private custody allocation.
pub(crate) struct PlironPassValidationHandleV1 {
    custody: Arc<()>,
    input_identity: PlironStructuralIdentityLabelV1,
    input_mutation_epoch: u64,
}

impl PlironPassValidationHandleV1 {
    pub(crate) fn same_custody(&self, checkpoint: &PlironPassCheckpointTokenV1) -> bool {
        Arc::ptr_eq(&self.custody, &checkpoint.custody)
    }

    pub(crate) fn same_report_custody(&self, report: &PlironPassPreservationReportV1) -> bool {
        Arc::ptr_eq(&self.custody, &report.custody)
    }

    pub(crate) const fn input_identity(&self) -> PlironStructuralIdentityLabelV1 {
        self.input_identity
    }

    pub(crate) const fn input_mutation_epoch(&self) -> u64 {
        self.input_mutation_epoch
    }
}

/// Unforgeable compiler checkpoint minted only after exact post-pass byte
/// comparison succeeds. It is small; canonical bytes remain in the provider
/// lineage and are retained once at session completion.
pub(crate) struct PlironPassCheckpointTokenV1 {
    custody: Arc<()>,
    position: usize,
    pass: KernelCheckPassKindV1,
    identity: PlironStructuralIdentityLabelV1,
    mutation_epoch: u64,
}

impl PlironPassCheckpointTokenV1 {
    pub(crate) const fn position(&self) -> usize {
        self.position
    }
    pub(crate) const fn pass(&self) -> KernelCheckPassKindV1 {
        self.pass
    }
    pub(crate) const fn identity(&self) -> PlironStructuralIdentityLabelV1 {
        self.identity
    }
    pub(crate) const fn mutation_epoch(&self) -> u64 {
        self.mutation_epoch
    }
}

/// Immutable report issued only after the complete fixed sequence is checked.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlironPassPreservationReportV1 {
    input_identity: PlironStructuralIdentityLabelV1,
    output_identity: PlironStructuralIdentityLabelV1,
    exact_output_identity: Arc<[u8]>,
    custody: Arc<()>,
    certificates: Vec<PlironPassPreservationCertificateV1>,
    input_mutation_epoch: u64,
    output_mutation_epoch: u64,
}

impl PlironPassPreservationReportV1 {
    pub const fn input_identity(&self) -> PlironStructuralIdentityLabelV1 {
        self.input_identity
    }

    pub const fn output_identity(&self) -> PlironStructuralIdentityLabelV1 {
        self.output_identity
    }

    pub fn certificates(&self) -> &[PlironPassPreservationCertificateV1] {
        &self.certificates
    }

    pub const fn input_mutation_epoch(&self) -> u64 {
        self.input_mutation_epoch
    }

    pub const fn output_mutation_epoch(&self) -> u64 {
        self.output_mutation_epoch
    }

    pub fn is_exact_identity(&self) -> bool {
        self.input_identity == self.output_identity
            && self.input_mutation_epoch == self.output_mutation_epoch
            && self.certificates.len() == MAX_PLIRON_PASS_CONTRACTS_V1
            && self.exact_output_identity.len() == self.output_identity.canonical_len()
            && self.certificates.iter().all(|certificate| {
                certificate.identity == self.output_identity
                    && certificate.mutation_epoch == self.output_mutation_epoch
            })
    }

    /// Compares retained canonical bytes. Digest labels are never used as the
    /// acceptance decision across production revalidation.
    pub fn exactly_matches_retained_output(&self, other: &Self) -> bool {
        self.exact_output_identity == other.exact_output_identity
    }

    pub const fn detects_persistent_structural_mutation(&self) -> bool {
        true
    }

    pub const fn detects_transient_mutation_attempts(&self) -> bool {
        true
    }

    pub const fn enforces_analysis_only_ir_immutability(&self) -> bool {
        true
    }

    pub const fn grants_analysis_result_soundness_authority(&self) -> bool {
        false
    }

    /// Compatibility query. Use the two explicit authority queries above.
    pub const fn grants_read_only_or_analysis_soundness_authority(&self) -> bool {
        false
    }
}

/// Stable fail-closed pass-manifest and session diagnostic. Codes below 020
/// are reserved for canonical snapshot and exact-comparison diagnostics.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PlironPassPreservationErrorV1 {
    PassOrderMismatch {
        position: usize,
        expected: KernelCheckPassKindV1,
        observed: KernelCheckPassKindV1,
    },
    OmittedPassDeclaration {
        position: usize,
        pass: KernelCheckPassKindV1,
    },
    StructuralIdentityChanged {
        pass: KernelCheckPassKindV1,
        source_code: &'static str,
        detail: String,
    },
    StaleInputIdentity {
        pass: KernelCheckPassKindV1,
        source_code: &'static str,
        detail: String,
    },
    MutationAttempted {
        pass: Option<KernelCheckPassKindV1>,
        before: u64,
        after: u64,
    },
    StaleMutationEpoch {
        pass: KernelCheckPassKindV1,
        expected: u64,
        observed: u64,
    },
    MutationEpochUnavailable {
        pass: Option<KernelCheckPassKindV1>,
        detail: String,
    },
    AnalysisPanicked {
        pass: KernelCheckPassKindV1,
    },
    IdentityUnavailable {
        source_code: &'static str,
        detail: String,
    },
    InvalidSessionState {
        detail: &'static str,
    },
}

impl PlironPassPreservationErrorV1 {
    pub const fn code(&self) -> &'static str {
        match self {
            Self::PassOrderMismatch { .. } => "FE2O3-PRESERVE-022",
            Self::OmittedPassDeclaration { .. } => "FE2O3-PRESERVE-024",
            Self::StructuralIdentityChanged { .. } => "FE2O3-PRESERVE-025",
            Self::StaleInputIdentity { .. } => "FE2O3-PRESERVE-026",
            Self::MutationAttempted { .. } => "FE2O3-PRESERVE-020",
            Self::StaleMutationEpoch { .. } => "FE2O3-PRESERVE-021",
            Self::MutationEpochUnavailable { .. } => "FE2O3-PRESERVE-023",
            Self::AnalysisPanicked { .. } => "FE2O3-PRESERVE-027",
            Self::IdentityUnavailable { .. } => "FE2O3-PRESERVE-028",
            Self::InvalidSessionState { .. } => "FE2O3-PRESERVE-029",
        }
    }
}

impl fmt::Display for PlironPassPreservationErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "error[{}]: ", self.code())?;
        match self {
            Self::PassOrderMismatch {
                position,
                expected,
                observed,
            } => write!(
                formatter,
                "production pass {observed:?} appears at position {position}; expected {expected:?}"
            ),
            Self::OmittedPassDeclaration { position, pass } => write!(
                formatter,
                "production pass {pass:?} is omitted from required position {position}"
            ),
            Self::StructuralIdentityChanged {
                pass,
                source_code,
                detail,
            } => write!(
                formatter,
                "analysis-only pass {pass:?} changed retained structural identity; error[{source_code}]: {detail}"
            ),
            Self::StaleInputIdentity {
                pass,
                source_code,
                detail,
            } => write!(
                formatter,
                "analysis-only pass {pass:?} received stale input; error[{source_code}]: {detail}"
            ),
            Self::MutationAttempted {
                pass,
                before,
                after,
            } => {
                if let Some(pass) = pass {
                    write!(
                        formatter,
                        "analysis-only pass {pass:?} attempted PLIRON mutation (context epoch {before} -> {after}); help: use only immutable analysis queries in this stage"
                    )
                } else {
                    write!(
                        formatter,
                        "compiler-owned PLIRON identity handling attempted mutation (context epoch {before} -> {after}); help: keep snapshot and report construction read-only"
                    )
                }
            }
            Self::StaleMutationEpoch {
                pass,
                expected,
                observed,
            } => write!(
                formatter,
                "analysis-only pass {pass:?} received a stale mutation capability (expected context epoch {expected}, observed {observed}); help: restart the fixed pipeline from a fresh structural snapshot"
            ),
            Self::MutationEpochUnavailable { pass, detail } => {
                if let Some(pass) = pass {
                    write!(
                        formatter,
                        "analysis-only pass {pass:?} cannot observe the PLIRON mutation epoch: {detail}; help: rebuild the context before verification"
                    )
                } else {
                    write!(
                        formatter,
                        "PLIRON mutation epoch is unavailable: {detail}; help: rebuild the context before verification"
                    )
                }
            }
            Self::AnalysisPanicked { pass } => write!(
                formatter,
                "analysis-only pass {pass:?} panicked; help: repair the pass so it returns a typed fail-closed diagnostic"
            ),
            Self::IdentityUnavailable {
                source_code,
                detail,
            } => {
                write!(
                    formatter,
                    "structural identity is unavailable; error[{source_code}]: {detail}"
                )
            }
            Self::InvalidSessionState { detail } => {
                write!(formatter, "invalid sealed pass session state: {detail}")
            }
        }
    }
}

impl std::error::Error for PlironPassPreservationErrorV1 {}

pub(crate) enum IdentityCaptureFailureV1 {
    Unavailable {
        source_code: &'static str,
        detail: String,
    },
}

pub(crate) struct MutationEpochCaptureFailureV1 {
    detail: String,
}

impl MutationEpochCaptureFailureV1 {
    pub(crate) fn new(detail: impl Into<String>) -> Self {
        Self {
            detail: detail.into(),
        }
    }
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
pub(crate) trait PlironStructuralIdentityProviderV1 {
    type Snapshot;

    fn mutation_epoch(&self) -> Result<u64, MutationEpochCaptureFailureV1>;
    fn capture(&mut self) -> Result<Self::Snapshot, IdentityCaptureFailureV1>;
    fn label(&self, snapshot: &Self::Snapshot) -> PlironStructuralIdentityLabelV1;
    fn require_exact_identity(
        &self,
        expected: &Self::Snapshot,
        observed: &Self::Snapshot,
    ) -> Result<(), IdentityComparisonFailureV1>;
    fn retain_exact_identity(&self, snapshot: Self::Snapshot) -> Arc<[u8]>;
}

fn identity_error(error: IdentityCaptureFailureV1) -> PlironPassPreservationErrorV1 {
    match error {
        IdentityCaptureFailureV1::Unavailable {
            source_code,
            detail,
        } => PlironPassPreservationErrorV1::IdentityUnavailable {
            source_code,
            detail,
        },
    }
}

struct PendingPassV1<S> {
    pass: KernelCheckPassKindV1,
    before: S,
    mutation_epoch: u64,
}

pub(crate) struct PlironPassContractSessionV1<P: PlironStructuralIdentityProviderV1> {
    provider: P,
    custody: Arc<()>,
    input_identity: PlironStructuralIdentityLabelV1,
    lineage: Option<P::Snapshot>,
    lineage_identity: PlironStructuralIdentityLabelV1,
    next: usize,
    pending: Option<PendingPassV1<P::Snapshot>>,
    certificates: Vec<PlironPassPreservationCertificateV1>,
    input_mutation_epoch: u64,
    lineage_mutation_epoch: u64,
}

impl<P: PlironStructuralIdentityProviderV1> PlironPassContractSessionV1<P> {
    fn new(mut provider: P) -> Result<Self, PlironPassPreservationErrorV1> {
        let input_mutation_epoch = provider.mutation_epoch().map_err(|error| {
            PlironPassPreservationErrorV1::MutationEpochUnavailable {
                pass: None,
                detail: error.detail,
            }
        })?;
        let input = provider.capture().map_err(identity_error)?;
        let after_capture = provider.mutation_epoch().map_err(|error| {
            PlironPassPreservationErrorV1::MutationEpochUnavailable {
                pass: None,
                detail: error.detail,
            }
        })?;
        if after_capture != input_mutation_epoch {
            return Err(PlironPassPreservationErrorV1::MutationAttempted {
                pass: None,
                before: input_mutation_epoch,
                after: after_capture,
            });
        }
        let input_identity = provider.label(&input);
        Ok(Self {
            provider,
            custody: Arc::new(()),
            input_identity,
            lineage: Some(input),
            lineage_identity: input_identity,
            next: 0,
            pending: None,
            certificates: Vec::with_capacity(MAX_PLIRON_PASS_CONTRACTS_V1),
            input_mutation_epoch,
            lineage_mutation_epoch: input_mutation_epoch,
        })
    }

    fn begin_pass(
        &mut self,
        pass: KernelCheckPassKindV1,
        revalidate_input: bool,
    ) -> Result<(), PlironPassPreservationErrorV1> {
        self.require_pass_can_begin(pass)?;
        let mutation_epoch = self.observe_mutation_epoch(Some(pass))?;
        let expected = self.take_lineage()?;
        let before = if revalidate_input || mutation_epoch != self.lineage_mutation_epoch {
            let before = self.capture()?;
            let after_capture = self.observe_mutation_epoch(Some(pass))?;
            if let Err(mismatch) = self.provider.require_exact_identity(&expected, &before) {
                return Err(PlironPassPreservationErrorV1::StaleInputIdentity {
                    pass,
                    source_code: mismatch.code,
                    detail: mismatch.detail,
                });
            }
            if after_capture != mutation_epoch {
                return Err(PlironPassPreservationErrorV1::MutationAttempted {
                    pass: Some(pass),
                    before: mutation_epoch,
                    after: after_capture,
                });
            }
            if mutation_epoch != self.lineage_mutation_epoch {
                return Err(PlironPassPreservationErrorV1::StaleMutationEpoch {
                    pass,
                    expected: self.lineage_mutation_epoch,
                    observed: mutation_epoch,
                });
            }
            before
        } else {
            expected
        };
        self.pending = Some(PendingPassV1 {
            pass,
            before,
            mutation_epoch,
        });
        Ok(())
    }

    /// Executes a pass directly from the preceding checkpoint, avoiding a
    /// duplicate walk when no code can run between adjacent sealed stages.
    pub(crate) fn run_contiguous_pass<T, E>(
        &mut self,
        pass: KernelCheckPassKindV1,
        execute: impl FnOnce() -> Result<T, E>,
    ) -> Result<Result<T, E>, PlironPassPreservationErrorV1> {
        self.begin_pass(pass, false)?;
        let result = catch_unwind(AssertUnwindSafe(execute));
        self.end_pass(pass)?;
        match result {
            Ok(result) => Ok(result),
            Err(_) => Err(PlironPassPreservationErrorV1::AnalysisPanicked { pass }),
        }
    }

    pub(crate) fn validation_handle(&self) -> PlironPassValidationHandleV1 {
        PlironPassValidationHandleV1 {
            custody: Arc::clone(&self.custody),
            input_identity: self.input_identity,
            input_mutation_epoch: self.input_mutation_epoch,
        }
    }

    /// Returns the most recently completed exact checkpoint. The token is
    /// issued only by this sealed session after [`Self::end_pass`] succeeds.
    pub(crate) fn last_checkpoint(
        &self,
    ) -> Result<PlironPassCheckpointTokenV1, PlironPassPreservationErrorV1> {
        let position =
            self.next
                .checked_sub(1)
                .ok_or(PlironPassPreservationErrorV1::InvalidSessionState {
                    detail: "no completed pass checkpoint is available",
                })?;
        let certificate = self.certificates.get(position).ok_or(
            PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "the completed pass certificate is absent",
            },
        )?;
        Ok(PlironPassCheckpointTokenV1 {
            custody: Arc::clone(&self.custody),
            position,
            pass: certificate.pass,
            identity: certificate.identity,
            mutation_epoch: certificate.mutation_epoch,
        })
    }

    fn require_pass_can_begin(
        &self,
        pass: KernelCheckPassKindV1,
    ) -> Result<(), PlironPassPreservationErrorV1> {
        if self.pending.is_some() {
            return Err(PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "a pass is already active",
            });
        }
        let contract = PRODUCTION_PLIRON_PASS_CONTRACTS_V1.get(self.next).ok_or(
            PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "pass executed after the fixed sequence",
            },
        )?;
        if contract.pass() != pass {
            return Err(PlironPassPreservationErrorV1::PassOrderMismatch {
                position: self.next,
                expected: contract.pass(),
                observed: pass,
            });
        }
        Ok(())
    }

    fn take_lineage(&mut self) -> Result<P::Snapshot, PlironPassPreservationErrorV1> {
        self.lineage
            .take()
            .ok_or(PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "the prior identity snapshot is absent",
            })
    }

    fn end_pass(
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
        let after = self.provider.capture().map_err(|error| match error {
            IdentityCaptureFailureV1::Unavailable {
                source_code,
                detail,
            } => PlironPassPreservationErrorV1::StructuralIdentityChanged {
                pass,
                source_code,
                detail: format!("post-pass structural identity is unavailable: {detail}"),
            },
        })?;
        let after_mutation_epoch = self.observe_mutation_epoch(Some(pass))?;
        if let Err(mismatch) = self
            .provider
            .require_exact_identity(&pending.before, &after)
        {
            return Err(PlironPassPreservationErrorV1::StructuralIdentityChanged {
                pass,
                source_code: mismatch.code,
                detail: mismatch.detail,
            });
        }
        if after_mutation_epoch != pending.mutation_epoch {
            return Err(PlironPassPreservationErrorV1::MutationAttempted {
                pass: Some(pass),
                before: pending.mutation_epoch,
                after: after_mutation_epoch,
            });
        }
        let identity = self.provider.label(&after);
        self.certificates.push(PlironPassPreservationCertificateV1 {
            pass,
            identity,
            mutation_epoch: after_mutation_epoch,
        });
        self.lineage = Some(after);
        self.lineage_identity = identity;
        self.lineage_mutation_epoch = after_mutation_epoch;
        self.next = self.next.saturating_add(1);
        Ok(())
    }

    fn observe_mutation_epoch(
        &self,
        pass: Option<KernelCheckPassKindV1>,
    ) -> Result<u64, PlironPassPreservationErrorV1> {
        self.provider.mutation_epoch().map_err(|error| {
            PlironPassPreservationErrorV1::MutationEpochUnavailable {
                pass,
                detail: error.detail,
            }
        })
    }

    fn capture(&mut self) -> Result<P::Snapshot, PlironPassPreservationErrorV1> {
        self.provider.capture().map_err(identity_error)
    }

    pub(crate) fn finish(
        mut self,
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
        let output_mutation_epoch = self.observe_mutation_epoch(None)?;
        if output_mutation_epoch != self.lineage_mutation_epoch {
            return Err(PlironPassPreservationErrorV1::MutationAttempted {
                pass: None,
                before: self.lineage_mutation_epoch,
                after: output_mutation_epoch,
            });
        }
        let output =
            self.lineage
                .take()
                .ok_or(PlironPassPreservationErrorV1::InvalidSessionState {
                    detail: "the final identity snapshot is absent",
                })?;
        let exact_output_identity = self.provider.retain_exact_identity(output);
        if exact_output_identity.len() != self.lineage_identity.canonical_len() {
            return Err(PlironPassPreservationErrorV1::InvalidSessionState {
                detail: "the retained identity length differs from its compact label",
            });
        }
        Ok(PlironPassPreservationReportV1 {
            input_identity: self.input_identity,
            output_identity: self.lineage_identity,
            exact_output_identity,
            custody: self.custody,
            certificates: self.certificates,
            input_mutation_epoch: self.input_mutation_epoch,
            output_mutation_epoch,
        })
    }
}

pub(crate) fn begin_production_pliron_pass_contract_session_v1<P>(
    provider: P,
) -> Result<PlironPassContractSessionV1<P>, PlironPassPreservationErrorV1>
where
    P: PlironStructuralIdentityProviderV1,
{
    PlironPassContractSessionV1::new(provider)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{cell::Cell, collections::VecDeque, rc::Rc};

    struct ScriptedIdentityProviderV1 {
        snapshots: VecDeque<Result<Vec<u8>, IdentityCaptureFailureV1>>,
        mutation_epoch: Rc<Cell<u64>>,
        epoch_unavailable: Rc<Cell<bool>>,
    }

    impl PlironStructuralIdentityProviderV1 for ScriptedIdentityProviderV1 {
        type Snapshot = Vec<u8>;

        fn mutation_epoch(&self) -> Result<u64, MutationEpochCaptureFailureV1> {
            if self.epoch_unavailable.get() {
                Err(MutationEpochCaptureFailureV1::new(
                    "scripted epoch exhausted",
                ))
            } else {
                Ok(self.mutation_epoch.get())
            }
        }

        fn capture(&mut self) -> Result<Self::Snapshot, IdentityCaptureFailureV1> {
            self.snapshots
                .pop_front()
                .unwrap_or(Err(IdentityCaptureFailureV1::Unavailable {
                    source_code: "FE2O3-PRESERVE-005",
                    detail: "script exhausted".to_owned(),
                }))
        }

        fn label(&self, snapshot: &Self::Snapshot) -> PlironStructuralIdentityLabelV1 {
            let mut label = [0_u8; 32];
            for (index, byte) in snapshot.iter().copied().enumerate() {
                label[index % label.len()] ^= byte;
            }
            PlironStructuralIdentityLabelV1::new(label, snapshot.len())
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

        fn retain_exact_identity(&self, snapshot: Self::Snapshot) -> Arc<[u8]> {
            Arc::from(snapshot)
        }
    }

    fn provider(values: &[u8]) -> ScriptedIdentityProviderV1 {
        provider_with_epoch(values).0
    }

    fn provider_with_epoch(
        values: &[u8],
    ) -> (ScriptedIdentityProviderV1, Rc<Cell<u64>>, Rc<Cell<bool>>) {
        let mutation_epoch = Rc::new(Cell::new(0));
        let epoch_unavailable = Rc::new(Cell::new(false));
        (
            ScriptedIdentityProviderV1 {
                snapshots: values.iter().map(|value| Ok(vec![*value; 4])).collect(),
                mutation_epoch: Rc::clone(&mutation_epoch),
                epoch_unavailable: Rc::clone(&epoch_unavailable),
            },
            mutation_epoch,
            epoch_unavailable,
        )
    }

    #[test]
    fn fixed_contract_order_is_exact_and_identity_only() {
        assert_eq!(PRODUCTION_PLIRON_PASS_CONTRACTS_V1.len(), 9);
        assert_eq!(
            PRODUCTION_PLIRON_PASS_CONTRACTS_V1.map(|contract| contract.pass()),
            crate::PRODUCTION_PLIRON_PRELOWERING_PASS_ORDER_V2,
        );
        assert!(PRODUCTION_PLIRON_PASS_CONTRACTS_V1.iter().all(|contract| {
            contract.allowed_effect() == PlironPassAllowedEffectV1::PreserveExactStructuralIdentity
        }));

        let mut session = begin_production_pliron_pass_contract_session_v1(provider(&[1])).unwrap();
        assert!(matches!(
            session.run_contiguous_pass(KernelCheckPassKindV1::MemoryBounds, || Ok::<_, ()>(())),
            Err(PlironPassPreservationErrorV1::PassOrderMismatch {
                expected: KernelCheckPassKindV1::TensorLayout,
                observed: KernelCheckPassKindV1::MemoryBounds,
                ..
            })
        ));
    }

    #[test]
    fn changed_identity_and_stale_input_wrap_the_exact_mismatch() {
        let mut changed =
            begin_production_pliron_pass_contract_session_v1(provider(&[1, 1, 2])).unwrap();
        changed
            .begin_pass(KernelCheckPassKindV1::TensorLayout, true)
            .unwrap();
        let error = changed
            .end_pass(KernelCheckPassKindV1::TensorLayout)
            .unwrap_err();
        assert_eq!(error.code(), "FE2O3-PRESERVE-025");
        assert!(error.to_string().contains("FE2O3-PRESERVE-010"));

        let mut stale =
            begin_production_pliron_pass_contract_session_v1(provider(&[1, 2])).unwrap();
        let error = stale
            .begin_pass(KernelCheckPassKindV1::TensorLayout, true)
            .unwrap_err();
        assert_eq!(error.code(), "FE2O3-PRESERVE-026");
        assert!(error.to_string().contains("FE2O3-PRESERVE-010"));
    }

    #[test]
    fn clean_fixed_pipeline_returns_one_compact_certificate_per_pass() {
        let values = vec![7; 1 + (2 * MAX_PLIRON_PASS_CONTRACTS_V1)];
        let mut session =
            begin_production_pliron_pass_contract_session_v1(provider(&values)).unwrap();
        for contract in PRODUCTION_PLIRON_PASS_CONTRACTS_V1 {
            session.begin_pass(contract.pass(), true).unwrap();
            session.end_pass(contract.pass()).unwrap();
        }
        let report = session.finish().unwrap();
        assert!(report.is_exact_identity());
        assert!(report.detects_persistent_structural_mutation());
        assert!(report.detects_transient_mutation_attempts());
        assert!(report.enforces_analysis_only_ir_immutability());
        assert!(!report.grants_analysis_result_soundness_authority());
        assert!(!report.grants_read_only_or_analysis_soundness_authority());
        assert_eq!(report.certificates().len(), 9);
        assert_eq!(report.input_identity().canonical_len(), 4);
        assert!(report.exactly_matches_retained_output(&report.clone()));
        let mut stale_certificate = report.clone();
        stale_certificate.certificates[3].mutation_epoch += 1;
        assert!(!stale_certificate.is_exact_identity());

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
    fn contiguous_pipeline_uses_one_initial_and_one_post_pass_snapshot() {
        let values = vec![9; MAX_PLIRON_PASS_CONTRACTS_V1 + 1];
        let mut session =
            begin_production_pliron_pass_contract_session_v1(provider(&values)).unwrap();
        for contract in PRODUCTION_PLIRON_PASS_CONTRACTS_V1 {
            session
                .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
                .unwrap()
                .unwrap();
        }
        assert!(session.finish().unwrap().is_exact_identity());
    }

    #[test]
    fn report_equality_retains_bytes_when_compact_labels_collide() {
        fn report(value: u8) -> PlironPassPreservationReportV1 {
            let mut snapshot = vec![0; 64];
            snapshot[0] = value;
            snapshot[32] = value;
            let provider = ScriptedIdentityProviderV1 {
                snapshots: (0..=MAX_PLIRON_PASS_CONTRACTS_V1)
                    .map(|_| Ok(snapshot.clone()))
                    .collect(),
                mutation_epoch: Rc::new(Cell::new(0)),
                epoch_unavailable: Rc::new(Cell::new(false)),
            };
            let mut session = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
            for contract in PRODUCTION_PLIRON_PASS_CONTRACTS_V1 {
                session
                    .run_contiguous_pass(contract.pass(), || Ok::<_, ()>(()))
                    .unwrap()
                    .unwrap();
            }
            session.finish().unwrap()
        }

        let first = report(1);
        let second = report(2);
        assert_eq!(first.output_identity(), second.output_identity());
        assert!(!first.exactly_matches_retained_output(&second));
        assert_ne!(first, second);
    }

    #[test]
    fn omitted_execution_and_identity_failures_fail_closed() {
        let values = vec![3; 1 + (2 * MAX_PLIRON_PASS_CONTRACTS_V1)];
        let session = begin_production_pliron_pass_contract_session_v1(provider(&values)).unwrap();
        assert!(matches!(
            session.finish(),
            Err(PlironPassPreservationErrorV1::OmittedPassDeclaration {
                pass: KernelCheckPassKindV1::TensorLayout,
                ..
            })
        ));

        let provider = ScriptedIdentityProviderV1 {
            snapshots: VecDeque::from([Err(IdentityCaptureFailureV1::Unavailable {
                source_code: "FE2O3-PRESERVE-002",
                detail: "error[FE2O3-PRESERVE-002]: canonical identity is too large".to_owned(),
            })]),
            mutation_epoch: Rc::new(Cell::new(0)),
            epoch_unavailable: Rc::new(Cell::new(false)),
        };
        let error = begin_production_pliron_pass_contract_session_v1(provider)
            .err()
            .expect("identity resource failure must reject the session");
        assert_eq!(error.code(), "FE2O3-PRESERVE-028");
        assert!(error.to_string().contains("FE2O3-PRESERVE-002"));

        let provider = ScriptedIdentityProviderV1 {
            snapshots: VecDeque::from([
                Ok(vec![1; 4]),
                Err(IdentityCaptureFailureV1::Unavailable {
                    source_code: "FE2O3-PRESERVE-001",
                    detail: "unsupported post-pass operation".to_owned(),
                }),
            ]),
            mutation_epoch: Rc::new(Cell::new(0)),
            epoch_unavailable: Rc::new(Cell::new(false)),
        };
        let mut session = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = session
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || Ok::<_, ()>(()))
            .unwrap_err();
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::StructuralIdentityChanged {
                pass: KernelCheckPassKindV1::TensorLayout,
                source_code: "FE2O3-PRESERVE-001",
                ..
            }
        ));
        assert!(error.to_string().contains("post-pass structural identity"));
    }

    #[test]
    fn transient_mutation_and_mutation_before_error_are_rejected() {
        let (provider, epoch, _) = provider_with_epoch(&[1, 1]);
        let mut session = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = session
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                epoch.set(epoch.get() + 2);
                Ok::<_, ()>(())
            })
            .unwrap_err();
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::MutationAttempted {
                pass: Some(KernelCheckPassKindV1::TensorLayout),
                before: 0,
                after: 2,
            }
        ));
        assert_eq!(error.code(), "FE2O3-PRESERVE-020");

        let (provider, epoch, _) = provider_with_epoch(&[1, 1]);
        let mut session = begin_production_pliron_pass_contract_session_v1(provider).unwrap();
        let error = session
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || {
                epoch.set(epoch.get() + 1);
                Err::<(), _>("analysis rejected after mutation")
            })
            .unwrap_err();
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::MutationAttempted {
                pass: Some(KernelCheckPassKindV1::TensorLayout),
                ..
            }
        ));
    }

    #[test]
    fn stale_epoch_panics_and_epoch_exhaustion_fail_closed() {
        let (stale_provider, epoch, _) = provider_with_epoch(&[1, 1, 1]);
        let mut session = begin_production_pliron_pass_contract_session_v1(stale_provider).unwrap();
        session
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || Ok::<_, ()>(()))
            .unwrap()
            .unwrap();
        epoch.set(1);
        let error = session
            .run_contiguous_pass(KernelCheckPassKindV1::MemoryBounds, || Ok::<_, ()>(()))
            .unwrap_err();
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::StaleMutationEpoch {
                pass: KernelCheckPassKindV1::MemoryBounds,
                expected: 0,
                observed: 1,
            }
        ));
        assert_eq!(error.code(), "FE2O3-PRESERVE-021");

        let mut panicking =
            begin_production_pliron_pass_contract_session_v1(provider(&[1, 1])).unwrap();
        let error = panicking
            .run_contiguous_pass(KernelCheckPassKindV1::TensorLayout, || -> Result<(), ()> {
                panic!("analysis panic")
            })
            .unwrap_err();
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::AnalysisPanicked {
                pass: KernelCheckPassKindV1::TensorLayout,
            }
        ));
        assert_eq!(error.code(), "FE2O3-PRESERVE-027");

        let (provider, _, unavailable) = provider_with_epoch(&[1]);
        unavailable.set(true);
        let error = begin_production_pliron_pass_contract_session_v1(provider)
            .err()
            .expect("exhausted mutation epoch rejects session construction");
        assert!(matches!(
            error,
            PlironPassPreservationErrorV1::MutationEpochUnavailable { pass: None, .. }
        ));
        assert_eq!(error.code(), "FE2O3-PRESERVE-023");
    }
}
