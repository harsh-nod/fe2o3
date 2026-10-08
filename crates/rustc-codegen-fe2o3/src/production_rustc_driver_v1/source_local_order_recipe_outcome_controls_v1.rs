//! Pure inert controls only; none of these results qualify a genuine workload.
use super::*;

fn bound(workload: Workload) -> Binding {
    Binding {
        workload,
        source: "src/lib.rs".into(),
        current_source_sha256: "22".repeat(32),
        origin_source_sha256: "11".repeat(32),
        origin_normal_sha256: "33".repeat(32),
        origin_source_initializer: [1, 2, 3, 4],
        recipe_sha256: "44".repeat(32),
        instance_axes: [[5; 32]; 5],
    }
}
fn rows(bytes: &[u8]) -> Vec<Row> {
    (0..35)
        .map(|i| Row {
            ordinal: i + 1,
            calibration: i < 5,
            elapsed_ns: if i < 5 { u64::MAX } else { (i - 4) as u64 },
            outcome_bytes: bytes.len(),
            outcome_sha256: sha(bytes),
            exact_ordinary_oracle_equal: true,
        })
        .collect()
}
#[test]
fn statistics_excludes_exactly_five_calibration_rows_and_keeps_outliers() {
    let bytes = b"ordinary";
    assert_eq!(
        statistics(&rows(bytes), bytes).unwrap(),
        Stats {
            p50_ns: 15,
            p95_ns: 29,
            max_ns: 30
        }
    );
    let mut samples = rows(bytes);
    samples[34].elapsed_ns = 1000;
    assert_eq!(statistics(&samples, bytes).unwrap().max_ns, 1000);
    assert_eq!(statistics(&samples, bytes).unwrap().p95_ns, 29);
}
#[test]
fn missing_extra_duplicate_out_of_order_and_wrong_calibration_refuse() {
    let bytes = b"ordinary";
    let samples = rows(bytes);
    assert!(statistics(&samples[..34], bytes).is_err());
    let mut changed = samples.clone();
    changed.push(changed[34].clone());
    assert!(statistics(&changed, bytes).is_err());
    let mut changed = samples.clone();
    changed[5].ordinal = 5;
    assert!(statistics(&changed, bytes).is_err());
    let mut changed = samples.clone();
    changed.swap(5, 6);
    assert!(statistics(&changed, bytes).is_err());
    let mut changed = samples;
    changed[5].calibration = true;
    assert!(statistics(&changed, bytes).is_err());
}
#[test]
fn full_oracle_byte_hash_and_comparison_refusals_are_not_samples() {
    for change in 0..3 {
        let mut samples = rows(b"ordinary");
        match change {
            0 => samples[9].outcome_bytes += 1,
            1 => samples[9].outcome_sha256 = "00".repeat(32),
            _ => samples[9].exact_ordinary_oracle_equal = false,
        }
        assert!(statistics(&samples, b"ordinary").is_err());
    }
    assert!(statistics(&rows(b"ordinary"), b"ordinary\n").is_err());
}
#[test]
fn only_original_designated_recipe_binding_error_is_expected() {
    let original = Failure::new(Phase::RecipeBinding, REFUSAL.into());
    check_refusal(&original).unwrap();
    assert!(check_refusal(&Failure::new(Phase::Eligibility, REFUSAL.into())).is_err());
    assert!(check_refusal(&Failure::new(Phase::RecipeBinding, "different".into())).is_err());
}
#[test]
fn expected_refusal_is_explicit_not_a_success_or_empty_output() {
    let original = Failure::new(Phase::RecipeBinding, REFUSAL.into());
    let bytes = outcome(Err(&original), &bound(Workload::ExactRevisionRefusal), 1, 1).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["status"], "refused");
    assert!(value["normal_utf8"].is_null());
    assert_eq!(value["failure"]["phase"], "RecipeBinding");
    assert_eq!(value["failure"]["diagnostic"], REFUSAL);
    assert_eq!(value["failure"]["compiler_fatal"], false);
    assert_eq!(value["grants_authority"], false);
    assert!(outcome(Err(&original), &bound(Workload::CheckedRebind), 1, 1).is_err());
}
#[test]
fn missing_repeated_and_non_genuine_callback_counts_refuse() {
    let original = Failure::new(Phase::RecipeBinding, REFUSAL.into());
    for (calls, entries) in [(0, 0), (2, 1), (1, 2), (1, 0)] {
        assert!(
            outcome(
                Err(&original),
                &bound(Workload::ExactRevisionRefusal),
                calls,
                entries
            )
            .is_err()
        );
    }
}
#[test]
fn closed_modes_do_not_admit_cancel_measured_or_caller_failure_phase() {
    assert!(serde_json::from_str::<RunMode>("\"cancel\"").is_err());
    assert!(serde_json::from_str::<RunMode>("\"measured\"").is_err());
    assert!(serde_json::from_str::<Workload>("\"anything_refused\"").is_err());
    assert!(serde_json::from_str::<Workload>("\"checked_rebind\"").is_ok());
}
#[test]
fn output_line_boundary_and_write_failures_are_preserved() {
    let mut bytes = b"test name ... ".to_vec();
    record_to(&mut bytes, OUTCOME_PREFIX, b"{}\n").unwrap();
    assert!(bytes.ends_with(b"\nFE2O3_RECIPE_OUTCOME_V1 {}\n"));
    let mut small = Bounded::new(2);
    assert!(record_to(&mut small, OUTCOME_PREFIX, b"{}\n").is_err());
}
#[test]
fn original_reservation_envelope_is_nonresetting_for_all35_calls() {
    let mut reservations = Reservations::new(true).unwrap();
    assert_eq!(reservations.envelope.calls, 35);
    assert_eq!(
        reservations
            .envelope
            .maximum_selected_retained_source_read_bytes,
        110_100_585
    );
    for _ in 0..35 {
        reservations.next().unwrap();
    }
    assert!(reservations.next().is_err());
    assert_eq!(reservations.remaining, 0);
    assert_eq!(3 * (NORMAL_LIMIT + 1), 1_572_867);
    assert_eq!(3 * (OUTCOME_LIMIT + 1), 3_170_307);
}

