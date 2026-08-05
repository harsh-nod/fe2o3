use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C, C)]
struct DuplicateRepr(u32);

fn main() {}
