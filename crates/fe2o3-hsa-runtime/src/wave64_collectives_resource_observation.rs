use crate::environment::{HsaRuntimeAdapterError, ReviewedHsaRuntimeAdapterV1};
use crate::lifecycle::{ReviewedHsaExecutableV1, ReviewedHsaKernelV1};
use fe2o3_core::ContextIdentity;
use fe2o3_host::{
    ReviewedWave64CollectivesRuntimeAdapterV1, Wave64CollectivesKernelResourceObservationV1,
};

const INVALID_RESOURCE_BINDING: &str = "exact Wave64 collectives kernel resource binding";

// SAFETY: the production adapter retains the exact GpuContext used to select
// its HSA agent. Both identities and resource values come from the supplied
// private executable/kernel tokens, and neither token exposes native handles.
unsafe impl ReviewedWave64CollectivesRuntimeAdapterV1 for ReviewedHsaRuntimeAdapterV1 {
    unsafe fn context_identity_v1(&mut self) -> ContextIdentity {
        self.core
            ._context
            .as_ref()
            .map(|context| context.identity())
            .unwrap_or_else(|| std::process::abort())
    }

    unsafe fn observe_wave64_collectives_resources_v1(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
    ) -> Result<Wave64CollectivesKernelResourceObservationV1, Self::Error> {
        observe_resources(executable, kernel)
    }
}

fn observe_resources(
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
) -> Result<Wave64CollectivesKernelResourceObservationV1, HsaRuntimeAdapterError> {
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
    Ok(Wave64CollectivesKernelResourceObservationV1::new(
        state.identity,
        kernel.identity,
        kernel.group_segment_size,
        kernel.private_segment_size,
    ))
}

fn invalid_binding() -> HsaRuntimeAdapterError {
    HsaRuntimeAdapterError::InvalidExecutableObservation(INVALID_RESOURCE_BINDING)
}
