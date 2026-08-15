use fe2o3_host::{CompletedWorkgroupScopedAtomicV1, ReviewedWorkgroupSyncRuntimeAdapterV1};

fn replay<A: ReviewedWorkgroupSyncRuntimeAdapterV1>(
    value: CompletedWorkgroupScopedAtomicV1<A>,
) {
    let _ = value.unload();
    let _ = value.unload();
}

fn main() {}
