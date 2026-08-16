use fe2o3_host::{GeneratedMoeTop2V1HostAdapterV1, join_moe_top2_v1};
use fe2o3_hsaco_finalize::PreparedFinalizedWorkgroupSyncHsacoV1;

fn substitute(
    receipt: PreparedFinalizedWorkgroupSyncHsacoV1,
    host: GeneratedMoeTop2V1HostAdapterV1<'_, '_, '_, '_, '_, '_, '_, '_>,
) {
    let _ = join_moe_top2_v1(receipt, host);
}

fn main() {}
