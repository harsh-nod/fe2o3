use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, KernelContext, RegisteredLaunch, Wave64, WaveLane,
};

enum KernelA {}

type Brand<'kernel> =
    KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;

fn escape<'kernel>(
    context: &KernelContext<'kernel, KernelA>,
) -> WaveLane<Wave64, Brand<'static>> {
    context.lane::<Wave64>()
}

fn main() {}
