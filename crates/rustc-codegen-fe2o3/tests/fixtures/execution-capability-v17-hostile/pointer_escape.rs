use fe2o3_device::{
    CapabilityMemoryView, CurrentTarget, KernelCapabilityBrand, KernelContext, PrivateAddressSpace,
    PrivateMemoryView, ReadOnly, RegisteredLaunch, UnsafeRawMemoryObligationV1,
};

enum Kernel {}

fn escape<'kernel>(
    context: &'kernel KernelContext<'kernel, Kernel>,
) -> PrivateMemoryView<
    'static,
    u32,
    ReadOnly,
    KernelCapabilityBrand<'static, Kernel, CurrentTarget, RegisteredLaunch>,
> {
    unsafe {
        CapabilityMemoryView::<
            u32,
            PrivateAddressSpace,
            ReadOnly,
            KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>,
        >::from_raw_parts(
            context,
            core::ptr::null_mut(),
            4,
            UnsafeRawMemoryObligationV1::for_view::<PrivateAddressSpace, ReadOnly>(),
        )
    }
}

