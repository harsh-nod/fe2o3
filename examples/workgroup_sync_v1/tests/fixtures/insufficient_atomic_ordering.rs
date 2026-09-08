// expected-boundary: FE2O3-CAP-ANALYSIS001 relaxed publication requires release ordering
use fe2o3_device::{
    AcquireRelease, AtomicReadWrite, DisjointWrite, Global, GlobalAddressSpace, GlobalMemory,
    Index1D, KernelContext, Relaxed, SystemScope, WorkgroupScope, kernel,
};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
)]
pub fn insufficient_atomic_ordering(
    mut context: KernelContext<'_>,
    mut payload: Global<'_, u32, DisjointWrite<Index1D>>,
    ready: Global<'_, u32, AtomicReadWrite<SystemScope>>,
) {
    let lane = context.invocation().index_1d().get();
    if !payload.store(context.invocation().index_1d().into_disjoint(), lane as u32) {
        fe2o3_device::trap();
    }
    context.with_workgroup(|workgroup| {
        let workgroup =
            workgroup.barrier::<WorkgroupScope, AcquireRelease, GlobalMemory>();
        if lane == 0 {
            let Some(location) = workgroup.global_atomic(&ready, 0) else {
                fe2o3_device::trap();
            };
            workgroup.atomic_store::<u32, GlobalAddressSpace, SystemScope, Relaxed>(&location, 1);
        }
    });
}
