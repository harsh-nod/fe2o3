//! Nominal V3 service custody over shared listener predicates and bounded acceptance.
use super::{
    ListenerFilesystemPolicyV1 as FilesystemPolicy, ProtectedIssuerSocketCustodyV1 as Socket,
    ProtectedIssuerSocketStateV1 as SocketState, SocketError,
    native_accept::{self as accept, AcceptError},
    require_socket_state,
};
use crate::{
    AcceptedCompilerExecutionHandoffV3 as Accepted, ProtectedIssuerCleanupServiceV2 as Cleanup,
    ProtectedIssuerSessionErrorV3 as SessionError, ProtectedIssuerSessionLimitsV3 as SessionLimits,
    ProtectedIssuerSupervisorErrorV3 as SupervisorError, ProtectedIssuerSupervisorV3 as Supervisor,
    ProtectedIssuerWaitV3 as Wait,
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
    ProtectedIssuerServiceV3,
    ProtectedIssuerServiceStorageV3,
    ProtectedIssuerSessionReportV3,
    ProtectedIssuerServiceErrorV3
);
use ProtectedIssuerServiceErrorV3 as Error;
use ProtectedIssuerServiceStorageV3 as Storage;
use ProtectedIssuerServiceV3 as Service;
use ProtectedIssuerSessionReportV3 as Report;
type Result<T> = std::result::Result<T, Error>;

include!("listener_native_body.rs");

#[path = "listener_native_controller_adapter.rs"]
mod controller;
controller::native_controller!(
    ProtectedIssuerDispatchLimitsV3,
    ProtectedIssuerDispatchReportV3,
    ProtectedIssuerDispatchStopV3
);
use ProtectedIssuerDispatchLimitsV3 as DispatchLimits;
use ProtectedIssuerDispatchReportV3 as DispatchReport;
use ProtectedIssuerDispatchStopV3 as DispatchStop;
include!("listener_native_controller_body.rs");

#[path = "listener_native_application_v3.rs"]
pub(crate) mod application;
