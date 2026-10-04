//! V3-only fixed FD3..12 intake, including the original root's control endpoint.
use crate::{
    COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1 as CLIENT,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PEER_FD_V1 as ANCHOR,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PIDFD_V1 as ANCHOR_PID,
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1 as MANIFEST,
    COMPILER_EXECUTION_ISSUER_PEER_FD_V1 as PEER, COMPILER_EXECUTION_ISSUER_POLICY_FD_V1 as POLICY,
    COMPILER_EXECUTION_ISSUER_READY_FD_V1 as READY,
    COMPILER_EXECUTION_ISSUER_ROOT_CONTROL_FD_V3 as ROOT_CONTROL,
    COMPILER_EXECUTION_ISSUER_ROOT_FD_V1 as ROOT,
    COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1 as KEY,
    CompilerExecutionIssuerLaunchInputsV3 as Inputs,
    NATIVE_INHERITED_CHECK_WORK_V3 as INHERITED_CHECK_WORK,
    NATIVE_INHERITED_DESCRIPTORS_V3 as INHERITED_DESCRIPTORS,
    PRIVATE_DESCRIPTOR_FLOOR_V3 as PRIVATE_DESCRIPTOR_FLOOR, close_inherited,
};
use fe2o3_broker_authority_service::{
    ExpectedClientProcessIdentityV1 as Expected, LiveClientPidfdIdentityV2 as Client,
    ProtectedCompilerExecutionIssuerAdmissionV3 as Admission,
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor, ProtectedIssuerProcessV1 as Process,
    ProtectedServiceAdmissionV2 as Service,
};
use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV3 as Key;
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV3 as Policy;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{
    fmt,
    fs::File,
    os::fd::{OwnedFd, RawFd},
};

const ROOT_CONTROL_STORAGE: usize = Admission::ROOT_CONTROL_ENDPOINT_STORAGE;
type RootControl = OwnedFd;

fn require_inherited_table()
-> std::result::Result<(), crate::CompilerExecutionIssuerEntrypointErrorV1> {
    crate::require_inherited_table(&INHERITED_DESCRIPTORS)
}

fn take_inherited(
    fd: RawFd,
) -> std::result::Result<OwnedFd, crate::CompilerExecutionIssuerEntrypointErrorV1> {
    crate::take_inherited_at_floor(fd, PRIVATE_DESCRIPTOR_FLOOR)
}

fn take_root_control() -> Result<RootControl> {
    Ok(take_inherited(ROOT_CONTROL)?)
}

fn serve(
    admission: Admission<'_>,
    inputs: &Inputs,
    writer: OwnedFd,
    root_control: RootControl,
    b: &mut Budget<'_>,
) -> Result<()> {
    Ok(admission.serve_native_with_root_readiness(inputs.manifest(), writer, root_control, b)?)
}

type Error = CompilerExecutionIssuerEntrypointErrorV3;
type Result<T> = std::result::Result<T, Error>;

use crate::CompilerExecutionIssuerLaunchInputErrorV3 as InputError;
use fe2o3_broker_authority_service::{
    ProtectedCompilerExecutionIssuerAdmissionErrorV3 as AdmissionError,
    ProtectedCompilerExecutionIssuerServiceErrorV3 as ServiceError,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionAttestationErrorV3 as PolicyError;

super::native_adapter::native_entrypoint!(
    run_inherited_compiler_execution_issuer_v3,
    CompilerExecutionIssuerEntrypointErrorV3
);
