use fe2o3_device::capability_memory::{Global, ReadOnly};

enum KernelBrand {}

fn require_sync<T: Sync>() {}

fn main() {
    require_sync::<Global<'static, u32, ReadOnly, KernelBrand>>();
}
