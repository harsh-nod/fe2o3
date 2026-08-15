use fe2o3_hsaco_finalize::PreparedFinalizedWorkgroupSyncHsacoV1;

fn extract(receipt: &PreparedFinalizedWorkgroupSyncHsacoV1) {
    let _ = receipt.exact_finalized_bytes_for_reviewed_workgroup_sync_runtime_v1();
}

fn main() {}
