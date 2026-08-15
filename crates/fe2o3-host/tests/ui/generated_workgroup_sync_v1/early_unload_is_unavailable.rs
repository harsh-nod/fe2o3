use fe2o3_host::{LoadedWorkgroupScopedAtomicV1, ReviewedWorkgroupSyncRuntimeAdapterV1};

fn early<A: ReviewedWorkgroupSyncRuntimeAdapterV1>(
    value: LoadedWorkgroupScopedAtomicV1<'_, '_, '_, A>,
) {
    let _ = value.unload();
}

fn main() {}
