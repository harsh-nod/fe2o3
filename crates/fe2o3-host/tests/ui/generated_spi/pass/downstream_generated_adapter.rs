use fe2o3_core::{DeviceBuffer, GpuContext, Stream};
use fe2o3_device::KernelMarkerV1;
use fe2o3_host::{
    GeneratedAdmittedLaunch, GeneratedReadDeviceSlice, GeneratedWriteDeviceSlice, KernelParams,
    LoadedKernel, ObservedContext, PreparedLaunch, ValidatedArtifactSelectionV1,
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

type VecAddResources<'allocation> = (
    GeneratedReadDeviceSlice<'allocation, u32>,
    GeneratedWriteDeviceSlice<'allocation, u32>,
);

unsafe fn assemble_generated_launch<'loaded, 'allocation, 'params>(
    loaded: &'loaded LoadedKernel<GeneratedMarker>,
    observed: &ObservedContext,
    prepared: PreparedLaunch<GeneratedMarker>,
    input: &'allocation DeviceBuffer<u32>,
    output: &'allocation mut DeviceBuffer<u32>,
    params: &'params mut KernelParams,
) -> Result<
    GeneratedAdmittedLaunch<
        'loaded,
        'allocation,
        'params,
        GeneratedMarker,
        VecAddResources<'allocation>,
    >,
    Box<dyn Error>,
> {
    let input = GeneratedReadDeviceSlice::new(observed, input)?;
    let output = GeneratedWriteDeviceSlice::new(observed, output)?;

    input.push_pointer_and_len(params);
    output.push_pointer_and_len(params);

    let admitted = prepared.admit_arguments([input.argument_access(), output.argument_access()])?;
    let admitted = loaded.bind_admitted(admitted)?;
    let paired = unsafe {
        GeneratedAdmittedLaunch::from_generated_unchecked(admitted, params, (input, output))
    };
    Ok(paired)
}

fn launch_paired(
    paired: GeneratedAdmittedLaunch<'_, '_, '_, GeneratedMarker, VecAddResources<'_>>,
    stream: &Stream,
) -> Result<(), fe2o3_host::LoadedLaunchError> {
    paired.launch_generated_scoped(stream, |_| {})
}

fn main() {}
