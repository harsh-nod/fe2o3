//! Closed historical event join for the separately pinned native producer.
//! Staged for CPU qualification until a new debugger ELF/closure is qualified.
//! No replay constructor, native call, PID attach, read or stop capability.
use super::protocol::{EventJoin, Phase};
use crate::parser::{MemoryObject, Refusal};
use serde::Serialize;

const PREFIX: &[u8] = b"=amd-runtime-observation-v1,";
const KEYS: [&[u8]; 16] = [
    b"version",
    b"sequence",
    b"generation",
    b"inferior",
    b"pid",
    b"process",
    b"event",
    b"event-kind",
    b"runtime-state",
    b"callback",
    b"breakpoint",
    b"thread",
    b"action",
    b"status",
    b"type",
    b"reason",
];

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
struct Record {
    sequence: u64,
    generation: u64,
    inferior: u64,
    pid: u64,
    process: u64,
    event: u64,
    event_kind: u64,
    runtime_state: u64,
    callback: u64,
    breakpoint: u64,
    thread: u64,
    action: u64,
    status: i64,
    kind: u64,
    reason: u64,
}
fn number(bytes: &[u8]) -> Result<u64, Refusal> {
    super::config::decimal(bytes)
}
fn signed(bytes: &[u8]) -> Result<i64, Refusal> {
    if let Some(magnitude) = bytes.strip_prefix(b"-") {
        let value = number(magnitude)?;
        if value == 0 || value > (1u64 << 63) {
            return Err(Refusal::Bound);
        }
        if value == 1u64 << 63 {
            Ok(i64::MIN)
        } else {
            Ok(-(value as i64))
        }
    } else {
        i64::try_from(number(bytes)?).map_err(|_| Refusal::Bound)
    }
}
fn parse(line: &[u8]) -> Result<Record, Refusal> {
    if line.len() > 1024 || line.last() != Some(&b'\n') {
        return Err(Refusal::Bound);
    }
    let body = line.strip_prefix(PREFIX).ok_or(Refusal::Shape)?;
    let mut parts = body[..body.len() - 1].split(|b| *b == b',');
    let mut values = [0u64; 16];
    let mut status = 0i64;
    for (index, key) in KEYS.iter().enumerate() {
        let field = parts.next().ok_or(Refusal::Shape)?;
        let raw = field
            .strip_prefix(*key)
            .and_then(|s| s.strip_prefix(b"=\""))
            .and_then(|s| s.strip_suffix(b"\""))
            .ok_or(Refusal::Shape)?;
        if index == 13 {
            status = signed(raw)?;
        } else {
            values[index] = number(raw)?;
        }
    }
    if parts.next().is_some()
        || values[0] != 1
        || values[2] != 1
        || !(1..=32).contains(&values[1])
        || !(1..=5).contains(&values[14])
        || values[15] > 19
    {
        return Err(Refusal::Shape);
    }
    Ok(Record {
        sequence: values[1],
        generation: values[2],
        inferior: values[3],
        pid: values[4],
        process: values[5],
        event: values[6],
        event_kind: values[7],
        runtime_state: values[8],
        callback: values[9],
        breakpoint: values[10],
        thread: values[11],
        action: values[12],
        status,
        kind: values[14],
        reason: values[15],
    })
}

/// Historical IDs are decimal strings, never rounded JSON numbers or reusable
/// native handles. This contains no current process, resource or stop owner.
#[derive(Debug, Serialize)]
pub(super) struct NativeJoinHistoryV1 {
    pub(super) schema: &'static str,
    pub(super) process_id: String,
    pub(super) native_process_id: String,
    pub(super) producer_generation: String,
    pub(super) runtime_event_id: String,
    pub(super) publication_callback_id: String,
    pub(super) publication_breakpoint_id: String,
    pub(super) records: usize,
    pub(super) code_object_events: usize,
    pub(super) runtime_event_and_ack_joined: bool,
    pub(super) publication_callback_and_ack_joined: bool,
    pub(super) same_owned_host_stop_original_elf_joined: bool,
    pub(super) exact_terminal_inferior_exit_observed: bool,
    pub(super) live_stop_authority: bool,
    pub(super) debugger_acceptance_authority: bool,
    pub(super) source_or_launch_authority: bool,
    pub(super) queue_or_dispatch_authority: bool,
    pub(super) physical_register_capture: bool,
}

