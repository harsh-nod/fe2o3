//! Inert parser/protocol controls. No GDB, target, process, KFD, queue or scope.
use super::*;
use serde_json::json;
use std::collections::VecDeque;

fn target() -> Vec<u8> {
    let v = json!({
        "schema":"fe2o3-gfx950-one-stop-target-observation-v1",
        "profile":"fe2o3.gfx950.one-stop-target.v1",
        "qualification":"UNQUALIFIED_NATIVE_TARGET_NOT_TO_RUN","status":"local_complete",
        "process_id":4242,
        "trace":{"last_returned":"local_complete","native_effects":"local_complete",
            "zero_gpu_fd_fence":true,"owned_descriptors_closed":true,
            "owner_failure_publication_possible":null},
        "facts":{"source_sha256":"e567e9b414f00822fbf6436984a452d9940630eccec40a0e86b2c5c191598017",
            "source_bytes":1767,
            "artifact_sha256":"cd3daab76db347104166c898a58dcc29c197ee1eac8b258b10a2c793e8779c87",
            "artifact_bytes":5312,
            "trap_sha256":"4ffea893ee53e018a629c19254855721882444517155251f1f45dd3519bc81fe",
            "trap_bytes":1116,"mapped_backing_bytes":"12288","metadata_retained_bytes":"4096",
            "closure_sha256":"1111111111111111111111111111111111111111111111111111111111111111",
            "queue_backing_bytes":"190296064","projected_native_bytes":"190312448",
            "projected_logical_bytes":"200000000"},
        "failure":null,"completion_requires_exit_zero":true,"valid_packet_publications":1,
        "doorbell_stores":1,"attached_debugger":null,"loaded_success":null,
        "event_acknowledged":null,"ttmp_readback":null,"sampler_exclusion":null,
        "unique_gpu_debug_stop":null,"physical_capture":false,"family_cleanup_established":false,
        "operational_qualification":false,"proc_snapshot_proves_general_exclusion":false,
        "source_or_launch_authority":false
    });
    let mut b = serde_json::to_vec(&v).unwrap();
    b.push(b'\n');
    b
}
fn row(s: u64, p: u64) -> Vec<u8> {
    format!("=fe2o3-owned-one-stop-v1,s=\"{s}\",p=\"{p}\",pid=\"4242\",i=\"1\",a=\"7\",t=\"1\"\n")
        .into_bytes()
}
fn b(s: &str) -> Vec<u8> {
    s.as_bytes().to_vec()
}
fn host_stop(symbol: &str, number: &str) -> Vec<u8> {
    b(&format!(
        "*stopped,reason=\"breakpoint-hit\",disp=\"del\",bkptno=\"{number}\",frame={{addr=\"0x1000\",func=\"{symbol}\",args=[]}},thread-id=\"1\",stopped-threads=\"all\"\n"
    ))
}
fn script() -> Vec<Vec<u8>> {
    let mut v = vec![b("=thread-group-added,id=\"i1\"\n")];
    for p in [
        "auto-load gdb-scripts",
        "auto-load libthread-db",
        "auto-load local-gdbinit",
        "auto-load python-scripts",
        "startup-with-shell",
    ] {
        v.push(b(&format!(
            "=cmd-param-changed,param=\"{p}\",value=\"off\"\n"
        )));
    }
    v.push(b("(gdb)\n"));
    for t in 1..=10 {
        v.push(b(&format!("{t}^done\n")));
        v.push(b("(gdb)\n"));
    }
    v.push(b(&format!("11^done,bkpt={{number=\"1\",type=\"breakpoint\",disp=\"del\",enabled=\"y\",addr=\"0x1000\",func=\"{ENTRY}\",thread-groups=[\"i1\"],times=\"0\",original-location=\"{ENTRY}\"}}\n")));
    v.push(b("(gdb)\n"));
    v.extend([
        b("=thread-group-started,id=\"i1\",pid=\"4242\"\n"),
        b("=thread-created,id=\"1\",group-id=\"i1\"\n"),
        b("12^running\n"),
        b("*running,thread-id=\"all\"\n"),
        b("(gdb)\n"),
        host_stop(ENTRY, "1"),
        b("=breakpoint-deleted,id=\"1\"\n"),
        row(1, 1),
        b("13^done\n"),
        b("(gdb)\n"),
        b("14^running\n"),
        b("(gdb)\n"),
    ]);
    for s in 2..=5 {
        v.push(row(s, 2));
    }
    v.extend([host_stop(CHECKPOINT,"-2"),row(6,3),row(7,4),
        row(8,5),row(9,6),b("15^running\n"),b("(gdb)\n"),
        b("=thread-created,id=\"2\",group-id=\"i1\"\n"),
        b("*stopped,reason=\"signal-received\",signal-name=\"SIGTRAP\",signal-meaning=\"Trace/breakpoint trap\",frame={addr=\"0x200050\",func=\"fe2o3_gfx950_one_stop_fixture\",args=[]},thread-id=\"2\",lane-id=\"0\",stopped-threads=\"all\"\n"),
        row(10,7),row(11,8),row(12,9),row(13,10),
        b("16^running\n"),b("(gdb)\n"),target(),
        b("=thread-exited,id=\"2\",group-id=\"i1\"\n"),
        b("=thread-exited,id=\"1\",group-id=\"i1\"\n"),
        b("=thread-group-exited,id=\"i1\",exit-code=\"0\"\n"),
        b("*stopped,reason=\"exited-normally\"\n"),row(14,10),row(15,11),
        b("17^exit\n")]);
    v
}
fn expected_commands() -> Vec<String> {
    let mut v = [
        "-gdb-set pagination off",
        "-gdb-set confirm off",
        "-gdb-set mi-async on",
        "-gdb-set follow-fork-mode parent",
        "-gdb-set detach-on-fork off",
        "-interpreter-exec console \"unset environment\"",
        "-gdb-set environment LANG=C",
        "-gdb-set environment LC_ALL=C",
    ]
    .map(str::to_string)
    .to_vec();
    v.extend([
        format!("-file-exec-and-symbols \"{TARGET_PATH}\""),
        "-exec-arguments --acknowledge-fixed-one-stop-observer-unqualified".into(),
        format!("-break-insert -t {ENTRY}"),
        "-exec-run".into(),
        "-interpreter-exec console \"fe2o3-one-stop-select-v1\"".into(),
        "-exec-continue".into(),
        "-exec-continue".into(),
        "-exec-continue".into(),
        "-gdb-exit".into(),
    ]);
    v
}
fn complete() -> Cleanup {
    Cleanup {
        owned_inferior_pidfd_exit_observed: true,
        debugger_direct_child_reaped: true,
        streams_complete: true,
        reader_threads_joined: true,
        ..Cleanup::default()
    }
}
struct Fake {
    lines: VecDeque<Vec<u8>>,
    sent: Vec<String>,
    child: usize,
    entry: usize,
    current: usize,
    cleanup: usize,
    finish: usize,
    closure: Cleanup,
    now: u64,
    late_finish: bool,
    custody_fail: bool,
    read_calls: usize,
    late_read: Option<usize>,
    rewind: bool,
    fail_send_token: Option<u64>,
}
impl Fake {
    fn new(lines: Vec<Vec<u8>>) -> Self {
        Self {
            lines: lines.into(),
            sent: Vec::new(),
            child: 0,
            entry: 0,
            current: 0,
            cleanup: 0,
            finish: 0,
            closure: complete(),
            now: 0,
            late_finish: false,
            custody_fail: false,
            read_calls: 0,
            late_read: None,
            rewind: false,
            fail_send_token: None,
        }
    }
}
impl Peer for Fake {
    fn send(&mut self, t: u64, c: &str) -> Result<(), Refusal> {
        if t != self.sent.len() as u64 + 1
            || expected_commands().get(self.sent.len()).map(String::as_str) != Some(c)
        {
            return Err(Refusal::Token);
        }
        self.sent.push(c.into());
        if self.fail_send_token == Some(t) {
            return Err(Refusal::Incomplete);
        }
        Ok(())
    }
    fn next(&mut self) -> Result<Option<Vec<u8>>, Refusal> {
        self.read_calls += 1;
        if self.late_read == Some(self.read_calls) {
            self.now = DEADLINE_NS;
        }
        if self.rewind && self.read_calls == 10 {
            self.now = 0;
        }
        Ok(self.lines.pop_front())
    }
    fn observe_child(&mut self, p: u32) -> Result<(), Refusal> {
        if p != 4242 || self.child != 0 {
            return Err(Refusal::Process);
        }
        self.child += 1;
        Ok(())
    }
    fn entry(&mut self) -> Result<(), Refusal> {
        if self.custody_fail {
            return Err(Refusal::Process);
        }
        self.entry += 1;
        Ok(())
    }
    fn current(&mut self) -> Result<(), Refusal> {
        self.current += 1;
        Ok(())
    }
    fn elapsed_ns(&mut self) -> Result<u64, Refusal> {
        Ok(self.now)
    }
    fn finish(&mut self) -> Result<Cleanup, Refusal> {
        self.finish += 1;
        if self.late_finish {
            self.now = DEADLINE_NS;
        }
        Ok(self.closure)
    }
    fn cleanup(&mut self) -> Cleanup {
        self.cleanup += 1;
        Cleanup::default()
    }
}
fn refuses(lines: Vec<Vec<u8>>) {
    let mut f = Fake::new(lines);
    assert!(observe(&mut f).is_err());
    assert_eq!(f.cleanup, 1);
    assert_eq!(f.finish, 0);
}
fn replace(lines: &mut [Vec<u8>], from: &str, to: &str) {
    let n = lines
        .iter()
        .position(|x| std::str::from_utf8(x).unwrap().contains(from))
        .unwrap();
    let s = String::from_utf8(lines[n].clone()).unwrap();
    lines[n] = s.replacen(from, to, 1).into_bytes();
}
#[test]
fn closed_positive_is_an_inert_observation_not_authority() {
    let mut f = Fake::new(script());
    let r = observe(&mut f).unwrap();
    assert_eq!(f.sent, expected_commands());
    assert_eq!(
        (f.child, f.entry, f.current, f.finish, f.cleanup),
        (1, 1, 4, 1, 0)
    );
    assert_eq!(r.producer_rows, 15);
    assert_eq!(r.observed_internal_checkpoint_id, "-2");
    assert!(!r.independent_native_tuple_replay);
    assert!(!r.physical_register_capture && !r.physical_memory_capture);
    assert!(!r.whole_family_cleanup_proved && !r.operational_qualification);
}
#[test]
fn every_transition_row_is_required() {
    let baseline = script();
    for i in 0..baseline.len() {
        if baseline[i].starts_with(b"=fe2o3-owned-one-stop-v1") {
            let mut v = baseline.clone();
            v.remove(i);
            refuses(v);
        }
    }
}
#[test]
fn foreign_pid_inferior_process_host_and_sequence_are_refused() {
    for (a, z) in [
        ("pid=\"4242\"", "pid=\"4243\""),
        ("i=\"1\"", "i=\"2\""),
        ("a=\"7\"", "a=\"8\""),
        ("t=\"1\"", "t=\"2\""),
        ("s=\"7\"", "s=\"8\""),
    ] {
        let mut v = script();
        // Mutate a noninitial row, so changed identities cannot form a new baseline.
        let i = v.iter().position(|x| x == &row(7, 4)).unwrap();
        v[i] = String::from_utf8(v[i].clone())
            .unwrap()
            .replace(a, z)
            .into_bytes();
        refuses(v);
    }
}
#[test]
fn provisional_phase_cannot_authorize_resume() {
    let mut v = script();
    replace(&mut v, "s=\"7\",p=\"4\"", "s=\"7\",p=\"3\"");
    refuses(v);
    let mut v = script();
    replace(&mut v, "s=\"11\",p=\"8\"", "s=\"11\",p=\"7\"");
    refuses(v);
}
#[test]
fn producer_phase_before_mi_stop_is_refused() {
    for needle in [row(6, 3), row(10, 7)] {
        let mut v = script();
        let i = v.iter().position(|x| *x == needle).unwrap();
        v.swap(i, i - 1);
        refuses(v);
    }
}
#[test]
fn no_legacy_domain_substitution() {
    let mut v = script();
    replace(
        &mut v,
        "=fe2o3-owned-one-stop-v1,",
        "=amd-runtime-observation-v1,",
    );
    refuses(v);
}
#[test]
fn producer_unknown_duplicate_fields_or_noncanonical_numbers_refused() {
    for (a, z) in [
        ("s=\"1\"", "s=\"01\""),
        ("s=\"1\"", "s=\"1\",s=\"1\""),
        ("a=\"7\"", "extra=\"7\""),
        ("p=\"1\"", "p=\"12\""),
    ] {
        let mut v = script();
        replace(&mut v, a, z);
        refuses(v);
    }
}
#[test]
fn entry_breakpoint_must_be_temporary_and_retired() {
    let mut v = script();
    replace(&mut v, "disp=\"del\"", "disp=\"keep\"");
    refuses(v);
    let mut v = script();
    v.retain(|x| x != b"=breakpoint-deleted,id=\"1\"\n");
    refuses(v);
}
#[test]
fn entry_custody_refusal_prevents_selection() {
    let mut f = Fake::new(script());
    f.custody_fail = true;
    assert!(observe(&mut f).is_err());
    assert_eq!(f.sent.len(), 12);
    assert_eq!(f.cleanup, 1);
}
#[test]
fn wrong_mi_result_token_and_error_refuse_without_retry() {
    for z in ["99^done", "13^error"] {
        let mut v = script();
        replace(&mut v, "13^done", z);
        refuses(v);
    }
}
#[test]
fn selected_unknown_notification_refuses() {
    let mut v = script();
    let i = v.iter().position(|x| x == &row(7, 4)).unwrap();
    v.insert(i + 1, b("=foreign-permission,ok=\"1\"\n"));
    refuses(v);
}
#[test]
fn host_and_gpu_stop_domains_cannot_be_swapped() {
    let mut v = script();
    replace(
        &mut v,
        "thread-id=\"2\",lane-id=\"0\"",
        "thread-id=\"1\",lane-id=\"0\"",
    );
    refuses(v);
    let mut v = script();
    replace(&mut v, "signal-name=\"SIGTRAP\"", "signal-name=\"SIGSEGV\"");
    refuses(v);
    let mut v = script();
    replace(&mut v, "lane-id=\"0\"", "lane-id=\"64\"");
    refuses(v);
}
#[test]
fn second_gpu_thread_and_foreign_group_refuse() {
    let mut v = script();
    let i = v
        .iter()
        .position(|x| x == b"=thread-created,id=\"2\",group-id=\"i1\"\n")
        .unwrap();
    v.insert(i + 1, b("=thread-created,id=\"3\",group-id=\"i1\"\n"));
    refuses(v);
    let mut v = script();
    replace(&mut v, "group-id=\"i1\"", "group-id=\"i2\"");
    refuses(v);
}
#[test]
fn lifecycle_disappearance_never_substitutes_for_normal_exit() {
    for n in [
        "*stopped,reason=\"exited-normally\"\n",
        "=thread-exited,id=\"1\",group-id=\"i1\"\n",
        "=thread-exited,id=\"2\",group-id=\"i1\"\n",
        "=thread-group-exited,id=\"i1\",exit-code=\"0\"\n",
    ] {
        let mut v = script();
        v.retain(|x| x != n.as_bytes());
        refuses(v);
    }
}
#[test]
fn killed_detached_or_nonzero_exit_never_accept() {
    for (a, z) in [
        ("reason=\"exited-normally\"", "reason=\"exited-signalled\""),
        ("exit-code=\"0\"", "exit-code=\"01\""),
        ("p=\"11\"", "p=\"12\""),
    ] {
        let mut v = script();
        replace(&mut v, a, z);
        refuses(v);
    }
}
#[test]
fn target_report_is_separate_required_evidence() {
    let mut v = script();
    v.retain(|x| !x.starts_with(b"{"));
    refuses(v);
    let mut v = script();
    replace(
        &mut v,
        "\"status\":\"local_complete\"",
        "\"status\":\"refused\"",
    );
    refuses(v);
}
#[test]
fn target_claim_cannot_manufacture_debugger_or_capture_evidence() {
    for key in [
        "loaded_success",
        "event_acknowledged",
        "ttmp_readback",
        "sampler_exclusion",
        "unique_gpu_debug_stop",
        "attached_debugger",
    ] {
        let mut v: serde_json::Value = serde_json::from_slice(&target()).unwrap();
        v[key] = json!(true);
        assert!(crate::target_report::check(&serde_json::to_vec(&v).unwrap(), 4242).is_err());
    }
}
#[test]
fn target_duplicate_unknown_missing_fields_refuse() {
    let s = String::from_utf8(target()).unwrap();
    for v in [
        s.replacen(
            "\"status\":",
            "\"status\":\"local_complete\",\"status\":",
            1,
        ),
        s.replacen("{", "{\"unknown\":false,", 1),
        s.replace("\"failure\":null,", ""),
    ] {
        assert!(crate::target_report::check(v.as_bytes(), 4242).is_err());
    }
}
#[test]
fn target_artifact_and_decimal_budget_mutations_refuse() {
    for (key, val) in [
        ("artifact_bytes", json!(5313)),
        ("queue_backing_bytes", json!("0")),
        ("mapped_backing_bytes", json!("012288")),
        ("projected_native_bytes", json!("190312447")),
        ("projected_logical_bytes", json!("268435457")),
        ("closure_sha256", json!("0".repeat(64))),
    ] {
        let mut v: serde_json::Value = serde_json::from_slice(&target()).unwrap();
        v["facts"][key] = val;
        assert!(crate::target_report::check(&serde_json::to_vec(&v).unwrap(), 4242).is_err());
    }
}
#[test]
fn target_early_or_repeated_report_refuses() {
    let mut v = script();
    v.insert(0, target());
    refuses(v);
    let mut v = script();
    let i = v.iter().position(|x| x.starts_with(b"{")).unwrap();
    v.insert(i, target());
    refuses(v);
}
#[test]
fn oversized_record_refuses_before_parse() {
    let mut v = script();
    v.insert(0, vec![b'x'; MAX_LINE + 1]);
    refuses(v);
}
#[test]
fn cumulative_record_bound_cannot_be_reset() {
    let mut v = vec![b("~\"inert\"\n"); MAX_RECORDS + 1];
    v.extend(script());
    refuses(v);
}
#[test]
fn immediate_deadline_and_late_read_are_refused() {
    let mut f = Fake::new(script());
    f.now = DEADLINE_NS;
    assert_eq!(observe(&mut f).unwrap_err().refusal, Refusal::Deadline);
    let mut f = Fake::new(script());
    f.late_read = Some(10);
    assert_eq!(observe(&mut f).unwrap_err().refusal, Refusal::Deadline);
}
#[test]
fn completed_teardown_does_not_turn_late_result_positive() {
    let mut f = Fake::new(script());
    f.late_finish = true;
    let e = observe(&mut f).unwrap_err();
    assert_eq!(e.refusal, Refusal::Deadline);
    assert!(e.cleanup.debugger_direct_child_reaped);
    assert_eq!((f.finish, f.cleanup), (1, 0));
}
#[test]
fn before_deadline_is_accepted_but_backwards_clock_is_not() {
    let mut f = Fake::new(script());
    f.now = DEADLINE_NS - 1;
    assert!(observe(&mut f).is_ok());
    let mut f = Fake::new(script());
    f.now = 1;
    f.rewind = true;
    assert_eq!(observe(&mut f).unwrap_err().refusal, Refusal::Deadline);
    assert_eq!(f.cleanup, 1);
}
#[test]
fn unknown_dispatched_continue_outcome_is_not_retried() {
    let mut f = Fake::new(script());
    f.fail_send_token = Some(14);
    assert_eq!(observe(&mut f).unwrap_err().refusal, Refusal::Incomplete);
    assert_eq!(f.sent.len(), 14);
    assert_eq!(f.cleanup, 1);
    assert_eq!(f.finish, 0);
}
#[test]
fn cleanup_each_missing_or_overclaimed_fact_refuses() {
    for i in 0..7 {
        let mut f = Fake::new(script());
        match i {
            0 => f.closure.owned_inferior_pidfd_exit_observed = false,
            1 => f.closure.debugger_direct_child_reaped = false,
            2 => f.closure.streams_complete = false,
            3 => f.closure.reader_threads_joined = false,
            4 => f.closure.unadmitted_inferior_cleanup_not_proven = true,
            5 => f.closure.inferior_reaped_by_controller = true,
            _ => f.closure.cleanup_deadline_expired = true,
        };
        assert_eq!(observe(&mut f).unwrap_err().refusal, Refusal::Incomplete);
        assert_eq!((f.finish, f.cleanup), (1, 0));
    }
}
#[test]
fn unknown_selected_commands_have_no_generation_path() {
    let c = expected_commands();
    assert_eq!(c.len(), 17);
    assert_eq!(
        &c[13..16],
        ["-exec-continue", "-exec-continue", "-exec-continue"]
    );
    assert_eq!(&c[16], "-gdb-exit");
    assert!(
        !c.iter().any(|x| x.contains("-data-")
            || x.contains("-file-list")
            || x.contains("-thread-select"))
    );
    assert_eq!(MI_INTERPRETER, "mi2");
}

