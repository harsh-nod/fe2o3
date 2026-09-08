use fe2o3_device::UnbrandedCapability;
use fe2o3_device::capability_memory::{Global, ReadOnly};

fn forge(values: &[u32]) {
    let _ = Global::<'_, u32, ReadOnly, UnbrandedCapability>::__compiler_bind_read_only(values);
}

fn main() {}
