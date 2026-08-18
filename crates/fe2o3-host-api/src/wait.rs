//! Bounded completion wait and observation contracts.

use alloc::vec::Vec;

use crate::canonical::EncoderV1;
use crate::common::{
    ContractFieldV1, DiagnosticSeverityV1, HostRequestIdentityV1, HostResultIdentityV1,
    HostResultReferenceV1, OperationResultClassV1, check_preimage_bound, encode_diagnostics,
    validate_diagnostics, validate_strictly_ordered,
};
use crate::{
    CompletionRecordIdV1, CompletionSignalIdV1, DeadlineIdV1, DispatchOutcomeV1, DispatchResultV1,
    HostContractErrorV1, HostDiagnosticV1, OperationContextV1, WaitRequestIdV1, WaitResultIdV1,
};

/// Hard maximum completion target count in one wait request.
pub const MAX_WAIT_TARGETS_V1: usize = 256;

/// Completion predicate selected by a wait request.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum WaitModeV1 {
    /// Satisfied after any named signal has a terminal observation.
    Any = 1,
    /// Satisfied only after every named signal has a terminal observation.
    All = 2,
}

/// Terminal status carried by one completion observation.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum CompletionStatusV1 {
    /// The described work completed successfully.
    Succeeded = 1,
    /// The described work was cancelled.
    Cancelled = 2,
    /// The described work completed with failure.
    Failed = 3,
}

/// One terminal observation of an inert completion-signal commitment.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CompletionObservationV1 {
    signal_identity: CompletionSignalIdV1,
    record_identity: CompletionRecordIdV1,
    status: CompletionStatusV1,
}

impl CompletionObservationV1 {
    /// Creates a completion observation.
    pub const fn new(
        signal_identity: CompletionSignalIdV1,
        record_identity: CompletionRecordIdV1,
        status: CompletionStatusV1,
    ) -> Self {
        Self {
            signal_identity,
            record_identity,
            status,
        }
    }

    /// Returns the observed completion-signal commitment.
    pub const fn signal_identity(self) -> CompletionSignalIdV1 {
        self.signal_identity
    }

    /// Returns the observed completion-record commitment.
    pub const fn record_identity(self) -> CompletionRecordIdV1 {
        self.record_identity
    }

    /// Returns the terminal completion status.
    pub const fn status(self) -> CompletionStatusV1 {
        self.status
    }

    fn encode(self, encoder: &mut EncoderV1) {
        encoder.digest(self.signal_identity.digest());
        encoder.digest(self.record_identity.digest());
        encoder.u8(self.status as u8);
    }
}

/// Result disposition of one wait request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum WaitOutcomeV1 {
    /// The selected `Any` or `All` predicate was satisfied.
    Satisfied(Vec<CompletionObservationV1>),
    /// The observation completed without satisfying the predicate.
    Pending(Vec<CompletionObservationV1>),
    /// The wait request itself was rejected.
    Rejected,
    /// The described wait operation failed.
    Failed,
}

impl WaitOutcomeV1 {
    /// Returns terminal observations carried by a successful wait operation.
    pub fn observations(&self) -> &[CompletionObservationV1] {
        match self {
            Self::Satisfied(observations) | Self::Pending(observations) => observations,
            Self::Rejected | Self::Failed => &[],
        }
    }
}

/// Complete runtime-neutral V1 wait request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaitRequestV1 {
    identity: WaitRequestIdV1,
    context: OperationContextV1,
    mode: WaitModeV1,
    targets: Vec<CompletionSignalIdV1>,
    deadline_identity: Option<DeadlineIdV1>,
}

