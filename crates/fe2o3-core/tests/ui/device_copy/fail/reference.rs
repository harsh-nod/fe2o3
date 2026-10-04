use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct ContainsReference {
    value: &'static u32,
}

fn main() {}
