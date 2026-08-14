use fe2o3_core::DeviceBuffer;
use fe2o3_host::{GeneratedScalarGemmV1ReadWriteDeviceSlice, ObservedContext};
use std::error::Error;

fn retain_guarded_output(
    observed: &ObservedContext,
    guarded: &mut DeviceBuffer<f32>,
    output_len: usize,
) -> Result<(), Box<dyn Error>> {
    let output_end = 2_usize.checked_add(output_len).ok_or("output overflow")?;
    let (left_canary, output, right_canary) = guarded.split_range_mut(2..output_end)?;
    drop((left_canary, right_canary));

    let output = GeneratedScalarGemmV1ReadWriteDeviceSlice::from_view_mut(observed, output)?;
    assert_eq!(output.len(), output_len);
    drop(output);

    let _parent_is_available_after_capability = guarded.len();
    Ok(())
}

fn retain_empty_guarded_output(
    observed: &ObservedContext,
    guarded: &mut DeviceBuffer<f32>,
) -> Result<(), Box<dyn Error>> {
    let (left_canary, output, right_canary) = guarded.split_range_mut(2..2)?;
    drop((left_canary, right_canary));
    let output = GeneratedScalarGemmV1ReadWriteDeviceSlice::from_view_mut(observed, output)?;
    assert!(output.is_empty());
    Ok(())
}

fn main() {}
