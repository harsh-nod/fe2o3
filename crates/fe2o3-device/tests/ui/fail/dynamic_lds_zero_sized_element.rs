use fe2o3_device::DynamicLds;

struct ZeroSized;

fn reject(_: DynamicLds<'_, ZeroSized>) {}

fn main() {}
