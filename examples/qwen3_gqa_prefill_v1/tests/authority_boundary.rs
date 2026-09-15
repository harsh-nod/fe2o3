use fe2o3_qwen3_gqa_prefill_v1::{
    B3PrefillBucketV1, GQA_PREFILL_ARTIFACT_LOAD_SUPPORTED_V1,
    GQA_PREFILL_ARTIFACT_PUBLICATION_SUPPORTED_V1, GQA_PREFILL_LAUNCH_SUPPORTED_V1,
    GQA_PREFILL_MACHINE_REFINEMENT_PROVED_V1, GQA_PREFILL_PRODUCTION_BLOCKER_V1,
    GQA_PREFILL_SOURCE_TO_KIR_SUPPORTED_V1, GQA_PREFILL_VERUS_PROOF_SUPPORTED_V1,
    GqaCandidateDescriptorV1, GqaPrefillProfileDescriptorV1, Qwen3AttentionRoleV1,
    validate_structural_gqa_candidate_v1,
};

const LIB_SOURCE: &str = include_str!("../src/lib.rs");
const README: &str = include_str!("../README.md");
const MANIFEST: &str = include_str!("../Cargo.toml");

#[test]
fn production_proof_and_machine_authorities_remain_withheld() {
    assert!(!std::hint::black_box(
        GQA_PREFILL_SOURCE_TO_KIR_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(GQA_PREFILL_VERUS_PROOF_SUPPORTED_V1));
    assert!(!std::hint::black_box(
        GQA_PREFILL_ARTIFACT_PUBLICATION_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(
        GQA_PREFILL_ARTIFACT_LOAD_SUPPORTED_V1
    ));
    assert!(!std::hint::black_box(GQA_PREFILL_LAUNCH_SUPPORTED_V1));
    assert!(!std::hint::black_box(
        GQA_PREFILL_MACHINE_REFINEMENT_PROVED_V1
    ));
    for required in [
        "same-session Rust MIR",
        "Verus discharge",
        "machine refinement",
    ] {
        assert!(GQA_PREFILL_PRODUCTION_BLOCKER_V1.contains(required));
    }
}

#[test]
fn copyable_structural_candidate_is_explicitly_non_authoritative() {
    let candidate = validate_structural_gqa_candidate_v1(GqaCandidateDescriptorV1::canonical(
        GqaPrefillProfileDescriptorV1::canonical(
            Qwen3AttentionRoleV1::Target8B,
            B3PrefillBucketV1::S1T128,
        ),
    ))
    .unwrap();
    let copied = candidate;
    assert_eq!(candidate, copied);
    assert!(!candidate.grants_production_authority());
}

#[test]
fn standalone_crate_has_no_gpu_compiler_runtime_or_process_path() {
    let complete = format!("{LIB_SOURCE}\n{README}");
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
        "contains no GPU source",
        "not Verus proof",
        "issue #174",
        "no GPU code, compilation",
        "do not establish real-number, IEEE-754, OCML, ISA",
    ] {
        assert!(complete.contains(required), "missing boundary: {required}");
    }
}
