//! Closed same-producer transition observations; not an independent dbgapi replay.
use crate::rocgdb_mi_parser_v3::MiResultsV3;
use crate::syntax::{Refusal, decimal, fields, positive, signed, text};

#[derive(Default)]
pub(crate) struct Transitions {
    pub(crate) phase: u64,
    pub(crate) count: usize,
    pub(crate) process: u64,
    host_rows: usize,
    terminal_rows: usize,
    legacy_runtime: u64,
    legacy_stop: u64,
}
impl Transitions {
    pub(crate) fn row(
        &mut self,
        r: &MiResultsV3,
        process_join: (u32, u64, u64),
        stage: u8,
        host_stop: bool,
        gpu_stop: bool,
    ) -> Result<(), Refusal> {
        let (pid, host, start) = process_join;
        const KEYS: &[&str] = &["s", "p", "pid", "i", "a", "t", "start"];
        let p = positive(text(r, "p")?)?;
        let keys = if p == 8 {
            crate::physical_snapshot_v2::KEYS
        } else {
            KEYS
        };
        fields(r, keys, keys)?;
        let seq = positive(text(r, "s")?)?;
        let process = positive(text(r, "a")?)?;
        if self.count == 16
            || seq != self.count as u64 + 1
            || positive(text(r, "pid")?)? != u64::from(pid)
            || positive(text(r, "i")?)? != 1
            || positive(text(r, "t")?)? != host
            || start == 0
            || positive(text(r, "start")?)? != start
            || (self.count > 0 && process != self.process)
        {
            return Err(Refusal::Changed);
        }
        let allowed = match (self.phase, p) {
            (0, 1) => stage == 1,
            (1, 2) => stage == 2,
            (2, 2) => stage == 2 && self.host_rows < 5,
            (2, 3) => stage == 2 && self.host_rows >= 4 && host_stop,
            (3, 4) => stage == 2 && host_stop,
            (4, 5) => stage == 3,
            (5, 6) => stage == 3,
            (6, 7) => stage == 3 && gpu_stop,
            (7, 8) => stage == 3 && gpu_stop,
            (8, 9) => stage == 4,
            (9, 10) => stage == 4,
            (10, 10) => stage == 4 && self.terminal_rows == 1,
            (10, 11) => stage == 4 && self.terminal_rows == 2,
            _ => false,
        };
        if !allowed {
            return Err(Refusal::State);
        }
        if p == 2 {
            self.host_rows += 1;
        }
        if p == 10 {
            self.terminal_rows += 1;
        }
        self.phase = p;
        self.count += 1;
        self.process = process;
        Ok(())
    }
    /// Existing observation namespaces remain syntactically bounded histories.
    /// They do not supply one-stop success, native handles or a second client.
    pub(crate) fn legacy(&mut self, class: &str, r: &MiResultsV3) -> Result<(), Refusal> {
        const BASE: &[&str] = &[
            "version",
            "sequence",
            "generation",
            "inferior",
            "pid",
            "process",
            "event",
            "event-kind",
            "runtime-state",
            "callback",
            "breakpoint",
            "thread",
            "action",
            "status",
            "type",
            "reason",
        ];
        const STOP: &[&str] = &[
            "schema",
            "profile",
            "queue-provenance",
            "version",
            "sequence",
            "generation",
            "inferior",
            "pid",
            "process",
            "event",
            "event-kind",
            "runtime-state",
            "callback",
            "breakpoint",
            "thread",
            "action",
            "status",
            "type",
            "reason",
            "stop-generation",
            "wave",
            "workgroup",
            "dispatch",
            "queue",
            "agent",
            "architecture",
            "os-queue",
            "packet",
            "os-agent",
            "private-bytes",
            "group-bytes",
            "wave-state",
            "stop-reason",
            "lanes",
            "wave-index",
            "group-x",
            "group-y",
            "group-z",
            "grid-x",
            "grid-y",
            "grid-z",
            "size-x",
            "size-y",
            "size-z",
            "queue-type",
            "queue-state",
            "elf-machine",
        ];
        let (keys, seen, max) = match class {
            "amd-runtime-observation-v1" => (BASE, &mut self.legacy_runtime, 32),
            "amd-stopped-wave-observation-v1" => (STOP, &mut self.legacy_stop, 64),
            _ => return Err(Refusal::Shape),
        };
        fields(r, keys, keys)?;
        for k in keys {
            match *k {
                "schema" if text(r, k)? == b"fe2o3-gfx950-native-stopped-wave-observation-v1" => {}
                "profile"
                    if text(r, k)? == b"fe2o3.gfx950.same-client-stopped-wave-observation.v1" => {}
                "queue-provenance" if text(r, k)? == b"unavailable" => {}
                "schema" | "profile" | "queue-provenance" => return Err(Refusal::Shape),
                "status" => signed(text(r, k)?)?,
                _ => {
                    decimal(text(r, k)?)?;
                }
            }
        }
        if positive(text(r, "version")?)? != 1
            || positive(text(r, "sequence")?)? != *seen + 1
            || *seen == max
        {
            return Err(Refusal::Changed);
        }
        *seen += 1;
        Ok(())
    }
}
