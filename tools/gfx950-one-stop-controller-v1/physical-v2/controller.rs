use crate::rocgdb_mi_parser_v3::{
    MiAsyncKindV3, MiRecordV3, MiResultsV3, MiStreamKindV3, MiValueV3,
};
use crate::syntax::{self, Refusal, decimal, fields, positive, text};
use crate::transitions::Transitions;
use crate::{FailedObservation, Peer, ProtocolObservation};

struct Session<'a, P: Peer> {
    peer: &'a mut P,
    clock: u64,
    bytes: usize,
    records: usize,
    token: u64,
    stage: u8,
    run_sent: bool,
    group_added: bool,
    group_removed: bool,
    pid: Option<u32>,
    host: Option<u64>,
    gpu: Option<u64>,
    host_exited: bool,
    gpu_exited: bool,
    entry_break: Option<u64>,
    entry_stop: bool,
    entry_deleted: bool,
    host_stop: bool,
    gpu_stop: bool,
    inferior_start: Option<u64>,
    gpu_pc: Option<u64>,
    physical: Option<crate::PhysicalSnapshotV2>,
    checkpoint_id: Option<i64>,
    normal_exit: bool,
    group_exited: bool,
    settings: [bool; 5],
    producer: Vec<u8>,
    transitions: Transitions,
}
impl<'a, P: Peer> Session<'a, P> {
    fn new(peer: &'a mut P) -> Self {
        Self {
            peer,
            clock: 0,
            bytes: 0,
            records: 0,
            token: 0,
            stage: 0,
            run_sent: false,
            group_added: false,
            group_removed: false,
            pid: None,
            host: None,
            gpu: None,
            host_exited: false,
            gpu_exited: false,
            entry_break: None,
            entry_stop: false,
            entry_deleted: false,
            host_stop: false,
            gpu_stop: false,
            inferior_start: None,
            gpu_pc: None,
            physical: None,
            checkpoint_id: None,
            normal_exit: false,
            group_exited: false,
            settings: [false; 5],
            producer: Vec::new(),
            transitions: Transitions::default(),
        }
    }
    fn timely(&mut self) -> Result<(), Refusal> {
        let now = self.peer.elapsed_ns()?;
        if now < self.clock || now >= crate::DEADLINE_NS {
            return Err(Refusal::Deadline);
        }
        self.clock = now;
        Ok(())
    }
    fn next(&mut self) -> Result<Option<MiRecordV3>, Refusal> {
        loop {
            self.timely()?;
            let line = self.peer.next()?;
            self.timely()?;
            let Some(line) = line else { return Ok(None) };
            self.records = self.records.checked_add(1).ok_or(Refusal::Bound)?;
            self.bytes = self.bytes.checked_add(line.len()).ok_or(Refusal::Bound)?;
            if line.is_empty()
                || line.len() > crate::MAX_LINE
                || self.records > crate::MAX_RECORDS
                || self.bytes > crate::MAX_BYTES
            {
                return Err(Refusal::Bound);
            }
            if line.starts_with(b"{") {
                self.output(&line)?;
                self.timely()?;
                continue;
            }
            let record = syntax::record(&line)?;
            self.timely()?;
            return Ok(Some(record));
        }
    }
    fn output(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        if self.stage != 4
            || self.transitions.phase < 9
            || bytes.len() > 64 * 1024 - self.producer.len()
        {
            return Err(Refusal::State);
        }
        self.producer.extend_from_slice(bytes);
        Ok(())
    }
    fn background(&mut self, r: MiRecordV3) -> Result<(), Refusal> {
        match r {
            MiRecordV3::Prompt => Ok(()),
            MiRecordV3::Stream {
                kind: MiStreamKindV3::Target,
                bytes,
            } => self.output(&bytes),
            // Bounded console/log observations are not predicates or commands.
            MiRecordV3::Stream {
                kind: MiStreamKindV3::Console | MiStreamKindV3::Log,
                ..
            } => Ok(()),
            MiRecordV3::Async {
                token: None,
                kind: MiAsyncKindV3::Notify,
                class,
                results,
            } => self.notification(&class, &results),
            MiRecordV3::Async {
                token: None,
                kind: MiAsyncKindV3::Exec,
                class,
                results,
            } => match class.as_str() {
                "running" => {
                    fields(&results, &["thread-id"], &["thread-id"])?;
                    let v = text(&results, "thread-id")?;
                    let episode_live = self.run_sent
                        && match self.stage {
                            0 => !self.entry_stop,
                            2 => !self.host_stop,
                            3 => !self.gpu_stop,
                            4 => {
                                !self.normal_exit
                                    && !self.group_exited
                                    && !self.host_exited
                                    && !self.gpu_exited
                            }
                            _ => false,
                        };
                    if !episode_live
                        || (v != b"all"
                            && Some(positive(v)?) != self.host
                            && Some(positive(v)?) != self.gpu)
                    {
                        return Err(Refusal::State);
                    }
                    Ok(())
                }
                "stopped" => self.stop(&results),
                _ => Err(Refusal::Stop),
            },
            _ => Err(Refusal::Shape),
        }
    }
    fn notification(&mut self, class: &str, r: &MiResultsV3) -> Result<(), Refusal> {
        match class {
            "fe2o3-owned-one-stop-v2" => {
                self.transitions.row(
                    r,
                    (
                        self.pid.ok_or(Refusal::Process)?,
                        self.host.ok_or(Refusal::Process)?,
                        self.inferior_start.ok_or(Refusal::Process)?,
                    ),
                    self.stage,
                    self.host_stop,
                    self.gpu_stop,
                )?;
                if self.transitions.phase == 8 {
                    if self.physical.is_some() {
                        return Err(Refusal::Duplicate);
                    }
                    self.physical = Some(crate::physical_snapshot_v2::parse(
                        r,
                        self.gpu.ok_or(Refusal::Stop)?,
                        self.gpu_pc.ok_or(Refusal::Stop)?,
                    )?);
                    self.timely()?;
                }
            }
            "amd-runtime-observation-v1" | "amd-stopped-wave-observation-v1" => {
                // Exact old grammar only. Never substituted for new transitions.
                self.transitions.legacy(class, r)?;
            }
            "cmd-param-changed" => {
                fields(r, &["param", "value"], &["param", "value"])?;
                const PARAMS: [&[u8]; 5] = [
                    b"auto-load gdb-scripts",
                    b"auto-load libthread-db",
                    b"auto-load local-gdbinit",
                    b"auto-load python-scripts",
                    b"startup-with-shell",
                ];
                if self.stage != 0 || self.run_sent || text(r, "value")? != b"off" {
                    return Err(Refusal::State);
                }
                let n = PARAMS
                    .iter()
                    .position(|p| *p == text(r, "param").unwrap_or(b""))
                    .ok_or(Refusal::Shape)?;
                if self.settings[n] {
                    return Err(Refusal::Duplicate);
                }
                self.settings[n] = true;
            }
            "thread-group-added" => {
                fields(r, &["id"], &["id"])?;
                if self.stage != 0 || self.group_added || text(r, "id")? != b"i1" {
                    return Err(Refusal::Process);
                }
                self.group_added = true;
            }
            "thread-group-started" => {
                fields(r, &["id", "pid"], &["id", "pid"])?;
                if !self.run_sent
                    || self.stage != 0
                    || !self.group_added
                    || self.pid.is_some()
                    || text(r, "id")? != b"i1"
                {
                    return Err(Refusal::Process);
                }
                let pid = u32::try_from(positive(text(r, "pid")?)?).map_err(|_| Refusal::Bound)?;
                self.peer.observe_child(pid)?;
                self.timely()?;
                self.pid = Some(pid);
            }
            "thread-created" => {
                fields(r, &["id", "group-id"], &["id", "group-id"])?;
                if self.pid.is_none() || text(r, "group-id")? != b"i1" {
                    return Err(Refusal::Process);
                }
                let id = positive(text(r, "id")?)?;
                if self.stage == 0 && self.host.is_none() {
                    self.host = Some(id);
                } else if self.stage == 3 && self.gpu.is_none() && self.host != Some(id) {
                    self.gpu = Some(id);
                } else {
                    return Err(Refusal::Process);
                }
            }
            "thread-selected" => {
                fields(r, &["id", "frame"], &["id"])?;
                let id = positive(text(r, "id")?)?;
                if self.host != Some(id) && self.gpu != Some(id) {
                    return Err(Refusal::Process);
                }
                if r.contains_key("frame") {
                    syntax::frame(r, None)?;
                }
            }
            "thread-exited" => {
                fields(r, &["id", "group-id"], &["id", "group-id"])?;
                if self.stage != 4 || self.transitions.phase < 9 || text(r, "group-id")? != b"i1" {
                    return Err(Refusal::Exit);
                }
                let id = positive(text(r, "id")?)?;
                if self.host == Some(id) && !self.host_exited {
                    self.host_exited = true;
                } else if self.gpu == Some(id) && !self.gpu_exited {
                    self.gpu_exited = true;
                } else {
                    return Err(Refusal::Process);
                }
            }
            "thread-group-exited" => {
                fields(r, &["id", "exit-code"], &["id", "exit-code"])?;
                if self.stage != 4
                    || self.group_exited
                    || text(r, "id")? != b"i1"
                    || !matches!(text(r, "exit-code")?, b"0" | b"00")
                {
                    return Err(Refusal::Exit);
                }
                self.group_exited = true;
            }
            "thread-group-removed" => {
                fields(r, &["id"], &["id"])?;
                if self.stage != 5
                    || self.group_removed
                    || !self.group_exited
                    || text(r, "id")? != b"i1"
                {
                    return Err(Refusal::Exit);
                }
                self.group_removed = true;
            }
            "breakpoint-deleted" => {
                fields(r, &["id"], &["id"])?;
                // Pinned MI suppresses all nonpositive breakpoint notifications.
                // Only the actual temporary user entry breakpoint appears here.
                let raw = text(r, "id")?;
                if self.stage == 0
                    && self.entry_stop
                    && !self.entry_deleted
                    && Some(positive(raw)?) == self.entry_break
                {
                    self.entry_deleted = true;
                } else {
                    return Err(Refusal::Stop);
                }
            }
            "breakpoint-modified" => {
                fields(r, &["bkpt"], &["bkpt"])?;
                let b = r
                    .get("bkpt")
                    .and_then(MiValueV3::as_tuple)
                    .ok_or(Refusal::Shape)?;
                if self.stage != 0
                    || !self.run_sent
                    || self.entry_deleted
                    || Some(positive(text(b, "number")?)?) != self.entry_break
                    || text(b, "disp")? != b"del"
                    || text(b, "type")? != b"breakpoint"
                {
                    return Err(Refusal::Stop);
                }
                if Some(syntax::entry_breakpoint(r, 1)?) != self.entry_break {
                    return Err(Refusal::Stop);
                }
                // Bounded exact entry presentation only; stop and deletion remain required.
            }
            "library-loaded" | "library-unloaded" => {
                fields(
                    r,
                    &[
                        "id",
                        "target-name",
                        "host-name",
                        "symbols-loaded",
                        "thread-group",
                        "ranges",
                    ],
                    &["id", "target-name", "host-name"],
                )?;
                for key in ["id", "target-name", "host-name"] {
                    let v = text(r, key)?;
                    if v.is_empty() || v.len() > 512 || v.iter().any(|b| b.is_ascii_control()) {
                        return Err(Refusal::Shape);
                    }
                }
                if let Some(g) = r.get("thread-group")
                    && g.as_const() != Some(b"i1")
                {
                    return Err(Refusal::Process);
                }
                // No object identity, origin, closure or code-byte authority.
            }
            _ => return Err(Refusal::Shape),
        }
        Ok(())
    }
    fn stop(&mut self, r: &MiResultsV3) -> Result<(), Refusal> {
        if self.stage == 4 {
            fields(r, &["reason"], &["reason"])?;
            if self.normal_exit || text(r, "reason")? != b"exited-normally" {
                return Err(Refusal::Exit);
            }
            self.normal_exit = true;
            return Ok(());
        }
        fields(
            r,
            &[
                "reason",
                "disp",
                "bkptno",
                "frame",
                "thread-id",
                "stopped-threads",
                "core",
                "signal-name",
                "signal-meaning",
                "lane-id",
            ],
            &["reason", "frame", "thread-id", "stopped-threads"],
        )?;
        if text(r, "stopped-threads")? != b"all" {
            return Err(Refusal::Stop);
        }
        let thread = positive(text(r, "thread-id")?)?;
        match self.stage {
            0 if self.run_sent && !self.entry_stop => {
                if Some(thread) != self.host
                    || text(r, "reason")? != b"breakpoint-hit"
                    || text(r, "disp")? != b"del"
                    || Some(positive(text(r, "bkptno")?)?) != self.entry_break
                    || r.contains_key("signal-name")
                    || r.contains_key("lane-id")
                {
                    return Err(Refusal::Stop);
                }
                syntax::frame(r, Some(crate::ENTRY))?;
                self.entry_stop = true;
            }
            2 if !self.host_stop => {
                if Some(thread) != self.host
                    || text(r, "reason")? != b"breakpoint-hit"
                    || text(r, "disp")? != b"del"
                    || !text(r, "bkptno")?.starts_with(b"-")
                    || r.contains_key("signal-name")
                    || r.contains_key("lane-id")
                {
                    return Err(Refusal::Stop);
                }
                let raw = text(r, "bkptno")?;
                syntax::signed(raw)?;
                self.checkpoint_id = Some(
                    std::str::from_utf8(raw)
                        .map_err(|_| Refusal::Shape)?
                        .parse::<i64>()
                        .map_err(|_| Refusal::Bound)?,
                );
                syntax::frame(r, Some(crate::CHECKPOINT))?;
                self.host_stop = true;
            }
            3 if !self.gpu_stop => {
                if Some(thread) != self.gpu
                    || self.gpu == self.host
                    || text(r, "reason")? != b"signal-received"
                    || text(r, "signal-name")? != b"SIGTRAP"
                    || r.contains_key("bkptno")
                    || r.contains_key("disp")
                {
                    return Err(Refusal::Stop);
                }
                if decimal(text(r, "lane-id")?)? >= 64 {
                    return Err(Refusal::Stop);
                }
                syntax::frame(r, None)?;
                let frame = r
                    .get("frame")
                    .and_then(MiValueV3::as_tuple)
                    .ok_or(Refusal::Stop)?;
                self.gpu_pc = Some(syntax::address(text(frame, "addr")?)?);
                self.gpu_stop = true;
            }
            _ => return Err(Refusal::Stop),
        }
        Ok(())
    }
    fn prompt(&mut self) -> Result<(), Refusal> {
        loop {
            let r = self.next()?.ok_or(Refusal::Incomplete)?;
            if r == MiRecordV3::Prompt {
                return Ok(());
            }
            self.background(r)?;
        }
    }
    fn command(&mut self, command: &str, expected: &str) -> Result<MiResultsV3, Refusal> {
        self.timely()?;
        if self.token == crate::MAX_COMMANDS
            || command.len() > 2048
            || command.contains(['\n', '\r'])
            || (self.stage >= 1
                && command != "-exec-continue"
                && command != "-gdb-exit"
                && !(self.stage == 1
                    && command == "-interpreter-exec console \"fe2o3-one-stop-select-v1\""))
        {
            return Err(Refusal::State);
        }
        self.token += 1;
        self.peer.send(self.token, command)?;
        self.timely()?;
        loop {
            let r = self.next()?.ok_or(Refusal::Incomplete)?;
            if let MiRecordV3::Result {
                token,
                class,
                results,
            } = r
            {
                if token != Some(self.token) {
                    return Err(Refusal::Token);
                }
                if class != expected {
                    return Err(Refusal::State);
                }
                if expected != "exit" {
                    self.prompt()?;
                }
                return Ok(results);
            }
            self.background(r)?;
        }
    }
    fn empty(&mut self, command: &str, expected: &str) -> Result<(), Refusal> {
        if !self.command(command, expected)?.is_empty() {
            return Err(Refusal::Shape);
        }
        Ok(())
    }
    fn asynchronous_record(&mut self) -> Result<(), Refusal> {
        // In mi-async mode, normal-stop and post-retirement observer records
        // do not end with a second prompt. Process one real record at a time.
        let record = self.next()?.ok_or(Refusal::Incomplete)?;
        self.background(record)?;
        self.timely()
    }
    fn witness(&mut self, phase: u64) -> Result<(), Refusal> {
        loop {
            self.timely()?;
            let ready = match phase {
                0 => self.entry_stop && self.entry_deleted,
                4 => self.host_stop && self.transitions.phase == 4,
                8 => self.gpu_stop && self.transitions.phase == 8 && self.physical.is_some(),
                _ => return Err(Refusal::State),
            };
            if ready {
                return Ok(());
            }
            // A prompt is neither a stop nor proof of breakpoint retirement.
            // The startup and command-result prompt barriers remain separate.
            self.asynchronous_record()?;
        }
    }
    fn current(&mut self) -> Result<(), Refusal> {
        self.timely()?;
        self.peer.current()?;
        self.timely()
    }
    fn execute(&mut self) -> Result<(), Refusal> {
        self.timely()?;
        self.producer
            .try_reserve_exact(64 * 1024)
            .map_err(|_| Refusal::Bound)?;
        if self.producer.capacity() > 64 * 1024 {
            return Err(Refusal::Bound);
        }
        self.prompt()?;
        for command in [
            "-gdb-set pagination off",
            "-gdb-set confirm off",
            "-gdb-set mi-async on",
            "-gdb-set follow-fork-mode parent",
            "-gdb-set detach-on-fork off",
            "-interpreter-exec console \"unset environment\"",
            "-gdb-set environment LANG=C",
            "-gdb-set environment LC_ALL=C",
        ] {
            self.empty(command, "done")?;
        }
        if self.settings != [true; 5] || !self.group_added {
            return Err(Refusal::State);
        }
        self.empty(
            &format!("-file-exec-and-symbols \"{}\"", crate::TARGET_PATH),
            "done",
        )?;
        self.empty(
            "-exec-arguments --acknowledge-fixed-one-stop-observer-unqualified",
            "done",
        )?;
        let result = self.command(&format!("-break-insert -t {}", crate::ENTRY), "done")?;
        self.entry_break = Some(syntax::entry_breakpoint(&result, 0)?);
        self.run_sent = true;
        self.empty("-exec-run", "running")?;
        self.witness(0)?;
        self.peer.entry()?;
        self.timely()?;
        let start = self.peer.observed_inferior_start()?;
        self.timely()?;
        if start == 0 {
            return Err(Refusal::Process);
        }
        self.inferior_start = Some(start);
        self.current()?;
        self.stage = 1;
        self.empty(
            "-interpreter-exec console \"fe2o3-one-stop-select-v1\"",
            "done",
        )?;
        if self.transitions.phase != 1 {
            return Err(Refusal::State);
        }
        self.current()?;
        self.stage = 2;
        self.empty("-exec-continue", "running")?;
        self.witness(4)?;
        self.current()?;
        self.stage = 3;
        self.empty("-exec-continue", "running")?;
        self.witness(8)?;
        self.current()?;
        self.stage = 4;
        self.empty("-exec-continue", "running")?;
        while !(self.normal_exit
            && self.group_exited
            && self.host_exited
            && self.gpu_exited
            && self.transitions.phase == 11)
        {
            self.asynchronous_record()?;
        }
        crate::target_report::check(&self.producer, self.pid.ok_or(Refusal::Process)?)?;
        self.timely()?;
        self.stage = 5;
        self.empty("-gdb-exit", "exit")?;
        while let Some(r) = self.next()? {
            self.background(r)?;
        }
        self.timely()
    }
}
/// Observe the closed sequence through a supplied owned transport. This function
/// does not launch or select a native process by itself. It cannot turn a mock,
/// retained transcript or caller-supplied peer into source/process authority.
/// A future executable must bind the reviewed native Peer and fresh MI2 closure.
pub fn observe<P: Peer>(peer: &mut P) -> Result<ProtocolObservation, FailedObservation> {
    let mut s = Session::new(peer);
    if let Err(refusal) = s.execute() {
        let cleanup = s.peer.cleanup();
        return Err(FailedObservation { refusal, cleanup });
    }
    let cleanup = match s.peer.finish() {
        Ok(c) => c,
        Err(refusal) => {
            return Err(FailedObservation {
                refusal,
                cleanup: s.peer.cleanup(),
            });
        }
    };
    // Do not repeat teardown after finish succeeded: retain actual facts while
    // refusing late acceptance. Caller publication must recheck the SAME clock.
    if let Err(refusal) = s.timely() {
        return Err(FailedObservation { refusal, cleanup });
    }
    if !cleanup.complete() {
        return Err(FailedObservation {
            refusal: Refusal::Incomplete,
            cleanup,
        });
    }
    let physical = s.physical.take().ok_or(FailedObservation {
        refusal: Refusal::Incomplete,
        cleanup,
    })?;
    let captured = physical.captured();
    let observation = ProtocolObservation {
        schema: "fe2o3-gfx950-one-stop-controller-observation-v2",
        process_id: s.pid.ok_or(FailedObservation {
            refusal: Refusal::Process,
            cleanup,
        })?,
        producer_process_id: s.transitions.process.to_string(),
        producer_rows: s.transitions.count,
        observed_host_thread: s.host.unwrap_or(0).to_string(),
        observed_gpu_thread: s.gpu.unwrap_or(0).to_string(),
        observed_internal_checkpoint_id: s.checkpoint_id.unwrap_or(0).to_string(),
        same_producer_transition_sequence_observed: true,
        independent_target_local_completion_observed: true,
        normal_mi_exit_observed: true,
        independent_native_tuple_replay: false,
        physical_register_capture: captured,
        physical_memory_capture: captured,
        physical_snapshot: physical,
        later_target_4096_output_validation_observed: true,
        post_completion_raw_bytes_recaptured: false,
        process_control_authority: false,
        source_or_launch_authority: false,
        whole_family_cleanup_proved: false,
        general_host_exclusion_proved: false,
        operational_qualification: false,
        cleanup,
    };
    if let Err(refusal) = s.timely() {
        return Err(FailedObservation { refusal, cleanup });
    }
    Ok(observation)
}
