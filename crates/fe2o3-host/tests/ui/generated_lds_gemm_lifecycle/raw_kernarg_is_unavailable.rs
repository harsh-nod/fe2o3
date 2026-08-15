use fe2o3_host::{
    JoinedExactLdsGemmSlice1V1, LoadedExactLdsGemmSlice1V1,
    ReviewedExactLdsGemmRuntimeAdapterV1,
};

fn joined_has_no_raw_kernarg(joined: &JoinedExactLdsGemmSlice1V1<'_, '_, '_>) {
    let _bytes = joined.raw_kernarg();
}

fn loaded_has_no_raw_kernarg<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    loaded: &LoadedExactLdsGemmSlice1V1<'_, '_, '_, A>,
) {
    let _bytes = loaded.kernarg_bytes();
}

fn main() {}
