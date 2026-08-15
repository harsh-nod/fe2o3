use fe2o3_host::{LoadedExactLdsGemmSlice1V1, ReviewedExactLdsGemmRuntimeAdapterV1};

fn dispatch_twice<'a, 'b, 'c, A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    loaded: LoadedExactLdsGemmSlice1V1<'a, 'b, 'c, A>,
) {
    let _first = loaded.dispatch_and_wait();
    let _second = loaded.dispatch_and_wait();
}

fn main() {}
