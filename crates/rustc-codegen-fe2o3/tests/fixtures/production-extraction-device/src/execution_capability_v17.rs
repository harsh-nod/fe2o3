use fe2o3_device::{
    Acquire, AcquireRelease, AtomicReadWrite, CapabilityMemoryView, DisjointWrite,
    ExclusiveReadWrite, Global, GlobalAddressSpace, GlobalAndWorkgroupMemory, Index1D,
    KernelContext, PrivateAddressSpace, PrivateMemoryView, ReadOnly, Release,
    SequentiallyConsistent, SubgroupScope, SubgroupWidth32, SubgroupWidth64, SystemScope,
    UnsafeRawMemoryObligationV1, WorkgroupAddressSpace, WorkgroupLdsUninitialized,
    WorkgroupMemoryView, WorkgroupScope, kernel,
};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub fn execution_capability_v17(mut context: KernelContext<'_>, input: Global<'_, u32, ReadOnly>) {
    let atomic_physical: &[u32] = &[];
    let atomic_view = CapabilityMemoryView::<
        u32,
        GlobalAddressSpace,
        AtomicReadWrite<SystemScope>,
        _,
    >::__compiler_bind_atomic(&context, atomic_physical);

    let mut private = context.private_memory::<u32, 7>();
    let _ = private.load(1);
    let _ = private.store(2, 3);

    context.with_workgroup(|workgroup| {
        let subgroup32 = workgroup.subgroup::<SubgroupWidth32>();
        let _ = subgroup32.reduce_sum(workgroup.epoch(), 11_u32);
        let _ = subgroup32.inclusive_scan_sum(workgroup.epoch(), 13_u32);

        let subgroup64 = workgroup.subgroup::<SubgroupWidth64>();
        subgroup64.fence::<SubgroupScope, Acquire, GlobalAndWorkgroupMemory>(workgroup.epoch());
        let _matrix = subgroup64.__compiler_matrix_access(workgroup.epoch());
        subgroup64.with_matrix(workgroup.epoch(), |_matrix, _lane| ());

        if let Some(location) = workgroup.global_atomic(&atomic_view, 0) {
            let _ =
                workgroup.atomic_load::<u32, GlobalAddressSpace, SystemScope, Acquire>(&location);
            workgroup.atomic_store::<u32, GlobalAddressSpace, SystemScope, Release>(&location, 17);
            let _ = workgroup
                .atomic_fetch_add::<u32, GlobalAddressSpace, SystemScope, SequentiallyConsistent>(
                    &location, 19,
                );
            let _ = workgroup.atomic_compare_exchange::<
                u32,
                GlobalAddressSpace,
                SystemScope,
                AcquireRelease,
                Acquire,
            >(&location, 19, 23);
        }

        workgroup.fence::<WorkgroupScope, Release, GlobalAndWorkgroupMemory>();

        let mut memory = workgroup.allocate_memory::<u32, 64>();
        if let Some(index) = workgroup.memory_index_1d() {
            let _ = memory.store(&workgroup, index.into_disjoint(), 31);
        }
        let (workgroup, published_memory) = workgroup.publish_memory(memory);
        let _ = published_memory.load(&workgroup, 0);

        let lds = workgroup.allocate_lds::<u32, 64>();
        let initialized = lds.initialize_by_invocation(&workgroup, 37);
        let (workgroup, published_lds) = workgroup.publish_lds(initialized);
        let _ = published_lds.read(&workgroup, 0);

        let destination = workgroup.allocate_lds::<u32, 64>();
        let pending = workgroup.async_copy_from_global(&input, 0, destination);
        let (workgroup, _published_copy) = workgroup.wait_for(pending);

        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        let (workgroup, _subgroup) = workgroup.subgroup_barrier::<
            SubgroupWidth64,
            SubgroupScope,
            AcquireRelease,
            GlobalAndWorkgroupMemory,
        >(subgroup);
        let workgroup =
            workgroup.barrier::<WorkgroupScope, AcquireRelease, GlobalAndWorkgroupMemory>();

        let scratch: fe2o3_device::WorkgroupLds<'_, u32, 64, WorkgroupLdsUninitialized, _, _> =
            workgroup.allocate_lds();
        let (workgroup, _scratch, _sum) = workgroup.reduce_sum(scratch, 41_u32);

        let scratch: fe2o3_device::WorkgroupLds<'_, u32, 64, WorkgroupLdsUninitialized, _, _> =
            workgroup.allocate_lds();
        let (workgroup, _scratch, _sum) = workgroup.inclusive_scan_sum(scratch, 43_u32);

        let scratch: fe2o3_device::WorkgroupLds<'_, u32, 64, WorkgroupLdsUninitialized, _, _> =
            workgroup.allocate_lds();
        let (_workgroup, _scratch, _sum) = workgroup.exclusive_scan_sum(scratch, 47_u32);
    });
}

#[kernel(
    unsafe_provider(raw_memory),
    launch(required = [64, 1, 1], max = [64, 1, 1])
)]
pub unsafe fn execution_raw_memory_provider_v17(mut context: KernelContext<'_>) {
    let private_read_only: PrivateMemoryView<'_, u32, ReadOnly, _> = unsafe {
        CapabilityMemoryView::<u32, PrivateAddressSpace, ReadOnly, _>::from_raw_parts(
            &context,
            core::ptr::null_mut(),
            7,
            UnsafeRawMemoryObligationV1::for_view::<PrivateAddressSpace, ReadOnly>(),
        )
    };
    let _ = private_read_only.load(0);

    let mut private_disjoint: PrivateMemoryView<'_, u32, DisjointWrite<Index1D>, _> = unsafe {
        CapabilityMemoryView::<u32, PrivateAddressSpace, DisjointWrite<Index1D>, _>::from_raw_parts(
            &context,
            core::ptr::null_mut(),
            7,
            UnsafeRawMemoryObligationV1::for_view::<PrivateAddressSpace, DisjointWrite<Index1D>>(),
        )
    };
    let private_index = context.invocation().index_1d().into_disjoint();
    let _ = private_disjoint.store(private_index, 5);

    context.with_workgroup(|workgroup| {
        let mut exclusive: WorkgroupMemoryView<'_, '_, u32, ExclusiveReadWrite, _, _> = unsafe {
            CapabilityMemoryView::<
                u32,
                WorkgroupAddressSpace,
                ExclusiveReadWrite,
                _,
            >::from_raw_parts(
                &workgroup,
                core::ptr::null_mut(),
                64,
                UnsafeRawMemoryObligationV1::for_view::<
                    WorkgroupAddressSpace,
                    ExclusiveReadWrite,
                >(),
            )
        };
        let _ = exclusive.load(&workgroup, 0);
        let _ = exclusive.store(&workgroup, 1, 29);
    });
}
