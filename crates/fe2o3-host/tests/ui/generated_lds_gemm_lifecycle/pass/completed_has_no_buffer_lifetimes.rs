use fe2o3_host::{CompletedExactLdsGemmSlice1V1, ReviewedExactLdsGemmRuntimeAdapterV1};

fn completed_type_has_only_adapter<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    completed: CompletedExactLdsGemmSlice1V1<A>,
) -> CompletedExactLdsGemmSlice1V1<A> {
    completed
}

fn main() {}
