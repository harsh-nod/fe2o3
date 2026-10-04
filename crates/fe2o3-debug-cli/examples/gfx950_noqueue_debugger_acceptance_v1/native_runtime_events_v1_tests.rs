//! Fixed-byte CPU fixtures only; none manufacture native process custody.
use super::*;

fn row(seq: u64, kind: u64, fields: [u64; 8]) -> Vec<u8> {
    let [event, ek, runtime, callback, bp, thread, action, reason] = fields;
    format!("=amd-runtime-observation-v1,version=\"1\",sequence=\"{seq}\",generation=\"1\",inferior=\"1\",pid=\"77\",process=\"9\",event=\"{event}\",event-kind=\"{ek}\",runtime-state=\"{runtime}\",callback=\"{callback}\",breakpoint=\"{bp}\",thread=\"{thread}\",action=\"{action}\",status=\"0\",type=\"{kind}\",reason=\"{reason}\"\n").into_bytes()
}
fn attach() -> Vec<u8> {
    row(1, 1, [0, 0, 0, 0, 0, 0, 0, 0])
}
fn runtime() -> Vec<u8> {
    row(2, 2, [100, 5, 1, 1, 0, 0, 0, 0])
}
fn objects() -> Vec<u8> {
    row(3, 3, [101, 3, 0, 1, 0, 0, 0, 0])
}
fn callback() -> Vec<u8> {
    row(4, 4, [102, 4, 0, 1, 20, 7, 2, 0])
}
fn terminal() -> Vec<u8> {
    row(5, 5, [0, 0, 0, 0, 0, 0, 0, 14])
}
fn object() -> MemoryObject {
    MemoryObject {
        pid: 77,
        address: 0x1000,
        bytes: 4,
    }
}
fn change(raw: &[u8], from: &str, to: &str) -> Vec<u8> {
    String::from_utf8(raw.to_vec())
        .unwrap()
        .replacen(from, to, 1)
        .into_bytes()
}
fn cold() -> NativeEventsV1 {
    let mut events = NativeEventsV1::new();
    events.native_line(&attach(), Phase::ToEntry).unwrap();
    events.owned_entry(77, b"7").unwrap();
    events.cold_absence().unwrap();
    events
}
fn published() -> NativeEventsV1 {
    let mut events = cold();
    for line in [runtime(), objects(), callback()] {
        events.native_line(&line, Phase::ToPublished).unwrap();
    }
    events
}
#[test]
fn exact_join_is_only_historical_after_all_explicit_stages() {
    let mut events = published();
    assert!(events.history().is_err());
    events.original_elf_joined(object()).unwrap();
    assert!(events.history().is_err());
    events.close_stop().unwrap();
    events.native_line(&terminal(), Phase::Exiting).unwrap();
    assert!(events.history().is_err());
    events.completed_teardown().unwrap();
    let h = events.history().unwrap();
    assert_eq!(h.native_process_id, "9");
    assert_eq!(h.publication_callback_id, "1");
    assert_eq!(h.records, 5);
    assert_eq!(h.code_object_events, 1);
    assert!(h.runtime_event_and_ack_joined);
    assert!(!h.live_stop_authority);
    assert!(!h.debugger_acceptance_authority);
    assert!(!h.source_or_launch_authority);
    assert!(!h.queue_or_dispatch_authority);
    assert!(!h.physical_register_capture);
}
#[test]
fn strict_wire_grammar_rejects_noncanonical_and_unknown_fields() {
    for (from, to) in [
        ("version=\"1\"", "version=\"2\""),
        ("generation=\"1\"", "generation=\"2\""),
        ("sequence=\"1\"", "sequence=\"01\""),
        ("pid=\"77\"", "pid=\"18446744073709551616\""),
        ("status=\"0\"", "status=\"-0\""),
        ("status=\"0\"", "status=\"9223372036854775808\""),
        ("status=\"0\"", "status=\"-9223372036854775809\""),
        ("pid=\"77\"", "pid=\"\\0677\""),
        ("pid=\"77\"", "pid=[\"77\"]"),
        ("pid=\"77\"", "pid=\"77\",pid=\"77\""),
        ("process=\"9\"", "foreign=\"9\""),
        ("type=\"1\"", "type=\"6\""),
        ("reason=\"0\"", "reason=\"20\""),
    ] {
        assert!(
            parse(&change(&attach(), from, to)).is_err(),
            "{from} -> {to}"
        );
    }
    let mut no_lf = attach();
    no_lf.pop();
    assert!(parse(&no_lf).is_err());
    let mut extra = attach();
    extra.extend_from_slice(b"\n");
    assert!(parse(&extra).is_err());
    assert!(parse(&vec![b'x'; 1025]).is_err());
    assert_eq!(
        parse(&change(
            &attach(),
            "status=\"0\"",
            "status=\"-9223372036854775808\""
        ))
        .unwrap()
        .status,
        i64::MIN
    );
}
#[test]
fn attach_facts_do_not_admit_foreign_entry_or_missing_owner() {
    for pid in [0, 78] {
        let mut events = NativeEventsV1::new();
        events.native_line(&attach(), Phase::ToEntry).unwrap();
        assert!(events.owned_entry(pid, b"7").is_err());
        assert!(events.cold_absence().is_err());
    }
    assert!(NativeEventsV1::new().owned_entry(77, b"7").is_err());
    assert!(
        NativeEventsV1::new()
            .native_line(&attach(), Phase::Setup)
            .is_err()
    );
}
#[test]
fn before_cold_runtime_and_nonzero_native_status_refuse_stickily() {
    let mut events = NativeEventsV1::new();
    events.native_line(&attach(), Phase::ToEntry).unwrap();
    assert!(events.native_line(&runtime(), Phase::ToEntry).is_err());
    assert!(events.owned_entry(77, b"7").is_err());
    let mut events = cold();
    assert!(
        events
            .native_line(
                &change(&runtime(), "status=\"0\"", "status=\"-1\""),
                Phase::ToPublished
            )
            .is_err()
    );
    assert!(events.native_line(&runtime(), Phase::ToPublished).is_err());
}
#[test]
fn sequence_owner_generation_and_runtime_state_are_exact() {
    for (from, to) in [
        ("sequence=\"2\"", "sequence=\"1\""),
        ("sequence=\"2\"", "sequence=\"3\""),
        ("inferior=\"1\"", "inferior=\"2\""),
        ("pid=\"77\"", "pid=\"78\""),
        ("process=\"9\"", "process=\"10\""),
        ("generation=\"1\"", "generation=\"2\""),
        ("event-kind=\"5\"", "event-kind=\"3\""),
        ("runtime-state=\"1\"", "runtime-state=\"2\""),
        ("runtime-state=\"1\"", "runtime-state=\"3\""),
        ("reason=\"0\"", "reason=\"6\""),
        ("event=\"100\"", "event=\"0\""),
    ] {
        assert!(
            cold()
                .native_line(&change(&runtime(), from, to), Phase::ToPublished)
                .is_err(),
            "{from} -> {to}"
        );
    }
}
#[test]
fn event_reuse_and_callback_coordinate_substitution_refuse() {
    for (from, to) in [
        ("event=\"101\"", "event=\"100\""),
        ("callback=\"1\"", "callback=\"0\""),
        ("callback=\"1\"", "callback=\"2\""),
        ("event-kind=\"3\"", "event-kind=\"5\""),
    ] {
        let mut events = cold();
        events.native_line(&runtime(), Phase::ToPublished).unwrap();
        assert!(
            events
                .native_line(&change(&objects(), from, to), Phase::ToPublished)
                .is_err()
        );
    }
}
#[test]
fn publication_requires_completed_same_thread_callback_and_ack() {
    for (from, to) in [
        ("thread=\"7\"", "thread=\"8\""),
        ("breakpoint=\"20\"", "breakpoint=\"0\""),
        ("action=\"2\"", "action=\"1\""),
        ("event-kind=\"4\"", "event-kind=\"3\""),
        ("callback=\"1\"", "callback=\"2\""),
        ("status=\"0\"", "status=\"1\""),
        ("event=\"102\"", "event=\"0\""),
    ] {
        let mut events = cold();
        events.native_line(&runtime(), Phase::ToPublished).unwrap();
        events.native_line(&objects(), Phase::ToPublished).unwrap();
        assert!(
            events
                .native_line(&change(&callback(), from, to), Phase::ToPublished)
                .is_err()
        );
        assert!(events.original_elf_joined(object()).is_err());
    }
    let mut events = cold();
    events.native_line(&runtime(), Phase::ToPublished).unwrap();
    events.native_line(&objects(), Phase::ToPublished).unwrap();
    assert!(events.original_elf_joined(object()).is_err());
}
#[test]
fn runtime_callback_cannot_be_closed_as_immediate_resume() {
    let mut events = cold();
    events.native_line(&runtime(), Phase::ToPublished).unwrap();
    assert!(
        events
            .native_line(&row(3, 4, [0, 0, 0, 1, 20, 7, 1, 0]), Phase::ToPublished)
            .is_err()
    );
}
#[test]
fn runtime_outside_callback_plus_genuine_publication_callback_is_supported() {
    let mut events = cold();
    events
        .native_line(
            &change(&runtime(), "callback=\"1\"", "callback=\"0\""),
            Phase::ToPublished,
        )
        .unwrap();
    events.native_line(&objects(), Phase::ToPublished).unwrap();
    events.native_line(&callback(), Phase::ToPublished).unwrap();
    events.original_elf_joined(object()).unwrap();
}
#[test]
fn every_native_invalidation_before_join_is_fatal() {
    for reason in 1..=19 {
        let mut events = cold();
        assert!(
            events
                .native_line(
                    &row(2, 5, [0, 0, 0, 0, 0, 0, 0, reason]),
                    Phase::ToPublished
                )
                .is_err()
        );
        assert!(events.history().is_err());
    }
}
#[test]
fn foreign_object_and_missing_runtime_or_object_event_cannot_join() {
    let mut events = published();
    assert!(
        events
            .original_elf_joined(MemoryObject {
                pid: 78,
                ..object()
            })
            .is_err()
    );
    assert!(cold().original_elf_joined(object()).is_err());
    let mut events = cold();
    events
        .native_line(
            &change(&runtime(), "callback=\"1\"", "callback=\"0\""),
            Phase::ToPublished,
        )
        .unwrap();
    assert!(events.original_elf_joined(object()).is_err());
}
#[test]
fn late_normal_events_or_preclose_exit_invalidate_the_relation() {
    let mut events = published();
    events.original_elf_joined(object()).unwrap();
    assert!(events.native_line(&terminal(), Phase::Published).is_err());
    let mut events = published();
    events.original_elf_joined(object()).unwrap();
    events.close_stop().unwrap();
    assert!(
        events
            .native_line(&row(5, 2, [105, 5, 1, 0, 0, 0, 0, 0]), Phase::Exiting)
            .is_err()
    );
}
#[test]
fn exit_is_required_but_never_substitutes_for_observed_teardown() {
    let mut events = published();
    events.original_elf_joined(object()).unwrap();
    events.close_stop().unwrap();
    assert!(events.completed_teardown().is_err());
    for reason in [1, 6, 8, 13, 15, 16, 17, 18, 19] {
        let mut events = published();
        events.original_elf_joined(object()).unwrap();
        events.close_stop().unwrap();
        assert!(
            events
                .native_line(&row(5, 5, [0, 0, 0, 0, 0, 0, 0, reason]), Phase::Exiting)
                .is_err()
        );
    }
}
#[test]
fn one_terminal_record_is_final_and_no_replay_restart_exists() {
    let mut events = published();
    events.original_elf_joined(object()).unwrap();
    events.close_stop().unwrap();
    events.native_line(&terminal(), Phase::Exiting).unwrap();
    assert!(
        events
            .native_line(&row(6, 5, [0, 0, 0, 0, 0, 0, 0, 14]), Phase::Exited)
            .is_err()
    );
    assert!(events.completed_teardown().is_err());
}

