use fe2o3_host::{GeneratedFlashAttentionV1HostAdapterV1, join_flash_attention_v1};
use fe2o3_hsaco_finalize::PreparedFinalizedWorkgroupSyncHsacoV1;

fn substitute(
    receipt: PreparedFinalizedWorkgroupSyncHsacoV1,
    host: GeneratedFlashAttentionV1HostAdapterV1<'_, '_, '_, '_>,
) {
    let _ = join_flash_attention_v1(receipt, host);
}

fn main() {}
