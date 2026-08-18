//! Shared bounded records and flow-erased identity references.

use alloc::{string::String, vec::Vec};
use core::fmt;

use crate::canonical::EncoderV1;
use crate::{
    AdmitEventIdV1, AdmitRequestIdV1, AdmitResultIdV1, AdmitStateIdV1, CompileEventIdV1,
    CompileRequestIdV1, CompileResultIdV1, CompileStateIdV1, DispatchEventIdV1,
    DispatchRequestIdV1, DispatchResultIdV1, DispatchStateIdV1, FlowScopeIdV1, HostDigestV1,
    LoadEventIdV1, LoadRequestIdV1, LoadResultIdV1, LoadStateIdV1, OperationIdV1,
    PayloadFormatIdV1, PayloadIdV1, ResourceIdV1, WaitEventIdV1, WaitRequestIdV1, WaitResultIdV1,
    WaitStateIdV1,
};

/// Hard maximum bytes described by one V1 payload descriptor.
pub const MAX_PAYLOAD_BYTES_V1: u64 = 256 * 1024 * 1024;
/// Hard maximum diagnostic count in one V1 result.
pub const MAX_DIAGNOSTICS_V1: usize = 64;
/// Hard maximum UTF-8 byte length of one diagnostic message.
pub const MAX_DIAGNOSTIC_MESSAGE_BYTES_V1: usize = 1_024;
/// Hard maximum number of causal events attached to one operation.
pub const MAX_CAUSAL_EVENTS_V1: usize = 64;
/// Upper bound for an identity preimage returned by a valid V1 record.
pub const MAX_IDENTITY_PREIMAGE_BYTES_V1: usize = 128 * 1024;

/// A field named by a stable V1 structural-validation error.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[non_exhaustive]
pub enum ContractFieldV1 {
    /// Operation attempt number.
    OperationAttempt,
    /// Parent operation identity.
    ParentOperation,
    /// Causal event collection.
    CausalEvents,
    /// Payload byte length.
    PayloadBytes,
    /// Diagnostic code.
    DiagnosticCode,
    /// Diagnostic message.
    DiagnosticMessage,
    /// Diagnostic collection.
    Diagnostics,
    /// Compile output size limit.
    CompileOutputBytes,
    /// Admission claim collection.
    AdmissionClaims,
    /// Load generation.
    LoadGeneration,
    /// Resource binding byte range.
    ResourceRange,
    /// Dispatch resource bindings.
    DispatchBindings,
    /// Dispatch completion dependencies.
    DispatchDependencies,
    /// Persistent service epoch.
    ServiceEpoch,
    /// Wait completion targets.
    WaitTargets,
    /// Wait completion observations.
    WaitObservations,
    /// Event batch.
    EventBatch,
    /// Request identity carried by a result or state.
    RequestIdentity,
    /// Upstream result identity.
    UpstreamResult,
    /// Upstream payload or object identity.
    UpstreamObject,
    /// Host flow kind.
    Flow,
    /// Operation identity.
    Operation,
    /// Flow scope identity.
    Scope,
    /// State revision.
    StateRevision,
    /// State predecessor identity.
    StatePredecessor,
    /// State identity.
    StateIdentity,
    /// Terminal result identity or disposition.
    TerminalResult,
}

