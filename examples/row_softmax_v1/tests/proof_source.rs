use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

const PROOF: &[u8] = include_bytes!("../verus/row_softmax_v1.rs");
const DUPLICATE_WRITER: &[u8] = include_bytes!("../verus/negative/duplicate_writer.rs");
const LANE_PLUS_ONE: &[u8] = include_bytes!("../verus/negative/lane_plus_one_out_of_bounds.rs");
const WRONG_NUMERATOR: &[u8] = include_bytes!("../verus/negative/wrong_numerator_index.rs");
const VERUS_CLOSURE_MANIFEST: &[u8] = include_bytes!("../verus/VERUS_CLOSURE_MANIFEST");
const RUNNER: &str = include_str!("../run-verus.sh");
const README: &str = include_str!("../README.md");

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

#[test]
fn proof_and_negative_mutations_have_exact_source_pins() {
    let pins = [
        (
            "positive proof",
            PROOF,
            11_143,
            "61f1453d267a8e9183334dfe1ca37bcd69c92df4b55d56606907627c5691a9f9",
        ),
        (
            "duplicate writer",
            DUPLICATE_WRITER,
            461,
            "3db467548fc10ea8dc00b275e1de23e341ad0da93a4ebbada1924e75ba8e0b51",
        ),
        (
            "lane plus one",
            LANE_PLUS_ONE,
            331,
            "658711c4f11e4a69eb3375c9fe706d1386c927437e987c92cec547e656b80839",
        ),
        (
            "wrong numerator index",
            WRONG_NUMERATOR,
            2_145,
            "fd06e5e50e655c738583f8901889a9bc9e9cc8803594e1ff7f8450ae00e00c7e",
        ),
    ];
    for (name, source, bytes, digest) in pins {
        assert_eq!(source.len(), bytes, "changed byte length for {name}");
        assert_eq!(sha256(source), digest, "changed source digest for {name}");
    }
}

#[test]
fn proof_names_all_layers_and_has_no_shortcuts() {
    let proof = std::str::from_utf8(PROOF).unwrap();
    let theorem_markers = [
        "pub proof fn fixed_row_is_nonempty_v1",
        "pub proof fn active_lane_indices_are_in_bounds_v1",
        "pub proof fn distinct_lanes_own_distinct_scratch_and_output_v1",
        "pub proof fn active_element_address_is_in_row_v1",
        "pub proof fn separate_input_and_output_accesses_do_not_alias_v1",
        "pub proof fn distinct_output_element_addresses_v1",
        "pub proof fn distinct_scratch_element_addresses_v1",
        "pub proof fn maximum_stable_shift_is_nonpositive_v1",
        "pub proof fn denominator_reduction_step_preserves_state_v1",
        "pub proof fn positive_prefix_has_positive_sum_v1",
        "pub proof fn positive_weight_premises_give_positive_denominator_v1",
        "pub proof fn stable_softmax_spec_preserves_lane_numerator_correspondence_v1",
        "pub proof fn finite_numerator_premises_give_positive_lane_v1",
        "pub proof fn finite_numerator_premises_transport_sum_to_denominator_v1",
        "pub proof fn stable_softmax_spec_premises_give_positive_denominator_v1",
    ];
    assert_eq!(
        proof.matches("pub proof fn ").count(),
        theorem_markers.len()
    );
    for marker in theorem_markers {
        assert!(proof.contains(marker), "missing theorem {marker}");
    }
    for marker in [
        "pub uninterp spec fn exp_real_v1",
        "pub open spec fn stable_softmax_spec_v1",
        "pub open spec fn denominator_state_v1",
        "pub open spec fn finite_numerator_premises_v1",
        "output[index] * prefix_sum_v1(weights, row_elements_v1()) == weights[index]",
    ] {
        assert!(proof.contains(marker), "missing contract marker {marker}");
    }
    for source in [PROOF, DUPLICATE_WRITER, LANE_PLUS_ONE, WRONG_NUMERATOR] {
        let source = std::str::from_utf8(source).unwrap();
        let normalized: String = source
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        for shortcut in ["admit(", "assume(", "#[verifier::external_body]"] {
            assert!(
                !normalized.contains(shortcut),
                "forbidden normalized construct {shortcut}"
            );
        }
    }
}

#[test]
fn wrong_numerator_mutates_the_spec_and_fails_its_correspondence_surface() {
    let proof = std::str::from_utf8(PROOF).unwrap();
    let mutation = std::str::from_utf8(WRONG_NUMERATOR).unwrap();
    let theorem = "stable_softmax_spec_preserves_lane_numerator_correspondence_v1";

    assert!(proof.contains(&format!("pub proof fn {theorem}")));
    assert!(proof.contains("== weights[lane as int]"));
    assert!(mutation.contains("pub open spec fn mutated_stable_softmax_spec_v1"));
    assert!(mutation.contains("pub open spec fn exp_weights_contract_v1"));
    assert!(!mutation.contains("pub uninterp spec fn exp_weights_contract_v1"));
    assert!(mutation.contains("== weights[0]"));
    assert!(mutation.contains(&format!("pub proof fn mutated_{theorem}")));
    assert!(mutation.contains("== weights[lane as int]"));
}

#[test]
fn documentation_states_the_numeric_and_artifact_boundaries() {
    for statement in [
        "V1 has no mask",
        "there is no all-masked fallback or zero-denominator rule",
        "`exp_real_v1` is uninterpreted",
        "to refine `f32`.",
        "address-set facts only",
        "not compute or establish normalization.",
        "ISA, HSACO, loading, launch",
    ] {
        assert!(
            README.contains(statement),
            "missing boundary statement {statement}"
        );
    }
}

#[test]
fn pinned_verus_identity_and_release_closure_are_exact() {
    assert_eq!(
        include_str!("../verus/VERUS_VERSION"),
        "0.2026.08.02.b677dd5\n"
    );
    assert_eq!(
        include_str!("../verus/VERUS_SHA256"),
        "ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd\n"
    );
    assert_eq!(
        sha256(VERUS_CLOSURE_MANIFEST),
        "d28df3fb5e0d747637543933dfc38cff45576da9b920d755b4b7e919e47a6019"
    );
    let closure = std::str::from_utf8(VERUS_CLOSURE_MANIFEST).unwrap();
    for marker in [
        "file-count=190",
        "required=verus|",
        "required=rust_verify|",
        "required=z3|",
        "subtree=vstd|130|",
    ] {
        assert!(closure.contains(marker), "missing closure marker {marker}");
    }
}

#[test]
fn runner_uses_the_authenticated_solver_under_a_minimal_environment() {
    for marker in [
        "verify-verus-closure.sh",
        "\"$env_path\" -i",
        "VERUS_Z3_PATH=$verus_root/z3",
        "RUSTUP_HOME=$runner_rustup_home",
        "CARGO_HOME=$runner_cargo_home",
    ] {
        assert!(RUNNER.contains(marker), "missing runner boundary {marker}");
    }
    assert!(!RUNNER.contains("VERUS_Z3_PATH=${VERUS_Z3_PATH"));
}

#[cfg(unix)]
#[test]
fn matching_version_fake_verus_is_rejected_by_executable_digest() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let fake = manifest.join("tests/fixtures/fake-verus-matching-version.sh");
    let runner = manifest.join("run-verus.sh");
    let output = Command::new(runner).env("VERUS", fake).output().unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8(output.stderr).unwrap();
    assert!(stderr.contains("Verus executable SHA-256"));
    assert!(stderr.contains("does not match pinned"));
    assert!(!stderr.contains("fake verifier must never reach proof execution"));
}
