//! Synthetic component records below only exercise accounting/format guards.
//! They never count as authentic source, timing samples or owner qualification.
use super::*;
use std::{cell::Cell, rc::Rc};

fn identity() -> Identity {
    Identity {
        canonical_identity: "a".repeat(64),
        canonical_bytes: 993,
        semantic_identity: "b".repeat(64),
        source_inventory_identity: "c".repeat(64),
        source_preflight_identity: "d".repeat(64),
        canonical_executable_receipt_bytes: 10,
        call_correspondence_receipt_bytes: 20,
        retained_receipts_sum_bytes: 30,
    }
}
fn samples() -> Vec<Sample> {
    (0..TOTAL)
        .map(|index| {
            let mut sample = Sample::new(index);
            sample.source_collection_ns = Some(index as u128 + 1);
            sample.original_validation_transition_ns = Some(index as u128 + 101);
            sample.observation = Some(identity());
            sample.baseline_equal_while_owner_live = true;
            sample.owner_constructed = true;
            sample.owner_dropped_before_sample_publication = true;
            sample
        })
        .collect()
}
#[test]
fn exact_calibration_and_nearest_rank_summary_do_not_include_calibration() {
    let mut value = samples();
    for sample in &mut value[..CALIBRATION] {
        sample.source_collection_ns = Some(1_000_000);
        sample.original_validation_transition_ns = Some(2_000_000);
    }
    assert_eq!(
        summary(&value, true),
        Some(Summary {
            count: 30,
            p50_ns: 20,
            p95_ns: 34,
            maximum_ns: 35,
        })
    );
    assert_eq!(
        summary(&value, false),
        Some(Summary {
            count: 30,
            p50_ns: 120,
            p95_ns: 134,
            maximum_ns: 135,
        })
    );
}
#[test]
fn incomplete_duplicate_reordered_and_wrong_phase_never_summarize() {
    let mut value = samples();
    value.pop();
    assert!(summary(&value, true).is_none());
    let mut value = samples();
    value.swap(4, 5);
    assert!(summary(&value, true).is_none());
    let mut value = samples();
    value[34].index = 33;
    assert!(summary(&value, true).is_none());
    let mut value = samples();
    value[5].phase = "calibration";
    assert!(summary(&value, true).is_none());
}
#[test]
fn every_failed_sample_or_missing_clock_prevents_qualification() {
    for index in 0..TOTAL {
        let mut value = samples();
        value[index].failure = Some(SeriesFailure::TimeBudget);
        assert!(summary(&value, true).is_none());
        let mut value = samples();
        value[index].owner_dropped_before_sample_publication = false;
        assert!(summary(&value, true).is_none());
        let mut value = samples();
        value[index].observation_failure = Some(Failure::BaselineMismatch);
        assert!(summary(&value, true).is_none());
    }
    for index in 0..TOTAL {
        let mut value = samples();
        value[index].source_collection_ns = None;
        assert!(summary(&value, true).is_none());
        let mut value = samples();
        value[index].original_validation_transition_ns = None;
        assert!(summary(&value, false).is_none());
    }
}
struct DropProbe(Rc<Cell<usize>>);
impl Drop for DropProbe {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}
#[test]
fn scoped_owner_is_dropped_before_success_or_refusal_is_returned() {
    let dropped = Rc::new(Cell::new(0));
    for expected in [Ok(7), Err("refusal")] {
        let previous = dropped.get();
        let result = observe_then_drop(DropProbe(dropped.clone()), |_| {
            assert_eq!(dropped.get(), previous);
            expected
        });
        assert_eq!(result, expected);
        assert_eq!(dropped.get(), previous + 1);
    }
}
#[test]
fn scoped_owner_unwinds_before_a_later_attempt_is_possible() {
    let dropped = Rc::new(Cell::new(0));
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        observe_then_drop(DropProbe(dropped.clone()), |_| panic!("component unwind"));
    }));
    assert!(result.is_err());
    assert_eq!(dropped.get(), 1);
}
#[test]
fn compact_component_sample_retains_worst_integer_clocks_within_fixed_cap() {
    let mut value = samples().remove(0);
    value.source_collection_ns = Some(u64::MAX as u128);
    value.original_validation_transition_ns = Some(u64::MAX as u128);
    let identity = value.observation.as_mut().unwrap();
    identity.canonical_bytes = usize::MAX;
    identity.canonical_executable_receipt_bytes = usize::MAX;
    identity.call_correspondence_receipt_bytes = usize::MAX;
    identity.retained_receipts_sum_bytes = usize::MAX;
    let text = serde_json::to_string(&serde_json::json!({
        "schema":"fe2o3-ordered-validation-sample-v1", "sample":value,
    }))
    .unwrap();
    assert!(text.len() <= SAMPLE_CAP);
    let complete = (0..TOTAL).map(|_| &value).collect::<Vec<_>>();
    let samples_bytes = serde_json::to_vec(&complete).unwrap().len();
    // Reserve 4 KiB for the fixed aggregate header/quantiles and extra digits
    // in real ordinals. This does not measure the entire compiler owner.
    assert!(samples_bytes + 4096 + TOTAL <= REPORT_CAP);
    assert!(TOTAL * SAMPLE_CAP + REPORT_CAP + 4096 <= 256 * 1024);
}
#[test]
fn old_measurement_remains_a_separate_report_and_new_series_has_no_generation_hook() {
    let source = include_str!("ordered_program_validation_series_v1_tests.rs");
    assert_eq!(
        source
            .matches("transaction.observe_ordered_program_v32()")
            .count(),
        1
    );
    assert_eq!(
        source.matches("transaction_in_active_session_v1(").count(),
        1
    );
    assert!(source.contains("observe_then_drop(owner,"));
    assert!(source.contains("observe_live_owner(owner, profile, baseline)"));
    let compact: String = source.split_whitespace().collect();
    assert!(compact.contains("generation_ns\":null"));
    assert!(compact.contains("warm_stage\":false"));
    for forbidden in [
        "override_queries",
        "eval_to_allocation_raw",
        "__checked_ordered_program_words_v1(",
        "CanonicalKernelIrWorkBudgetV1::new",
        "release_storage(",
    ] {
        assert!(!source.contains(forbidden), "{forbidden}");
    }
    let original = include_str!("ordered_program_stage_measurement_v1_tests.rs");
    assert!(original.contains("fe2o3-ordered-stage-measurement-v1"));
    assert!(original.contains("generation_ns: None"));
}
