use fe2o3_host::{CompletedExactLdsGemmSlice1V1, ReviewedExactLdsGemmRuntimeAdapterV1};

fn unload_twice<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    completed: CompletedExactLdsGemmSlice1V1<A>,
) {
    let _first = completed.unload();
    let _second = completed.unload();
}

fn main() {}
