use fe2o3_artifacts::PayloadDigest;
use fe2o3_core::ContextIdentity;
use fe2o3_host::{
    ExactLdsGemmKernelResourceObservationV1, HsaCodeObjectLoadObservationV1,
    HsaDispatchObservationV1, HsaEnvironmentObservationV1,
    HsaImplicitKernargInitializationObservationV1, HsaKernelResolutionObservationV1,
    HsaLaunchGeometryV1, HsaUnloadObservationV1, ReviewedExactLdsGemmRuntimeAdapterV1,
    ReviewedHsaExecutableLifecycleAdapterV1, ReviewedHsaImplicitKernargAdapterV1,
};

struct Adapter;

unsafe impl ReviewedHsaExecutableLifecycleAdapterV1 for Adapter {
    type Executable = ();
    type Kernel = ();
    type Error = ();

    unsafe fn observe_environment(&mut self) -> Result<HsaEnvironmentObservationV1, Self::Error> {
        todo!()
    }

    unsafe fn load_executable(
        &mut self,
        _bytes: &[u8],
        _digest: PayloadDigest,
    ) -> Result<(Self::Executable, HsaCodeObjectLoadObservationV1), Self::Error> {
        todo!()
    }

    unsafe fn resolve_kernel(
        &mut self,
        _executable: &Self::Executable,
        _symbol: &str,
    ) -> Result<(Self::Kernel, HsaKernelResolutionObservationV1), Self::Error> {
        todo!()
    }

    unsafe fn launch_and_wait(
        &mut self,
        _executable: &Self::Executable,
        _kernel: &Self::Kernel,
        _geometry: HsaLaunchGeometryV1,
        _kernarg: &mut [u8],
    ) -> Result<HsaDispatchObservationV1, Self::Error> {
        todo!()
    }

    unsafe fn unload_executable(
        &mut self,
        _executable: Self::Executable,
    ) -> Result<HsaUnloadObservationV1, Self::Error> {
        todo!()
    }
}

unsafe impl ReviewedHsaImplicitKernargAdapterV1 for Adapter {
    unsafe fn initialize_implicit_kernarg(
        &mut self,
        _executable: &Self::Executable,
        _kernel: &Self::Kernel,
        _geometry: HsaLaunchGeometryV1,
        _explicit_byte_len: usize,
        _implicit_byte_offset: usize,
        _implicit_byte_len: usize,
        _kernarg: &mut [u8],
    ) -> Result<HsaImplicitKernargInitializationObservationV1, Self::Error> {
        todo!()
    }
}

impl ReviewedExactLdsGemmRuntimeAdapterV1 for Adapter {
    unsafe fn context_identity_v1(&mut self) -> ContextIdentity {
        todo!()
    }

    unsafe fn observe_exact_lds_gemm_kernel_resources_v1(
        &mut self,
        _executable: &Self::Executable,
        _kernel: &Self::Kernel,
    ) -> Result<ExactLdsGemmKernelResourceObservationV1, Self::Error> {
        todo!()
    }
}

fn main() {}
