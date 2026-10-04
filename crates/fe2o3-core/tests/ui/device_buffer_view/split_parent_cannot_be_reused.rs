use fe2o3_core::DeviceBuffer;

fn rejected(buffer: &mut DeviceBuffer<u32>) {
    let (left, right) = buffer.split_at_mut(1).unwrap();
    let _length = buffer.len();
    drop((left, right));
}

fn main() {}
