//! Transactional V1 compile outputs.

use core::fmt;

use crate::{
    CandidateFormatIdentityV1, CandidateIdentityV1, CanonicalDiagnosticV1, CompileRequestV1,
    CompilerStageV1, PipelineSelectorV1, ReceiptOutcomeV1, RequestIdentityV1, SnapshotIdentityV1,
    StageReceiptV1, StageSnapshotV1,
};

/// Hard maximum byte length of one V1 executable candidate.
pub const MAX_EXECUTABLE_CANDIDATE_BYTES_V1: usize = 256 * 1024 * 1024;

/// Why an executable candidate was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutableCandidateErrorV1 {
    /// An executable candidate cannot be empty.
    Empty,
    /// The candidate exceeds the hard V1 byte limit.
    PayloadTooLarge {
        /// Observed payload length.
        actual: usize,
        /// Hard V1 payload limit.
        maximum: usize,
    },
}

impl fmt::Display for ExecutableCandidateErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => formatter.write_str("executable candidate must not be empty"),
            Self::PayloadTooLarge { actual, maximum } => write!(
                formatter,
                "executable candidate is {actual} bytes, exceeding the {maximum}-byte limit"
            ),
        }
    }
}

impl std::error::Error for ExecutableCandidateErrorV1 {}

/// Opaque executable candidate produced by a compiler pipeline.
///
/// This record is not an admitted artifact and grants no publication, module
/// loading, dispatch, or launch authority.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutableCandidateV1 {
    identity: CandidateIdentityV1,
    format_identity: CandidateFormatIdentityV1,
    source_snapshot_identity: SnapshotIdentityV1,
    bytes: Vec<u8>,
}

impl ExecutableCandidateV1 {
    /// Creates a nonempty, hard-bounded opaque candidate.
    pub fn new(
        identity: CandidateIdentityV1,
        format_identity: CandidateFormatIdentityV1,
        source_snapshot_identity: SnapshotIdentityV1,
        bytes: Vec<u8>,
    ) -> Result<Self, ExecutableCandidateErrorV1> {
        if bytes.is_empty() {
            return Err(ExecutableCandidateErrorV1::Empty);
        }
        if bytes.len() > MAX_EXECUTABLE_CANDIDATE_BYTES_V1 {
            return Err(ExecutableCandidateErrorV1::PayloadTooLarge {
                actual: bytes.len(),
                maximum: MAX_EXECUTABLE_CANDIDATE_BYTES_V1,
            });
        }
        Ok(Self {
            identity,
            format_identity,
            source_snapshot_identity,
            bytes,
        })
    }

    /// Returns the producer-supplied candidate commitment.
    pub const fn identity(&self) -> CandidateIdentityV1 {
        self.identity
    }

    /// Returns the candidate format commitment.
    pub const fn format_identity(&self) -> CandidateFormatIdentityV1 {
        self.format_identity
    }

    /// Returns the HSACO snapshot from which this candidate was taken.
    pub const fn source_snapshot_identity(&self) -> SnapshotIdentityV1 {
        self.source_snapshot_identity
    }

    /// Borrows the opaque candidate bytes.
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}

/// Semantic disposition of a complete compiler invocation.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompileDispositionV1 {
    /// Compilation rejected the input transactionally.
    Rejected = 1,
    /// Inspect-only shadow processing completed without an executable candidate.
    ShadowOnly = 2,
    /// An artifact-producing pipeline returned an opaque executable candidate.
    CandidateProduced = 3,
}

/// Bounded resource named by output validation.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum OutputResourceV1 {
    /// Stage snapshot records.
    StageSnapshots,
    /// Stage receipt records.
    StageReceipts,
    /// Diagnostic records.
    Diagnostics,
    /// Aggregate stage snapshot bytes.
    TotalSnapshotBytes,
    /// Executable candidate bytes.
    CandidateBytes,
}

