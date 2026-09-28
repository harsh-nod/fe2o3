//! SubjectV3 policy admission. Decoding is not provisioning or execution proof.
use crate::{
    CompilerExecutionAttestationErrorV1, CompilerExecutionIssuerMeasurementV1 as Measurement,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
    issuer_policy_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

use fe2o3_artifact_transaction::CompilerExecutionSubjectErrorV3 as SubjectError;
crate::issuer_policy_adapter::issuer_policy_adapter!(
    V3,
    "3",
    "5",
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V3,
    COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3,
    COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3,
    CompilerExecutionIssuerPolicyIdentityV3,
    CompilerExecutionIssuerPolicyV3,
    CompilerExecutionAttestationErrorV3
);
