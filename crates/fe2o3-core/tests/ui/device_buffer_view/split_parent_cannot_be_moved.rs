use fe2o3_core::DeviceBuffer;

fn rejected(mut buffer: DeviceBuffer<u32>) {
    let (left, right) = buffer.split_at_mut(1).unwrap();
    drop(buffer);
    drop((left, right));
}

fn main() {}