/// Why a complete compile output was rejected.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CompileOutputErrorV1 {
    /// A collection or payload exceeds the request's checked limit.
    ResourceLimitExceeded {
        /// Resource whose limit was exceeded.
        resource: OutputResourceV1,
        /// Observed count or byte length.
        actual: usize,
        /// Requested maximum count or byte length.
        maximum: usize,
    },
    /// One snapshot exceeds the request's per-snapshot byte limit.
    SnapshotLimitExceeded {
        /// Zero-based snapshot index.
        index: usize,
        /// Observed byte length.
        actual: usize,
        /// Requested maximum byte length.
        maximum: usize,
    },
    /// Aggregate snapshot byte accounting overflowed `usize`.
    SnapshotByteCountOverflow,
    /// An output snapshot reused the request input identity.
    SnapshotDuplicatesInput {
        /// Zero-based output snapshot index.
        index: usize,
    },
    /// Two output snapshots have the same commitment.
    DuplicateSnapshotIdentity {
        /// Earlier zero-based snapshot index.
        first: usize,
        /// Later zero-based snapshot index.
        second: usize,
    },
    /// Receipt sequence numbers are not zero-based and contiguous.
    ReceiptSequenceMismatch {
        /// Zero-based vector index.
        index: usize,
        /// Sequence number stored in the receipt.
        actual: u16,
    },
    /// Diagnostic sequence numbers are not zero-based and contiguous.
    DiagnosticSequenceMismatch {
        /// Zero-based vector index.
        index: usize,
        /// Sequence number stored in the diagnostic.
        actual: u16,
    },
    /// A receipt did not consume the previous committed snapshot.
    ReceiptChainMismatch {
        /// Zero-based receipt index.
        index: usize,
    },
    /// A receipt did not consume the previous committed obligation set.
    ReceiptObligationChainMismatch {
        /// Zero-based receipt index.
        index: usize,
    },
    /// A receipt output has no corresponding snapshot in receipt order.
    SnapshotReceiptCountMismatch {
        /// Number of snapshots supplied.
        snapshots: usize,
        /// Number of snapshots produced by receipts.
        produced_receipts: usize,
    },
    /// A snapshot commitment does not match its producing receipt.
    SnapshotReceiptIdentityMismatch {
        /// Zero-based produced snapshot index.
        index: usize,
    },
    /// A snapshot stage does not match its producing receipt.
    SnapshotReceiptStageMismatch {
        /// Zero-based produced snapshot index.
        index: usize,
        /// Stage recorded in the receipt.
        receipt_stage: CompilerStageV1,
        /// Stage recorded in the snapshot.
        snapshot_stage: CompilerStageV1,
    },
    /// A rejected receipt was followed by another receipt.
    RejectedReceiptNotTerminal {
        /// Zero-based rejected receipt index.
        index: usize,
    },
    /// A non-rejected output contains a rejected receipt.
    RejectedReceiptInSuccessfulOutput {
        /// Zero-based rejected receipt index.
        index: usize,
    },
    /// A successful disposition has no stage receipts.
    SuccessfulOutputWithoutReceipts,
    /// A rejected output does not include an error diagnostic.
    RejectedOutputWithoutError,
    /// A successful output includes an error diagnostic.
    SuccessfulOutputWithError,
    /// A rejected output included an executable candidate.
    RejectedOutputWithCandidate,
    /// Shadow output included an executable candidate.
    ShadowOutputWithCandidate,
    /// The shadow disposition was used with a non-shadow selector.
    ShadowDispositionSelectorMismatch {
        /// Selector copied from the request.
        selector: PipelineSelectorV1,
    },
    /// A candidate disposition was selected for an inspect-only pipeline.
    CandidateNotAllowedForSelector {
        /// Selector copied from the request.
        selector: PipelineSelectorV1,
    },
    /// Candidate-producing disposition omitted the candidate.
    CandidateDispositionWithoutCandidate,
    /// A candidate was supplied for a disposition that does not admit it.
    UnexpectedCandidate,
    /// Candidate-producing output did not terminate at an HSACO snapshot.
    CandidateTerminalStageNotHsaco {
        /// Observed terminal stage, or `None` if no snapshot was produced.
        actual: Option<CompilerStageV1>,
    },
    /// Candidate source commitment does not match the terminal HSACO snapshot.
    CandidateSourceSnapshotMismatch,
}

impl fmt::Display for CompileOutputErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid compile output: {self:?}")
    }
}

impl std::error::Error for CompileOutputErrorV1 {}

