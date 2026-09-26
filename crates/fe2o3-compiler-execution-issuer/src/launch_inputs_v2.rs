//! Native launch-input custody, not activation of the protected issuer service.
use crate::{
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1 as LAUNCH_FD,
    COMPILER_EXECUTION_ISSUER_POLICY_FD_V1 as POLICY_FD,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as CapabilityError,
    CompilerExecutionCapabilityStorageV2 as CapabilityStorage,
    CompilerExecutionPolicyCapabilityV2 as PolicyCapability,
    CompilerExecutionServiceLaunchCapabilityV2 as LaunchCapability,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V2 as POLICY_BYTES,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V2 as MANIFEST_BYTES,
    CompilerExecutionIssuerPolicyV2 as Policy,
    CompilerExecutionServiceLaunchManifestErrorV2 as ManifestError,
    CompilerExecutionServiceLaunchManifestV2 as Manifest,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of, os::fd::RawFd};

/// Inert, move-only native inputs freshly admitted from fixed launch slots 6/8.
/// The two independently decoded images must name the same native policy.
/// No legacy-policy fallback or conversion of an admitted legacy owner exists.
/// Agreement does not independently pin policy provenance: a consistently
/// replaced pair also agrees. Trusted installation and program admission must
/// supply that pin before service activation.
///
/// This does not authenticate the supervisor, client, anchor, executable, key,
/// process profile or durable state. It cannot sign, publish readiness, serve,
/// load, or launch a kernel. The production V1 serving entrypoint is unchanged.
///
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::CompilerExecutionIssuerLaunchInputsV2;
/// fn duplicate(value: CompilerExecutionIssuerLaunchInputsV2) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::CompilerExecutionIssuerLaunchInputsV2;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<CompilerExecutionIssuerLaunchInputsV2>();
/// ```
pub struct CompilerExecutionIssuerLaunchInputsV2 {
    policy: PolicyCapability,
    launch: LaunchCapability,
}

#[path = "launch_inputs_native.rs"]
mod adapter;
adapter::launch_inputs!(
    CompilerExecutionIssuerLaunchInputsV2,
    CompilerExecutionIssuerLaunchInputStorageV2,
    CompilerExecutionIssuerLaunchInputErrorV2
);

#[cfg(test)]
mod tests {
    use super::{
        CompilerExecutionIssuerLaunchInputErrorV2 as InputError,
        CompilerExecutionIssuerLaunchInputsV2 as Inputs,
    };
    use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2::{
        Launch as LaunchDecodeError, Policy as PolicyDecodeError,
    };
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V2 as POLICY_STORAGE,
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V2 as POLICY_WORK,
        COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V2 as MANIFEST_STORAGE,
        COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V2 as MANIFEST_WORK,
        CompilerExecutionAttestationErrorV2 as PolicyError,
        CompilerExecutionIssuerPolicyV3 as OtherPolicy,
        CompilerExecutionServiceLaunchManifestV3 as OtherManifest,
    };
    const CHILD_TEST: &str = "launch_inputs_v2::tests::inherited_slot_child";
    include!("launch_inputs_tests.rs");
}