fn inert_binding_fixture(workload: Workload) -> (Input, Vec<u8>, Vec<u8>) {
    let axes = codec::InstanceBinding {
        function: [1; 32],
        item: [2; 32],
        monomorphization: [3; 32],
        generic_types: [4; 32],
        const_arguments: [5; 32],
    };
    let identity = || {
        serde_json::from_value::<codec::ProgramIdentity>(
            serde_json::json!({"digest":vec![6_u8;32],"canonical_length":9}),
        )
        .unwrap()
    };
    let source_binding = match workload {
        Workload::CheckedRebind => codec::SourceBinding::RebindCurrent {},
        Workload::ExactRevisionRefusal => codec::SourceBinding::ExactRevision {
            expected_source_sha256: [17; 32],
            expected_original: identity(),
            expected_prefix: identity(),
        },
    };
    let recipe = codec::Recipe::new(
        axes,
        codec::Origin {
            source: [17; 32],
            semantic: [6; 32],
            bound: [7; 32],
        },
        codec::Order::ReverseReady,
        codec::Constraint {
            relation: codec::Relation::OrBeforeXor,
            strength: codec::Strength::Exact,
        },
        source_binding,
    )
    .encode()
    .unwrap();
    let mode = if workload == Workload::CheckedRebind {
        "rebind_current"
    } else {
        "exact_revision"
    };
    let origin = json_bytes(&serde_json::json!({
        "created_recipe":recipe,"grants_artifact_or_launch_authority":false,
        "evidence":{"instance_axes":[axes.function,axes.item,axes.monomorphization,axes.generic_types,axes.const_arguments],
            "source_sha256":vec![17_u8;32],"recipe_sha256":<[u8;32]>::from(Sha256::digest(&recipe)),
            "source_initializer":[1,2,3,4],"created":true,"source_binding_mode":mode,
            "requested_order":"reverse_ready","requested_relation":"or_before_xor","strength":"exact",
            "actual_relation":"or_before_xor","constraint_outcome":{"status":"honored","relation":"or_before_xor"},
            "grants_authority":false}
    }), NORMAL_LIMIT).unwrap();
    let input = Input {
        schema: INPUT_SCHEMA.into(),
        mode: RunMode::Ordinary,
        workload,
        invocation: Config {
            schema: SCHEMA.into(),
            mode: Mode::Ordinary,
            source: "src/renamed.rs".into(),
            source_sha256: "22".repeat(32),
            intent: Intent::Replay {
                recipe: PinnedFile {
                    path: "/unused".into(),
                    sha256: sha(&recipe),
                },
            },
            rustc_args: vec!["rustc".into()],
            oracle: None,
        },
        origin_oracle: PinnedFile {
            path: "/unused".into(),
            sha256: sha(&origin),
        },
        ordinary_oracle: None,
    };
    (input, recipe, origin)
}
#[test]
fn inert_recipe_origin_binding_joins_both_closed_workloads() {
    for workload in [Workload::CheckedRebind, Workload::ExactRevisionRefusal] {
        let (input, recipe, origin) = inert_binding_fixture(workload);
        let value = binding(&input, &recipe, &origin).unwrap();
        assert_eq!(value.workload, workload);
        assert_eq!(value.origin_source_sha256, "11".repeat(32));
        assert_eq!(value.recipe_sha256, sha(&recipe));
        assert_eq!(value.origin_normal_sha256, sha(&origin));
    }
}
#[test]
fn unchanged_source_wrong_mode_and_substituted_origin_recipe_refuse() {
    let (mut input, recipe, origin) = inert_binding_fixture(Workload::CheckedRebind);
    input.invocation.source_sha256 = "11".repeat(32);
    assert!(binding(&input, &recipe, &origin).is_err());
    input.invocation.source_sha256 = "22".repeat(32);
    input.workload = Workload::ExactRevisionRefusal;
    assert!(binding(&input, &recipe, &origin).is_err());
    input.workload = Workload::CheckedRebind;
    let mut changed: serde_json::Value = serde_json::from_slice(&origin).unwrap();
    changed["created_recipe"][0] = serde_json::json!(0);
    assert!(
        binding(
            &input,
            &recipe,
            &json_bytes(&changed, NORMAL_LIMIT).unwrap()
        )
        .is_err()
    );
}
#[test]
fn compiler_fatal_refusal_never_counts_as_the_designated_negative() {
    let failure = api::finish_callback(
        Some(Err(Failure::new(Phase::RecipeBinding, REFUSAL.into()))),
        1,
        true,
    )
    .unwrap_err();
    assert!(failure.compiler_fatal());
    assert!(check_refusal(&failure).is_err());
}

