use fe2o3_core::DeviceBuffer;

fn rejected(buffer: &mut DeviceBuffer<u32>) {
    let view = buffer.view_mut(..).unwrap();
    let _length = buffer.len();
    drop(view);
}

fn main() {}