/// Structural rejection from a V1 host API constructor or relation check.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum HostContractErrorV1 {
    /// A required scalar or collection was empty or zero.
    Empty {
        /// Rejected field.
        field: ContractFieldV1,
    },
    /// A hard V1 item-count limit was exceeded.
    TooManyItems {
        /// Rejected collection.
        field: ContractFieldV1,
        /// Observed item count.
        actual: usize,
        /// Hard maximum item count.
        maximum: usize,
    },
    /// A hard V1 scalar or byte-length limit was exceeded.
    LimitExceeded {
        /// Rejected field.
        field: ContractFieldV1,
        /// Observed value.
        actual: u64,
        /// Hard maximum value.
        maximum: u64,
    },
    /// Checked arithmetic rejected an overflowing byte range or revision.
    ArithmeticOverflow {
        /// Rejected field.
        field: ContractFieldV1,
    },
    /// A canonical ordered collection was not strictly increasing.
    NonCanonicalOrder {
        /// Rejected collection.
        field: ContractFieldV1,
    },
    /// A set-like collection contained a duplicate.
    Duplicate {
        /// Rejected collection.
        field: ContractFieldV1,
    },
    /// An identity, flow, outcome, or predecessor did not match its source.
    Mismatch {
        /// Rejected relation.
        field: ContractFieldV1,
    },
    /// A diagnostic message contained a forbidden NUL byte.
    DiagnosticContainsNul,
    /// A rejection or failure omitted its required diagnostic.
    MissingFailureDiagnostic,
    /// An outcome carried fields forbidden for that outcome.
    InvalidOutcome,
    /// A state was not the unique valid revision-zero initial state.
    InvalidInitialState,
    /// A state transition was not permitted by the V1 transition relation.
    InvalidStateTransition,
    /// A state transition attempted to leave a terminal state.
    TerminalStateTransition,
    /// A wait observation did not name one of the request's targets.
    UnknownWaitTarget,
    /// A wait disposition contradicted its mode and observation set.
    InvalidWaitDisposition,
    /// A dispatch result needed by a wait request was absent or inconsistent.
    MissingDispatchResult,
}

impl fmt::Display for HostContractErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "invalid V1 host contract: {self:?}")
    }
}

impl core::error::Error for HostContractErrorV1 {}

/// One of the five V1 host orchestration flows.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum FlowKindV1 {
    /// Compiler request/result flow.
    Compile = 1,
    /// Artifact admission-assessment flow.
    Admit = 2,
    /// Runtime-neutral load-description flow.
    Load = 3,
    /// Finite-kernel or persistent-task dispatch-description flow.
    Dispatch = 4,
    /// Completion-observation flow.
    Wait = 5,
}

/// Flow-erased request identity retaining its domain-specific type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostRequestIdentityV1 {
    /// Compile request identity.
    Compile(CompileRequestIdV1),
    /// Admission request identity.
    Admit(AdmitRequestIdV1),
    /// Load request identity.
    Load(LoadRequestIdV1),
    /// Dispatch request identity.
    Dispatch(DispatchRequestIdV1),
    /// Wait request identity.
    Wait(WaitRequestIdV1),
}

/// Flow-erased result identity retaining its domain-specific type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostResultIdentityV1 {
    /// Compile result identity.
    Compile(CompileResultIdV1),
    /// Admission result identity.
    Admit(AdmitResultIdV1),
    /// Load result identity.
    Load(LoadResultIdV1),
    /// Dispatch result identity.
    Dispatch(DispatchResultIdV1),
    /// Wait result identity.
    Wait(WaitResultIdV1),
}

/// Flow-erased state identity retaining its domain-specific type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostStateIdentityV1 {
    /// Compile state identity.
    Compile(CompileStateIdV1),
    /// Admission state identity.
    Admit(AdmitStateIdV1),
    /// Load state identity.
    Load(LoadStateIdV1),
    /// Dispatch state identity.
    Dispatch(DispatchStateIdV1),
    /// Wait state identity.
    Wait(WaitStateIdV1),
}

/// Flow-erased event identity retaining its domain-specific type.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum HostEventIdentityV1 {
    /// Compile event identity.
    Compile(CompileEventIdV1),
    /// Admission event identity.
    Admit(AdmitEventIdV1),
    /// Load event identity.
    Load(LoadEventIdV1),
    /// Dispatch event identity.
    Dispatch(DispatchEventIdV1),
    /// Wait event identity.
    Wait(WaitEventIdV1),
}

macro_rules! erased_identity_methods {
    ($name:ident, $($variant:ident),+ $(,)?) => {
        impl $name {
            /// Returns the flow encoded by this typed identity.
            pub const fn flow(self) -> FlowKindV1 {
                match self {
                    $(Self::$variant(_) => FlowKindV1::$variant,)+
                }
            }

            /// Returns the opaque digest without authenticating it.
            pub const fn digest(self) -> HostDigestV1 {
                match self {
                    $(Self::$variant(identity) => identity.digest(),)+
                }
            }
        }
    };
}

