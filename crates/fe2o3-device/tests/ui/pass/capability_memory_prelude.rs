use fe2o3_device::prelude::*;

enum Kernel {}

type Brand<'kernel> =
    KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn hierarchy<'kernel>(mut context: KernelContext<'kernel, Kernel>) {
    let _: Invocation3D<Brand<'kernel>> = context.invocation();
    let _: Option<Grid<'kernel, Brand<'kernel>>> = context.grid();
    let _: Workgroup<'kernel, Brand<'kernel>> = context.workgroup();
    let _: DeviceMath<Brand<'kernel>> = context.math();

    context.with_workgroup(|workgroup| {
        let subgroup: Subgroup<'_, SubgroupWidth64, Brand<'kernel>, InitialEpoch> =
            workgroup.subgroup();
        let _: &WorkgroupEpoch<'_, Brand<'kernel>, InitialEpoch> = workgroup.epoch();
        let _: &SubgroupLane<SubgroupWidth64, _> = subgroup.lane();
    });
}

fn main() {
    let _ = hierarchy;
}
