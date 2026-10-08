//! Pure format/command controls. Only empty-argv original preflight is invoked.
//! No compiler callback, filesystem source admission, source execution or GPU.
use super::*;
use rustc_codegen_fe2o3::{
    SourceLocalOrderRecipeRequestV1 as Request, run_source_local_order_recipe_driver_v1,
};
fn refused_attempt() -> Attempt {
    let request = Request::create(
        "src/lib.rs",
        [17; 32],
        Order::ReverseReady,
        Relation::OrBeforeXor,
        Strength::Exact,
        Binding::ExactRevision,
    )
    .unwrap();
    run_source_local_order_recipe_driver_v1(&[], request)
}
fn parsed(attempt: &Attempt) -> Value {
    serde_json::from_slice(&render(attempt, Publication::NotAttempted).unwrap()).unwrap()
}
#[test]
fn legacy_and_additive_commands_are_exact_closed_choices() {
    assert_eq!(
        command(Some("create")),
        Some((Action::Create, Format::Human))
    );
    assert_eq!(
        command(Some("replay")),
        Some((Action::Replay, Format::Human))
    );
    assert_eq!(
        command(Some("create-json")),
        Some((Action::Create, Format::Json))
    );
    assert_eq!(
        command(Some("replay-json")),
        Some((Action::Replay, Format::Json))
    );
    for value in [
        None,
        Some(""),
        Some("Create"),
        Some("create-json "),
        Some("--json"),
        Some("cancel"),
        Some("resume"),
        Some("caller-pass-list"),
    ] {
        assert_eq!(command(value), None);
    }
}
#[test]
fn every_existing_typed_failure_phase_has_an_explicit_stable_label() {
    for (value, label) in [
        (Phase::Request, "request"),
        (Phase::Frontend, "frontend"),
        (Phase::Eligibility, "eligibility"),
        (Phase::RecipeBinding, "recipe_binding"),
        (Phase::Continuation, "continuation"),
        (Phase::Constraint, "constraint"),
        (Phase::Observation, "observation"),
        (Phase::SourceCurrentness, "source_currentness"),
    ] {
        assert_eq!(phase(value), label);
    }
}
#[test]
fn scheduling_labels_preserve_exact_advisory_and_actual_outcomes() {
    assert_eq!(order(Order::SourceOrder), "source_order");
    assert_eq!(order(Order::ReverseReady), "reverse_ready");
    assert_eq!(relation(Relation::XorBeforeOr), "xor_before_or");
    assert_eq!(relation(Relation::OrBeforeXor), "or_before_xor");
    assert_eq!(strength(Strength::Exact), "exact");
    assert_eq!(strength(Strength::Advisory), "advisory");
    assert_eq!(binding(Binding::ExactRevision), "exact_revision");
    assert_eq!(binding(Binding::RebindCurrent), "rebind_current");
    assert_eq!(
        constraint(Constraint::Honored {
            relation: Relation::OrBeforeXor
        }),
        json!({"status":"honored","relation":"or_before_xor"})
    );
    assert_eq!(
        constraint(Constraint::NotHonored {
            requested: Relation::OrBeforeXor,
            actual: Relation::XorBeforeOr
        }),
        json!({"status":"not_honored","requested":"or_before_xor","actual":"xor_before_or"})
    );
}
#[test]
fn original_preflight_refusal_retains_phase_diagnostic_and_zero_callbacks() {
    let attempt = refused_attempt();
    assert_eq!(attempt.callback_count(), 0);
    assert_eq!(attempt.compiler_callback_count(), 0);
    let original = attempt.result().unwrap_err();
    let value = parsed(&attempt);
    assert_eq!(value["result"]["status"], "refused");
    assert_eq!(value["result"]["failure"]["phase"], "request");
    assert_eq!(
        value["result"]["failure"]["diagnostic"],
        original.diagnostic()
    );
    assert_eq!(
        value["result"]["failure"]["compiler_fatal"],
        original.compiler_fatal()
    );
    assert_eq!(value["callback_count"], 0);
    assert_eq!(value["compiler_callback_count"], 0);
    for key in ["evidence", "llvm_returned", "recipe_returned"] {
        assert!(value["result"][key].is_null());
    }
}
#[test]
fn request_predicates_are_not_reported_as_authenticated_current_source() {
    let attempt = refused_attempt();
    let value = parsed(&attempt);
    assert_eq!(value["request"]["action"], "create");
    assert_eq!(value["request"]["source"], attempt.request().source_path());
    assert_eq!(
        value["request"]["expected_current_source_sha256"],
        json!(vec![17_u8; 32])
    );
    assert_eq!(value["execution_authenticated_by_report"], false);
    assert_eq!(value["authority"], "observation_only");
}
#[test]
fn diagnostic_report_has_exact_top_level_fields_and_explicit_unavailable_metrics() {
    let value = parsed(&refused_attempt());
    let mut keys = value
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    let mut expected = vec![
        "schema",
        "authority",
        "request",
        "callback_count",
        "compiler_callback_count",
        "result",
        "publication",
        "timing",
        "complete_owner_memory",
        "execution_authenticated_by_report",
        "proof_authority",
        "artifact_authority",
        "launch_authority",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected);
    assert_eq!(value["schema"], "fe2o3-source-local-order-report-v1");
    assert!(value["timing"].is_null());
    assert!(value["complete_owner_memory"].is_null());
    for field in ["proof_authority", "artifact_authority", "launch_authority"] {
        assert_eq!(value[field], false);
    }
}
#[test]
fn original_refusal_cannot_claim_output_publication() {
    let attempt = refused_attempt();
    assert!(render(&attempt, Publication::Completed).is_err());
    assert_eq!(
        parsed(&attempt)["publication"],
        json!({
            "status":"not_attempted","transactional":false,"output_file_currentness_authenticated":false
        })
    );
}
#[test]
fn bounded_encoding_counts_final_lf_and_refuses_short_capacity() {
    assert_eq!(encode(&7_u32, 2).unwrap(), b"7\n");
    assert!(encode(&7_u32, 1).is_err());
    assert!(encode(&"x".repeat(LIMIT), LIMIT).is_err());
    let bytes = render(&refused_attempt(), Publication::NotAttempted).unwrap();
    assert!(bytes.len() <= LIMIT && bytes.ends_with(b"\n"));
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap()["schema"],
        "fe2o3-source-local-order-report-v1"
    );
}
#[test]
fn output_prefix_starts_a_fresh_line_and_record_keeps_exact_json_bytes() {
    let bytes = render(&refused_attempt(), Publication::NotAttempted).unwrap();
    let mut out = b"earlier stdout".to_vec();
    write_record(&mut out, &bytes).unwrap();
    let mut expected = b"earlier stdout\n".to_vec();
    expected.extend_from_slice(PREFIX);
    expected.extend_from_slice(&bytes);
    assert_eq!(out, expected);
    assert!(write_record(&mut Vec::new(), b"{}").is_err());
}
#[test]
fn stream_failure_is_not_swallowed_or_relabelled_as_compiler_refusal() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("controlled writer"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    assert_eq!(
        write_record(&mut Broken, b"{}\n").unwrap_err().to_string(),
        "controlled writer"
    );
}
#[test]
fn serializer_failure_does_not_emit_a_partial_record() {
    struct Broken;
    impl Serialize for Broken {
        fn serialize<S: serde::Serializer>(&self, _: S) -> Result<S::Ok, S::Error> {
            Err(serde::ser::Error::custom("controlled serializer"))
        }
    }
    assert!(encode(&Broken, LIMIT).is_err());
}
#[test]
fn ordinary_example_keeps_the_single_original_driver_and_create_new_publication() {
    let source = include_str!("../source_local_order_recipe_v1.rs");
    assert_eq!(
        source
            .matches("run_source_local_order_recipe_driver_v1(rustc, request)")
            .count(),
        1
    );
    assert!(source.contains(".create_new(true)"));
    assert!(source.contains(".mode(0o600)"));
    assert!(source.contains("let output = match attempt.result()"));
    assert!(source.contains("report::emit(&attempt, report::Publication::NotAttempted)?"));
    assert!(source.contains("report::emit(&attempt, report::Publication::Completed)?"));
    assert!(!source.contains("run_source_local_order_recipe_probe_v1"));
    assert!(!source.contains("run_source_local_order_recipe_driver_measured_v1"));
}
