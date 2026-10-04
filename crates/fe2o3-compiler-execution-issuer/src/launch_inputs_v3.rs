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
use std::{
    error::Error,
    fmt,
    fs::File,
    mem::size_of,
    os::fd::{FromRawFd, RawFd},
};

// Two scalar inspections, two duplications and their failure-path closes. The
// enclosing frame prepays the transient Files; capability/codec work nests on b.
const READ_WORK: usize = 8 + 6 * (1024 + 64);

fn preflight_inputs(policy: RawFd, launch: RawFd) -> Result<()> {
    for fd in [policy, launch] {
        if fd < 3 {
            return Err(CapabilityError::Rejected("inherited descriptor overlaps stdio").into());
        }
        // SAFETY: F_GETFD checks the raw slot without borrowing or adopting it.
        let flags = unsafe { libc::fcntl(fd, libc::F_GETFD) };
        if flags < 0 {
            return Err(input_io_error("inspect inherited descriptor").into());
        }
        if flags & libc::FD_CLOEXEC != 0 {
            return Err(CapabilityError::Rejected("inherited descriptor is close-on-exec").into());
        }
    }
    Ok(())
}

fn input_io_error(operation: &'static str) -> CapabilityError {
    CapabilityError::Io {
        operation,
        errno: std::io::Error::last_os_error()
            .raw_os_error()
            .unwrap_or(libc::EIO),
    }
}

fn duplicate_input(fd: RawFd) -> Result<File> {
    // SAFETY: both sources were preflighted before any duplication. Success
    // yields a new owner above the entire V3 inherited table, including FD12.
    let duplicate = unsafe {
        libc::fcntl(
            fd,
            libc::F_DUPFD_CLOEXEC,
            crate::PRIVATE_DESCRIPTOR_FLOOR_V3,
        )
    };
    if duplicate < 0 {
        return Err(input_io_error("retain inherited descriptor").into());
    }
    // SAFETY: this successful duplication transfers its unique ownership to File.
    Ok(unsafe { File::from_raw_fd(duplicate) })
}

fn read_policy(fd: RawFd, b: &mut Budget<'_>) -> Result<(PolicyCapability, usize)> {
    let (value, charge) = PolicyCapability::from_file(duplicate_input(fd)?, b)?;
    // from_file returns growth over the consumed File. The inherited source is
    // still borrowed, so this reader returns the full new owner's charge.
    let full = PolicyCapability::FILE_STORAGE
        .checked_add(charge.additional_storage())
        .ok_or(Resource::Arithmetic)?;
    Ok((value, full))
}

fn read_launch(fd: RawFd, b: &mut Budget<'_>) -> Result<(LaunchCapability, usize)> {
    let (value, charge) = LaunchCapability::from_file(duplicate_input(fd)?, b)?;
    let full = LaunchCapability::FILE_STORAGE
        .checked_add(charge.additional_storage())
        .ok_or(Resource::Arithmetic)?;
    Ok((value, full))
}

/// Both owners use actual SubjectV3 policy and manifest types. The shared launch
/// wire does not establish a family: the policy is independently decoded as V3
/// and the manifest must bind that exact policy identity.
///
/// Inert, move-only native inputs freshly admitted from fixed launch slots 6/8.
/// The two independently decoded images must name the same native policy.
/// Private duplicates start at FD13, leaving the V3 root-control slot FD12 intact.
/// This pair reader does not preflight the other service slots; the V3 entrypoint
/// checks the full FD3..12 table before calling it or allocating any descriptors.
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
    const EXPECTED_PRIVATE_FLOOR: RawFd = 13;
    include!("launch_inputs_tests.rs");
}
