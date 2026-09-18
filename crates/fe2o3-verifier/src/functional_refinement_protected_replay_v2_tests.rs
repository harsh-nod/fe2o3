use super::*;

#[test]
#[ignore = "requires the administrator-provisioned pinned runtime and protected process execution"]
fn protected_integer_replays_prove_and_output_substitution_is_rejected() {
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open(
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
    )
    .expect("open the real protected runtime; no synthetic lease");
    let cases = [
        (
            "single output",
            include_str!("../verus/mir_pliron_per_compilation_generated_fixture_v1.rs"),
            true,
        ),
        (
            "two outputs",
            include_str!(
                "../verus/mir_pliron_per_compilation_generated_multi_output_fixture_v1.rs"
            ),
            true,
        ),
        (
            "changed second output",
            include_str!(
                "../verus/negative/mir_pliron_per_compilation_multi_output_substitution_v1.rs"
            ),
            false,
        ),
    ];
    for (name, text, should_prove) in cases {
        let source = CanonicalGeneratedVerusProofInputV3::new(text.as_bytes().to_vec())
            .expect("bounded exact generated fixture");
        let deadline = Instant::now() + Duration::from_secs(60);
        let observed = runtime
            .execute_generated_rust_verify(
                &source,
                deadline,
                MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2,
            )
            .unwrap_or_else(|error| panic!("{name}: {error}"));
        runtime.revalidate().expect("retained runtime unchanged");
        assert!(Instant::now() < deadline, "{name}: deadline exceeded");
        let result = validate_proved_output(&observed);
        if should_prove {
            result.unwrap_or_else(|error| panic!("{name}: {error}"));
        } else {
            assert_eq!(
                result.unwrap_err().kind(),
                FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult,
            );
            assert_eq!(observed.exit_code, Some(1));
            assert_eq!(observed.signal, None);
            assert!(String::from_utf8_lossy(&observed.stderr).contains("assertion failed"));
        }
    }
}
