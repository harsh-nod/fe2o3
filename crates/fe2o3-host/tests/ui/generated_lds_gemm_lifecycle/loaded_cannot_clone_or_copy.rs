use fe2o3_host::{LoadedExactLdsGemmSlice1V1, ReviewedExactLdsGemmRuntimeAdapterV1};

fn require_clone<T: Clone>() {}
fn require_copy<T: Copy>() {}

fn loaded_is_linear<A: ReviewedExactLdsGemmRuntimeAdapterV1>() {
    require_clone::<LoadedExactLdsGemmSlice1V1<'static, 'static, 'static, A>>();
    require_copy::<LoadedExactLdsGemmSlice1V1<'static, 'static, 'static, A>>();
}

fn main() {}
