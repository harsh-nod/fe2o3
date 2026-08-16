use fe2o3_host::{CompletedFlashAttentionV1, ReviewedFlashAttentionV1RuntimeAdapterV1};

fn unload_twice<A: ReviewedFlashAttentionV1RuntimeAdapterV1>(
    completed: CompletedFlashAttentionV1<A>,
) {
    let _first = completed.unload();
    let _second = completed.unload();
}

fn main() {}
