use fe2o3_host::{
    CompletedExactLdsGemmSlice1V1, LoadedExactLdsGemmSlice1V1,
    ReviewedExactLdsGemmRuntimeAdapterV1,
};

fn loaded_has_no_native_handles<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    loaded: &LoadedExactLdsGemmSlice1V1<'_, '_, '_, A>,
) {
    let _executable = loaded.native_executable_handle();
    let _kernel = loaded.native_kernel_handle();
}

fn completed_has_no_native_handles<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    completed: &CompletedExactLdsGemmSlice1V1<A>,
) {
    let _executable = completed.native_executable_handle();
    let _kernel = completed.native_kernel_handle();
}

fn main() {}
