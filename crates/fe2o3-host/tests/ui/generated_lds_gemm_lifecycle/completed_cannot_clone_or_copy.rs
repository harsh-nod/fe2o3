use fe2o3_host::{CompletedExactLdsGemmSlice1V1, ReviewedExactLdsGemmRuntimeAdapterV1};

fn require_clone<T: Clone>() {}
fn require_copy<T: Copy>() {}

fn completed_is_linear<A: ReviewedExactLdsGemmRuntimeAdapterV1>() {
    require_clone::<CompletedExactLdsGemmSlice1V1<A>>();
    require_copy::<CompletedExactLdsGemmSlice1V1<A>>();
}

fn main() {}
