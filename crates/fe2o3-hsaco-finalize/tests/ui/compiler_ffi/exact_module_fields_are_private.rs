use fe2o3_hsaco_finalize::{WorkerInputKindV1, stage_exact_compiler_module_artifact_v1};

fn main() {
    let artifact = stage_exact_compiler_module_artifact_v1(
        WorkerInputKindV1::LlvmBitcode,
        vec![1],
    )
    .unwrap();
    let _ = artifact.input;
}
