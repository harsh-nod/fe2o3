use fe2o3_host::{
    CompletedExactLdsGemmSlice1V1, JoinedExactLdsGemmSlice1V1,
    LoadedExactLdsGemmSlice1V1, ReviewedExactLdsGemmRuntimeAdapterV1,
};

fn extract_joined(joined: JoinedExactLdsGemmSlice1V1<'_, '_, '_>) {
    let JoinedExactLdsGemmSlice1V1 { artifact, host } = joined;
    let _ = (artifact, host);
}

fn extract_loaded<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    loaded: LoadedExactLdsGemmSlice1V1<'_, '_, '_, A>,
) {
    let LoadedExactLdsGemmSlice1V1 { state } = loaded;
    let _ = state;
}

fn extract_completed<A: ReviewedExactLdsGemmRuntimeAdapterV1>(
    completed: CompletedExactLdsGemmSlice1V1<A>,
) {
    let CompletedExactLdsGemmSlice1V1 { state, receipt } = completed;
    let _ = (state, receipt);
}

fn main() {}