#[test]
fn process_memory_does_not_expand_the_closed_outcome_input() {
    assert!(serde_json::from_str::<RunMode>("\"process_memory\"").is_err());
    let (mut input, _, _) = inert_binding_fixture(Workload::CheckedRebind);
    input.invocation.rustc_args = vec!["rustc".into(), "-C".into(), "overflow-checks=on".into()];
    // Selection is a distinct ignored test, not a caller-controlled JSON mode
    // or a newly accepted failure kind.
    assert!(serde_json::from_str::<Workload>("\"rss_refusal\"").is_err());
    assert_eq!(input.mode, RunMode::Ordinary);
}
#[test]
fn process_memory_presence_does_not_substitute_for_the_original_refusal_oracle() {
    let original = Failure::new(Phase::RecipeBinding, REFUSAL.into());
    let bound = bound(Workload::ExactRevisionRefusal);
    let before = outcome(Err(&original), &bound, 1, 1).unwrap();
    let profile = process_memory::Profile::new(
        Some(&"11".repeat(32)),
        &bound.current_source_sha256,
        "exact_revision_refusal",
        35,
    )
    .unwrap();
    assert!(profile.enabled());
    let after = outcome(Err(&original), &bound, 1, 1).unwrap();
    assert_eq!(before, after);
    assert!(check_refusal(&Failure::new(Phase::Observation, REFUSAL.into())).is_err());
}
