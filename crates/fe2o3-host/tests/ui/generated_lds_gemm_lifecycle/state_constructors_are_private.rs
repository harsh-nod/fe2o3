use fe2o3_host::{
    CompletedExactLdsGemmSlice1V1, JoinedExactLdsGemmSlice1V1,
    LoadedExactLdsGemmSlice1V1, ReviewedExactLdsGemmRuntimeAdapterV1,
};

fn constructors_are_not_public<'a, 'b, 'c, A: ReviewedExactLdsGemmRuntimeAdapterV1>() {
    let _joined = JoinedExactLdsGemmSlice1V1::<'a, 'b, 'c>::new();
    let _loaded = LoadedExactLdsGemmSlice1V1::<'a, 'b, 'c, A>::new();
    let _completed = CompletedExactLdsGemmSlice1V1::<A>::new();
}

fn main() {}
