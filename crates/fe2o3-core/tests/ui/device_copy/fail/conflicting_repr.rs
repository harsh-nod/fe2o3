use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C, transparent)]
struct ConflictingRepr(u32);

fn main() {}
