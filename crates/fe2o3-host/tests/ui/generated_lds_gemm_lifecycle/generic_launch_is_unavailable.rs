use fe2o3_host::{LoadedExactLdsGemmSlice1V1, ReviewedExactLdsGemmRuntimeAdapterV1};

fn no_generic_launch<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    loaded: LoadedExactLdsGemmSlice1V1<'_, '_, '_, A>,
) {
    loaded.launch();
}

fn main() {}