#[test]
fn running_is_allowed_only_within_each_actual_resume_episode() {
    for thread in ["all", "1"] {
        let mut v = script();
        for token in [14, 15, 16] {
            let marker = b(&format!("{token}^running\n"));
            let i = v.iter().position(|x| *x == marker).unwrap();
            v.insert(i + 1, b(&format!("*running,thread-id=\"{thread}\"\n")));
        }
        let mut f = Fake::new(v);
        observe(&mut f).unwrap();
        assert_eq!(f.sent, expected_commands());
        assert_eq!((f.finish, f.cleanup), (1, 0));
    }
}
#[test]
fn running_after_each_stop_is_not_a_new_resume_episode() {
    observe(&mut Fake::new(script())).unwrap();
    let gpu = b(
        "*stopped,reason=\"signal-received\",signal-name=\"SIGTRAP\",signal-meaning=\"Trace/breakpoint trap\",frame={addr=\"0x200050\",func=\"fe2o3_gfx950_one_stop_fixture\",args=[]},thread-id=\"2\",lane-id=\"0\",stopped-threads=\"all\"\n",
    );
    for stop in [host_stop(ENTRY, "1"), host_stop(CHECKPOINT, "-2"), gpu] {
        let mut v = script();
        let i = v.iter().position(|x| *x == stop).unwrap();
        v.insert(i + 1, b("*running,thread-id=\"all\"\n"));
        refuses(v);
    }
}
#[test]
fn selected_arming_and_final_exit_stages_refuse_running() {
    observe(&mut Fake::new(script())).unwrap();
    for marker in [row(1, 1), b("17^exit\n")] {
        let mut v = script();
        let i = v.iter().position(|x| *x == marker).unwrap();
        v.insert(i + 1, b("*running,thread-id=\"all\"\n"));
        refuses(v);
    }
}
#[test]
fn every_exit_observation_individually_closes_the_last_running_episode() {
    let exits = [
        b("=thread-exited,id=\"2\",group-id=\"i1\"\n"),
        b("=thread-exited,id=\"1\",group-id=\"i1\"\n"),
        b("=thread-group-exited,id=\"i1\",exit-code=\"0\"\n"),
        b("*stopped,reason=\"exited-normally\"\n"),
    ];
    // Each flag is tested as the FIRST exit observation, so refusal cannot be
    // accidentally satisfied by a different previously set terminal flag.
    for selected in &exits {
        let mut v = script();
        v.retain(|x| !exits.contains(x));
        let i = v.iter().position(|x| *x == row(14, 10)).unwrap();
        let mut ordered = vec![selected.clone()];
        ordered.extend(exits.iter().filter(|x| *x != selected).cloned());
        v.splice(i..i, ordered);
        observe(&mut Fake::new(v.clone())).unwrap(); // validate every baseline first
        v.insert(i + 1, b("*running,thread-id=\"all\"\n"));
        refuses(v);
    }
}

// Independently send-gated async transcript controls; no native transport.
#[path = "async_stop_tests.rs"]
mod async_stop_tests;