erased_identity_methods!(HostRequestIdentityV1, Compile, Admit, Load, Dispatch, Wait);
erased_identity_methods!(HostResultIdentityV1, Compile, Admit, Load, Dispatch, Wait);
erased_identity_methods!(HostStateIdentityV1, Compile, Admit, Load, Dispatch, Wait);
erased_identity_methods!(HostEventIdentityV1, Compile, Admit, Load, Dispatch, Wait);

/// Context shared by retries and causally related parallel operations.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OperationContextV1 {
    scope_id: FlowScopeIdV1,
    operation_id: OperationIdV1,
    attempt: u16,
    parent_operation_id: Option<OperationIdV1>,
    causal_events: Vec<HostEventIdentityV1>,
}

impl OperationContextV1 {
    /// Creates a bounded context with a canonical causal-event set.
    pub fn new(
        scope_id: FlowScopeIdV1,
        operation_id: OperationIdV1,
        attempt: u16,
        parent_operation_id: Option<OperationIdV1>,
        causal_events: Vec<HostEventIdentityV1>,
    ) -> Result<Self, HostContractErrorV1> {
        if attempt == 0 {
            return Err(HostContractErrorV1::Empty {
                field: ContractFieldV1::OperationAttempt,
            });
        }
        if parent_operation_id == Some(operation_id) {
            return Err(HostContractErrorV1::Mismatch {
                field: ContractFieldV1::ParentOperation,
            });
        }
        validate_strictly_ordered(
            &causal_events,
            MAX_CAUSAL_EVENTS_V1,
            ContractFieldV1::CausalEvents,
        )?;
        Ok(Self {
            scope_id,
            operation_id,
            attempt,
            parent_operation_id,
            causal_events,
        })
    }

    /// Returns the namespace in which this operation can run in parallel.
    pub const fn scope_id(&self) -> FlowScopeIdV1 {
        self.scope_id
    }

    /// Returns the logical operation identity.
    pub const fn operation_id(&self) -> OperationIdV1 {
        self.operation_id
    }

    /// Returns the nonzero retry attempt.
    pub const fn attempt(&self) -> u16 {
        self.attempt
    }

    /// Returns the optional logical parent operation.
    pub const fn parent_operation_id(&self) -> Option<OperationIdV1> {
        self.parent_operation_id
    }

    /// Returns the canonical causal-event set.
    pub fn causal_events(&self) -> &[HostEventIdentityV1] {
        &self.causal_events
    }

    pub(crate) fn encode(&self, encoder: &mut EncoderV1) {
        encoder.digest(self.scope_id.digest());
        encoder.digest(self.operation_id.digest());
        encoder.u16(self.attempt);
        encoder.optional_digest(self.parent_operation_id.map(OperationIdV1::digest));
        encoder.usize_as_u16(self.causal_events.len());
        for event in &self.causal_events {
            encoder.u8(event.flow() as u8);
            encoder.digest(event.digest());
        }
    }
}

/// Bounded description of an opaque payload.
///
/// This descriptor neither owns bytes nor establishes their identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PayloadDescriptorV1 {
    identity: PayloadIdV1,
    format_identity: PayloadFormatIdV1,
    byte_len: u64,
}

impl PayloadDescriptorV1 {
    /// Creates a nonempty, hard-bounded payload description.
    pub fn new(
        identity: PayloadIdV1,
        format_identity: PayloadFormatIdV1,
        byte_len: u64,
    ) -> Result<Self, HostContractErrorV1> {
        if byte_len == 0 {
            return Err(HostContractErrorV1::Empty {
                field: ContractFieldV1::PayloadBytes,
            });
        }
        if byte_len > MAX_PAYLOAD_BYTES_V1 {
            return Err(HostContractErrorV1::LimitExceeded {
                field: ContractFieldV1::PayloadBytes,
                actual: byte_len,
                maximum: MAX_PAYLOAD_BYTES_V1,
            });
        }
        Ok(Self {
            identity,
            format_identity,
            byte_len,
        })
    }

    /// Returns the declared payload commitment.
    pub const fn identity(self) -> PayloadIdV1 {
        self.identity
    }

    /// Returns the declared payload-format commitment.
    pub const fn format_identity(self) -> PayloadFormatIdV1 {
        self.format_identity
    }

