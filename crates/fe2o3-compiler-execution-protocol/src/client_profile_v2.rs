//! Native trust inputs; no service activation or fallback to a V1 profile.
use crate::{
    CompilerExecutionAttestationErrorV2 as PolicyError,
    CompilerExecutionClientProfileErrorV1 as FramingError,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerPolicyV2 as Policy,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV2 as Storage},
    client_profile_codec as codec, issuer_policy_codec,
    issuer_policy_v2::{
        COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2 as POLICY_STORAGE,
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as POLICY_WORK, RETAINED as POLICY_RETAINED,
    },
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

crate::client_profile_adapter::client_profile_adapter!(
    V2,
    "2",
    "1",
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V2,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V2,
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V2,
    CompilerExecutionClientProfileIdentityV2,
    CompilerExecutionClientProfileV2,
    CompilerExecutionClientProfileErrorV2
);
