use fe2o3_core::DeviceBuffer;
use fe2o3_host::{
    ArgumentAliasValidator, GeneratedReadDeviceSlice, GeneratedReadWriteDeviceSlice,
    ObservedContext,
};
use std::error::Error;

fn retain_checked_regions(
    observed: &ObservedContext,
    input: &DeviceBuffer<f32>,
    state: &mut DeviceBuffer<f32>,
) -> Result<(), Box<dyn Error>> {
    let input = GeneratedReadDeviceSlice::from_view(observed, input.view(1..7)?)?;
    let state = GeneratedReadWriteDeviceSlice::from_view_mut(observed, state.view_mut(2..8)?)?;
    let admission = ArgumentAliasValidator::new().admit(
        observed,
        [input.argument_access(), state.argument_access()],
        &[],
    )?;
    drop((admission, input, state));
    Ok(())
}

fn main() {}