    /// Returns the declared byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub(crate) fn encode(self, encoder: &mut EncoderV1) {
        encoder.digest(self.identity.digest());
        encoder.digest(self.format_identity.digest());
        encoder.u64(self.byte_len);
    }
}

/// Severity of one stable host diagnostic.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum DiagnosticSeverityV1 {
    /// Informational observation.
    Note = 1,
    /// Nonfatal warning.
    Warning = 2,
    /// Operation rejection or failure.
    Error = 3,
}

/// Nonempty, bounded UTF-8 diagnostic text.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct DiagnosticMessageV1(String);

impl DiagnosticMessageV1 {
    /// Creates bounded text and rejects embedded NUL bytes.
    pub fn new(message: String) -> Result<Self, HostContractErrorV1> {
        if message.is_empty() {
            return Err(HostContractErrorV1::Empty {
                field: ContractFieldV1::DiagnosticMessage,
            });
        }
        if message.len() > MAX_DIAGNOSTIC_MESSAGE_BYTES_V1 {
            return Err(HostContractErrorV1::LimitExceeded {
                field: ContractFieldV1::DiagnosticMessage,
                actual: message.len() as u64,
                maximum: MAX_DIAGNOSTIC_MESSAGE_BYTES_V1 as u64,
            });
        }
        if message.as_bytes().contains(&0) {
            return Err(HostContractErrorV1::DiagnosticContainsNul);
        }
        Ok(Self(message))
    }

    /// Returns the diagnostic text.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One bounded, stable-code host diagnostic.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HostDiagnosticV1 {
    code: u32,
    severity: DiagnosticSeverityV1,
    subject: Option<HostDigestV1>,
    message: DiagnosticMessageV1,
}

impl HostDiagnosticV1 {
    /// Creates a diagnostic with a nonzero stable code.
    pub fn new(
        code: u32,
        severity: DiagnosticSeverityV1,
        subject: Option<HostDigestV1>,
        message: DiagnosticMessageV1,
    ) -> Result<Self, HostContractErrorV1> {
        if code == 0 {
            return Err(HostContractErrorV1::Empty {
                field: ContractFieldV1::DiagnosticCode,
            });
        }
        Ok(Self {
            code,
            severity,
            subject,
            message,
        })
    }

    /// Returns the stable numeric diagnostic code.
    pub const fn code(&self) -> u32 {
        self.code
    }

    /// Returns the diagnostic severity.
    pub const fn severity(&self) -> DiagnosticSeverityV1 {
        self.severity
    }

    /// Returns the optional opaque subject commitment.
    pub const fn subject(&self) -> Option<HostDigestV1> {
        self.subject
    }

    /// Returns the bounded diagnostic text.
    pub const fn message(&self) -> &DiagnosticMessageV1 {
        &self.message
    }

    pub(crate) fn encode(&self, encoder: &mut EncoderV1) {
        encoder.u32(self.code);
        encoder.u8(self.severity as u8);
        encoder.optional_digest(self.subject);
        encoder.text(self.message.as_str());
    }
}

/// Access class declared for a dispatch resource binding.
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
#[repr(u8)]
pub enum AccessModeV1 {
    /// Read-only access.
    Read = 1,
    /// Write-only access.
    Write = 2,
    /// Read/write access.
    ReadWrite = 3,
    /// Atomic read/write access under a separately identified contract.
    Atomic = 4,
}

/// One runtime-neutral resource range bound to a dispatch argument ordinal.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceBindingV1 {
    ordinal: u16,
    resource_identity: ResourceIdV1,
    access: AccessModeV1,
    byte_offset: u64,
    byte_len: u64,
}

impl ResourceBindingV1 {
    /// Creates a nonempty resource range and rejects offset overflow.
    pub fn new(
        ordinal: u16,
        resource_identity: ResourceIdV1,
        access: AccessModeV1,
        byte_offset: u64,
        byte_len: u64,
    ) -> Result<Self, HostContractErrorV1> {
        if byte_len == 0 {
            return Err(HostContractErrorV1::Empty {
                field: ContractFieldV1::ResourceRange,
            });
        }
        byte_offset
            .checked_add(byte_len)
            .ok_or(HostContractErrorV1::ArithmeticOverflow {
                field: ContractFieldV1::ResourceRange,
            })?;
        Ok(Self {
            ordinal,
            resource_identity,
            access,
            byte_offset,
            byte_len,
        })
    }

