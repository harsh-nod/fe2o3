use fe2o3_rmsnorm_residual_v1::{
    B3RmsNormBucketV1, Qwen3ModelRoleV1, RMSNORM_FOUNDATION_OBLIGATIONS_V1,
    RMSNORM_RESIDUAL_ARTIFACT_LOAD_SUPPORTED_V1,
    RMSNORM_RESIDUAL_ARTIFACT_PUBLICATION_SUPPORTED_V1, RMSNORM_RESIDUAL_GPU_LAUNCH_SUPPORTED_V1,
    RMSNORM_RESIDUAL_MACHINE_REFINEMENT_PROVED_V1, RMSNORM_RESIDUAL_PRODUCTION_BLOCKER_V1,
    RMSNORM_RESIDUAL_SOURCE_TO_KIR_SUPPORTED_V1, RmsNormCandidateDescriptorV1, RmsNormObligationV1,
    RmsNormProfileDescriptorV1, validate_structural_candidate_v1,
};

const LIB_SOURCE: &str = include_str!("../src/lib.rs");
const README: &str = include_str!("../README.md");
const MANIFEST: &str = include_str!("../Cargo.toml");

#[test]
fn all_production_authorities_remain_withheld() {
    assert!(!std::hint::black_box(
        RMSNORM_RESIDUAL_SOURCE_TO_KIR_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(
        RMSNORM_RESIDUAL_ARTIFACT_PUBLICATION_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(
        RMSNORM_RESIDUAL_ARTIFACT_LOAD_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(
        RMSNORM_RESIDUAL_GPU_LAUNCH_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(
        RMSNORM_RESIDUAL_MACHINE_REFINEMENT_PROVED_V1
    ));
    assert!(RMSNORM_RESIDUAL_PRODUCTION_BLOCKER_V1.contains("same-session Rust MIR"));
    assert!(RMSNORM_RESIDUAL_PRODUCTION_BLOCKER_V1.contains("machine refinement"));
}

#[test]
fn foundation_obligations_do_not_smuggle_machine_refinement() {
    assert_eq!(RMSNORM_FOUNDATION_OBLIGATIONS_V1.len(), 10);
    assert!(
        !RMSNORM_FOUNDATION_OBLIGATIONS_V1
            .contains(&RmsNormObligationV1::MachineRefinementBoundary)
    );
}

#[test]
fn structural_candidate_is_copyable_only_because_it_is_non_authoritative() {
    let candidate = validate_structural_candidate_v1(RmsNormCandidateDescriptorV1::canonical(
        RmsNormProfileDescriptorV1::canonical(
            Qwen3ModelRoleV1::Target8B,
            B3RmsNormBucketV1::DecodeS1,
        ),
    ))
    .unwrap();
    let copied = candidate;
    assert_eq!(candidate, copied);
    assert!(!candidate.grants_production_authority());
}

#[test]
fn standalone_foundation_has_no_compiler_runtime_or_process_escape_path() {
    for forbidden in [
        "fe2o3-compiler",
        "fe2o3-runtime",
        "std::process",
        "Command::new",
        "comgr",
        "hipModuleLoad",
        "hipModuleLaunchKernel",
    ] {
        assert!(
            !MANIFEST.contains(forbidden),
            "manifest contains {forbidden}"
        );
        assert!(!LIB_SOURCE.contains(forbidden), "lib contains {forbidden}");
    }
    for required in [
        "not compiler evidence",
        "proof evidence and expose no compilation",
        "publication, load, dispatch, or",
        "issue #174",
        "not source, MIR, KIR, LLVM, object, or HSACO identities",
    ] {
        let complete = format!("{LIB_SOURCE}\n{README}");
        assert!(complete.contains(required), "missing boundary: {required}");
    }
}