pub(super) struct NativeEventsV1 {
    rows: [Record; 32],
    count: usize,
    failed: bool,
    owned_pid: Option<u32>,
    owned_thread: u64,
    cold: bool,
    runtime_event: u64,
    next_callback: u64,
    open_callback: u64,
    pending_objects: usize,
    pending_runtime: bool,
    objects: usize,
    joined_callback: u64,
    joined_breakpoint: u64,
    sealed: bool,
    closed: bool,
    terminal: bool,
    finished: bool,
}
const _: () = assert!(std::mem::size_of::<Record>() <= 128);
const _: () = assert!(std::mem::size_of::<NativeEventsV1>() <= 4608);
impl NativeEventsV1 {
    pub(super) fn new() -> Self {
        Self {
            rows: [Record::default(); 32],
            count: 0,
            failed: false,
            owned_pid: None,
            owned_thread: 0,
            cold: false,
            runtime_event: 0,
            next_callback: 1,
            open_callback: 0,
            pending_objects: 0,
            pending_runtime: false,
            objects: 0,
            joined_callback: 0,
            joined_breakpoint: 0,
            sealed: false,
            closed: false,
            terminal: false,
            finished: false,
        }
    }
    fn check(&mut self, result: Result<(), Refusal>) -> Result<(), Refusal> {
        if self.failed {
            return Err(Refusal::State);
        }
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn consume(&mut self, line: &[u8], phase: Phase) -> Result<(), Refusal> {
        if self.failed || self.terminal || self.finished || self.count == 32 {
            return Err(Refusal::State);
        }
        let r = parse(line)?;
        // Producer slot 32 is reserved for its terminal invalidation.
        if self.count == 31 && r.kind != 5 {
            return Err(Refusal::Bound);
        }
        if r.sequence != (self.count as u64 + 1)
            || r.inferior != 1
            || r.pid == 0
            || r.process == 0
            || r.callback > 32
        {
            return Err(Refusal::Process);
        }
        if self.count != 0
            && (r.pid != self.rows[0].pid
                || r.process != self.rows[0].process
                || r.generation != self.rows[0].generation)
        {
            return Err(Refusal::Changed);
        }
        if self.owned_pid.is_some_and(|pid| r.pid != u64::from(pid)) {
            return Err(Refusal::Process);
        }
        if r.event != 0
            && self.rows[..self.count]
                .iter()
                .any(|old| old.event == r.event)
        {
            return Err(Refusal::Duplicate);
        }
        let unused = |values: &[u64]| -> Result<(), Refusal> {
            if values.iter().any(|value| *value != 0) {
                Err(Refusal::Shape)
            } else {
                Ok(())
            }
        };
        if r.kind == 5 {
            // A normal exit invalidates the former live relation. No native
            // "exit" claim substitutes for the separate normal MI/OS teardown.
            if !self.closed
                || !matches!(phase, Phase::Exiting | Phase::Exited)
                || r.reason != 14
                || r.status != 0
            {
                return Err(Refusal::Changed);
            }
            unused(&[
                r.event,
                r.event_kind,
                r.runtime_state,
                r.callback,
                r.breakpoint,
                r.thread,
                r.action,
            ])?;
            self.terminal = true;
        } else {
            if r.status != 0 || r.reason != 0 || self.closed || self.sealed {
                return Err(Refusal::State);
            }
            if r.kind == 1 {
                if self.count != 0 || phase != Phase::ToEntry {
                    return Err(Refusal::State);
                }
                unused(&[
                    r.event,
                    r.event_kind,
                    r.runtime_state,
                    r.callback,
                    r.breakpoint,
                    r.thread,
                    r.action,
                ])?;
            } else {
                if self.count == 0
                    || !self.cold
                    || self.owned_pid.is_none()
                    || !matches!(phase, Phase::ToPublished | Phase::Published)
                {
                    return Err(Refusal::State);
                }
                if r.callback != 0 {
                    if r.callback != self.next_callback {
                        return Err(Refusal::State);
                    }
                    self.open_callback = r.callback;
                } else if self.open_callback != 0 {
                    return Err(Refusal::State);
                }
                match r.kind {
                    2 => {
                        if self.runtime_event != 0
                            || r.event == 0
                            || r.event_kind != 5
                            || r.runtime_state != 1
                        {
                            return Err(Refusal::State);
                        }
                        unused(&[r.breakpoint, r.thread, r.action])?;
                        self.runtime_event = r.event;
                        self.pending_runtime = r.callback != 0;
                    }
                    3 => {
                        if self.runtime_event == 0
                            || r.event == 0
                            || r.event_kind != 3
                            || r.callback == 0
                        {
                            return Err(Refusal::State);
                        }
                        unused(&[r.runtime_state, r.breakpoint, r.thread, r.action])?;
                        self.pending_objects += 1;
                    }
                    4 => {
                        if r.callback == 0
                            || r.breakpoint == 0
                            || r.thread != self.owned_thread
                            || r.runtime_state != 0
                        {
                            return Err(Refusal::Process);
                        }
                        match r.action {
                            1 if r.event == 0
                                && r.event_kind == 0
                                && self.pending_objects == 0
                                && !self.pending_runtime => {}
                            2 if r.event != 0 && r.event_kind == 4 => {}
                            _ => return Err(Refusal::State),
                        }
                        if self.pending_objects != 0 {
                            self.objects += self.pending_objects;
                            self.joined_callback = r.callback;
                            self.joined_breakpoint = r.breakpoint;
                            self.pending_objects = 0;
                        }
                        self.open_callback = 0;
                        self.pending_runtime = false;
                        self.next_callback += 1;
                    }
                    _ => return Err(Refusal::Shape),
                }
            }
        }
        self.rows[self.count] = r;
        self.count += 1;
        Ok(())
    }
    pub(super) fn history(&self) -> Result<NativeJoinHistoryV1, Refusal> {
        if self.failed || !self.finished {
            return Err(Refusal::Incomplete);
        }
        Ok(NativeJoinHistoryV1 {
            schema: "diagnostic-owned-noqueue-native-join-history-v1",
            process_id: self.rows[0].pid.to_string(),
            native_process_id: self.rows[0].process.to_string(),
            producer_generation: self.rows[0].generation.to_string(),
            runtime_event_id: self.runtime_event.to_string(),
            publication_callback_id: self.joined_callback.to_string(),
            publication_breakpoint_id: self.joined_breakpoint.to_string(),
            records: self.count,
            code_object_events: self.objects,
            runtime_event_and_ack_joined: true,
            publication_callback_and_ack_joined: true,
            same_owned_host_stop_original_elf_joined: true,
            exact_terminal_inferior_exit_observed: true,
            live_stop_authority: false,
            debugger_acceptance_authority: false,
            source_or_launch_authority: false,
            queue_or_dispatch_authority: false,
            physical_register_capture: false,
        })
    }
}
impl EventJoin for NativeEventsV1 {
    fn prompt_barrier(&self) -> bool {
        true
    }
    fn native_line(&mut self, line: &[u8], phase: Phase) -> Result<(), Refusal> {
        let result = self.consume(line, phase);
        self.check(result)
    }
    fn owned_entry(&mut self, pid: u32, thread: &[u8]) -> Result<(), Refusal> {
        let result = (|| {
            if self.owned_pid.is_some()
                || self.count != 1
                || self.rows[0].kind != 1
                || self.rows[0].pid != u64::from(pid)
            {
                return Err(Refusal::Process);
            }
            let thread = number(thread)?;
            if thread == 0 {
                return Err(Refusal::Process);
            }
            self.owned_pid = Some(pid);
            self.owned_thread = thread;
            Ok(())
        })();
        self.check(result)
    }
    fn cold_absence(&mut self) -> Result<(), Refusal> {
        let result = if self.owned_pid.is_some() && self.count == 1 && !self.cold {
            self.cold = true;
            Ok(())
        } else {
            Err(Refusal::State)
        };
        self.check(result)
    }
    fn original_elf_joined(&mut self, object: MemoryObject) -> Result<(), Refusal> {
        let result = if self.owned_pid == Some(object.pid)
            && !self.sealed
            && self.cold
            && self.runtime_event != 0
            && self.objects != 0
            && self.open_callback == 0
            && self.pending_objects == 0
        {
            self.sealed = true;
            Ok(())
        } else {
            Err(Refusal::Incomplete)
        };
        self.check(result)
    }
    fn close_stop(&mut self) -> Result<(), Refusal> {
        let result = if self.sealed && !self.closed {
            self.closed = true;
            Ok(())
        } else {
            Err(Refusal::State)
        };
        self.check(result)
    }
    fn completed_teardown(&mut self) -> Result<(), Refusal> {
        let result = if self.closed && self.terminal && !self.finished {
            self.finished = true;
            Ok(())
        } else {
            Err(Refusal::Incomplete)
        };
        self.check(result)
    }
}

#[cfg(test)]
#[path = "native_runtime_events_v1_tests.rs"]
mod tests;
