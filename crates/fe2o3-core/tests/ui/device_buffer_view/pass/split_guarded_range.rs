use fe2o3_core::{DeviceBuffer, DeviceBufferRangeError};

fn guarded_region(buffer: &mut DeviceBuffer<f32>) -> Result<(), DeviceBufferRangeError> {
    let (left_canary, output, right_canary) = buffer.split_range_mut(2..10)?;
    assert_eq!(left_canary.region_byte_range(), 0..8);
    assert_eq!(output.region_byte_range(), 8..40);
    assert_eq!(right_canary.region_byte_range(), 40..48);
    Ok(())
}

fn main() {}
