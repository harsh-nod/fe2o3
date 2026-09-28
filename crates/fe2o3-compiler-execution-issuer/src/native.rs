//! Native entrypoint over the production launcher's existing fixed descriptor ABI.
use crate::{
    COMPILER_EXECUTION_ISSUER_CLIENT_PIDFD_V1 as CLIENT,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PEER_FD_V1 as ANCHOR,
    COMPILER_EXECUTION_ISSUER_EXTERNAL_ANCHOR_PIDFD_V1 as ANCHOR_PID,
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1 as MANIFEST,
    COMPILER_EXECUTION_ISSUER_PEER_FD_V1 as PEER, COMPILER_EXECUTION_ISSUER_POLICY_FD_V1 as POLICY,
    COMPILER_EXECUTION_ISSUER_READY_FD_V1 as READY, COMPILER_EXECUTION_ISSUER_ROOT_FD_V1 as ROOT,
    COMPILER_EXECUTION_ISSUER_SIGNING_KEY_FD_V1 as KEY,
    CompilerExecutionIssuerLaunchInputsV2 as Inputs, close_inherited, take_inherited,
};
use fe2o3_broker_authority_service::{
    ExpectedClientProcessIdentityV1 as Expected, LiveClientPidfdIdentityV2 as Client,
    ProtectedCompilerExecutionIssuerAdmissionV2 as Admission,
    ProtectedExternalAnchorServiceAdmissionV2 as Anchor, ProtectedIssuerProcessV1 as Process,
    ProtectedServiceAdmissionV2 as Service,
};
use fe2o3_compiler_closure_capability::CompilerExecutionSigningKeyCapabilityV2 as Key;
use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2 as Policy;
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{fmt, fs::File};

type Error = CompilerExecutionIssuerEntrypointErrorV2;
type Result<T> = std::result::Result<T, Error>;

use crate::CompilerExecutionIssuerLaunchInputErrorV2 as InputError;
use fe2o3_broker_authority_service::{
    ProtectedCompilerExecutionIssuerAdmissionErrorV2 as AdmissionError,
    ProtectedCompilerExecutionIssuerServiceErrorV2 as ServiceError,
};
use fe2o3_compiler_execution_protocol::CompilerExecutionAttestationErrorV2 as PolicyError;

super::native_adapter::native_entrypoint!(
    run_inherited_compiler_execution_issuer_v2,
    CompilerExecutionIssuerEntrypointErrorV2
);
