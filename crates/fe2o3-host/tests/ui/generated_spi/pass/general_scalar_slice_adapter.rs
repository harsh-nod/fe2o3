use fe2o3_core::DeviceBuffer;
use fe2o3_host::{
    ArgumentAliasValidator, GeneratedArgumentPackingPlanV1, GeneratedReadDeviceSlice,
    GeneratedReadWriteDeviceSlice, ObservedContext,
};
use std::error::Error;

fn prepare_generated<'allocation>(
    observed: &ObservedContext,
    plan: &GeneratedArgumentPackingPlanV1,
    input: &'allocation DeviceBuffer<f32>,
    state: &'allocation mut DeviceBuffer<f32>,
) -> Result<(), Box<dyn Error>> {
    let input = GeneratedReadDeviceSlice::new(observed, input)?;
    let state = GeneratedReadWriteDeviceSlice::new(observed, state)?;
    let admission = ArgumentAliasValidator::new().admit(
        observed,
        [input.argument_access(), state.argument_access()],
        &[],
    )?;
    let packed = plan.pack([
        plan.scalar(0, 2.5_f32)?,
        input.bind_argument(plan, 1)?,
        state.bind_argument(plan, 2)?,
    ])?;
    let prepared_scope = (packed, admission, input, state);
    drop(prepared_scope);
    Ok(())
}

fn main() {}
