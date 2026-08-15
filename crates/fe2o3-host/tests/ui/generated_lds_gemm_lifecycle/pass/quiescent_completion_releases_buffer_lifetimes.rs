use fe2o3_host::{
    CompletedExactLdsGemmSlice1V1, ExactLdsGemmSlice1DispatchErrorV1,
    LoadedExactLdsGemmSlice1V1, ReviewedExactLdsGemmRuntimeAdapterV1,
};

fn complete<'a, 'b, 'c, A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    loaded: LoadedExactLdsGemmSlice1V1<'a, 'b, 'c, A>,
) -> Result<
    CompletedExactLdsGemmSlice1V1<A>,
    ExactLdsGemmSlice1DispatchErrorV1<A::Error>,
> {
    loaded.dispatch_and_wait()
}

fn main() {}