impl WaitRequestV1 {
    /// Creates a wait over a nonempty canonical completion-signal set.
    pub fn new(
        identity: WaitRequestIdV1,
        context: OperationContextV1,
        mode: WaitModeV1,
        targets: Vec<CompletionSignalIdV1>,
        deadline_identity: Option<DeadlineIdV1>,
    ) -> Result<Self, HostContractErrorV1> {
        if targets.is_empty() {
            return Err(HostContractErrorV1::Empty {
                field: ContractFieldV1::WaitTargets,
            });
        }
        validate_strictly_ordered(&targets, MAX_WAIT_TARGETS_V1, ContractFieldV1::WaitTargets)?;
        Ok(Self {
            identity,
            context,
            mode,
            targets,
            deadline_identity,
        })
    }

    /// Returns the caller-supplied request commitment.
    pub const fn identity(&self) -> WaitRequestIdV1 {
        self.identity
    }

    /// Returns the parallel-operation context.
    pub const fn context(&self) -> &OperationContextV1 {
        &self.context
    }

    /// Returns the `Any` or `All` completion predicate.
    pub const fn mode(&self) -> WaitModeV1 {
        self.mode
    }

    /// Returns the nonempty canonical completion-signal set.
    pub fn targets(&self) -> &[CompletionSignalIdV1] {
        &self.targets
    }

    /// Returns the optional runtime-neutral deadline commitment.
    pub const fn deadline_identity(&self) -> Option<DeadlineIdV1> {
        self.deadline_identity
    }

    /// Checks that exact successful dispatch results produced every target.
    pub fn validate_dispatch_results(
        &self,
        dispatch_results: &[DispatchResultV1],
    ) -> Result<(), HostContractErrorV1> {
        if dispatch_results.len() != self.targets.len() {
            return Err(HostContractErrorV1::MissingDispatchResult);
        }
        for target in &self.targets {
            let matching = dispatch_results
                .iter()
                .filter(|result| {
                    matches!(
                        result.outcome(),
                        DispatchOutcomeV1::Submitted {
                            completion_signal_identity,
                            ..
                        } if completion_signal_identity == *target
                    )
                })
                .count();
            if matching != 1 {
                return Err(HostContractErrorV1::MissingDispatchResult);
            }
        }
        Ok(())
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(WaitRequestIdV1::DOMAIN_V1);
        self.context.encode(&mut encoder);
        encoder.u8(self.mode as u8);
        encoder.usize_as_u16(self.targets.len());
        for target in &self.targets {
            encoder.digest(target.digest());
        }
        encoder.optional_digest(self.deadline_identity.map(DeadlineIdV1::digest));
        check_preimage_bound(encoder.finish())
    }
}

/// Complete inert result of one V1 wait request.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WaitResultV1 {
    identity: WaitResultIdV1,
    request_identity: WaitRequestIdV1,
    mode: WaitModeV1,
    outcome: WaitOutcomeV1,
    diagnostics: Vec<HostDiagnosticV1>,
}

impl WaitResultV1 {
    /// Creates a wait result and checks target membership and predicate truth.
    pub fn new(
        identity: WaitResultIdV1,
        request: &WaitRequestV1,
        outcome: WaitOutcomeV1,
        diagnostics: Vec<HostDiagnosticV1>,
    ) -> Result<Self, HostContractErrorV1> {
        let require_error = matches!(outcome, WaitOutcomeV1::Rejected | WaitOutcomeV1::Failed);
        validate_diagnostics(&diagnostics, require_error)?;
        match &outcome {
            WaitOutcomeV1::Satisfied(observations) | WaitOutcomeV1::Pending(observations) => {
                validate_observations(request, observations)?;
                let satisfied = match request.mode {
                    WaitModeV1::Any => !observations.is_empty(),
                    WaitModeV1::All => observations.len() == request.targets.len(),
                };
                if satisfied != matches!(outcome, WaitOutcomeV1::Satisfied(_)) {
                    return Err(HostContractErrorV1::InvalidWaitDisposition);
                }
                if diagnostics
                    .iter()
                    .any(|diagnostic| diagnostic.severity() == DiagnosticSeverityV1::Error)
                {
                    return Err(HostContractErrorV1::InvalidOutcome);
                }
            }
            WaitOutcomeV1::Rejected | WaitOutcomeV1::Failed => {}
        }
        Ok(Self {
            identity,
            request_identity: request.identity,
            mode: request.mode,
            outcome,
            diagnostics,
        })
    }

