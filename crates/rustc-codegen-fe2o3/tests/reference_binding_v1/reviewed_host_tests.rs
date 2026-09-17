//! Source extraction checks using the compiler's fixed protected Verus runtime.
//! Successful ranked verification here grants no artifact or launch authority.

use super::*;

const VERIFIED_CALLBACK_PREFIX: &str = "fe2o3 production extraction: Rust -> semantic MIR -> ranked PLIRON -> safety-verified lowering input for `";
const VERIFIED_CALLBACK_SUFFIX: &str =
    "artifact/launch authority false, all mandatory kernel checks clean true, bounds clean true";
const FUNCTIONAL_PROOF_FAILURE: &str = "functional-refinement proof execution failed: functional-refinement Verus execution failed: UnexpectedProofResult";

fn run_protected_feature(feature: &str, expected_code: i32) -> String {
    // A fresh target and no cache wrapper force this source through the driver callback.
    let target = ScratchTarget::new();
    let mut command = Command::new("timeout");
    command
        // Match ci-local's outer step/kill bounds; compiler proof deadlines remain unchanged.
        .args(["--signal=TERM", "--kill-after=15s", "3000s"])
        .arg(env!("CARGO"))
        .env("CARGO_BUILD_JOBS", "1")
        .env("CARGO_INCREMENTAL", "0")
        .env("CARGO_PROFILE_DEV_DEBUG", "1")
        .env("CARGO_TERM_COLOR", "never")
        .env("RUSTC_WRAPPER", "")
        .env_remove("CARGO_BUILD_RUSTC_WRAPPER")
        .env_remove("CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER");
    let (status, stderr) = run_feature_command(command, &target.0, feature);
    assert_eq!(
        status.code(),
        Some(expected_code),
        "protected source feature {feature} had unexpected status {status}:\n{stderr}",
    );
    stderr
}

#[test]
#[ignore = "requires the reviewed Linux x86_64 host, GNU timeout, pinned nightly rust-src/AMD target, and installed root-owned pinned functional-refinement runtime"]
fn protected_rust_reference_positive_reaches_verified_callback() {
    let stderr = run_protected_feature("reference-positive", 0);
    let callbacks = stderr
        .lines()
        .filter(|line| line.starts_with(VERIFIED_CALLBACK_PREFIX))
        .collect::<Vec<_>>();
    assert_eq!(
        callbacks.len(),
        1,
        "positive source must execute exactly one ranked verification callback:\n{stderr}",
    );
    assert!(
        callbacks[0].ends_with(VERIFIED_CALLBACK_SUFFIX),
        "positive source did not pass mandatory kernel and bounds checks:\n{stderr}",
    );
    assert!(
        !stderr.contains("functional-refinement proof runtime unavailable")
            && !stderr.contains("functional-refinement proof execution failed"),
        "positive source must complete the real functional proof:\n{stderr}",
    );
}

#[test]
#[ignore = "requires the reviewed Linux x86_64 host, GNU timeout, pinned nightly rust-src/AMD target, and installed root-owned pinned functional-refinement runtime"]
fn protected_rust_reference_mutation_fails_functional_proof() {
    let stderr = run_protected_feature("reference-mutated", 101);
    assert!(
        stderr.lines().any(|line| {
            line.contains(FUNCTIONAL_PROOF_FAILURE)
                && line.contains("exit=Some(1), signal=None")
                && line.contains("verified, 1 errors\\n")
                && line.contains("assertion failed")
                && line.ends_with("compilation stopped before artifact emission")
        }),
        "CPU reference 18 versus GPU write 17 must fail the functional proof:\n{stderr}",
    );
    assert!(
        !stderr.contains("functional-refinement proof runtime unavailable")
            && !stderr.contains("source-to-proof V2 effect mismatch")
            && !stderr.contains(VERIFIED_CALLBACK_PREFIX),
        "mutated source must reach proof rejection without a verified callback:\n{stderr}",
    );
}
