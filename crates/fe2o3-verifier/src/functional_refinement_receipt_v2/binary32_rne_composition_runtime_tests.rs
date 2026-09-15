#[test]
#[ignore = "requires the root-protected pinned functional-refinement runtime"]
fn production_runtime_proves_composition_and_rejects_too_tight_bound() {
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open(
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
    )
    .expect("composition regression must use normal protected-runtime admission");
    for (index, source) in programs().into_iter().enumerate() {
        let output = runtime
            .execute_generated_rust_verify(
                &source,
                std::time::Instant::now() + std::time::Duration::from_secs(120),
                MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2,
            )
            .expect("bounded retained composition proof execution must complete");
        runtime.revalidate().unwrap();
        if index == 0 {
            assert_eq!(
                output.stdout.as_slice(),
                b"verification results:: 24 verified, 0 errors\n"
            );
            validate_proved_output(&output).unwrap_or_else(|error| {
                panic!(
                    "composition failed: {error}; stdout={} stderr={}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            });
        } else {
            assert_eq!(output.exit_code, Some(1));
            assert!(output.signal.is_none());
            assert_eq!(
                output.stdout.as_slice(),
                b"verification results:: 24 verified, 1 errors\n"
            );
            assert!(String::from_utf8_lossy(&output.stderr).contains("assertion failed"));
            assert!(String::from_utf8_lossy(&output.stderr).lines().any(|line| {
                line.ends_with(
                    " |     assert(fe2o3_ar_goal_v1(33554432, 33554440, 33554432, 1, 1, 16777216));",
                )
            }));
            assert_eq!(
                validate_proved_output(&output).unwrap_err().kind(),
                FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult
            );
        }
    }
}
