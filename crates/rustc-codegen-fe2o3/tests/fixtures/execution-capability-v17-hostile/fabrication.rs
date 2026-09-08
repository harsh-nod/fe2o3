use fe2o3_device::{CapabilityMemoryView, PrivateAddressSpace, ReadOnly};

fn fabricate<'memory, Brand>(physical: &'memory [u32]) {
    let _ = CapabilityMemoryView::<u32, PrivateAddressSpace, ReadOnly, Brand> {
        physical,
        _lifetime: core::marker::PhantomData,
        _element: core::marker::PhantomData,
        _space: core::marker::PhantomData,
        _role: core::marker::PhantomData,
        _brand: core::marker::PhantomData,
        _not_send_sync: core::marker::PhantomData,
    };
}

