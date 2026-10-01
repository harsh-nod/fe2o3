//! Inert controls: synthetic fixed-ledger data, delegation/order helpers and
//! callback closure. These never run rustc or manufacture generation samples.
use super::*;
use ledger::{DEPTH_CAP, Key, Ledger, ROW_CAP, Snapshot};
use rustc_hir::def_id::{CRATE_DEF_INDEX, LOCAL_CRATE};
use std::cell::Cell;
use std::time::Duration;

fn key() -> Key {
    Key {
        definition: DefId {
            krate: LOCAL_CRATE,
            index: CRATE_DEF_INDEX,
        },
        item: true,
        empty_args: true,
        no_promoted: true,
        fully_monomorphized: true,
    }
}
fn armed() -> (Ledger, Instant) {
    let start = Instant::now();
    let mut ledger = Ledger::new();
    assert!(ledger.arm(7, start));
    (ledger, start)
}
fn completed() -> Snapshot {
    let (mut ledger, start) = armed();
    let row = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
    ledger.complete(
        row,
        start + Duration::from_nanos(3),
        start + Duration::from_nanos(11),
        Some(true),
    );
    ledger.fence();
    ledger.snapshot()
}
fn refusal(result: Observed) -> FailureRecord {
    match result {
        Err(error) => error,
        Ok(_) => panic!("control expected an exact refusal"),
    }
}

