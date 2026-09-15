use gpu_device::{
    AtomicReadWrite, CurrentTarget, Global, KernelCapabilityBrand, KernelContext, RegisteredLaunch,
    SystemScope,
};

enum Kernel {}
type Brand<'a> = KernelCapabilityBrand<'a, Kernel, CurrentTarget, RegisteredLaunch>;

fn bind<'a>(
    context: &KernelContext<'a, Kernel>,
    physical: &'a [u32],
) -> Global<'a, u32, AtomicReadWrite<SystemScope>, Brand<'a>> {
    Global::__compiler_bind_atomic(context, physical)
}

fn main() {
    let _ = bind;
}
