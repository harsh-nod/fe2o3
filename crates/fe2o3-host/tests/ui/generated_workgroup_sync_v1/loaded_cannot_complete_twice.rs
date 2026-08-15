use fe2o3_host::{LoadedWorkgroupLdsReductionV1, ReviewedWorkgroupSyncRuntimeAdapterV1};

fn replay<A: ReviewedWorkgroupSyncRuntimeAdapterV1>(
    value: LoadedWorkgroupLdsReductionV1<'_, '_, A>,
) {
    let _ = value.dispatch_and_wait();
    let _ = value.dispatch_and_wait();
}

fn main() {}
