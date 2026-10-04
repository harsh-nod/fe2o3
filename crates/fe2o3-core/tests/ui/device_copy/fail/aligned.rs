use fe2o3_core::DeviceCopy;

#[derive(Clone, Copy, DeviceCopy)]
#[repr(C, align(8))]
struct ExplicitlyAligned(u64);

fn main() {}
