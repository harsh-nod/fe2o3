use crate::environment::{HsaRuntimeAdapterError, ReviewedHsaRuntimeAdapterV1};
use crate::lifecycle::{ReviewedHsaExecutableV1, ReviewedHsaKernelV1};
use fe2o3_core::ContextIdentity;
use fe2o3_host::{
    ReviewedWorkgroupSyncRuntimeAdapterV1, WorkgroupSyncImplicitKernargObservationV1,
    WorkgroupSyncKernelResourceObservationV1, WorkgroupSyncProfileKindV1,
};

const INVALID_RESOURCE_BINDING: &str = "exact workgroup-sync kernel resource binding";

// SAFETY: the production adapter retains the exact GpuContext and private HSA
// executable/kernel objects. The profile-specific initializer validates and
// records the hidden COV6 dynamic-LDS word and pending AQL group-segment value.
unsafe impl ReviewedWorkgroupSyncRuntimeAdapterV1 for ReviewedHsaRuntimeAdapterV1 {
    unsafe fn context_identity_v1(&mut self) -> ContextIdentity {
        self.core
            ._context
            .as_ref()
            .map(|context| context.identity())
            .unwrap_or_else(|| std::process::abort())
    }

    unsafe fn initialize_workgroup_sync_implicit_kernarg_v1(
        &mut self,
        profile: WorkgroupSyncProfileKindV1,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
        geometry: fe2o3_host::HsaLaunchGeometryV1,
        explicit_byte_len: usize,
        implicit_byte_offset: usize,
        implicit_byte_len: usize,
        kernarg: &mut [u8],
    ) -> Result<WorkgroupSyncImplicitKernargObservationV1, Self::Error> {
        crate::dispatch::prepare_workgroup_sync_implicit_kernarg(
            &mut self.core,
            &mut self.pending_dispatch,
            profile,
            executable,
            kernel,
            geometry,
            explicit_byte_len,
            implicit_byte_offset,
            implicit_byte_len,
            kernarg,
        )
    }

    unsafe fn observe_workgroup_sync_resources_v1(
        &mut self,
        profile: WorkgroupSyncProfileKindV1,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
    ) -> Result<WorkgroupSyncKernelResourceObservationV1, Self::Error> {
        observe_resources(profile, executable, kernel)
    }
}

fn observe_resources(
    profile: WorkgroupSyncProfileKindV1,
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
) -> Result<WorkgroupSyncKernelResourceObservationV1, HsaRuntimeAdapterError> {
    let state = executable.state.as_ref().ok_or_else(invalid_binding)?;
    let invalid = state.identity.as_bytes().iter().all(|byte| *byte == 0)
        || kernel
            .executable_identity
            .as_bytes()
            .iter()
            .all(|byte| *byte == 0)
        || kernel.identity.as_bytes().iter().all(|byte| *byte == 0)
        || state.reader == 0
        || state.executable == 0
        || state._loaded_code_object == 0
        || state.bytes.is_empty()
        || kernel.symbol == 0
        || kernel.kernel_object == 0
        || kernel.executable_identity != state.identity;
    if invalid {
        return Err(invalid_binding());
    }
    Ok(WorkgroupSyncKernelResourceObservationV1::new(
        profile,
        state.identity,
        kernel.identity,
        kernel.group_segment_size,
        kernel.private_segment_size,
    ))
}

fn invalid_binding() -> HsaRuntimeAdapterError {
    HsaRuntimeAdapterError::InvalidExecutableObservation(INVALID_RESOURCE_BINDING)
}
