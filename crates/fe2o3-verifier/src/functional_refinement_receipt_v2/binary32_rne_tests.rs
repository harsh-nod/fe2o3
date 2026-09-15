const SOURCE: &str = include_str!("../../verus/binary32_rne_v1.rs");
const EVEN_TIE: &str = "2 * r == d && q % 2 != 0";
const QUARTER_ULP: &str = r#"
verus! {
proof fn fe2o3_negative_quarter_ulp_v1() {
    fe2o3_f32_rne_local_error_v1(16777217, 2, -23);
    assert(4 * fe2o3_rne_abs_v1(fe2o3_rne_units_v1(16777217, 2) * 2 - 16777217) <= 2);
}
}
"#;

fn programs() -> [CanonicalGeneratedVerusProofInputV3; 3] {
    assert_eq!(SOURCE.matches(EVEN_TIE).count(), 2);
    [
        SOURCE.to_owned(),
        SOURCE.replacen(EVEN_TIE, "2 * r == d && q % 2 == 0", 1),
        format!("{SOURCE}{QUARTER_ULP}"),
    ]
    .map(|source| CanonicalGeneratedVerusProofInputV3::new(source.into_bytes()).unwrap())
}

#[test]
fn binary32_model_inputs_are_canonical_distinct_and_not_admission_authority() {
    // The retained controller admits one solver; these query forms spin off more.
    assert!(!SOURCE.contains("nonlinear_arith"));
    assert!(!SOURCE.contains("by (bit_vector)"));
    let [positive, wrong_tie, tight_bound] = programs();
    assert_ne!(positive.identity(), wrong_tie.identity());
    assert_ne!(positive.identity(), tight_bound.identity());
    assert_ne!(wrong_tie.identity(), tight_bound.identity());
    for source in [&positive, &wrong_tie, &tight_bound] {
        assert!(!source.authenticates_verus_execution());
        assert!(!source.grants_artifact_or_runtime_authority());
    }
}

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
#[test]
#[ignore = "requires the root-protected pinned functional-refinement runtime"]
fn production_runtime_proves_binary32_model_and_rejects_wrong_tie_and_tight_bound() {
    let runtime = FunctionalRefinementVerusRuntimeLeaseV1::open(
        "/opt/fe2o3/verus-runtime-v2/functional-refinement-0.2026.08.02-b677dd5",
    )
    .expect("the model regression must use normal protected-runtime admission");
    for (index, source) in programs().into_iter().enumerate() {
        let output = runtime
            .execute_generated_rust_verify(
                &source,
                std::time::Instant::now() + std::time::Duration::from_secs(120),
                MAX_FUNCTIONAL_REFINEMENT_VERUS_OUTPUT_BYTES_V2,
            )
            .expect("bounded retained proof execution must complete");
        runtime.revalidate().unwrap();
        if index == 0 {
            validate_proved_output(&output).unwrap_or_else(|error| {
                panic!(
                    "model proof failed: {error}; stdout={} stderr={}",
                    String::from_utf8_lossy(&output.stdout),
                    String::from_utf8_lossy(&output.stderr)
                );
            });
        } else {
            assert_eq!(output.exit_code, Some(1));
            assert!(output.signal.is_none());
            assert!(String::from_utf8_lossy(&output.stderr).contains("assertion failed"));
            assert_eq!(
                validate_proved_output(&output).unwrap_err().kind(),
                FunctionalRefinementVerusExecutionErrorKindV2::UnexpectedProofResult,
            );
        }
    }
}
