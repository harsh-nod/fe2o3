use crate::environment::{HsaRuntimeAdapterError, ReviewedHsaRuntimeAdapterV1};
use crate::lifecycle::{ReviewedHsaExecutableV1, ReviewedHsaKernelV1};
use fe2o3_core::ContextIdentity;
use fe2o3_host::{ExactLdsGemmKernelResourceObservationV1, ReviewedExactLdsGemmRuntimeAdapterV1};

const INVALID_RESOURCE_BINDING: &str = "exact LDS GEMM kernel resource binding";

// SAFETY: the production adapter retains the exact GpuContext used to select
// its HSA agent. Executable and kernel identities are derived when those
// private native objects are created, and the queried resource values remain
// stored in the private kernel token until that token is consumed.
unsafe impl ReviewedExactLdsGemmRuntimeAdapterV1 for ReviewedHsaRuntimeAdapterV1 {
    unsafe fn context_identity_v1(&mut self) -> ContextIdentity {
        self.core
            ._context
            .as_ref()
            .map(|context| context.identity())
            .unwrap_or_else(|| std::process::abort())
    }

    unsafe fn observe_exact_lds_gemm_kernel_resources_v1(
        &mut self,
        executable: &Self::Executable,
        kernel: &Self::Kernel,
    ) -> Result<ExactLdsGemmKernelResourceObservationV1, Self::Error> {
        observe_exact_lds_gemm_kernel_resources(executable, kernel)
    }
}

fn observe_exact_lds_gemm_kernel_resources(
    executable: &ReviewedHsaExecutableV1,
    kernel: &ReviewedHsaKernelV1,
) -> Result<ExactLdsGemmKernelResourceObservationV1, HsaRuntimeAdapterError> {
    let state = executable.state.as_ref().ok_or_else(invalid_binding)?;

    let executable_identity_is_zero = state.identity.as_bytes().iter().all(|byte| *byte == 0);
    let kernel_executable_identity_is_zero = kernel
        .executable_identity
        .as_bytes()
        .iter()
        .all(|byte| *byte == 0);
    let kernel_identity_is_zero = kernel.identity.as_bytes().iter().all(|byte| *byte == 0);
    let native_executable_state_is_zero = state.reader == 0
        || state.executable == 0
        || state._loaded_code_object == 0
        || state.bytes.is_empty();
    let native_kernel_state_is_zero = kernel.symbol == 0 || kernel.kernel_object == 0;

    if executable_identity_is_zero
        || kernel_executable_identity_is_zero
        || kernel_identity_is_zero
        || native_executable_state_is_zero
        || native_kernel_state_is_zero
        || kernel.executable_identity != state.identity
    {
        return Err(invalid_binding());
    }

    Ok(ExactLdsGemmKernelResourceObservationV1::new(
        state.identity,
        kernel.identity,
        kernel.group_segment_size,
        kernel.private_segment_size,
    ))
}

fn invalid_binding() -> HsaRuntimeAdapterError {
    HsaRuntimeAdapterError::InvalidExecutableObservation(INVALID_RESOURCE_BINDING)
}
