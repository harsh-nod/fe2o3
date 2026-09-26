//! SubjectV3 trust inputs; no service activation or cross-family fallback.
use crate::{
    CompilerExecutionAttestationErrorV3 as PolicyError,
    CompilerExecutionClientProfileErrorV1 as FramingError,
    CompilerExecutionExternalAnchorServiceIdentityV1 as Service,
    CompilerExecutionIssuerPolicyV3 as Policy,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
    client_profile_codec as codec, issuer_policy_codec,
    issuer_policy_v3::{
        COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3 as POLICY_STORAGE,
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK, RETAINED as POLICY_RETAINED,
    },
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

/// Exclusive production trust path. No environment override or V2/V1 fallback.
pub const COMPILER_EXECUTION_CLIENT_PROFILE_PATH_V3: &str =
    "/etc/fe2o3/compiler-execution/client-profile-v3";

crate::client_profile_adapter::client_profile_adapter!(
    V3,
    "3",
    "2",
    COMPILER_EXECUTION_CLIENT_PROFILE_BYTES_V3,
    COMPILER_EXECUTION_CLIENT_PROFILE_WORK_V3,
    COMPILER_EXECUTION_CLIENT_PROFILE_STORAGE_V3,
    CompilerExecutionClientProfileIdentityV3,
    CompilerExecutionClientProfileV3,
    CompilerExecutionClientProfileErrorV3
);
