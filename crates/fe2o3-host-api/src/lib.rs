#![no_std]
#![forbid(unsafe_code)]
#![deny(missing_docs)]

//! Target-neutral, runtime-neutral host orchestration contracts.
//!
//! These records describe compile, admission, load, dispatch, and wait flows.
//! They execute nothing and grant no authority. Caller-supplied commitments
//! remain untrusted even after every structural invariant in this crate holds.

extern crate alloc;

mod admit;
mod canonical;
mod common;
mod compile;
mod dispatch;
mod event;
mod identity;
mod load;
mod state;
mod wait;

pub use admit::{AdmitOutcomeV1, AdmitRequestV1, AdmitResultV1, MAX_ADMISSION_CLAIMS_V1};
pub use common::{
    AccessModeV1, ContractFieldV1, DiagnosticMessageV1, DiagnosticSeverityV1, FlowKindV1,
    HostContractErrorV1, HostDiagnosticV1, HostEventIdentityV1, HostRequestIdentityV1,
    HostResultIdentityV1, HostResultReferenceV1, HostStateIdentityV1, MAX_CAUSAL_EVENTS_V1,
    MAX_DIAGNOSTIC_MESSAGE_BYTES_V1, MAX_DIAGNOSTICS_V1, MAX_IDENTITY_PREIMAGE_BYTES_V1,
    MAX_PAYLOAD_BYTES_V1, OperationContextV1, OperationResultClassV1, PayloadDescriptorV1,
    ResourceBindingV1,
};
pub use compile::{CompileOutcomeV1, CompileRequestV1, CompileResultV1};
pub use dispatch::{
    DispatchDependencyV1, DispatchKindV1, DispatchOutcomeV1, DispatchRequestV1, DispatchResultV1,
    MAX_DISPATCH_BINDINGS_V1, MAX_DISPATCH_DEPENDENCIES_V1,
};
pub use event::{HostEventBatchV1, HostEventV1, MAX_EVENT_BATCH_V1};
pub use identity::*;
pub use load::{LoadOutcomeV1, LoadRequestV1, LoadResultV1};
pub use state::{HostOperationStateV1, OperationPhaseV1};
pub use wait::{
    CompletionObservationV1, CompletionStatusV1, MAX_WAIT_TARGETS_V1, WaitModeV1, WaitOutcomeV1,
    WaitRequestV1, WaitResultV1,
};

/// Schema version represented by all V1 host API types.
pub const HOST_API_SCHEMA_VERSION_V1: u16 = 1;
