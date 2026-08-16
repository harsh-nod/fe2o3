use fe2o3_core::DeviceBuffer;

fn overlap(buffer: &mut DeviceBuffer<f32>) {
    let query = buffer.view(..128).unwrap();
    let output = buffer.view_mut(..128).unwrap();
    drop((query, output));
}

fn main() {}
