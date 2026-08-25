use fe2o3_functional_proof::MIR_PLIRON_SEMANTIC_REFINEMENT_THEOREM_SHA256_V1;
use sha2::{Digest, Sha256};

const POSITIVE: &[u8] = include_bytes!("../verus/mir_pliron_semantic_refinement_v1.rs");
const MANIFEST: &str = include_str!("../verus/pins/MIR_PLIRON_SEMANTIC_REFINEMENT_V1.manifest");
const RUNNER: &str = include_str!("../../../scripts/test-mir-pliron-semantic-refinement-verus.sh");
const CI_LOCAL: &str = include_str!("../../../scripts/ci-local.sh");

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn sha256(bytes: &[u8]) -> String {
    hex(&Sha256::digest(bytes))
}

#[test]
fn shared_theorem_and_four_workload_instantiations_are_pinned() {
    let source = std::str::from_utf8(POSITIVE).unwrap();
    for required in [
        "exact_total_output_refines_safe_reference_v1",
        "finite_recurrence_refines_reference_v1",
        "gemm_k_fold_refines_cpu_v1",
        "softmax_maximum_refines_cpu_v1",
        "attention_value_recurrence_refines_cpu_v1",
        "moe_routing_is_injective_v1",
    ] {
        assert!(source.contains(required), "missing {required}");
    }
    for forbidden in [
        "assume(",
        "admit(",
        "#[verifier::external_body]",
        "#[verifier::external]",
    ] {
        assert!(!source.contains(forbidden), "forbidden {forbidden}");
    }
    assert!(MANIFEST.contains(&format!("positive|{}|", sha256(POSITIVE))));
    assert_eq!(
        MIR_PLIRON_SEMANTIC_REFINEMENT_THEOREM_SHA256_V1.as_bytes(),
        Sha256::digest(POSITIVE).as_slice(),
    );
    assert!(MANIFEST.contains("scope|safe-reference-mir-to-kernel-mir-to-pliron"));
    assert!(MANIFEST.contains("excluded|llvm-isa-artifact-launch-hardware"));
}

#[test]
fn fail_closed_runner_pins_verus_and_requires_all_negative_rejections() {
    assert!(RUNNER.contains("${VERUS:?set VERUS to the pinned Verus executable}"));
    assert!(RUNNER.contains("ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd"));
    assert!(RUNNER.contains("verification results:: 8 verified, 0 errors"));
    for fixture in [
        "mir_pliron_gemm_wrong_term_v1",
        "mir_pliron_softmax_wrong_max_v1",
        "mir_pliron_attention_missing_rescale_v1",
        "mir_pliron_moe_noninjective_v1",
    ] {
        assert!(RUNNER.contains(fixture));
        assert!(MANIFEST.contains(fixture));
    }
    assert!(!RUNNER.contains("SKIP"));
    assert!(CI_LOCAL.contains("test-mir-pliron-semantic-refinement-verus.sh"));
}
