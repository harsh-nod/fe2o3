use fe2o3_core::DeviceBuffer;

fn rejected(buffer: &mut DeviceBuffer<u32>) {
    let (left, _right) = buffer.split_at_mut(1).unwrap();
    let _raw = left.raw_device_ptr();
}

fn main() {}
