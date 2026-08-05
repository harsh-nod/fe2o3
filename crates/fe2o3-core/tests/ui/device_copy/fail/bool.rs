use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct ContainsBool {
    value: bool,
}

fn main() {}