    /// Returns the caller-supplied result commitment.
    pub const fn identity(&self) -> WaitResultIdV1 {
        self.identity
    }

    /// Returns the exact wait request commitment.
    pub const fn request_identity(&self) -> WaitRequestIdV1 {
        self.request_identity
    }

    /// Returns the request's completion predicate.
    pub const fn mode(&self) -> WaitModeV1 {
        self.mode
    }

    /// Returns the wait disposition and any terminal observations.
    pub const fn outcome(&self) -> &WaitOutcomeV1 {
        &self.outcome
    }

    /// Returns bounded diagnostics in producer order.
    pub fn diagnostics(&self) -> &[HostDiagnosticV1] {
        &self.diagnostics
    }

    /// Returns a flow-erased binding for terminal-state validation.
    pub const fn result_reference(&self) -> HostResultReferenceV1 {
        let class = match self.outcome {
            WaitOutcomeV1::Satisfied(_) | WaitOutcomeV1::Pending(_) => {
                OperationResultClassV1::Succeeded
            }
            WaitOutcomeV1::Rejected => OperationResultClassV1::Rejected,
            WaitOutcomeV1::Failed => OperationResultClassV1::Failed,
        };
        HostResultReferenceV1::new(
            HostResultIdentityV1::Wait(self.identity),
            HostRequestIdentityV1::Wait(self.request_identity),
            class,
        )
    }

    /// Encodes the bounded canonical identity preimage, excluding `identity`.
    pub fn encode_identity_preimage(&self) -> Vec<u8> {
        let mut encoder = EncoderV1::new(WaitResultIdV1::DOMAIN_V1);
        encoder.digest(self.request_identity.digest());
        encoder.u8(self.mode as u8);
        match &self.outcome {
            WaitOutcomeV1::Satisfied(observations) => {
                encoder.u8(1);
                encode_observations(observations, &mut encoder);
            }
            WaitOutcomeV1::Pending(observations) => {
                encoder.u8(2);
                encode_observations(observations, &mut encoder);
            }
            WaitOutcomeV1::Rejected => encoder.u8(3),
            WaitOutcomeV1::Failed => encoder.u8(4),
        }
        encode_diagnostics(&self.diagnostics, &mut encoder);
        check_preimage_bound(encoder.finish())
    }
}

fn validate_observations(
    request: &WaitRequestV1,
    observations: &[CompletionObservationV1],
) -> Result<(), HostContractErrorV1> {
    if observations.len() > MAX_WAIT_TARGETS_V1 {
        return Err(HostContractErrorV1::TooManyItems {
            field: ContractFieldV1::WaitObservations,
            actual: observations.len(),
            maximum: MAX_WAIT_TARGETS_V1,
        });
    }
    for pair in observations.windows(2) {
        if pair[0].signal_identity == pair[1].signal_identity {
            return Err(HostContractErrorV1::Duplicate {
                field: ContractFieldV1::WaitObservations,
            });
        }
        if pair[0].signal_identity > pair[1].signal_identity {
            return Err(HostContractErrorV1::NonCanonicalOrder {
                field: ContractFieldV1::WaitObservations,
            });
        }
    }
    if observations.iter().any(|observation| {
        request
            .targets
            .binary_search(&observation.signal_identity)
            .is_err()
    }) {
        return Err(HostContractErrorV1::UnknownWaitTarget);
    }
    Ok(())
}

fn encode_observations(observations: &[CompletionObservationV1], encoder: &mut EncoderV1) {
    encoder.usize_as_u16(observations.len());
    for observation in observations {
        observation.encode(encoder);
    }
}
