use fe2o3_host::{
    GeneratedWorkgroupScopedAtomicV1HostAdapterV1, join_workgroup_lds_reduction_v1,
};
use fe2o3_hsaco_finalize::PreparedFinalizedWorkgroupSyncHsacoV1;

fn substitute(
    receipt: PreparedFinalizedWorkgroupSyncHsacoV1,
    atomic: GeneratedWorkgroupScopedAtomicV1HostAdapterV1<'_, '_, '_>,
) {
    let _ = join_workgroup_lds_reduction_v1(receipt, atomic);
}

fn main() {}
