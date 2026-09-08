use core::mem::{align_of, size_of};

use fe2o3_device::{
    CurrentTarget, DeviceMath, DynamicLds, Grid, Group, Invocation3D, KernelCapabilityBrand,
    KernelContext, KernelContextTypeV1, LdsUninitialized, MatrixCapability, RegisteredLaunch,
    SubgroupTile, Wave64, WaveLane, Workgroup, WorkgroupCollectives, WorkgroupLdsScope,
    WorkgroupSynchronization,
};

enum KernelA {}

type Brand<'kernel> =
    KernelCapabilityBrand<'kernel, KernelA, CurrentTarget, RegisteredLaunch>;

fn capability_surface<'kernel>(mut context: KernelContext<'kernel, KernelA>) {
    let invocation: Invocation3D<Brand<'kernel>> = context.invocation();
    let grid: Grid<'kernel, Brand<'kernel>> = context.grid().unwrap();
    let workgroup: Workgroup<'kernel, Brand<'kernel>> = context.workgroup();
    let lane: WaveLane<Wave64, Brand<'kernel>> = context.lane::<Wave64>();
    let subgroup: SubgroupTile<'kernel, 32, Brand<'kernel>> = context.wave64_tile::<32>();
    let collectives: WorkgroupCollectives<Brand<'kernel>> = context.workgroup_collectives();
    let _math: DeviceMath<Brand<'kernel>> = context.math();
    let _matrix: MatrixCapability<Brand<'kernel>> = context.matrix();

    assert!(invocation.workitem_id().x() < invocation.workgroup_size().x());
    assert!(grid.thread_rank() < grid.size());
    assert!(workgroup.thread_rank() < workgroup.size());
    assert!(lane.get() < 64);
    assert!(subgroup.thread_rank() < subgroup.size());

    let mut lds_scope: WorkgroupLdsScope<'_, Brand<'kernel>> = context.workgroup_lds();
    let scratch: DynamicLds<'_, u32, LdsUninitialized, Brand<'kernel>> =
        DynamicLds::exact_current::<64>(&mut lds_scope);
    let _sum = collectives.reduce_sum_portable(scratch, 1_u32);
}

fn workgroup_contract(_: &WorkgroupSynchronization) {}

fn genuine_context<C: KernelContextTypeV1>() {}

fn main() {
    type Context = KernelContext<'static, (), CurrentTarget, RegisteredLaunch>;
    assert_eq!(size_of::<Context>(), 0);
    assert_eq!(align_of::<Context>(), 1);
    genuine_context::<Context>();
    let _ = capability_surface;
    let _ = workgroup_contract as fn(&WorkgroupSynchronization);
}
