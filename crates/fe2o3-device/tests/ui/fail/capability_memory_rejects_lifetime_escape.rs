use fe2o3_device::capability_memory::{Global, ReadOnly};

enum KernelBrand {}

fn escape<'kernel>(
    view: Global<'kernel, u32, ReadOnly, KernelBrand>,
) -> Global<'static, u32, ReadOnly, KernelBrand> {
    view
}

fn main() {}
