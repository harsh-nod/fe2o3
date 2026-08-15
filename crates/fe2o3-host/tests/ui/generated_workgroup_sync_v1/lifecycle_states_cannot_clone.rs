use fe2o3_host::{
    CompletedWorkgroupLdsReductionV1, JoinedWorkgroupLdsReductionV1,
    LoadedWorkgroupLdsReductionV1, ReviewedWorkgroupSyncRuntimeAdapterV1,
};

fn joined(value: JoinedWorkgroupLdsReductionV1<'_, '_>) {
    let _ = value.clone();
}

fn loaded<A: ReviewedWorkgroupSyncRuntimeAdapterV1>(
    value: LoadedWorkgroupLdsReductionV1<'_, '_, A>,
) {
    let _ = value.clone();
}

fn completed<A: ReviewedWorkgroupSyncRuntimeAdapterV1>(
    value: CompletedWorkgroupLdsReductionV1<A>,
) {
    let _ = value.clone();
}

fn main() {}
