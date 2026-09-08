//! Shared source shape for epoch-aware workgroup collectives.

use fe2o3_device::{
    AcquireRelease, InitialEpoch, LdsElement, WorkgroupCapability, WorkgroupCollectiveElement,
    WorkgroupMemory, WorkgroupScope,
};

/// Results produced by one convergent traversal of the workgroup hierarchy.
#[allow(dead_code)]
pub(crate) struct WorkgroupCollectiveResultsV1<T> {
    pub(crate) reduction: T,
    pub(crate) inclusive: T,
    pub(crate) exclusive: T,
}

/// Initializes and publishes LDS, then reuses one scratch allocation across
/// reduction and both scan epochs.
pub(crate) fn execute_workgroup_collectives_v1<'workgroup, T, const ELEMENTS: usize, KernelBrand>(
    workgroup: WorkgroupCapability<'workgroup, KernelBrand, InitialEpoch>,
    value: T,
) -> WorkgroupCollectiveResultsV1<T>
where
    T: Copy + LdsElement + WorkgroupCollectiveElement + 'workgroup,
{
    let initialized = workgroup
        .allocate_lds::<T, ELEMENTS>()
        .initialize_by_invocation(&workgroup, value);
    let (workgroup, published) = workgroup.publish_lds(initialized);
    let Some(value) = published.read(&workgroup, workgroup.invocation_rank() as usize) else {
        fe2o3_device::trap();
    };
    drop(published);

    let workgroup = workgroup.barrier::<WorkgroupScope, AcquireRelease, WorkgroupMemory>();
    let scratch = workgroup.allocate_lds::<T, ELEMENTS>();
    let (workgroup, scratch, reduction) = workgroup.reduce_sum(scratch, value);
    let (workgroup, scratch, inclusive) = workgroup.inclusive_scan_sum(scratch, value);
    let (_workgroup, _scratch, exclusive) = workgroup.exclusive_scan_sum(scratch, value);

    WorkgroupCollectiveResultsV1 {
        reduction,
        inclusive,
        exclusive,
    }
}
