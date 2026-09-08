use fe2o3_device::{
    Acquire, AcquireRelease, AtomicReadWrite, Global, GlobalAddressSpace, GlobalAndWorkgroupMemory,
    Group, InitialEpoch, KernelCapabilityBrand, KernelContext, ReadOnly, RegisteredLaunch, Release,
    SequentiallyConsistent, SubgroupScope, SubgroupWidth64, SystemScope, WorkgroupLdsUninitialized,
    WorkgroupScope,
};

enum Kernel {}

type Brand<'kernel> =
    KernelCapabilityBrand<'kernel, Kernel, fe2o3_device::CurrentTarget, RegisteredLaunch>;

fn capability_flow<'kernel>(
    mut context: KernelContext<'kernel, Kernel>,
    input: Global<'kernel, u32, ReadOnly, Brand<'kernel>>,
    counters: Global<'kernel, u32, AtomicReadWrite<SystemScope>, Brand<'kernel>>,
) {
    let invocation = context.invocation();
    let grid = invocation.grid().unwrap();
    let workgroup_observation = invocation.workgroup();
    assert_eq!(grid.thread_rank(), grid.thread_rank());
    assert_eq!(
        workgroup_observation.thread_rank(),
        workgroup_observation.thread_rank()
    );
    drop(workgroup_observation);
    drop(grid);
    drop(invocation);

    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup
            .fence::<SubgroupScope, AcquireRelease, GlobalAndWorkgroupMemory>(workgroup.epoch());
        subgroup.with_matrix(workgroup.epoch(), |_matrix, lane| {
            assert!(lane.get() < 64);
        });
        let (workgroup, subgroup) = workgroup.subgroup_barrier::<
            SubgroupWidth64,
            SubgroupScope,
            AcquireRelease,
            GlobalAndWorkgroupMemory,
        >(subgroup);
        drop(subgroup);

        let location = workgroup.global_atomic(&counters, 0).unwrap();
        let _: u32 =
            workgroup.atomic_load::<u32, GlobalAddressSpace, SystemScope, Acquire>(&location);
        workgroup.atomic_store::<u32, GlobalAddressSpace, SystemScope, Release>(&location, 1);
        let _: u32 = workgroup
            .atomic_fetch_add::<u32, GlobalAddressSpace, SystemScope, SequentiallyConsistent>(
                &location, 1,
            );
        let _: Result<u32, u32> = workgroup.atomic_compare_exchange::<
            u32,
            GlobalAddressSpace,
            SystemScope,
            AcquireRelease,
            Acquire,
        >(&location, 1, 2);

        let lds = workgroup.allocate_lds::<u32, 64>();
        let initialized = lds.initialize_by_invocation(&workgroup, 0);
        let (workgroup, published) = workgroup.publish_lds(initialized);
        let _ = published.read(&workgroup, 0);

        workgroup.fence::<WorkgroupScope, AcquireRelease, GlobalAndWorkgroupMemory>();
        let destination = workgroup.allocate_lds::<u32, 64>();
        let pending = workgroup.async_copy_from_global(&input, 0, destination);
        let (workgroup, _published_copy) = workgroup.wait_for(pending);
        let workgroup =
            workgroup.barrier::<WorkgroupScope, AcquireRelease, GlobalAndWorkgroupMemory>();

        let scratch: fe2o3_device::WorkgroupLds<
            '_,
            u32,
            64,
            WorkgroupLdsUninitialized,
            Brand<'kernel>,
            _,
        > = workgroup.allocate_lds();
        let (_workgroup, _scratch, _sum) = workgroup.reduce_sum(scratch, 1_u32);
    });
}

fn epoch_is_initial(_: &fe2o3_device::WorkgroupEpoch<'_, Brand<'_>, InitialEpoch>) {}

fn main() {
    let _ = capability_flow;
    let _ = epoch_is_initial;
}
