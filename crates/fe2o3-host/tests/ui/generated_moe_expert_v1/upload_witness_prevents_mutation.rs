use fe2o3_core::{DeviceBuffer, Stream};
use fe2o3_host::upload_moe_expert_offsets_v1;

fn mutate(stream: &Stream, buffer: &mut DeviceBuffer<u32>) {
    let witness = upload_moe_expert_offsets_v1(stream, buffer, [0, 4, 8, 12, 16]).unwrap();
    let mutable = buffer.view_mut(..).unwrap();
    drop((witness, mutable));
}

fn main() {}
