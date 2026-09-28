//! Native V3 inert helper configuration bound to an actual anchor deployment.
use crate::{
    CompilerExecutionExternalAnchorDeploymentIdentityV3 as DeploymentIdentity,
    CompilerExecutionExternalAnchorDeploymentV3 as Deployment,
    CompilerExecutionExternalAnchorProvisioningErrorV1 as Framing,
    CompilerExecutionIssuerMeasurementV1 as Measurement,
    attestation_resources::{self as resources, CompilerExecutionAttestationStorageV3 as Storage},
    external_anchor_provisioning_codec as codec,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of};

crate::external_anchor_provisioning_adapter::external_anchor_provisioning_adapter!(
    V3,
    "3",
    "2",
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_BYTES_V3,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_WORK_V3,
    COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_STORAGE_V3,
    MAX_COMPILER_EXECUTION_EXTERNAL_ANCHOR_PROVISIONING_HELPER_BYTES_V3,
    CompilerExecutionExternalAnchorProvisioningIdentityV3,
    CompilerExecutionExternalAnchorProvisioningV3,
    CompilerExecutionExternalAnchorProvisioningErrorV3
);
