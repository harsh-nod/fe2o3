use fe2o3_device::DynamicLds;

#[repr(align(32))]
struct OverAligned(u32);

fn reject(_: DynamicLds<'_, OverAligned>) {}

fn main() {}
