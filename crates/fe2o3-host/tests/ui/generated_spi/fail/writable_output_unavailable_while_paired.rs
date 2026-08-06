use fe2o3_core::{DeviceBuffer, Stream};
use fe2o3_host::{
    GeneratedAdmittedLaunch, GeneratedWriteDeviceSlice, KernelParams,
    LoadedArgumentAdmittedLaunch, ObservedContext,
};
use std::error::Error;

struct Kernel;

fn rejected<'loaded, 'allocation>(
    admitted: LoadedArgumentAdmittedLaunch<'loaded, 'allocation, Kernel>,
    observed: &ObservedContext,
    output: &'allocation mut DeviceBuffer<u32>,
    stream: &Stream,
) -> Result<(), Box<dyn Error>> {
    let output_capability = GeneratedWriteDeviceSlice::new(observed, output)?;
    let mut params = KernelParams::new();
    output_capability.push_pointer_and_len(&mut params);
    let paired = unsafe {
        GeneratedAdmittedLaunch::from_generated_unchecked(
            admitted,
            params,
            (output_capability,),
        )
    };

    let _ = output.to_host_vec(stream)?;
    let _ = paired;
    Ok(())
}

fn main() {}
