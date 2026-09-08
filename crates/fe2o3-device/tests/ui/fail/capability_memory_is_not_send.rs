use fe2o3_device::capability_memory::{Global, ReadOnly};

enum KernelBrand {}

fn require_send<T: Send>() {}

fn main() {
    require_send::<Global<'static, u32, ReadOnly, KernelBrand>>();
}