/// Fully validated result of one V1 compile request.
///
/// Construction enforces deterministic ordering, receipt chaining, caller
/// limits, transactional rejection, and selector disposition. It does not
/// authenticate any commitment or grant authority over the candidate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompileOutputV1 {
    request_identity: RequestIdentityV1,
    selector: PipelineSelectorV1,
    disposition: CompileDispositionV1,
    snapshots: Vec<StageSnapshotV1>,
    receipts: Vec<StageReceiptV1>,
    diagnostics: Vec<CanonicalDiagnosticV1>,
    candidate: Option<ExecutableCandidateV1>,
}

impl CompileOutputV1 {
    /// Validates and constructs one complete compiler output.
    pub fn new(
        request: &CompileRequestV1,
        disposition: CompileDispositionV1,
        snapshots: Vec<StageSnapshotV1>,
        receipts: Vec<StageReceiptV1>,
        diagnostics: Vec<CanonicalDiagnosticV1>,
        candidate: Option<ExecutableCandidateV1>,
    ) -> Result<Self, CompileOutputErrorV1> {
        validate_counts(
            request,
            &snapshots,
            &receipts,
            &diagnostics,
            candidate.as_ref(),
        )?;
        validate_diagnostic_sequence(&diagnostics)?;
        validate_snapshot_identities(request, &snapshots)?;
        validate_receipts(request, disposition, &snapshots, &receipts)?;
        validate_disposition(
            request,
            disposition,
            &snapshots,
            &diagnostics,
            candidate.as_ref(),
        )?;

        Ok(Self {
            request_identity: request.identity(),
            selector: request.selector(),
            disposition,
            snapshots,
            receipts,
            diagnostics,
            candidate,
        })
    }

    /// Returns the request commitment copied from the validated request.
    pub const fn request_identity(&self) -> RequestIdentityV1 {
        self.request_identity
    }

    /// Returns the selected compiler pipeline.
    pub const fn selector(&self) -> PipelineSelectorV1 {
        self.selector
    }

    /// Returns the output disposition.
    pub const fn disposition(&self) -> CompileDispositionV1 {
        self.disposition
    }

    /// Returns output snapshots in receipt order.
    pub fn snapshots(&self) -> &[StageSnapshotV1] {
        &self.snapshots
    }

    /// Returns stage receipts in contiguous sequence order.
    pub fn receipts(&self) -> &[StageReceiptV1] {
        &self.receipts
    }

    /// Returns diagnostics in contiguous sequence order.
    pub fn diagnostics(&self) -> &[CanonicalDiagnosticV1] {
        &self.diagnostics
    }

    /// Returns the opaque executable candidate when one was produced.
    pub const fn candidate(&self) -> Option<&ExecutableCandidateV1> {
        self.candidate.as_ref()
    }
}

fn validate_counts(
    request: &CompileRequestV1,
    snapshots: &[StageSnapshotV1],
    receipts: &[StageReceiptV1],
    diagnostics: &[CanonicalDiagnosticV1],
    candidate: Option<&ExecutableCandidateV1>,
) -> Result<(), CompileOutputErrorV1> {
    let limits = request.limits();
    check_resource(
        OutputResourceV1::StageSnapshots,
        snapshots.len(),
        usize::from(limits.max_stage_snapshots()),
    )?;
    check_resource(
        OutputResourceV1::StageReceipts,
        receipts.len(),
        usize::from(limits.max_stage_receipts()),
    )?;
    check_resource(
        OutputResourceV1::Diagnostics,
        diagnostics.len(),
        usize::from(limits.max_diagnostics()),
    )?;

    let mut total_snapshot_bytes = 0_usize;
    for (index, snapshot) in snapshots.iter().enumerate() {
        let length = snapshot.canonical_bytes().len();
        if length > limits.max_snapshot_bytes() as usize {
            return Err(CompileOutputErrorV1::SnapshotLimitExceeded {
                index,
                actual: length,
                maximum: limits.max_snapshot_bytes() as usize,
            });
        }
        total_snapshot_bytes = total_snapshot_bytes
            .checked_add(length)
            .ok_or(CompileOutputErrorV1::SnapshotByteCountOverflow)?;
    }
    check_resource(
        OutputResourceV1::TotalSnapshotBytes,
        total_snapshot_bytes,
        limits.max_total_snapshot_bytes() as usize,
    )?;
    if let Some(candidate) = candidate {
        check_resource(
            OutputResourceV1::CandidateBytes,
            candidate.bytes().len(),
            limits.max_candidate_bytes() as usize,
        )?;
    }
    Ok(())
}

