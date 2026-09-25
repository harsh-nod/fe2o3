//! Mock-only V2 controls. No source/native/process authority from these bytes.
use super::*;
fn positive() -> ProtocolObservation {
    observe(&mut Fake::new(script())).unwrap()
}
fn mutate(from: &str, to: &str) {
    positive();
    let mut s = script();
    replace(&mut s, from, to);
    refuses(s);
}
fn unavailable(reason: &str) -> Vec<Vec<u8>> {
    let mut s = script();
    replace(
        &mut s,
        "status=\"available\"",
        &format!("status=\"{reason}\""),
    );
    replace(&mut s, "reg=\"df9b5713\"", "reg=\"\"");
    replace(
        &mut s,
        &format!("mem=\"{}\"", snapshot_payload()),
        "mem=\"\"",
    );
    s
}
#[test]
fn fixed_available_actual_bytes_and_independent_later_target_fact() {
    let value = serde_json::to_value(positive()).unwrap();
    assert_eq!(
        value["schema"],
        "fe2o3-gfx950-one-stop-controller-observation-v2"
    );
    assert_eq!(value["physical_snapshot"]["status"], "available");
    assert_eq!(value["physical_snapshot"]["register_hex"], "df9b5713");
    assert_eq!(value["physical_snapshot"]["memory_hex"], snapshot_payload());
    assert_eq!(value["physical_snapshot"]["context"]["start"], "100");
    assert_eq!(value["physical_snapshot"]["fixture_oracle_matched"], true);
    assert_eq!(value["later_target_4096_output_validation_observed"], true);
    assert_eq!(value["post_completion_raw_bytes_recaptured"], false);
    assert_eq!(value["source_or_launch_authority"], false);
}
#[test]
fn each_unavailable_is_typed_and_never_serializes_placeholder_bytes() {
    positive();
    for reason in ["register-unavailable", "memory-unavailable", "short-memory"] {
        let mut f = Fake::new(unavailable(reason));
        let r = observe(&mut f).unwrap();
        assert!(!r.physical_register_capture && !r.physical_memory_capture);
        assert_eq!(f.sent, expected_commands());
        assert_eq!((f.finish, f.cleanup), (1, 0));
        let v = serde_json::to_value(r).unwrap();
        assert_eq!(v["physical_snapshot"]["status"], "unavailable");
        assert_eq!(v["physical_snapshot"]["reason"], reason);
        assert!(v["physical_snapshot"].get("register_hex").is_none());
        assert!(v["physical_snapshot"].get("memory_hex").is_none());
        assert!(
            v["physical_snapshot"]
                .get("fixture_oracle_matched")
                .is_none()
        );
    }
}
#[test]
fn unavailable_cannot_smuggle_any_actual_or_fake_bytes() {
    positive();
    for reason in ["register-unavailable", "memory-unavailable", "short-memory"] {
        for (from, to) in [("reg=\"\"", "reg=\"00000000\""), ("mem=\"\"", "mem=\"00\"")] {
            let mut s = unavailable(reason);
            replace(&mut s, from, to);
            refuses(s);
        }
    }
}
#[test]
fn exact_sizes_hex_status_and_schema_are_closed() {
    for (a, b) in [
        ("dwarf=\"44\"", "dwarf=\"12\""),
        ("rbytes=\"4\"", "rbytes=\"8\""),
        ("mbytes=\"272\"", "mbytes=\"256\""),
        ("reg=\"df9b5713\"", "reg=\"DF9B5713\""),
        ("reg=\"df9b5713\"", "reg=\"df9b571\""),
        ("reg=\"df9b5713\"", "reg=\"df9b571300\""),
        ("status=\"available\"", "status=\"not-started\""),
        ("status=\"available\"", "x=\"1\",status=\"available\""),
        ("dwarf=\"44\"", "dwarf=\"044\""),
        ("regid=\"10\"", "regid=\"0\""),
        (
            "status=\"available\"",
            "status=\"available\",status=\"available\"",
        ),
    ] {
        mutate(a, b);
    }
}
#[test]
fn every_captured_byte_is_compared_not_reconstructed_from_oracle() {
    positive();
    let original = snapshot_payload();
    for byte in 0..272 {
        let mut changed = original.clone().into_bytes();
        changed[byte * 2] = if changed[byte * 2] == b'0' {
            b'1'
        } else {
            b'0'
        };
        mutate(
            &format!("mem=\"{original}\""),
            &format!("mem=\"{}\"", String::from_utf8(changed).unwrap()),
        );
    }
    for byte in 0..4 {
        let mut changed = b"df9b5713".to_vec();
        changed[byte * 2] = b'0';
        mutate(
            "reg=\"df9b5713\"",
            &format!("reg=\"{}\"", String::from_utf8(changed).unwrap()),
        );
    }
}
#[test]
fn same_owned_start_process_gpu_pc_and_layout_are_required() {
    for (a, b) in [
        ("start=\"100\"", "start=\"101\""),
        ("a=\"7\"", "a=\"8\""),
        ("g=\"2\"", "g=\"1\""),
        ("g=\"2\"", "g=\"3\""),
        ("pc=\"2097232\"", "pc=\"2097233\""),
        ("entry=\"2097152\"", "entry=\"2097153\""),
        ("desc=\"2092992\"", "desc=\"2092993\""),
        ("out=\"458752\"", "out=\"458760\""),
        ("out=\"458752\"", "out=\"18446744073709551616\""),
        ("c=\"262144\"", "c=\"18446744073709551615\""),
        ("w=\"3\"", "w=\"0\""),
        ("cp=\"327680\"", "cp=\"0\""),
        ("co=\"9\"", "co=\"0\""),
    ] {
        mutate(a, b);
    }
}
#[test]
fn all_v1_and_mixed_transition_domains_refuse_no_downgrade() {
    positive();
    let mut old = script();
    for x in &mut old {
        *x = String::from_utf8(x.clone())
            .unwrap()
            .replace("fe2o3-owned-one-stop-v2", "fe2o3-owned-one-stop-v1")
            .into_bytes();
    }
    refuses(old);
    let baseline = script();
    for n in 0..baseline.len() {
        if baseline[n].starts_with(b"=fe2o3-owned-one-stop-v2") {
            let mut s = baseline.clone();
            s[n] = String::from_utf8(s[n].clone())
                .unwrap()
                .replace("fe2o3-owned-one-stop-v2", "fe2o3-owned-one-stop-v1")
                .into_bytes();
            refuses(s);
        }
    }
}
#[test]
fn snapshot_cannot_be_early_duplicate_or_after_completion_command() {
    positive();
    let baseline = script();
    let at = baseline.iter().position(|x| *x == row(11, 8)).unwrap();
    for where_at in [0, at - 2, at + 2, baseline.len() - 1] {
        let mut s = baseline.clone();
        s.remove(at);
        s.insert(where_at, row(11, 8));
        refuses(s);
    }
    let mut duplicate = baseline;
    duplicate.insert(at, row(11, 8));
    refuses(duplicate);
}
#[test]
fn existing_full_target_report_still_required_independently() {
    positive();
    let mut s = script();
    let at = s.iter().position(|x| *x == target()).unwrap();
    s[at] = String::from_utf8(s[at].clone())
        .unwrap()
        .replace("\"zero_gpu_fd_fence\":true", "\"zero_gpu_fd_fence\":false")
        .into_bytes();
    refuses(s);
    let mut s = script();
    s.remove(at);
    refuses(s);
}
#[test]
fn original_deadline_covers_actual_snapshot_parse_and_final_acceptance() {
    positive();
    let input = script();
    let row = input.iter().position(|x| *x == row(11, 8)).unwrap() + 1;
    let mut f = Fake::new(input);
    f.late_read = Some(row);
    assert_eq!(observe(&mut f).unwrap_err().refusal, Refusal::Deadline);
    assert_eq!(f.cleanup, 1);
    let mut f = Fake::new(script());
    f.now = DEADLINE_NS - 1;
    assert!(observe(&mut f).is_ok());
    let mut f = Fake::new(script());
    f.late_finish = true;
    let failure = observe(&mut f).unwrap_err();
    assert_eq!(failure.refusal, Refusal::Deadline);
    assert_eq!(f.cleanup, 0);
    assert!(failure.cleanup.complete());
}
#[test]
fn zero_start_or_stamp_before_entry_does_not_construct_authority() {
    let mut f = Fake::new(script());
    assert_eq!(f.observed_inferior_start(), Err(Refusal::State));
    mutate("start=\"100\"", "start=\"0\"");
}
#[test]
fn huge_u64_is_preserved_lexically_without_float_roundtrip() {
    positive();
    let mut s = script();
    replace(&mut s, "regid=\"10\"", "regid=\"18446744073709551615\"");
    let v = serde_json::to_value(observe(&mut Fake::new(s)).unwrap()).unwrap();
    assert_eq!(
        v["physical_snapshot"]["context"]["register_id"],
        "18446744073709551615"
    );
}

