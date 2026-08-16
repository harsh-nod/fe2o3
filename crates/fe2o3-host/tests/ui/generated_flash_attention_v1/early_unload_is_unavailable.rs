use fe2o3_host::{LoadedFlashAttentionV1, ReviewedFlashAttentionV1RuntimeAdapterV1};

fn unload_early<A: ReviewedFlashAttentionV1RuntimeAdapterV1>(
    loaded: LoadedFlashAttentionV1<'_, '_, '_, '_, A>,
) {
    let _ = loaded.unload();
}

fn main() {}
