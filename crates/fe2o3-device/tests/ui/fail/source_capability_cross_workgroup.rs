use fe2o3_device::{
    CurrentTarget, InitialEpoch, KernelCapabilityBrand, KernelContext, RegisteredLaunch,
    WorkgroupCapability,
};

enum Kernel {}

type Brand<'kernel> = KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn escape<'kernel>(
    context: &mut KernelContext<'kernel, Kernel>,
) -> WorkgroupCapability<'static, Brand<'kernel>, InitialEpoch> {
    context.with_workgroup(|workgroup| workgroup)
}

fn main() {}
