use core::marker::PhantomData;
use fe2o3_device::KernelContext;

fn main() {
    let _: KernelContext<'static> = KernelContext {
        _kernel: PhantomData,
        _kernel_identity: PhantomData,
        _target: PhantomData,
        _launch: PhantomData,
        _not_send_sync: PhantomData,
    };
}
