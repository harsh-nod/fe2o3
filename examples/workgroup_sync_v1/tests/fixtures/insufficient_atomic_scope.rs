// expected-boundary: FE2O3-CAP-ANALYSIS001 insufficient atomic scope at final-graph analysis
use fe2o3_device::{
    AtomicReadWrite, Global, GlobalAddressSpace, KernelContext, Relaxed, WorkgroupScope, kernel,
};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [2, 1, 1])
)]
pub fn insufficient_atomic_scope(
    mut context: KernelContext<'_>,
    target: Global<'_, u32, AtomicReadWrite<WorkgroupScope>>,
) {
    context.with_workgroup(|workgroup| {
        let Some(location) = workgroup.global_atomic(&target, 0) else {
            fe2o3_device::trap();
        };
        let _ = workgroup.atomic_fetch_add::<
            u32,
            GlobalAddressSpace,
            WorkgroupScope,
            Relaxed,
        >(&location, 1);
    });
}
