//! Pure report/API controls. No compiler callback or source authority fabricated.
use super::*;

#[test]
fn admission_command_has_public_bounded_driver_signature() {
    let _: fn(&[String], &Path) -> Result<(), String> =
        run_bf16_generated_source_admission_driver_v1;
}

#[test]
fn initial_report_is_failed_and_has_no_execution_or_production_authority() {
    let r = generated_report();
    assert_eq!(r["status"], "failed");
    for key in [
        "source_postflight_ok",
        "source_admitted",
        "nominal_pre_ranked_materialized",
        "normal_ranked_attempted",
        "normal_ranked_admitted",
        "source_writes_performed",
        "simulation_performed",
        "artifact_emitted",
        "native_execution",
        "hardware_observed",
        "grants_artifact_or_launch_authority",
    ] {
        assert_eq!(r[key], false, "{key}");
    }
    assert!(r["normal_refusal"].is_null() && r["admission"].is_null());
}

#[test]
fn inherited_bounded_report_writer_retains_failure_and_never_relabels_success() {
    let r = generated_report();
    let mut bytes = Vec::new();
    let error = finish_report_with_writer(&r, Some("actual admission refused".into()), |b| {
        bytes.extend_from_slice(b);
        Ok(())
    })
    .unwrap_err();
    assert!(error.contains("retained observation.json"));
    let actual: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(actual, r);
    assert_eq!(actual["status"], "failed");
}

#[test]
fn inherited_report_cap_refuses_before_publisher() {
    let mut r = generated_report();
    r["admission"] = "x".repeat(REPORT_CAP).into();
    let mut called = false;
    assert!(
        finish_report_with_writer(&r, None, |_| {
            called = true;
            Ok(())
        })
        .is_err()
    );
    assert!(!called);
}

#[test]
fn no_source_publication_or_test_only_cpu_seam_in_admission_driver() {
    let source = include_str!("bf16_generated_source_driver_v1.rs");
    for forbidden in [
        "for_test_v1(",
        "publish_bf16_tile_source_candidate_v1(",
        "snapshot(",
        "Command::new(",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
    assert!(source.contains("inspect_bf16_generated_source_admission_v1()"));
    assert!(source.contains("finish_report(output, &report, failure)"));
}
