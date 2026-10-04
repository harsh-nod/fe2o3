use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedArgumentPackingPlanV1, GeneratedReadWriteDeviceSlice, ObservedContext};

fn reuse_buffer(
    observed: &ObservedContext,
    plan: &GeneratedArgumentPackingPlanV1,
    mut state: DeviceBuffer<f32>,
) {
    let capability = GeneratedReadWriteDeviceSlice::new(observed, &mut state).unwrap();
    let input = capability.bind_argument(plan, 0).unwrap();
    drop(capability);
    let _ = state.len();
    drop(input);
}

fn drop_buffer(
    observed: &ObservedContext,
    plan: &GeneratedArgumentPackingPlanV1,
    mut state: DeviceBuffer<f32>,
) {
    let capability = GeneratedReadWriteDeviceSlice::new(observed, &mut state).unwrap();
    let input = capability.bind_argument(plan, 0).unwrap();
    drop(capability);
    drop(state);
    drop(input);
}

fn mutably_reborrow_buffer(
    observed: &ObservedContext,
    plan: &GeneratedArgumentPackingPlanV1,
    mut state: DeviceBuffer<f32>,
) {
    let capability = GeneratedReadWriteDeviceSlice::new(observed, &mut state).unwrap();
    let input = capability.bind_argument(plan, 0).unwrap();
    drop(capability);
    let _second_borrow = &mut state;
    drop(input);
}

fn main() {}
