use fe2o3_core::{DeviceBuffer, Stream};
use fe2o3_host::{GeneratedArgumentPackingPlanV1, GeneratedReadWriteDeviceSlice, ObservedContext};
use std::error::Error;

fn rejected<'allocation>(
    observed: &ObservedContext,
    plan: &GeneratedArgumentPackingPlanV1,
    state: &'allocation mut DeviceBuffer<f32>,
    stream: &Stream,
) -> Result<(), Box<dyn Error>> {
    let state_capability = GeneratedReadWriteDeviceSlice::new(observed, state)?;
    let state_input = state_capability.bind_argument(plan, 0)?;
    let prepared_scope = (state_capability, state_input);

    let _ = state.to_host_vec(stream)?;
    drop(prepared_scope);
    Ok(())
}

fn main() {}
