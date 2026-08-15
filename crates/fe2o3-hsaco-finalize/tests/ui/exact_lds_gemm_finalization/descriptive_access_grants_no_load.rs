use fe2o3_hsaco_finalize::FinalizedExactLdsGemmHsacoV1;

fn attempt_load(artifact: FinalizedExactLdsGemmHsacoV1) {
    let _identity = artifact.identity();
    let _bytes = artifact.exact_finalized_bytes();
    artifact.load();
}

fn main() {}
