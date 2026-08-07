use fe2o3_host::{
    HsaCodeObjectLoadObservationV1, HsaDispatchObservationV1, HsaEnvironmentObservationV1,
    HsaKernelResolutionObservationV1, HsaLaunchGeometryV1, HsaUnloadObservationV1,
    ReviewedHsaExecutableLifecycleAdapterV1,
};
use fe2o3_artifacts::PayloadDigest;

struct Adapter;

impl ReviewedHsaExecutableLifecycleAdapterV1 for Adapter {
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

fn main() {}
