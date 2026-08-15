use fe2o3_hsaco_finalize::FinalizedExactLdsGemmHsacoV1;

fn attempt_launch(artifact: FinalizedExactLdsGemmHsacoV1) {
    let _identity = artifact.identity();
    let _bytes = artifact.exact_finalized_bytes();
    artifact.launch();
}

fn main() {}
