use fe2o3_device::capability_memory::{CapabilityMemoryView, Global, ReadOnly};

enum KernelBrand {}

fn forge<'kernel>(physical: &'kernel [u32]) -> Global<'kernel, u32, ReadOnly, KernelBrand> {
    CapabilityMemoryView {
        physical,
        _lifetime: Default::default(),
        _element: Default::default(),
        _space: Default::default(),
        _role: Default::default(),
        _brand: Default::default(),
        _not_send_sync: Default::default(),
    }
}

fn main() {}
