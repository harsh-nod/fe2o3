use fe2o3_core::{DeviceBuffer, GpuContext, Stream};
use fe2o3_device::KernelMarkerV1;
use fe2o3_host::{
    GeneratedKernelParams, KernelParams, LoadedArgumentAdmittedLaunch, LoadedKernel,
    LoadedLaunchError, ObservedContext, ValidatedArtifactSelectionV1,
};
use std::error::Error;
use std::sync::Arc;

struct GeneratedMarker;

fn generated_kernel() {}

unsafe impl KernelMarkerV1 for GeneratedMarker {
    type Function = fn();
    type Registration = ();

    const LOGICAL_NAME: &'static str = "vector_add";
    const EXPORT_NAME: &'static str = "vector_add.kd";
    const FUNCTION: Self::Function = generated_kernel;
    const REGISTRATION: &'static Self::Registration = &();
}

unsafe fn bind_and_load(
    validated: &ValidatedArtifactSelectionV1,
    observed: &ObservedContext,
    context: &Arc<GpuContext>,
) -> Result<LoadedKernel<GeneratedMarker>, Box<dyn Error>> {
    let binding = unsafe { validated.bind_generated_marker::<GeneratedMarker>() }?;
    let loaded = unsafe {
        LoadedKernel::load_generated(binding, validated, observed, context)
    }?;
    Ok(loaded)
}

unsafe fn launch_with_generated_pack<'loaded, 'allocation>(
    admitted: LoadedArgumentAdmittedLaunch<'loaded, 'allocation, GeneratedMarker>,
    stream: &Stream,
    params: &mut KernelParams,
    input: &'allocation DeviceBuffer<u32>,
    output: &'allocation mut DeviceBuffer<u32>,
) -> Result<(), LoadedLaunchError> {
    let packed = unsafe {
        GeneratedKernelParams::<GeneratedMarker, _>::from_generated_unchecked(
            params,
            (input, output),
        )
    };
    admitted.launch_generated_scoped(stream, packed, |_| {})
}

fn main() {}
