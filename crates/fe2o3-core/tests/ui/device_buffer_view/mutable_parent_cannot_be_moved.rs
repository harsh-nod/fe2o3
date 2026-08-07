use fe2o3_core::DeviceBuffer;

fn rejected(mut buffer: DeviceBuffer<u32>) {
    let view = buffer.view_mut(..).unwrap();
    drop(buffer);
    drop(view);
}

fn main() {}
