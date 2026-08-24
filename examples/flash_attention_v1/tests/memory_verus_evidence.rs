use std::path::Path;
use std::process::Command;

use sha2::{Digest, Sha256};

const PROOF: &[u8] = include_bytes!("../verus/flash_attention_memory_v1.rs");
const KERNEL: &[u8] = include_bytes!("../src/kernel.rs");
const CLOSURE: &[u8] = include_bytes!("../verus/MEMORY_VERUS_CLOSURE_MANIFEST");
const RUNNER: &str = include_str!("../run-memory-verus.sh");
const NEGATIVES: &str = include_str!("../verus/MEMORY_NEGATIVE_SHA256");

const PROOF_SHA256: &str = "ba9931ba3657cd697ad5ffb853fd1193bd300f3f458b4b780019a033ef826c13";
const KERNEL_SHA256: &str = "da4f51b86cec00886d0261a35a2d4f97b67515c5449bb776feba4d4e5e1417cf";
const CLOSURE_SHA256: &str = "f06883e4ce463bcb9a3c8f911064ac85054c7822dc331db1a79f75f9e8878b01";
const TRANSCRIPT: &str = "FE2O3_FLASH_ATTENTION_MEMORY_V1_VERUS_OK mutations=8 obligations=13";
const TRANSCRIPT_SHA256: &str = "b72d9ec94325fc134abe7f1aa0f1bb434f2d14882807497eadb43c2746f369f5";

fn sha256(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn assert_proof_source_identity_matches_kernel(proof: &[u8], kernel_sha256: &str) {
    assert_eq!(kernel_sha256.len(), 64);
    let proof = std::str::from_utf8(proof).unwrap();
    let source_identity = proof
        .split_once("pub open spec fn source_identity_v1()")
        .expect("proof source identity")
        .1
        .split_once("\n}")
        .expect("proof source identity terminator")
        .0;
    assert_eq!(source_identity.matches("0x").count(), 4);
    for offset in (0..64).step_by(16) {
        assert!(
            source_identity.contains(&format!("0x{}u64", &kernel_sha256[offset..offset + 16])),
            "proof source identity omits kernel digest limb {}",
            &kernel_sha256[offset..offset + 16]
        );
    }
}

#[test]
fn bounded_memory_proof_and_execution_closure_are_identity_bound() {
    assert_eq!(PROOF.len(), 9_810);
    assert_eq!(sha256(PROOF), PROOF_SHA256);
    assert_eq!(sha256(KERNEL), KERNEL_SHA256);
    assert_proof_source_identity_matches_kernel(PROOF, KERNEL_SHA256);
    assert_eq!(sha256(CLOSURE), CLOSURE_SHA256);
    assert_eq!(sha256(TRANSCRIPT.as_bytes()), TRANSCRIPT_SHA256);
    assert_eq!(
        include_str!("../verus/MEMORY_MODEL_SHA256").trim(),
        PROOF_SHA256
    );
    assert_eq!(
        include_str!("../verus/MEMORY_VERUS_CLOSURE_MANIFEST_SHA256").trim(),
        CLOSURE_SHA256
    );
    assert_eq!(
        include_str!("../verus/MEMORY_TRANSCRIPT_SHA256").trim(),
        TRANSCRIPT_SHA256
    );
}

#[test]
fn proof_names_exact_memory_ownership_and_conservative_boundaries() {
    let proof = std::str::from_utf8(PROOF).unwrap();
    for marker in [
        "qkv_reads_are_within_128_f32_v1",
        "qkv_read_addresses_are_inside_regions_v1",
        "assigned_output_write_is_inside_region_v1",
        "distinct_lane_slots_have_disjoint_output_writes_v1",
        "causal_key_row_is_bounded_v1",
        "reads_precede_owned_output_commit_v1",
        "published_machine_body_identity_v1",
        "analyzer_profile_identity_v1",
        "compiler_refinement_claimed_v1() -> bool { false }",
        "logical_address_refinement_claimed_v1() -> bool { false }",
        "isa_refinement_claimed_v1() -> bool { false }",
        "generalized_machine_memory_safety_claimed_v1() -> bool { false }",
        "generalized_gpu_race_freedom_claimed_v1() -> bool { false }",
        "gpu_execution_claimed_v1() -> bool { false }",
    ] {
        assert!(proof.contains(marker), "missing proof boundary {marker}");
    }
    for forbidden in ["assume(", "admit(", "external_body", "uninterp spec"] {
        assert!(!proof.contains(forbidden), "proof contains {forbidden}");
    }
}

#[test]
fn all_eight_verus_mutations_are_pinned_and_executed() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let entries: Vec<_> = NEGATIVES.lines().collect();
    assert_eq!(entries.len(), 8);
    for entry in entries {
        let (digest, relative) = entry.split_once("  ").unwrap();
        let source = std::fs::read(manifest.join("verus").join(relative)).unwrap();
        assert_eq!(sha256(&source), digest, "mutation drifted: {relative}");
        let stem = Path::new(relative).file_stem().unwrap().to_str().unwrap();
        assert!(RUNNER.contains(stem), "runner omits {relative}");
    }
}

#[test]
fn runner_is_fail_closed_and_transcript_is_pinned() {
    for marker in [
        "verification results:: 13 verified, 0 errors",
        "verification results:: 0 verified, 1 errors",
        "check_digest \"$expected_proof\" \"$proof\"",
        "check_digest \"$expected_verus\" \"$verus_path\"",
        "\"$closure_checker\" \"$verus_root\" \"$closure_manifest\"",
        "actual_transcript=$(printf '%s' \"$transcript\"",
        TRANSCRIPT,
    ] {
        assert!(RUNNER.contains(marker), "runner is missing {marker}");
    }
    for forbidden in ["SKIP:", "exit 77", "VERUS_PROOF", "VERUS_KERNEL"] {
        assert!(!RUNNER.contains(forbidden), "runner contains {forbidden}");
    }
}

#[test]
fn transcript_and_proof_mutations_change_expected_evidence_identity() {
    let mut proof = PROOF.to_vec();
    proof[PROOF.len() / 2] ^= 1;
    assert_ne!(sha256(&proof), PROOF_SHA256);
    assert_ne!(
        sha256(
            TRANSCRIPT
                .replace("obligations=13", "obligations=12")
                .as_bytes()
        ),
        TRANSCRIPT_SHA256
    );
}

#[test]
#[ignore = "requires the exact pinned Verus closure"]
fn exact_memory_verus_runner_executes_pinned_evidence() {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let output = Command::new(manifest.join("run-memory-verus.sh"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains(TRANSCRIPT));
}
