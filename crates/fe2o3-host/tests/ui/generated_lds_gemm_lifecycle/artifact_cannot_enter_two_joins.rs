use fe2o3_host::{GeneratedLdsGemmSlice1HostAdapterV1, join_exact_lds_gemm_slice1_v1};
use fe2o3_hsaco_finalize::FinalizedExactLdsGemmHsacoV1;

fn join_artifact_twice<'a, 'b, 'c, 'd, 'e, 'f>(
    artifact: FinalizedExactLdsGemmHsacoV1,
    first: GeneratedLdsGemmSlice1HostAdapterV1<'a, 'b, 'c>,
    second: GeneratedLdsGemmSlice1HostAdapterV1<'d, 'e, 'f>,
) {
    let _first = join_exact_lds_gemm_slice1_v1(artifact, first);
    let _second = join_exact_lds_gemm_slice1_v1(artifact, second);
}

fn main() {}