fn check_resource(
    resource: OutputResourceV1,
    actual: usize,
    maximum: usize,
) -> Result<(), CompileOutputErrorV1> {
    if actual > maximum {
        return Err(CompileOutputErrorV1::ResourceLimitExceeded {
            resource,
            actual,
            maximum,
        });
    }
    Ok(())
}

fn validate_diagnostic_sequence(
    diagnostics: &[CanonicalDiagnosticV1],
) -> Result<(), CompileOutputErrorV1> {
    for (index, diagnostic) in diagnostics.iter().enumerate() {
        if usize::from(diagnostic.sequence()) != index {
            return Err(CompileOutputErrorV1::DiagnosticSequenceMismatch {
                index,
                actual: diagnostic.sequence(),
            });
        }
    }
    Ok(())
}

fn validate_snapshot_identities(
    request: &CompileRequestV1,
    snapshots: &[StageSnapshotV1],
) -> Result<(), CompileOutputErrorV1> {
    for (index, snapshot) in snapshots.iter().enumerate() {
        if snapshot.identity() == request.input().identity() {
            return Err(CompileOutputErrorV1::SnapshotDuplicatesInput { index });
        }
        if let Some(first) = snapshots[..index]
            .iter()
            .position(|prior| prior.identity() == snapshot.identity())
        {
            return Err(CompileOutputErrorV1::DuplicateSnapshotIdentity {
                first,
                second: index,
            });
        }
    }
    Ok(())
}

fn validate_receipts(
    request: &CompileRequestV1,
    disposition: CompileDispositionV1,
    snapshots: &[StageSnapshotV1],
    receipts: &[StageReceiptV1],
) -> Result<(), CompileOutputErrorV1> {
    if disposition != CompileDispositionV1::Rejected && receipts.is_empty() {
        return Err(CompileOutputErrorV1::SuccessfulOutputWithoutReceipts);
    }

    let mut expected_input = request.input().identity();
    let mut expected_obligations = request.input_obligations_identity();
    let mut produced = 0_usize;
    for (index, receipt) in receipts.iter().copied().enumerate() {
        if usize::from(receipt.sequence()) != index {
            return Err(CompileOutputErrorV1::ReceiptSequenceMismatch {
                index,
                actual: receipt.sequence(),
            });
        }
        if receipt.input_snapshot_identity() != expected_input {
            return Err(CompileOutputErrorV1::ReceiptChainMismatch { index });
        }
        if receipt.input_obligations_identity() != expected_obligations {
            return Err(CompileOutputErrorV1::ReceiptObligationChainMismatch { index });
        }
        match receipt.outcome() {
            ReceiptOutcomeV1::Produced => {
                let output = receipt
                    .output_snapshot_identity()
                    .expect("StageReceiptV1 validates produced output");
                let Some(snapshot) = snapshots.get(produced) else {
                    return Err(CompileOutputErrorV1::SnapshotReceiptCountMismatch {
                        snapshots: snapshots.len(),
                        produced_receipts: produced + 1,
                    });
                };
                if snapshot.identity() != output {
                    return Err(CompileOutputErrorV1::SnapshotReceiptIdentityMismatch {
                        index: produced,
                    });
                }
                if snapshot.stage() != receipt.stage() {
                    return Err(CompileOutputErrorV1::SnapshotReceiptStageMismatch {
                        index: produced,
                        receipt_stage: receipt.stage(),
                        snapshot_stage: snapshot.stage(),
                    });
                }
                expected_input = output;
                expected_obligations = receipt
                    .output_obligations_identity()
                    .expect("StageReceiptV1 validates produced obligations");
                produced += 1;
            }
            ReceiptOutcomeV1::Rejected => {
                if index + 1 != receipts.len() {
                    return Err(CompileOutputErrorV1::RejectedReceiptNotTerminal { index });
                }
                if disposition != CompileDispositionV1::Rejected {
                    return Err(CompileOutputErrorV1::RejectedReceiptInSuccessfulOutput { index });
                }
            }
        }
    }

    if produced != snapshots.len() {
        return Err(CompileOutputErrorV1::SnapshotReceiptCountMismatch {
            snapshots: snapshots.len(),
            produced_receipts: produced,
        });
    }
    Ok(())
}