    /// Returns the canonical argument ordinal.
    pub const fn ordinal(self) -> u16 {
        self.ordinal
    }

    /// Returns the opaque resource commitment.
    pub const fn resource_identity(self) -> ResourceIdV1 {
        self.resource_identity
    }

    /// Returns the declared access class.
    pub const fn access(self) -> AccessModeV1 {
        self.access
    }

    /// Returns the first byte offset.
    pub const fn byte_offset(self) -> u64 {
        self.byte_offset
    }

    /// Returns the nonzero byte length.
    pub const fn byte_len(self) -> u64 {
        self.byte_len
    }

    pub(crate) fn encode(self, encoder: &mut EncoderV1) {
        encoder.u16(self.ordinal);
        encoder.digest(self.resource_identity.digest());
        encoder.u8(self.access as u8);
        encoder.u64(self.byte_offset);
        encoder.u64(self.byte_len);
    }
}

/// Terminal class used to bind a state to a result disposition.
#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(u8)]
pub enum OperationResultClassV1 {
    /// The host operation produced its described success result.
    Succeeded = 1,
    /// The request was structurally or policy rejected.
    Rejected = 2,
    /// The described implementation reported an operational failure.
    Failed = 3,
}

/// Flow-erased exact result binding used by terminal state validation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostResultReferenceV1 {
    identity: HostResultIdentityV1,
    request_identity: HostRequestIdentityV1,
    class: OperationResultClassV1,
}

impl HostResultReferenceV1 {
    /// Returns the exact result identity.
    pub const fn identity(self) -> HostResultIdentityV1 {
        self.identity
    }

    /// Returns the exact request identity named by the result.
    pub const fn request_identity(self) -> HostRequestIdentityV1 {
        self.request_identity
    }

    /// Returns the result's terminal class.
    pub const fn class(self) -> OperationResultClassV1 {
        self.class
    }

    pub(crate) const fn new(
        identity: HostResultIdentityV1,
        request_identity: HostRequestIdentityV1,
        class: OperationResultClassV1,
    ) -> Self {
        Self {
            identity,
            request_identity,
            class,
        }
    }
}

pub(crate) fn validate_diagnostics(
    diagnostics: &[HostDiagnosticV1],
    require_error: bool,
) -> Result<(), HostContractErrorV1> {
    if diagnostics.len() > MAX_DIAGNOSTICS_V1 {
        return Err(HostContractErrorV1::TooManyItems {
            field: ContractFieldV1::Diagnostics,
            actual: diagnostics.len(),
            maximum: MAX_DIAGNOSTICS_V1,
        });
    }
    if require_error
        && !diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverityV1::Error)
    {
        return Err(HostContractErrorV1::MissingFailureDiagnostic);
    }
    Ok(())
}

pub(crate) fn encode_diagnostics(diagnostics: &[HostDiagnosticV1], encoder: &mut EncoderV1) {
    encoder.usize_as_u16(diagnostics.len());
    for diagnostic in diagnostics {
        diagnostic.encode(encoder);
    }
}

pub(crate) fn validate_strictly_ordered<T: Ord>(
    values: &[T],
    maximum: usize,
    field: ContractFieldV1,
) -> Result<(), HostContractErrorV1> {
    if values.len() > maximum {
        return Err(HostContractErrorV1::TooManyItems {
            field,
            actual: values.len(),
            maximum,
        });
    }
    for pair in values.windows(2) {
        if pair[0] == pair[1] {
            return Err(HostContractErrorV1::Duplicate { field });
        }
        if pair[0] > pair[1] {
            return Err(HostContractErrorV1::NonCanonicalOrder { field });
        }
    }
    Ok(())
}

pub(crate) fn check_preimage_bound(bytes: Vec<u8>) -> Vec<u8> {
    debug_assert!(bytes.len() <= MAX_IDENTITY_PREIMAGE_BYTES_V1);
    bytes
}