#[test]
fn post_parse_oracle_deadline_refuses_before_completion_command() {
    positive();
    let mut f = Fake::new(script());
    f.late_snapshot_parse = true;
    let e = observe(&mut f).unwrap_err();
    assert_eq!(e.refusal, Refusal::Deadline);
    assert_eq!(f.checks_after_snapshot, Some(3));
    assert_eq!(f.sent, expected_commands()[..15]);
    assert_eq!((f.finish, f.cleanup), (0, 1));
}
#[test]
fn bad_values_do_not_cause_an_extra_command_or_retry() {
    positive();
    let mut s = script();
    replace(&mut s, "reg=\"df9b5713\"", "reg=\"00000000\"");
    let mut f = Fake::new(s);
    assert_eq!(observe(&mut f).unwrap_err().refusal, Refusal::Artifact);
    assert_eq!(f.sent, expected_commands()[..15]);
    assert_eq!((f.finish, f.cleanup), (0, 1));
}

#[test]
fn actual_mi_gpu_pc_must_match_unchanged_physical_row_before_continue() {
    positive();
    let mut rows = script();
    replace(
        &mut rows,
        "frame={addr=\"0x200050\"",
        "frame={addr=\"0x200051\"",
    );
    let mut peer = Fake::new(rows);
    assert_eq!(observe(&mut peer).unwrap_err().refusal, Refusal::Stop);
    assert_eq!(peer.sent, expected_commands()[..15]);
    assert_eq!((peer.finish, peer.cleanup), (0, 1));
}
