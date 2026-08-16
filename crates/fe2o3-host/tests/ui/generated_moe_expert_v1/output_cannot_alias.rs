use fe2o3_core::DeviceBuffer;

fn overlap(buffer: &mut DeviceBuffer<f32>) {
    let expert_output = buffer.view_mut(..1024).unwrap();
    let compact_output = buffer.view_mut(0..256).unwrap();
    drop((expert_output, compact_output));
}

fn main() {}