#[test]
fn final_native_slot_is_not_an_extra_ordinary_event_budget() {
    let mut events = cold();
    for callback in 1..=28 {
        events
            .native_line(
                &row(callback + 1, 4, [0, 0, 0, callback, 20, 7, 1, 0]),
                Phase::ToPublished,
            )
            .unwrap();
    }
    events
        .native_line(&row(30, 2, [100, 5, 1, 0, 0, 0, 0, 0]), Phase::ToPublished)
        .unwrap();
    events
        .native_line(&row(31, 3, [101, 3, 0, 29, 0, 0, 0, 0]), Phase::ToPublished)
        .unwrap();
    assert_eq!(
        events.native_line(
            &row(32, 4, [102, 4, 0, 29, 20, 7, 2, 0]),
            Phase::ToPublished
        ),
        Err(Refusal::Bound),
    );
    assert!(parse(&row(33, 5, [0, 0, 0, 0, 0, 0, 0, 2])).is_err());
}
#[test]
fn final_history_serializes_handle_coordinates_losslessly_as_text() {
    let mut events = published();
    events.original_elf_joined(object()).unwrap();
    events.close_stop().unwrap();
    events.native_line(&terminal(), Phase::Exiting).unwrap();
    events.completed_teardown().unwrap();
    let value = serde_json::to_value(events.history().unwrap()).unwrap();
    for key in [
        "process_id",
        "native_process_id",
        "producer_generation",
        "runtime_event_id",
        "publication_callback_id",
        "publication_breakpoint_id",
    ] {
        assert!(value[key].is_string(), "{key}");
    }
}