#[test]
fn delegate_calls_once_and_returns_the_same_owned_error() {
    let calls = Cell::new(0);
    let original = Box::new([1_u8, 2, 3, 4]);
    let address = original.as_ref().as_ptr();
    let (returned, start, end): (Result<(), Box<[u8; 4]>>, _, _) = call_once(|| {
        calls.set(calls.get() + 1);
        Err(original)
    });
    let error = returned.unwrap_err();
    assert_eq!(calls.get(), 1);
    assert_eq!(error.as_ref().as_ptr(), address);
    assert_eq!(*error, [1, 2, 3, 4]);
    assert!(end >= start);
}
#[test]
fn delegation_never_retries_a_panic() {
    let calls = Cell::new(0);
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        call_once(|| {
            calls.set(calls.get() + 1);
            panic!("original panic payload");
        })
    }));
    assert_eq!(calls.get(), 1);
    assert_eq!(
        panic.unwrap_err().downcast_ref::<&str>(),
        Some(&"original panic payload")
    );
}
#[test]
fn previous_override_runs_once_before_selected_pointer_capture() {
    struct State {
        selected: usize,
        unrelated: usize,
    }
    let calls = Cell::new(0);
    let mut state = State {
        selected: 2,
        unrelated: 99,
    };
    let selected = chain_then(
        &mut state,
        |state| {
            calls.set(calls.get() + 1);
            state.selected = 7;
        },
        |state| state.selected,
    );
    assert_eq!(calls.get(), 1);
    assert_eq!(selected, 7);
    assert_eq!(state.unrelated, 99);
}
#[test]
fn completed_clock_is_lossless_and_zero_is_not_absence() {
    let snapshot = completed();
    let (ordinal, row) = snapshot.unique_completed(key().definition).unwrap();
    assert_eq!(ordinal, 0);
    assert_eq!(row.start_ns, Some(3));
    assert_eq!(row.end_ns, Some(11));
    assert_eq!(row.elapsed_ns, Some(8));
    let wire = serde_json::to_value(row.wire(ordinal)).unwrap();
    assert_eq!(wire["elapsed_ns"], "8");
    let mut zero = snapshot;
    zero.rows[0].as_mut().unwrap().end_ns = Some(3);
    zero.rows[0].as_mut().unwrap().elapsed_ns = Some(0);
    assert_eq!(
        zero.unique_completed(key().definition)
            .unwrap()
            .1
            .elapsed_ns,
        Some(0)
    );
}
#[test]
fn nested_rows_keep_inclusive_intervals_and_parent_ordinals() {
    let (mut ledger, start) = armed();
    let outer = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
    let inner = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
    ledger.complete(
        inner,
        start + Duration::from_nanos(4),
        start + Duration::from_nanos(9),
        Some(true),
    );
    ledger.complete(
        outer,
        start + Duration::from_nanos(2),
        start + Duration::from_nanos(12),
        Some(true),
    );
    ledger.fence();
    let snapshot = ledger.snapshot();
    assert!(snapshot.ready());
    assert_eq!(snapshot.rows[0].unwrap().parent, None);
    assert_eq!(snapshot.rows[1].unwrap().parent, Some(0));
    assert_eq!(snapshot.rows[0].unwrap().elapsed_ns, Some(10));
    assert_eq!(snapshot.rows[1].unwrap().elapsed_ns, Some(5));
    // No total of 15 is emitted or interpreted as the outer interval.
}
#[test]
fn exact_row_cap_preserves_prefix_and_refuses_extra_admission() {
    let (mut ledger, start) = armed();
    for ordinal in 0..ROW_CAP {
        let row = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
        assert_eq!(row, ordinal);
        ledger.complete(row, start, start, Some(true));
    }
    assert_eq!(ledger.begin(key(), 7, std::thread::current().id()), None);
    ledger.fence();
    let snapshot = ledger.snapshot();
    assert_eq!(snapshot.count, ROW_CAP);
    assert!(snapshot.failed);
    assert!(!snapshot.ready());
    assert!(
        snapshot
            .rows
            .iter()
            .all(|row| row.unwrap().returned_ok == Some(true))
    );
}
#[test]
fn exact_depth_cap_has_no_overflow_or_silent_reset() {
    let (mut ledger, start) = armed();
    for ordinal in 0..DEPTH_CAP {
        assert_eq!(
            ledger.begin(key(), 7, std::thread::current().id()),
            Some(ordinal)
        );
    }
    assert_eq!(ledger.begin(key(), 7, std::thread::current().id()), None);
    for ordinal in (0..DEPTH_CAP).rev() {
        ledger.complete(ordinal, start, start, Some(true));
    }
    ledger.fence();
    let snapshot = ledger.snapshot();
    assert_eq!(snapshot.count, DEPTH_CAP);
    assert_eq!(snapshot.active_depth, 0);
    assert!(snapshot.failed);
}
#[test]
fn wrong_session_refuses_without_reserving_a_row() {
    let (mut ledger, _) = armed();
    assert_eq!(ledger.begin(key(), 8, std::thread::current().id()), None);
    assert_eq!(ledger.snapshot().count, 0);
    assert!(ledger.snapshot().failed);
}
#[test]
fn different_provider_thread_is_sticky_refusal() {
    let (mut ledger, start) = armed();
    let row = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
    ledger.complete(row, start, start, Some(true));
    let other = std::thread::spawn(|| std::thread::current().id())
        .join()
        .unwrap();
    assert_eq!(ledger.begin(key(), 7, other), None);
    ledger.fence();
    assert_eq!(ledger.snapshot().count, 1);
    assert!(ledger.snapshot().failed);
}
#[test]
fn repeated_installation_refuses_without_erasing_rows() {
    let (mut ledger, start) = armed();
    let row = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
    ledger.complete(row, start, start, Some(true));
    assert!(!ledger.arm(9, start));
    assert_eq!(ledger.snapshot().count, 1);
    assert!(ledger.snapshot().failed);
}
#[test]
fn active_provider_at_fence_is_incomplete() {
    let (mut ledger, _) = armed();
    ledger.begin(key(), 7, std::thread::current().id()).unwrap();
    ledger.fence();
    assert_eq!(ledger.snapshot().active_depth, 1);
    assert!(!ledger.snapshot().ready());
    assert!(
        ledger
            .snapshot()
            .unique_completed(key().definition)
            .is_err()
    );
}
#[test]
fn unwound_provider_keeps_elapsed_but_never_a_completed_sample() {
    let (mut ledger, start) = armed();
    let row = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
    ledger.complete(row, start, start + Duration::from_nanos(5), None);
    ledger.fence();
    let snapshot = ledger.snapshot();
    let row = snapshot.rows[0].unwrap();
    assert_eq!(row.elapsed_ns, Some(5));
    assert_eq!(row.returned_ok, None);
    assert!(row.unwound);
    assert!(!snapshot.ready());
}
#[test]
fn duplicate_and_out_of_order_completion_are_refused() {
    let (mut ledger, start) = armed();
    let first = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
    let second = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
    ledger.complete(first, start, start, Some(true));
    assert!(ledger.snapshot().failed);
    ledger.complete(second, start, start, Some(true));
    ledger.complete(first, start, start, Some(true));
    ledger.complete(first, start, start, Some(true));
    ledger.fence();
    assert!(ledger.snapshot().failed);
}
#[test]
fn reversed_monotonic_interval_is_refused() {
    let (mut ledger, start) = armed();
    let row = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
    ledger.complete(row, start + Duration::from_nanos(1), start, Some(true));
    ledger.fence();
    assert!(ledger.snapshot().failed);
    assert_eq!(ledger.snapshot().rows[0].unwrap().elapsed_ns, None);
}
#[test]
fn unsupported_key_components_are_independently_refused() {
    for field in 0..4 {
        let mut snapshot = completed();
        let observation_key = &mut snapshot.rows[0].as_mut().unwrap().key;
        match field {
            0 => observation_key.item = false,
            1 => observation_key.empty_args = false,
            2 => observation_key.no_promoted = false,
            _ => observation_key.fully_monomorphized = false,
        }
        assert_eq!(
            snapshot.unique_completed(key().definition).err(),
            Some("selected provider key or completion differs")
        );
    }
}
#[test]
fn failed_original_result_cannot_be_a_positive_sample() {
    let mut snapshot = completed();
    snapshot.rows[0].as_mut().unwrap().returned_ok = Some(false);
    assert_eq!(
        snapshot.unique_completed(key().definition).err(),
        Some("selected provider key or completion differs")
    );
}
#[test]
fn no_provider_row_is_missing_evidence_not_zero_nanoseconds() {
    let (mut ledger, _) = armed();
    ledger.fence();
    assert_eq!(
        ledger.snapshot().unique_completed(key().definition).err(),
        Some("selected const has no actual provider invocation")
    );
}
#[test]
fn repeated_provider_key_is_not_a_warm_series() {
    let (mut ledger, start) = armed();
    for _ in 0..2 {
        let row = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
        ledger.complete(row, start, start, Some(true));
    }
    ledger.fence();
    assert_eq!(
        ledger.snapshot().unique_completed(key().definition).err(),
        Some("selected const has multiple provider invocations")
    );
}
#[test]
fn fenced_sidecar_work_does_not_mutate_timed_rows() {
    let (mut ledger, start) = armed();
    let row = ledger.begin(key(), 7, std::thread::current().id()).unwrap();
    ledger.complete(row, start, start, Some(true));
    ledger.fence();
    assert_eq!(ledger.begin(key(), 9, std::thread::current().id()), None);
    let snapshot = ledger.snapshot();
    assert!(snapshot.ready());
    assert_eq!(snapshot.count, 1);
}
#[test]
fn callback_fatal_and_reentry_dominate_a_saved_result() {
    let snapshot = completed();
    let saved = || Some(Err(FailureRecord::refused("saved observation failure")));
    assert_eq!(
        refusal(final_observation(
            saved(),
            1,
            true,
            1,
            true,
            false,
            snapshot
        ))
        .stage,
        Some(Failure::CompilerFatal)
    );
    assert_eq!(
        refusal(final_observation(
            saved(),
            1,
            true,
            2,
            false,
            false,
            snapshot
        ))
        .stage,
        Some(Failure::RepeatedCallback)
    );
    assert_eq!(
        refusal(final_observation(
            saved(),
            1,
            true,
            0,
            false,
            false,
            snapshot
        ))
        .stage,
        Some(Failure::MissingCallback)
    );
    assert_eq!(
        refusal(final_observation(
            saved(),
            1,
            true,
            1,
            false,
            true,
            snapshot
        ))
        .refusal,
        Some("compiler unwound; no qualified sample")
    );
    assert_eq!(
        refusal(final_observation(
            saved(),
            2,
            true,
            1,
            false,
            false,
            snapshot
        ))
        .refusal,
        Some("one configured compiler session required")
    );
    assert_eq!(
        refusal(final_observation(
            saved(),
            1,
            true,
            1,
            false,
            false,
            snapshot
        ))
        .refusal,
        Some("saved observation failure")
    );
}
#[test]
fn exact_u128_clock_strings_are_not_float_rounded() {
    let mut snapshot = completed();
    let row = snapshot.rows[0].as_mut().unwrap();
    row.start_ns = Some(0);
    row.end_ns = Some(u128::MAX);
    row.elapsed_ns = Some(u128::MAX);
    let wire = serde_json::to_value(row.wire(0)).unwrap();
    assert_eq!(wire["elapsed_ns"], u128::MAX.to_string());
    assert_eq!(wire["end_ns"], u128::MAX.to_string());
}

#[test]
fn full_fixed_row_roster_has_predeclared_output_headroom() {
    let mut row = completed().rows[0].unwrap();
    row.start_ns = Some(u128::MAX);
    row.end_ns = Some(u128::MAX);
    row.elapsed_ns = Some(u128::MAX);
    row.parent = Some(ROW_CAP - 1);
    row.depth = DEPTH_CAP;
    let rows: Vec<_> = (0..ROW_CAP).map(|ordinal| row.wire(ordinal)).collect();
    let bytes = serde_json::to_vec(&rows).unwrap();
    // The remaining fixed-shape record, bounded hashes/identities and failure
    // labels receive a separate conservative 8 KiB envelope.
    assert!(
        bytes
            .len()
            .checked_add(8192 + PREFIX.len() + 2)
            .is_some_and(|bytes| bytes <= OUTPUT_CAP)
    );
}
