use fe2o3_core::DeviceBuffer;

fn overlap(buffer: &mut DeviceBuffer<u32>) {
    let top2 = buffer.view_mut(..16).unwrap();
    let inverse = buffer.view_mut(..16).unwrap();
    drop((top2, inverse));
}

fn main() {}
