//! Same closed protocol and fake Peer, now with separate native diagnostics.
//! No subprocess, native handle, process owner or debugger ELF is admitted.
use super::*;
use crate::native_runtime_events_v1::{NativeEventsV1, NativeJoinHistoryV1};

#[derive(Clone, Copy, Default)]
enum NativeFault {
    #[default]
    None,
    NoAttach,
    ForeignPid,
    ForeignProcess,
    EarlyRuntime,
    NoRuntime,
    NoObjects,
    NoCallback,
    ForeignThread,
    FailedAck,
    DeferredInvalidation,
    NoPrompt,
    NoExitInvalidation,
    WrongExitInvalidation,
    RowAfterTerminal,
}
fn record(sequence: u64, kind: u64, fields: [u64; 8]) -> Vec<u8> {
    let [event, ek, state, callback, bp, thread, action, reason] = fields;
    format!("=amd-runtime-observation-v1,version=\"1\",sequence=\"{sequence}\",generation=\"1\",inferior=\"1\",pid=\"77\",process=\"9\",event=\"{event}\",event-kind=\"{ek}\",runtime-state=\"{state}\",callback=\"{callback}\",breakpoint=\"{bp}\",thread=\"{thread}\",action=\"{action}\",status=\"0\",type=\"{kind}\",reason=\"{reason}\"\n").into_bytes()
}
struct RuntimeFake {
    base: Fake,
    fault: NativeFault,
    sequence: u64,
}
impl RuntimeFake {
    fn new(fault: NativeFault, base: Fault) -> Self {
        Self {
            base: Fake::new(base),
            fault,
            sequence: 0,
        }
    }
    fn native(&mut self, kind: u64, fields: [u64; 8]) -> Vec<u8> {
        let [event, ek, state, callback, bp, thread, action, reason] = fields;
        self.sequence += 1;
        record(
            self.sequence,
            kind,
            [event, ek, state, callback, bp, thread, action, reason],
        )
    }
}
impl Peer for RuntimeFake {
    fn send(&mut self, token: u64, command: &str) -> Result<(), Refusal> {
        self.base.send(token, command)?;
        if command == "-exec-run" && !matches!(self.fault, NativeFault::NoAttach) {
            let mut attach = self.native(1, [0, 0, 0, 0, 0, 0, 0, 0]);
            if matches!(self.fault, NativeFault::ForeignPid) {
                attach = String::from_utf8(attach)
                    .unwrap()
                    .replace("pid=\"77\"", "pid=\"78\"")
                    .into_bytes();
            }
            self.base.records.push_front(attach);
            if matches!(self.fault, NativeFault::EarlyRuntime) {
                let runtime = self.native(2, [100, 5, 1, 0, 0, 0, 0, 0]);
                self.base.records.insert(1, runtime);
            }
        }
        if command == "-exec-continue" && self.base.resumes == 2 {
            let mut rows = Vec::new();
            if !matches!(self.fault, NativeFault::NoRuntime) {
                let mut runtime = self.native(2, [100, 5, 1, 1, 0, 0, 0, 0]);
                if matches!(self.fault, NativeFault::ForeignProcess) {
                    runtime = String::from_utf8(runtime)
                        .unwrap()
                        .replace("process=\"9\"", "process=\"10\"")
                        .into_bytes();
                }
                rows.push(runtime);
            }
            if !matches!(self.fault, NativeFault::NoObjects) {
                rows.push(self.native(3, [101, 3, 0, 1, 0, 0, 0, 0]));
            }
            if !matches!(self.fault, NativeFault::NoCallback) {
                let thread = if matches!(self.fault, NativeFault::ForeignThread) {
                    8
                } else {
                    7
                };
                let mut callback = self.native(4, [102, 4, 0, 1, 20, thread, 2, 0]);
                if matches!(self.fault, NativeFault::FailedAck) {
                    callback = String::from_utf8(callback)
                        .unwrap()
                        .replace("status=\"0\"", "status=\"-1\"")
                        .into_bytes();
                }
                rows.push(callback);
            }
            for row in rows.into_iter().rev() {
                self.base.records.push_front(row);
            }
        }
        if command == "-data-read-memory-bytes 0x1000 4"
            && matches!(self.fault, NativeFault::DeferredInvalidation)
        {
            // Actual producer may flush this AFTER ^done, before the prompt.
            // Reject without emitting the final -exec-continue.
            let invalid = self.native(5, [0, 0, 0, 0, 0, 0, 0, 17]);
            self.base.line(invalid);
        }
        if command == "-exec-continue"
            && self.base.resumes == 3
            && !matches!(self.fault, NativeFault::NoExitInvalidation)
        {
            let reason = if matches!(self.fault, NativeFault::WrongExitInvalidation) {
                17
            } else {
                14
            };
            let exit = self.native(5, [0, 0, 0, 0, 0, 0, 0, reason]);
            self.base.line(exit);
            if matches!(self.fault, NativeFault::RowAfterTerminal) {
                let again = self.native(5, [0, 0, 0, 0, 0, 0, 0, 14]);
                self.base.line(again);
            }
        }
        if command != "-gdb-exit" && !matches!(self.fault, NativeFault::NoPrompt) {
            self.base.line("(gdb) \n");
        }
        Ok(())
    }
    fn next(&mut self) -> Result<Option<Vec<u8>>, Refusal> {
        self.base.next()
    }
    fn observe_child(&mut self, pid: u32) -> Result<(), Refusal> {
        self.base.observe_child(pid)
    }
    fn entry(&mut self) -> Result<(), Refusal> {
        self.base.entry()
    }
    fn current(&mut self) -> Result<(), Refusal> {
        self.base.current()
    }
    fn finish(&mut self) -> Result<wire::Cleanup, Refusal> {
        self.base.finish()
    }
}
fn run_native(
    fake: &mut RuntimeFake,
) -> Result<(protocol::Observation, NativeJoinHistoryV1), Refusal> {
    let mut events = NativeEventsV1::new();
    let result = protocol::run_with_events(fake, "/fixture/observer", &arguments(), &mut events)?;
    Ok((result, events.history()?))
}
#[test]
fn native_history_joins_the_same_closed_live_protocol_without_new_commands() {
    let mut native = RuntimeFake::new(NativeFault::None, Fault::None);
    let (old, history) = run_native(&mut native).unwrap();
    assert!(old.same_host_stop_original_elf_content_matched);
    assert!(!old.debugger_acceptance_observed);
    assert!(!old.runtime_loaded_success_observed);
    assert!(history.runtime_event_and_ack_joined);
    assert!(history.publication_callback_and_ack_joined);
    assert!(!history.live_stop_authority);
    assert!(!history.physical_register_capture);
    let mut old_fake = Fake::new(Fault::None);
    run(&mut old_fake).unwrap();
    assert_eq!(native.base.commands, old_fake.commands);
}
#[test]
fn installed_debugger_route_does_not_silently_admit_native_extension() {
    let mut native = RuntimeFake::new(NativeFault::None, Fault::None);
    assert_eq!(
        protocol::run(&mut native, "/fixture/observer", &arguments()).err(),
        Some(Refusal::Shape)
    );
    assert_eq!(native.base.resumes, 0);
}
#[test]
fn entry_custody_is_required_before_any_gpu_effects_in_native_route() {
    for base in [
        Fault::Entry,
        Fault::Current,
        Fault::ExtraThread,
        Fault::DuplicateGroup,
    ] {
        let mut fake = RuntimeFake::new(NativeFault::None, base);
        assert!(run_native(&mut fake).is_err());
        assert_eq!(fake.base.resumes, 0);
    }
    for fault in [
        NativeFault::NoAttach,
        NativeFault::ForeignPid,
        NativeFault::EarlyRuntime,
    ] {
        let mut fake = RuntimeFake::new(fault, Fault::None);
        assert!(run_native(&mut fake).is_err());
        assert_eq!(fake.base.resumes, 0);
    }
}
#[test]
fn native_runtime_callback_and_actual_ack_are_all_required_before_resume() {
    for fault in [
        NativeFault::ForeignProcess,
        NativeFault::NoRuntime,
        NativeFault::NoObjects,
        NativeFault::NoCallback,
        NativeFault::ForeignThread,
        NativeFault::FailedAck,
    ] {
        let mut fake = RuntimeFake::new(fault, Fault::None);
        assert!(run_native(&mut fake).is_err());
        assert_eq!(fake.base.resumes, 2);
    }
}
#[test]
fn post_result_prompt_barrier_catches_buffered_invalidation_before_continue() {
    let mut fake = RuntimeFake::new(NativeFault::DeferredInvalidation, Fault::None);
    assert_eq!(run_native(&mut fake).err(), Some(Refusal::Changed));
    assert_eq!(fake.base.resumes, 2);
}
#[test]
fn absent_prompt_is_not_a_synthetic_barrier() {
    let mut fake = RuntimeFake::new(NativeFault::NoPrompt, Fault::None);
    assert!(run_native(&mut fake).is_err());
    assert!(!fake.base.observed);
}
#[test]
fn object_list_and_exact_elf_are_not_replaced_by_native_event_success() {
    for base in [
        Fault::PresentCold,
        Fault::MissingPublished,
        Fault::ForeignUri,
        Fault::WrongElf,
    ] {
        let mut fake = RuntimeFake::new(NativeFault::None, base);
        assert!(run_native(&mut fake).is_err());
        assert!(fake.base.resumes < 3);
    }
}
#[test]
fn terminal_invalidation_is_closed_and_requires_independent_teardown() {
    for fault in [
        NativeFault::NoExitInvalidation,
        NativeFault::WrongExitInvalidation,
        NativeFault::RowAfterTerminal,
    ] {
        assert!(run_native(&mut RuntimeFake::new(fault, Fault::None)).is_err());
    }
    for base in [
        Fault::NonzeroExit,
        Fault::NoNormalExit,
        Fault::WrongProducer,
        Fault::NoPidfdExit,
        Fault::NoReap,
        Fault::NoStreams,
        Fault::NoJoin,
        Fault::FalseReapClaim,
    ] {
        assert!(run_native(&mut RuntimeFake::new(NativeFault::None, base)).is_err());
    }
}
