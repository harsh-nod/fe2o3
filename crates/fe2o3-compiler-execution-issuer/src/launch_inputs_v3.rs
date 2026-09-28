//! Native launch-input custody, not activation of the protected issuer service.
use crate::{
    COMPILER_EXECUTION_ISSUER_LAUNCH_MANIFEST_FD_V1 as LAUNCH_FD,
    COMPILER_EXECUTION_ISSUER_POLICY_FD_V1 as POLICY_FD,
};
use fe2o3_compiler_closure_capability::{
    CompilerExecutionCapabilityErrorV2 as CapabilityError,
    CompilerExecutionCapabilityStorageV2 as CapabilityStorage,
    CompilerExecutionPolicyCapabilityV3 as PolicyCapability,
    CompilerExecutionServiceLaunchCapabilityV3 as LaunchCapability,
};
use fe2o3_compiler_execution_protocol::{
    COMPILER_EXECUTION_ISSUER_POLICY_BYTES_V3 as POLICY_BYTES,
    COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_BYTES_V3 as MANIFEST_BYTES,
    CompilerExecutionIssuerPolicyV3 as Policy,
    CompilerExecutionServiceLaunchManifestErrorV3 as ManifestError,
    CompilerExecutionServiceLaunchManifestV3 as Manifest,
};
use fe2o3_kernel_ir::{
    CanonicalKernelIrVerificationResourceBudgetV1 as Budget,
    CanonicalKernelIrVerificationResourceErrorV1 as Resource,
};
use std::{error::Error, fmt, mem::size_of, os::fd::RawFd};

/// Both owners use actual SubjectV3 policy and manifest types. The shared launch
/// wire does not establish a family: the policy is independently decoded as V3
/// and the manifest must bind that exact policy identity.
///
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
/// use fe2o3_compiler_execution_issuer::CompilerExecutionIssuerLaunchInputsV3;
/// fn duplicate(value: CompilerExecutionIssuerLaunchInputsV3) { let _ = value.clone(); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::CompilerExecutionIssuerLaunchInputsV3;
/// fn descriptor<T: std::os::fd::AsFd>() {}
/// descriptor::<CompilerExecutionIssuerLaunchInputsV3>();
/// ```
/// ```
/// use fe2o3_compiler_execution_issuer::{
///     CompilerExecutionIssuerLaunchInputsV3 as Inputs,
///     CompilerExecutionIssuerLaunchInputStorageV3 as Storage,
///     CompilerExecutionIssuerLaunchInputErrorV3 as Error,
/// };
/// use fe2o3_compiler_execution_protocol::{
///     CompilerExecutionIssuerPolicyV3 as Policy,
///     CompilerExecutionServiceLaunchManifestV3 as Manifest,
/// };
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn retain(budget: &mut Budget<'_>) -> Result<Inputs, Error> {
///     budget.reserve_storage(Inputs::INPUT_STORAGE)?;
///     let (inputs, charge) = Inputs::from_inherited(budget)?;
///     let charge: Storage = charge;
///     budget.reserve_storage(charge.additional_storage())?;
///     let _: &Policy = inputs.policy();
///     let _: &Manifest = inputs.manifest();
///     inputs.revalidate(budget)?;
///     Ok(inputs)
/// }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::CompilerExecutionIssuerLaunchInputsV3 as Inputs;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn arbitrary(budget: &mut Budget<'_>) { let _ = Inputs::read_at(20, 21, budget); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::CompilerExecutionIssuerLaunchInputsV3 as Inputs;
/// use fe2o3_kernel_ir::CanonicalKernelIrVerificationResourceBudgetV1 as Budget;
/// fn path(budget: &mut Budget<'_>) { let _ = Inputs::from_path("/tmp/policy", budget); }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::{
///     CompilerExecutionIssuerLaunchInputsV2 as V2, CompilerExecutionIssuerLaunchInputsV3 as V3,
/// };
/// fn upgrade(value: V2) -> V3 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::{
///     CompilerExecutionIssuerLaunchInputsV2 as V2, CompilerExecutionIssuerLaunchInputsV3 as V3,
/// };
/// fn downgrade(value: V3) -> V2 { value.into() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::CompilerExecutionIssuerLaunchInputsV3 as Inputs;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionIssuerPolicyV2 as Policy;
/// fn mix(inputs: &Inputs) -> &Policy { inputs.policy() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::CompilerExecutionIssuerLaunchInputsV3 as Inputs;
/// use fe2o3_compiler_execution_protocol::CompilerExecutionServiceLaunchManifestV2 as Manifest;
/// fn mix(inputs: &Inputs) -> &Manifest { inputs.manifest() }
/// ```
/// ```compile_fail
/// use fe2o3_compiler_execution_issuer::{
///     CompilerExecutionIssuerLaunchInputErrorV2 as V2, CompilerExecutionIssuerLaunchInputErrorV3 as V3,
/// };
/// fn mix(error: V2) -> V3 { error.into() }
/// ```
pub struct CompilerExecutionIssuerLaunchInputsV3 {
    policy: PolicyCapability,
    launch: LaunchCapability,
}

#[path = "launch_inputs_native.rs"]
mod adapter;
adapter::launch_inputs!(
    CompilerExecutionIssuerLaunchInputsV3,
    CompilerExecutionIssuerLaunchInputStorageV3,
    CompilerExecutionIssuerLaunchInputErrorV3
);

#[cfg(test)]
mod tests {
    use super::{
        CompilerExecutionIssuerLaunchInputErrorV3 as InputError,
        CompilerExecutionIssuerLaunchInputsV3 as Inputs,
    };
    use fe2o3_compiler_closure_capability::CompilerExecutionCapabilityErrorV2::{
        LaunchV3 as LaunchDecodeError, PolicyV3 as PolicyDecodeError,
    };
    use fe2o3_compiler_execution_protocol::{
        COMPILER_EXECUTION_ISSUER_POLICY_STORAGE_V3 as POLICY_STORAGE,
        COMPILER_EXECUTION_ISSUER_POLICY_WORK_V3 as POLICY_WORK,
        COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_STORAGE_V3 as MANIFEST_STORAGE,
        COMPILER_EXECUTION_SERVICE_LAUNCH_MANIFEST_WORK_V3 as MANIFEST_WORK,
        CompilerExecutionAttestationErrorV3 as PolicyError,
        CompilerExecutionIssuerPolicyV2 as OtherPolicy,
        CompilerExecutionServiceLaunchManifestV2 as OtherManifest,
    };
    const CHILD_TEST: &str = "launch_inputs_v3::tests::inherited_slot_child";
    include!("launch_inputs_tests.rs");
}