fn validate_disposition(
    request: &CompileRequestV1,
    disposition: CompileDispositionV1,
    snapshots: &[StageSnapshotV1],
    diagnostics: &[CanonicalDiagnosticV1],
    candidate: Option<&ExecutableCandidateV1>,
) -> Result<(), CompileOutputErrorV1> {
    let has_error = diagnostics.iter().any(CanonicalDiagnosticV1::is_error);
    match disposition {
        CompileDispositionV1::Rejected => {
            if candidate.is_some() {
                return Err(CompileOutputErrorV1::RejectedOutputWithCandidate);
            }
            if !has_error {
                return Err(CompileOutputErrorV1::RejectedOutputWithoutError);
            }
        }
        CompileDispositionV1::ShadowOnly => {
            if request.selector() != PipelineSelectorV1::PlironShadow {
                return Err(CompileOutputErrorV1::ShadowDispositionSelectorMismatch {
                    selector: request.selector(),
                });
            }
            if candidate.is_some() {
                return Err(CompileOutputErrorV1::ShadowOutputWithCandidate);
            }
            if has_error {
                return Err(CompileOutputErrorV1::SuccessfulOutputWithError);
            }
        }
        CompileDispositionV1::CandidateProduced => {
            if !request.selector().may_produce_candidate() {
                return Err(CompileOutputErrorV1::CandidateNotAllowedForSelector {
                    selector: request.selector(),
                });
            }
            if has_error {
                return Err(CompileOutputErrorV1::SuccessfulOutputWithError);
            }
            let candidate =
                candidate.ok_or(CompileOutputErrorV1::CandidateDispositionWithoutCandidate)?;
            let terminal = snapshots.last();
            if terminal.map(StageSnapshotV1::stage) != Some(CompilerStageV1::Hsaco) {
                return Err(CompileOutputErrorV1::CandidateTerminalStageNotHsaco {
                    actual: terminal.map(StageSnapshotV1::stage),
                });
            }
            if terminal.map(StageSnapshotV1::identity) != Some(candidate.source_snapshot_identity())
            {
                return Err(CompileOutputErrorV1::CandidateSourceSnapshotMismatch);
            }
        }
    }

    if disposition != CompileDispositionV1::CandidateProduced && candidate.is_some() {
        return Err(CompileOutputErrorV1::UnexpectedCandidate);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        CompileLimitsV1, CompilerProfileIdentityV1, DiagnosticCodeV1, DiagnosticMessageV1,
        DiagnosticSeverityV1, KernelInstanceIdentityV1, ObligationSetIdentityV1,
        PipelineConfigurationIdentityV1, SnapshotFormatIdentityV1, TargetProfileIdentityV1,
        TransformConfigurationIdentityV1, TransformIdentityV1,
    };

    fn id(byte: u8) -> SnapshotIdentityV1 {
        SnapshotIdentityV1::from_untrusted_bytes([byte; 32])
    }

    fn snapshot(stage: CompilerStageV1, byte: u8, bytes: usize) -> StageSnapshotV1 {
        StageSnapshotV1::new(
            stage,
            id(byte),
            SnapshotFormatIdentityV1::from_untrusted_bytes([0xf0; 32]),
            vec![byte; bytes],
        )
        .unwrap()
    }

    fn limits() -> CompileLimitsV1 {
        CompileLimitsV1::new(4, 4, 4, 16, 32, 16).unwrap()
    }

    fn request(selector: PipelineSelectorV1, limits: CompileLimitsV1) -> CompileRequestV1 {
        CompileRequestV1::new(
            RequestIdentityV1::from_untrusted_bytes([1; 32]),
            KernelInstanceIdentityV1::from_untrusted_bytes([2; 32]),
            CompilerProfileIdentityV1::from_untrusted_bytes([3; 32]),
            TargetProfileIdentityV1::from_untrusted_bytes([4; 32]),
            PipelineConfigurationIdentityV1::from_untrusted_bytes([5; 32]),
            ObligationSetIdentityV1::from_untrusted_bytes([9; 32]),
            selector,
            snapshot(CompilerStageV1::FrontendInput, 6, 1),
            limits,
        )
        .unwrap()
    }

    fn receipt(sequence: u16, stage: CompilerStageV1, input: u8, output: u8) -> StageReceiptV1 {
        StageReceiptV1::new(
            sequence,
            stage,
            TransformIdentityV1::from_untrusted_bytes([7; 32]),
            TransformConfigurationIdentityV1::from_untrusted_bytes([8; 32]),
            id(input),
            Some(id(output)),
            ObligationSetIdentityV1::from_untrusted_bytes([9; 32]),
            Some(ObligationSetIdentityV1::from_untrusted_bytes([9; 32])),
            ReceiptOutcomeV1::Produced,
        )
        .unwrap()
    }

    fn rejected_receipt(sequence: u16, input: u8) -> StageReceiptV1 {
        StageReceiptV1::new(
            sequence,
            CompilerStageV1::Kernel,
            TransformIdentityV1::from_untrusted_bytes([7; 32]),
            TransformConfigurationIdentityV1::from_untrusted_bytes([8; 32]),
            id(input),
            None,
            ObligationSetIdentityV1::from_untrusted_bytes([9; 32]),
            None,
            ReceiptOutcomeV1::Rejected,
        )
        .unwrap()
    }

    fn diagnostic(sequence: u16, severity: DiagnosticSeverityV1) -> CanonicalDiagnosticV1 {
        CanonicalDiagnosticV1::new(
            sequence,
            DiagnosticCodeV1::new(100).unwrap(),
            severity,
            None,
            None,
            DiagnosticMessageV1::new("compiler diagnostic").unwrap(),
        )
    }

    fn candidate(source: u8, bytes: usize) -> ExecutableCandidateV1 {
        ExecutableCandidateV1::new(
            CandidateIdentityV1::from_untrusted_bytes([11; 32]),
            CandidateFormatIdentityV1::from_untrusted_bytes([12; 32]),
            id(source),
            vec![0x7f; bytes],
        )
        .unwrap()
    }

    #[test]
    fn artifact_pipeline_returns_only_a_candidate_bound_to_terminal_hsaco() {
        let request = request(PipelineSelectorV1::PlironV1, limits());
        let output = CompileOutputV1::new(
            &request,
            CompileDispositionV1::CandidateProduced,
            vec![snapshot(CompilerStageV1::Hsaco, 13, 4)],
            vec![receipt(0, CompilerStageV1::Hsaco, 6, 13)],
            vec![],
            Some(candidate(13, 4)),
        )
        .unwrap();

        assert_eq!(output.request_identity(), request.identity());
        assert_eq!(output.selector(), PipelineSelectorV1::PlironV1);
        assert_eq!(output.snapshots().len(), 1);
        assert_eq!(output.receipts().len(), 1);
        assert_eq!(output.candidate().unwrap().bytes(), &[0x7f; 4]);
    }

    #[test]
    fn shadow_pipeline_can_return_inspection_records_but_no_candidate() {
        let request = request(PipelineSelectorV1::PlironShadow, limits());
        let output = CompileOutputV1::new(
            &request,
            CompileDispositionV1::ShadowOnly,
            vec![snapshot(CompilerStageV1::Kernel, 13, 4)],
            vec![receipt(0, CompilerStageV1::Kernel, 6, 13)],
            vec![diagnostic(0, DiagnosticSeverityV1::Note)],
            None,
        )
        .unwrap();

        assert_eq!(output.disposition(), CompileDispositionV1::ShadowOnly);
        assert!(output.candidate().is_none());
    }

    #[test]
    fn selector_and_disposition_cannot_broaden_candidate_policy() {
        let shadow = request(PipelineSelectorV1::PlironShadow, limits());
        assert_eq!(
            CompileOutputV1::new(
                &shadow,
                CompileDispositionV1::CandidateProduced,
                vec![snapshot(CompilerStageV1::Hsaco, 13, 1)],
                vec![receipt(0, CompilerStageV1::Hsaco, 6, 13)],
                vec![],
                Some(candidate(13, 1)),
            ),
            Err(CompileOutputErrorV1::CandidateNotAllowedForSelector {
                selector: PipelineSelectorV1::PlironShadow,
            })
        );

        let pliron = request(PipelineSelectorV1::PlironV1, limits());
        assert_eq!(
            CompileOutputV1::new(
                &pliron,
                CompileDispositionV1::ShadowOnly,
                vec![snapshot(CompilerStageV1::Kernel, 13, 1)],
                vec![receipt(0, CompilerStageV1::Kernel, 6, 13)],
                vec![],
                None,
            ),
            Err(CompileOutputErrorV1::ShadowDispositionSelectorMismatch {
                selector: PipelineSelectorV1::PlironV1,
            })
        );
    }

    #[test]
    fn rejected_output_requires_an_error_and_never_a_candidate() {
        let request = request(PipelineSelectorV1::PlironV1, limits());
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::Rejected,
                vec![],
                vec![],
                vec![],
                None,
            ),
            Err(CompileOutputErrorV1::RejectedOutputWithoutError)
        );
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::Rejected,
                vec![],
                vec![],
                vec![diagnostic(0, DiagnosticSeverityV1::Error)],
                Some(candidate(6, 1)),
            ),
            Err(CompileOutputErrorV1::RejectedOutputWithCandidate)
        );
        assert!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::Rejected,
                vec![],
                vec![rejected_receipt(0, 6)],
                vec![diagnostic(0, DiagnosticSeverityV1::Error)],
                None,
            )
            .is_ok()
        );
    }

    #[test]
    fn successful_outputs_require_receipts_and_reject_error_diagnostics() {
        let request = request(PipelineSelectorV1::PlironShadow, limits());
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![],
                vec![],
                vec![],
                None,
            ),
            Err(CompileOutputErrorV1::SuccessfulOutputWithoutReceipts)
        );
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![snapshot(CompilerStageV1::Kernel, 13, 1)],
                vec![receipt(0, CompilerStageV1::Kernel, 6, 13)],
                vec![diagnostic(0, DiagnosticSeverityV1::Error)],
                None,
            ),
            Err(CompileOutputErrorV1::SuccessfulOutputWithError)
        );
    }

    #[test]
    fn diagnostic_and_receipt_sequences_are_contiguous() {
        let request = request(PipelineSelectorV1::PlironShadow, limits());
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![snapshot(CompilerStageV1::Kernel, 13, 1)],
                vec![receipt(1, CompilerStageV1::Kernel, 6, 13)],
                vec![],
                None,
            ),
            Err(CompileOutputErrorV1::ReceiptSequenceMismatch {
                index: 0,
                actual: 1,
            })
        );
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![snapshot(CompilerStageV1::Kernel, 13, 1)],
                vec![receipt(0, CompilerStageV1::Kernel, 6, 13)],
                vec![diagnostic(1, DiagnosticSeverityV1::Note)],
                None,
            ),
            Err(CompileOutputErrorV1::DiagnosticSequenceMismatch {
                index: 0,
                actual: 1,
            })
        );
    }

    #[test]
    fn receipt_chain_and_snapshot_order_are_exact() {
        let request = request(PipelineSelectorV1::PlironShadow, limits());
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![snapshot(CompilerStageV1::Mir, 13, 1)],
                vec![receipt(0, CompilerStageV1::Mir, 99, 13)],
                vec![],
                None,
            ),
            Err(CompileOutputErrorV1::ReceiptChainMismatch { index: 0 })
        );
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![
                    snapshot(CompilerStageV1::Kernel, 14, 1),
                    snapshot(CompilerStageV1::Mir, 13, 1),
                ],
                vec![
                    receipt(0, CompilerStageV1::Mir, 6, 13),
                    receipt(1, CompilerStageV1::Kernel, 13, 14),
                ],
                vec![],
                None,
            ),
            Err(CompileOutputErrorV1::SnapshotReceiptIdentityMismatch { index: 0 })
        );

        let wrong_obligations = StageReceiptV1::new(
            0,
            CompilerStageV1::Mir,
            TransformIdentityV1::from_untrusted_bytes([7; 32]),
            TransformConfigurationIdentityV1::from_untrusted_bytes([8; 32]),
            id(6),
            Some(id(13)),
            ObligationSetIdentityV1::from_untrusted_bytes([99; 32]),
            Some(ObligationSetIdentityV1::from_untrusted_bytes([9; 32])),
            ReceiptOutcomeV1::Produced,
        )
        .unwrap();
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![snapshot(CompilerStageV1::Mir, 13, 1)],
                vec![wrong_obligations],
                vec![],
                None,
            ),
            Err(CompileOutputErrorV1::ReceiptObligationChainMismatch { index: 0 })
        );
    }

    #[test]
    fn snapshots_are_unique_and_match_receipt_stage() {
        let request = request(PipelineSelectorV1::PlironShadow, limits());
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![snapshot(CompilerStageV1::Kernel, 6, 1)],
                vec![receipt(0, CompilerStageV1::Kernel, 6, 6)],
                vec![],
                None,
            ),
            Err(CompileOutputErrorV1::SnapshotDuplicatesInput { index: 0 })
        );
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![snapshot(CompilerStageV1::Gpu, 13, 1)],
                vec![receipt(0, CompilerStageV1::Kernel, 6, 13)],
                vec![],
                None,
            ),
            Err(CompileOutputErrorV1::SnapshotReceiptStageMismatch {
                index: 0,
                receipt_stage: CompilerStageV1::Kernel,
                snapshot_stage: CompilerStageV1::Gpu,
            })
        );
    }

    #[test]
    fn request_limits_apply_to_counts_and_payloads() {
        let tight = CompileLimitsV1::new(1, 1, 1, 2, 2, 2).unwrap();
        let request = request(PipelineSelectorV1::PlironShadow, tight);
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![snapshot(CompilerStageV1::Kernel, 13, 3)],
                vec![receipt(0, CompilerStageV1::Kernel, 6, 13)],
                vec![],
                None,
            ),
            Err(CompileOutputErrorV1::SnapshotLimitExceeded {
                index: 0,
                actual: 3,
                maximum: 2,
            })
        );
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![snapshot(CompilerStageV1::Kernel, 13, 1)],
                vec![receipt(0, CompilerStageV1::Kernel, 6, 13)],
                vec![
                    diagnostic(0, DiagnosticSeverityV1::Note),
                    diagnostic(1, DiagnosticSeverityV1::Note),
                ],
                None,
            ),
            Err(CompileOutputErrorV1::ResourceLimitExceeded {
                resource: OutputResourceV1::Diagnostics,
                actual: 2,
                maximum: 1,
            })
        );
    }

    #[test]
    fn candidate_must_bind_to_terminal_hsaco_snapshot() {
        let request = request(PipelineSelectorV1::Legacy, limits());
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::CandidateProduced,
                vec![snapshot(CompilerStageV1::Object, 13, 1)],
                vec![receipt(0, CompilerStageV1::Object, 6, 13)],
                vec![],
                Some(candidate(13, 1)),
            ),
            Err(CompileOutputErrorV1::CandidateTerminalStageNotHsaco {
                actual: Some(CompilerStageV1::Object),
            })
        );
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::CandidateProduced,
                vec![snapshot(CompilerStageV1::Hsaco, 13, 1)],
                vec![receipt(0, CompilerStageV1::Hsaco, 6, 13)],
                vec![],
                Some(candidate(14, 1)),
            ),
            Err(CompileOutputErrorV1::CandidateSourceSnapshotMismatch)
        );
    }

    #[test]
    fn rejected_receipt_is_terminal_and_only_valid_for_rejection() {
        let request = request(PipelineSelectorV1::PlironShadow, limits());
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::Rejected,
                vec![snapshot(CompilerStageV1::Mir, 13, 1)],
                vec![
                    rejected_receipt(0, 6),
                    receipt(1, CompilerStageV1::Mir, 6, 13)
                ],
                vec![diagnostic(0, DiagnosticSeverityV1::Error)],
                None,
            ),
            Err(CompileOutputErrorV1::RejectedReceiptNotTerminal { index: 0 })
        );
        assert_eq!(
            CompileOutputV1::new(
                &request,
                CompileDispositionV1::ShadowOnly,
                vec![],
                vec![rejected_receipt(0, 6)],
                vec![],
                None,
            ),
            Err(CompileOutputErrorV1::RejectedReceiptInSuccessfulOutput { index: 0 })
        );
    }

    #[test]
    fn executable_candidate_rejects_empty_bytes() {
        assert_eq!(
            ExecutableCandidateV1::new(
                CandidateIdentityV1::from_untrusted_bytes([1; 32]),
                CandidateFormatIdentityV1::from_untrusted_bytes([2; 32]),
                id(3),
                vec![],
            ),
            Err(ExecutableCandidateErrorV1::Empty)
        );
    }
}
