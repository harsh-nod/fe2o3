use fe2o3_device::DynamicLds;

struct ContainsReference<'a>(&'a u32);

fn reject(_: DynamicLds<'_, ContainsReference<'_>>) {}

fn main() {}
