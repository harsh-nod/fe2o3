use fe2o3_core::{DeviceBuffer, DeviceBufferRangeError};

fn accepted(buffer: &mut DeviceBuffer<u32>) -> Result<(), DeviceBufferRangeError> {
    let (mut left, right) = buffer.split_at_mut(4)?;
    let (head, tail) = left.split_at_mut(2)?;

    let _simultaneous_regions = (
        head.region_byte_range(),
        tail.region_byte_range(),
        right.region_byte_range(),
    );
    Ok(())
}

fn main() {}
