use fe2o3_device::{
    CurrentTarget, KernelCapabilityBrand, KernelContext, RegisteredLaunch, Wave64, WaveLane,
};

enum KernelA {}
enum KernelB {}

type BrandA<'kernel> =
    KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;

fn needs_kernel_a(_: WaveLane<Wave64, BrandA<'_>>) {}

fn substitute<'kernel>(context: &KernelContext<'kernel, KernelB>) {
    needs_kernel_a(context.lane::<Wave64>());
}

fn main() {}
