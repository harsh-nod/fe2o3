// expected-boundary: FE2O3-CAP-ANALYSIS001 workgroup collective has incomplete participation
use fe2o3_device::{Global, KernelContext, ReadOnly, kernel};

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1], max_grid = [1, 1, 1])
)]
pub fn incomplete_collective_participation(
    mut context: KernelContext<'_>,
    participates: Global<'_, u32, ReadOnly>,
) {
    let lane = context.invocation().index_1d().get();
    let Some(participates) = participates.load(lane) else {
        fe2o3_device::trap();
    };
    context.with_workgroup(|workgroup| {
        if participates != 0 {
            let scratch = workgroup.allocate_lds::<u32, 64>();
            let _ = workgroup.reduce_sum(scratch, 1_u32);
        }
    });
}
