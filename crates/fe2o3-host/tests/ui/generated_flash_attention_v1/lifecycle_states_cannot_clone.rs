use fe2o3_host::{JoinedFlashAttentionV1, ReviewedFlashAttentionV1RuntimeAdapterV1};

fn clone_joined(value: JoinedFlashAttentionV1<'_, '_, '_, '_>) {
    let _copy = value.clone();
}

fn require_adapter<A: ReviewedFlashAttentionV1RuntimeAdapterV1>() {}

fn main() {}
