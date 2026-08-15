use std::path::{Path, PathBuf};
use std::process::Command;

const PROOF: &str = include_str!("../verus/wave64_collectives_v1.rs");
const ACTIVE_EXCLUSION_WRONG: &str = include_str!("../verus/negative/active_exclusion_wrong.rs");
const BOUNDS_WRONG: &str = include_str!("../verus/negative/bounds_wrong.rs");
const OWNERSHIP_WRONG: &str = include_str!("../verus/negative/ownership_wrong.rs");
const REDUCTION_WRONG: &str = include_str!("../verus/negative/reduction_wrong.rs");
const SCAN_RECURRENCE_WRONG: &str = include_str!("../verus/negative/scan_recurrence_wrong.rs");
const RUNNER: &str = include_str!("../run-verus.sh");
const SCANNER: &str = include_str!("../check-proof-source.py");
const README: &str = include_str!("../README.md");

fn example_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

#[test]
fn proof_names_every_phase_a_obligation() {
    for marker in [
        "pub open spec fn explicit_wave64_mask_v1",
        "mask_bits as nat == mask_prefix_value_v1",
        "pub proof fn active_lane_indices_are_in_bounds_v1",
        "pub proof fn inactive_lane_is_excluded_and_publishes_zero_v1",
        "pub proof fn distinct_lanes_have_injective_output_ownership_v1",
        "pub proof fn masked_reduction_step_recurrence_v1",
        "pub proof fn reduction_is_full_masked_prefix_v1",
        "pub proof fn active_lane_scan_recurrence_v1",
        "pub proof fn empty_mask_has_zero_reduction_and_scans_v1",
    ] {
        assert!(PROOF.contains(marker), "missing formal obligation {marker}");
    }
    assert!(!PROOF.contains("macro_rules!"));
}

#[test]
fn expected_negatives_mutate_each_requested_property() {
    for (source, marker) in [
        (
            ACTIVE_EXCLUSION_WRONG,
            "mutated_inactive_lane_contributes_zero_v1",
        ),
        (BOUNDS_WRONG, "mutated_lane_63_output_is_bounded_v1"),
        (
            OWNERSHIP_WRONG,
            "mutated_distinct_lanes_have_distinct_owners_v1",
        ),
        (REDUCTION_WRONG, "mutated_reduction_equals_full_sum_v1"),
        (
            SCAN_RECURRENCE_WRONG,
            "mutated_inclusive_obeys_recurrence_v1",
        ),
    ] {
        assert!(source.contains(marker), "missing negative fixture {marker}");
    }
}

#[test]
fn scanner_is_token_aware_and_fail_closed() {
    for marker in [
        "unicodedata.normalize(\"NFKC\"",
        "raw_string_end",
        "unterminated block comment",
        "FORBIDDEN_IDENTIFIERS",
        "conditional proof source is forbidden",
    ] {
        assert!(SCANNER.contains(marker), "missing scanner rule {marker}");
    }

    let scanner = example_root().join("check-proof-source.py");
    let accepted =
        example_root().join("tests/fixtures/source-scanner/accept/comments_and_strings.rs");
    assert!(
        Command::new(&scanner)
            .arg(&accepted)
            .output()
            .unwrap()
            .status
            .success()
    );
    for rejected in ["assume.rs", "external_body.rs", "include.rs"] {
        let output = Command::new(&scanner)
            .arg(
                example_root()
                    .join("tests/fixtures/source-scanner/reject")
                    .join(rejected),
            )
            .output()
            .unwrap();
        assert!(!output.status.success(), "scanner accepted {rejected}");
        assert!(String::from_utf8_lossy(&output.stderr).contains("forbidden proof identifier"));
    }
}

#[test]
fn runner_pins_identity_closure_and_expected_failures() {
    for marker in [
        "VERUS_SHA256",
        "verify-verus-closure.sh",
        "env -i",
        "verification results:: 12 verified, 0 errors",
        "expected-negative proof unexpectedly verified",
        "FE2O3_WAVE64_COLLECTIVES_V1_VERUS_OK",
    ] {
        assert!(RUNNER.contains(marker), "missing runner boundary {marker}");
    }
    assert_eq!(
        include_str!("../verus/VERUS_VERSION"),
        "0.2026.08.02.b677dd5\n"
    );
    assert_eq!(
        include_str!("../verus/VERUS_SHA256"),
        "ad2669f579d898ede53f2bf84e80a1daf4e3578739b0f5807ef209a0c9f382dd\n"
    );
}

#[test]
fn documentation_keeps_phase_a_boundaries_explicit() {
    for marker in [
        "source/oracle/formal-evidence phase",
        "not compiler authentication",
        "not source-to-machine correspondence",
        "not artifact admission",
        "not hardware execution",
        "No COMGR",
        "no shell linker",
    ] {
        assert!(
            README.contains(marker),
            "missing documentation boundary {marker}"
        );
    }
}
