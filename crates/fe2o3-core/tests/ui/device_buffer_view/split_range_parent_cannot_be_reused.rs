use fe2o3_core::DeviceBuffer;

fn rejected(mut buffer: DeviceBuffer<f32>) {
    let (left, output, right) = buffer.split_range_mut(1..3).unwrap();
    let _reuse = buffer.len();
    drop((left, output, right));
}

fn main() {}
