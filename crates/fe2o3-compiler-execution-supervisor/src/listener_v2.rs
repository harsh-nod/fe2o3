//! Nominal V2 service custody over shared listener predicates and bounded acceptance.
use super::{
    ListenerFilesystemPolicyV1 as FilesystemPolicy, ProtectedIssuerSocketCustodyV1 as Socket,
    ProtectedIssuerSocketStateV1 as SocketState, SocketError,
    native_accept::{self as accept, AcceptError},
    require_socket_state,
};
use crate::{
    AcceptedCompilerExecutionHandoffV2 as Accepted, ProtectedIssuerCleanupServiceV2 as Cleanup,
    ProtectedIssuerSessionErrorV2 as SessionError, ProtectedIssuerSessionLimitsV2 as SessionLimits,
    ProtectedIssuerSupervisorErrorV2 as SupervisorError, ProtectedIssuerSupervisorV2 as Supervisor,
    ProtectedIssuerWaitV2 as Wait,
};
use fe2o3_compiler_execution_protocol::COMPILER_EXECUTION_SUPERVISOR_SOCKET_PATH_V1 as SOCKET_PATH;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{mem::size_of, os::fd::OwnedFd, path::Path};

#[path = "listener_native_adapter.rs"]
mod adapter;
adapter::native_service!(
    ProtectedIssuerServiceV2,
    ProtectedIssuerServiceStorageV2,
    ProtectedIssuerSessionReportV2,
    ProtectedIssuerServiceErrorV2
);
use ProtectedIssuerServiceErrorV2 as Error;
use ProtectedIssuerServiceStorageV2 as Storage;
use ProtectedIssuerServiceV2 as Service;
use ProtectedIssuerSessionReportV2 as Report;
type Result<T> = std::result::Result<T, Error>;

include!("listener_native_body.rs");

#[path = "listener_native_controller_adapter.rs"]
mod controller;
controller::native_controller!(
    ProtectedIssuerDispatchLimitsV2,
    ProtectedIssuerDispatchReportV2,
    ProtectedIssuerDispatchStopV2
);
use ProtectedIssuerDispatchLimitsV2 as DispatchLimits;
use ProtectedIssuerDispatchReportV2 as DispatchReport;
use ProtectedIssuerDispatchStopV2 as DispatchStop;
include!("listener_native_controller_body.rs");
