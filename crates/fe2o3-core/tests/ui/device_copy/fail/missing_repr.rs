use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
struct MissingRepr {
    value: u32,
}

fn main() {}
