use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C)]
struct ContainsPointer {
    value: *const u32,
}

fn main() {}
