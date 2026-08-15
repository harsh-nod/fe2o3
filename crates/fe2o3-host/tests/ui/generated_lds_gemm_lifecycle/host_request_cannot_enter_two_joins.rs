use fe2o3_host::{GeneratedLdsGemmSlice1HostAdapterV1, join_exact_lds_gemm_slice1_v1};
use fe2o3_hsaco_finalize::FinalizedExactLdsGemmHsacoV1;

fn join_host_request_twice<'a, 'b, 'c>(
    first_artifact: FinalizedExactLdsGemmHsacoV1,
    second_artifact: FinalizedExactLdsGemmHsacoV1,
    host: GeneratedLdsGemmSlice1HostAdapterV1<'a, 'b, 'c>,
) {
    let _first = join_exact_lds_gemm_slice1_v1(first_artifact, host);
    let _second = join_exact_lds_gemm_slice1_v1(second_artifact, host);
}

fn main() {}
