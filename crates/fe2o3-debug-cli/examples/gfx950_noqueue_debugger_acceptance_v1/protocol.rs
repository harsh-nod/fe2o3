//! Closed single-inferior host rendezvous protocol shared by native and fake peers.
//! A transcript is never a process capability or a physical GPU sample.
use super::config::{decimal, hash, quote};
use super::wire::Cleanup;
use crate::parser::{self, MemoryObject, Refusal};
use crate::rocgdb_mi_parser_v3::{
    MiAsyncKindV3, MiRecordV3, MiResultsV3, MiStreamKindV3, MiValueV3,
};
use serde::Serialize;

pub(super) trait Peer {
    fn send(&mut self, token: u64, command: &str) -> Result<(), Refusal>;
    fn next(&mut self) -> Result<Option<Vec<u8>>, Refusal>;
    fn observe_child(&mut self, pid: u32) -> Result<(), Refusal>;
    fn entry(&mut self) -> Result<(), Refusal>;
    fn current(&mut self) -> Result<(), Refusal>;
    fn finish(&mut self) -> Result<Cleanup, Refusal>;
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Setup,
    ToEntry,
    Entry,
    ToCold,
    Cold,
    ToPublished,
    Published,
    Exiting,
    Exited,
}
#[derive(Serialize)]
pub(super) struct Observation {
    pub(super) process_id: u32,
    pub(super) actual_entry_executable_and_start_identity_checked: bool,
    pub(super) pre_activation_code_object_absent: bool,
    pub(super) post_publication_code_object_present: bool,
    pub(super) same_host_stop_original_elf_content_matched: bool,
    pub(super) producer_registration_record_matched: bool,
    pub(super) runtime_loaded_success_observed: bool,
    pub(super) debugger_acceptance_observed: bool,
    pub(super) physical_register_capture: bool,
    pub(super) queue_created: bool,
    pub(super) kernel_dispatched: bool,
    pub(super) process_control_authority: bool,
    pub(super) source_or_launch_authority: bool,
    pub(super) cleanup: Cleanup,
}
fn text<'a>(r: &'a MiResultsV3, k: &str) -> Result<&'a [u8], Refusal> {
    r.get(k).and_then(MiValueV3::as_const).ok_or(Refusal::Shape)
}
fn fields(r: &MiResultsV3, allowed: &[&str], required: &[&str]) -> Result<(), Refusal> {
    if r.keys().any(|k| !allowed.contains(&k.as_str()))
        || required.iter().any(|k| !r.contains_key(*k))
    {
        return Err(Refusal::Shape);
    }
    Ok(())
}
fn id(raw: &[u8]) -> Result<Vec<u8>, Refusal> {
    if decimal(raw)? == 0 || raw.len() > 10 {
        return Err(Refusal::Shape);
    }
    Ok(raw.to_vec())
}
pub(super) fn breakpoint(r: &MiResultsV3, symbol: &str) -> Result<Vec<u8>, Refusal> {
    fields(r, &["bkpt"], &["bkpt"])?;
    let b = r
        .get("bkpt")
        .and_then(MiValueV3::as_tuple)
        .ok_or(Refusal::Shape)?;
    fields(
        b,
        &[
            "number",
            "type",
            "disp",
            "enabled",
            "addr",
            "func",
            "file",
            "fullname",
            "line",
            "thread-groups",
            "times",
            "original-location",
        ],
        &[
            "number",
            "type",
            "disp",
            "enabled",
            "addr",
            "func",
            "times",
            "original-location",
        ],
    )?;
    if text(b, "type")? != b"breakpoint"
        || text(b, "disp")? != b"keep"
        || text(b, "enabled")? != b"y"
        || text(b, "func")? != symbol.as_bytes()
        || text(b, "original-location")? != symbol.as_bytes()
    {
        return Err(Refusal::Stop);
    }
    let address = text(b, "addr")?.strip_prefix(b"0x").ok_or(Refusal::Stop)?;
    if address.is_empty()
        || address.len() > 16
        || !address.iter().all(u8::is_ascii_hexdigit)
        || u64::from_str_radix(
            std::str::from_utf8(address).map_err(|_| Refusal::Shape)?,
            16,
        )
        .map_err(|_| Refusal::Shape)?
            == 0
    {
        return Err(Refusal::Stop);
    }
    if decimal(text(b, "times")?)? > 1 {
        return Err(Refusal::Stop);
    }
    if let Some(groups) = b.get("thread-groups") {
        let groups = groups.as_values().ok_or(Refusal::Shape)?;
        if groups.len() != 1 || groups[0].as_const() != Some(b"i1") {
            return Err(Refusal::Process);
        }
    }
    id(text(b, "number")?)
}
struct Session<'a, P: Peer> {
    peer: &'a mut P,
    phase: Phase,
    token: u64,
    pid: Option<u32>,
    thread: Option<Vec<u8>>,
    group_added: bool,
    startup_settings_seen: [bool; 5],
    group_exited: bool,
    thread_exited: bool,
    normal_exit: bool,
    breaks: Vec<(Vec<u8>, &'static str)>,
    stop: Option<Vec<u8>>,
    producer: Vec<u8>,
}
impl<'a, P: Peer> Session<'a, P> {
    fn new(peer: &'a mut P) -> Self {
        Self {
            peer,
            phase: Phase::Setup,
            token: 0,
            pid: None,
            thread: None,
            group_added: false,
            startup_settings_seen: [false; 5],
            group_exited: false,
            thread_exited: false,
            normal_exit: false,
            breaks: Vec::new(),
            stop: None,
            producer: Vec::new(),
        }
    }
    fn output(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        if self.phase != Phase::Exiting || self.producer.len() + bytes.len() > 4096 {
            return Err(Refusal::State);
        }
        self.producer.extend_from_slice(bytes);
        Ok(())
    }
    fn background(&mut self, line: Vec<u8>, record: MiRecordV3) -> Result<(), Refusal> {
        match record {
            MiRecordV3::Prompt => Ok(()),
            MiRecordV3::Stream {
                kind: MiStreamKindV3::Target,
                bytes,
            } => self.output(&bytes),
            MiRecordV3::Stream {
                kind: MiStreamKindV3::Console | MiStreamKindV3::Log,
                ..
            } => Ok(()),
            MiRecordV3::Async {
                token: None,
                kind: MiAsyncKindV3::Exec,
                class,
                results,
            } => match class.as_str() {
                "running" => {
                    fields(&results, &["thread-id"], &["thread-id"])?;
                    if !matches!(
                        self.phase,
                        Phase::ToEntry | Phase::ToCold | Phase::ToPublished | Phase::Exiting
                    ) || (text(&results, "thread-id")? != b"all"
                        && self.thread.as_deref() != Some(text(&results, "thread-id")?))
                    {
                        return Err(Refusal::State);
                    }
                    Ok(())
                }
                "stopped" if self.phase == Phase::Exiting => {
                    fields(&results, &["reason"], &["reason"])?;
                    if text(&results, "reason")? != b"exited-normally" || self.normal_exit {
                        return Err(Refusal::Exit);
                    }
                    self.normal_exit = true;
                    Ok(())
                }
                "stopped"
                    if matches!(
                        self.phase,
                        Phase::ToEntry | Phase::ToCold | Phase::ToPublished
                    ) =>
                {
                    if self.stop.replace(line).is_some() {
                        return Err(Refusal::Duplicate);
                    }
                    Ok(())
                }
                _ => Err(Refusal::Stop),
            },
            MiRecordV3::Async {
                token: None,
                kind: MiAsyncKindV3::Notify,
                class,
                results,
            } => self.notification(&class, &results),
            _ => Err(Refusal::Shape),
        }
    }
    fn notification(&mut self, class: &str, r: &MiResultsV3) -> Result<(), Refusal> {
        match class {
            "cmd-param-changed" => {
                fields(r, &["param", "value"], &["param", "value"])?;
                // Precisely the fixed -iex startup effects observed from the
                // pinned debugger; not a generic setting-notification sink.
                const PARAMETERS: [&[u8]; 5] = [
                    b"auto-load gdb-scripts",
                    b"auto-load libthread-db",
                    b"auto-load local-gdbinit",
                    b"auto-load python-scripts",
                    b"startup-with-shell",
                ];
                if self.phase != Phase::Setup || text(r, "value")? != b"off" {
                    return Err(Refusal::State);
                }
                let parameter = text(r, "param")?;
                let index = PARAMETERS
                    .iter()
                    .position(|known| *known == parameter)
                    .ok_or(Refusal::Shape)?;
                if self.startup_settings_seen[index] {
                    return Err(Refusal::Duplicate);
                }
                self.startup_settings_seen[index] = true;
            }
            "thread-group-added" => {
                fields(r, &["id"], &["id"])?;
                if text(r, "id")? != b"i1" || self.group_added {
                    return Err(Refusal::Duplicate);
                }
                self.group_added = true;
            }
            "thread-group-started" => {
                fields(r, &["id", "pid"], &["id", "pid"])?;
                if !self.group_added
                    || text(r, "id")? != b"i1"
                    || self.pid.is_some()
                    || self.phase != Phase::ToEntry
                {
                    return Err(Refusal::Process);
                }
                let pid = u32::try_from(decimal(text(r, "pid")?)?).map_err(|_| Refusal::Bound)?;
                if pid == 0 {
                    return Err(Refusal::Process);
                }
                self.peer.observe_child(pid)?;
                self.pid = Some(pid);
            }
            "thread-created" => {
                fields(r, &["id", "group-id"], &["id", "group-id"])?;
                if self.pid.is_none()
                    || text(r, "group-id")? != b"i1"
                    || self.thread.is_some()
                    || self.phase != Phase::ToEntry
                {
                    return Err(Refusal::Process);
                }
                self.thread = Some(id(text(r, "id")?)?);
            }
            "thread-selected" => {
                fields(r, &["id", "frame"], &["id"])?;
                if self.thread.as_deref() != Some(text(r, "id")?) {
                    return Err(Refusal::Process);
                }
            }
            "thread-exited" => {
                fields(r, &["id", "group-id"], &["id", "group-id"])?;
                if self.phase != Phase::Exiting
                    || self.thread_exited
                    || text(r, "group-id")? != b"i1"
                    || self.thread.as_deref() != Some(text(r, "id")?)
                {
                    return Err(Refusal::Process);
                }
                self.thread_exited = true;
            }
            "thread-group-exited" => {
                fields(r, &["id", "exit-code"], &["id", "exit-code"])?;
                if self.phase != Phase::Exiting
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
                if self.phase != Phase::Exited || text(r, "id")? != b"i1" || !self.group_exited {
                    return Err(Refusal::Exit);
                }
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
                if let Some(group) = r.get("thread-group")
                    && group.as_const() != Some(b"i1")
                {
                    return Err(Refusal::Process);
                }
                // Mere notification and symbols-loaded are deliberately not an
                // acceptance predicate. The stopped list and original bytes
                // must be queried independently later.
            }
            "breakpoint-modified" => {
                let number = r
                    .get("bkpt")
                    .and_then(MiValueV3::as_tuple)
                    .ok_or(Refusal::Shape)
                    .and_then(|v| text(v, "number"))?;
                let (_, symbol) = self
                    .breaks
                    .iter()
                    .find(|(id, _)| id == number)
                    .ok_or(Refusal::Stop)?;
                if breakpoint(r, symbol)? != number {
                    return Err(Refusal::Stop);
                }
            }
            _ => return Err(Refusal::Shape),
        }
        Ok(())
    }
    fn next(&mut self) -> Result<Option<(Vec<u8>, MiRecordV3)>, Refusal> {
        loop {
            let Some(line) = self.peer.next()? else {
                return Ok(None);
            };
            if line.starts_with(b"{") {
                self.output(&line)?;
                continue;
            }
            let value = parser::record(&line)?;
            return Ok(Some((line, value)));
        }
    }
    fn command(
        &mut self,
        command: &str,
        expected: &str,
    ) -> Result<(Vec<u8>, MiResultsV3, u64), Refusal> {
        self.token = self.token.checked_add(1).ok_or(Refusal::Bound)?;
        let token = self.token;
        self.peer.send(token, command)?;
        loop {
            let (line, value) = self.next()?.ok_or(Refusal::Incomplete)?;
            if let MiRecordV3::Result {
                token: actual,
                class,
                results,
            } = value
            {
                if actual != Some(token) {
                    return Err(Refusal::Token);
                }
                if class != expected {
                    return Err(Refusal::State);
                }
                return Ok((line, results, token));
            }
            self.background(line, value)?;
        }
    }
    fn empty(&mut self, command: &str, expected: &str) -> Result<(), Refusal> {
        if !self.command(command, expected)?.1.is_empty() {
            return Err(Refusal::Shape);
        }
        Ok(())
    }
    fn stop(&mut self, index: usize) -> Result<(), Refusal> {
        while self.stop.is_none() {
            let (line, value) = self.next()?.ok_or(Refusal::Incomplete)?;
            self.background(line, value)?;
        }
        let (number, symbol) = self.breaks.get(index).ok_or(Refusal::State)?;
        parser::parse_host_stop(
            &self.stop.take().ok_or(Refusal::Stop)?,
            number,
            self.thread.as_deref().ok_or(Refusal::Process)?,
            symbol,
        )?;
        if index == 0 {
            self.peer.entry()?;
            self.phase = Phase::Entry;
        } else {
            self.peer.current()?;
            self.phase = if index == 1 {
                Phase::Cold
            } else {
                Phase::Published
            };
        }
        Ok(())
    }
    fn list(&mut self, bytes: usize) -> Result<Option<MemoryObject>, Refusal> {
        self.peer.current()?;
        let (line, _, token) = self.command("-file-list-shared-libraries", "done")?;
        self.peer.current()?;
        parser::parse_libraries(&line, token, self.pid.ok_or(Refusal::Process)?, bytes)
    }
}
pub(super) fn run<P: Peer>(
    peer: &mut P,
    observer: &str,
    arguments: &[String; 12],
) -> Result<Observation, Refusal> {
    super::config::validate_arguments(arguments)?;
    let bytes = usize::try_from(decimal(arguments[5].as_bytes())?).map_err(|_| Refusal::Bound)?;
    let sha = hash(&arguments[6])?;
    let mut s = Session::new(peer);
    // No attach, eval, arbitrary CLI, signal command, kernel breakpoint or
    // register command exists in this closed production flow.
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
        s.empty(command, "done")?;
    }
    s.empty(
        &format!("-file-exec-and-symbols {}", quote(observer)?),
        "done",
    )?;
    let args = arguments
        .iter()
        .map(|a| quote(a))
        .collect::<Result<Vec<_>, _>>()?
        .join(" ");
    s.empty(&format!("-exec-arguments {args}"), "done")?;
    for symbol in [
        parser::ENTRY_HOST_SYMBOL,
        parser::PRE_HOST_SYMBOL,
        parser::POST_HOST_SYMBOL,
    ] {
        let (_, result, _) = s.command(&format!("-break-insert {symbol}"), "done")?;
        let number = breakpoint(&result, symbol)?;
        if s.breaks.iter().any(|(n, _)| *n == number) {
            return Err(Refusal::Duplicate);
        }
        s.breaks.push((number, symbol));
    }
    s.phase = Phase::ToEntry;
    s.empty("-exec-run", "running")?;
    s.stop(0)?;
    // This is the only edge permitting the observer to run its KFD/VM setup.
    s.peer.current()?;
    s.phase = Phase::ToCold;
    s.empty("-exec-continue", "running")?;
    s.stop(1)?;
    if s.list(bytes)?.is_some() {
        return Err(Refusal::State);
    }
    // Native activation is attempted once. A refusal never retries it.
    s.peer.current()?;
    s.phase = Phase::ToPublished;
    s.empty("-exec-continue", "running")?;
    s.stop(2)?;
    let selected = s.list(bytes)?.ok_or(Refusal::Artifact)?;
    s.peer.current()?;
    let (line, _, token) = s.command(
        &format!(
            "-data-read-memory-bytes 0x{:x} {}",
            selected.address, selected.bytes
        ),
        "done",
    )?;
    s.peer.current()?;
    parser::parse_original_elf(&line, token, selected, sha)?;
    s.phase = Phase::Exiting;
    s.empty("-exec-continue", "running")?;
    while !(s.group_exited && s.thread_exited && s.normal_exit) {
        let (line, value) = s.next()?.ok_or(Refusal::Incomplete)?;
        s.background(line, value)?;
    }
    check_producer(&s.producer, s.pid.ok_or(Refusal::Process)?, arguments)?;
    s.phase = Phase::Exited;
    s.empty("-gdb-exit", "exit")?;
    while let Some((line, value)) = s.next()? {
        s.background(line, value)?;
    }
    let cleanup = s.peer.finish()?;
    if !cleanup.owned_inferior_pidfd_exit_observed
        || !cleanup.debugger_direct_child_reaped
        || !cleanup.streams_complete
        || !cleanup.reader_threads_joined
        || cleanup.unadmitted_inferior_cleanup_not_proven
        || cleanup.inferior_reaped_by_controller
    {
        return Err(Refusal::Incomplete);
    }
    Ok(Observation {
        process_id: s.pid.ok_or(Refusal::Process)?,
        actual_entry_executable_and_start_identity_checked: true,
        pre_activation_code_object_absent: true,
        post_publication_code_object_present: true,
        same_host_stop_original_elf_content_matched: true,
        producer_registration_record_matched: true,
        runtime_loaded_success_observed: false,
        debugger_acceptance_observed: false,
        physical_register_capture: false,
        queue_created: false,
        kernel_dispatched: false,
        process_control_authority: false,
        source_or_launch_authority: false,
        cleanup,
    })
}
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct Producer {
    schema: String,
    status: String,
    observation: ProducerFacts,
}
#[derive(serde::Deserialize, serde::Serialize)]
#[serde(deny_unknown_fields)]
struct ProducerFacts {
    process_id: u32,
    process_entry_snapshot_checked: bool,
    preactivation_snapshot_checked: bool,
    proc_snapshot_proves_general_foreign_exclusion: bool,
    caller_contract: String,
    target: String,
    wave_width: u32,
    node: u32,
    unique_id: String,
    gpu_id: u32,
    device_profile_sha256: String,
    artifact_sha256: String,
    artifact_bytes: String,
    selected_kernel: String,
    trap_sha256: String,
    trap_bytes: usize,
    mapped_backing_bytes: String,
    metadata_retained_bytes: String,
    file_snapshot_rechecked: bool,
    native_vm_and_mappings_prepared: bool,
    retention: String,
    metadata_version: u32,
    metadata_published: bool,
    trap_registered: bool,
    debug_runtime_enabled: bool,
    queue_created: bool,
    kernel_dispatched: bool,
    gpu_trap_execution_qualified: bool,
    cleanup_acknowledged: bool,
    host_process_entry_rendezvous_reached: bool,
    host_pre_activation_rendezvous_reached: bool,
    host_post_publication_rendezvous_reached: bool,
    debugger_acceptance_observed: bool,
    physical_register_capture: bool,
    authority: String,
}

pub(super) fn check_producer(raw: &[u8], pid: u32, a: &[String; 12]) -> Result<(), Refusal> {
    if raw.is_empty()
        || raw.len() > 4096
        || raw.last() != Some(&b'\n')
        || raw[..raw.len() - 1].contains(&b'\n')
    {
        return Err(Refusal::Shape);
    }
    let typed: Producer = serde_json::from_slice(raw).map_err(|_| Refusal::Syntax)?;
    let v = serde_json::to_value(typed).map_err(|_| Refusal::Syntax)?;
    let root = v.as_object().ok_or(Refusal::Shape)?;
    if root.len() != 3
        || v["schema"] != "diagnostic-gfx950-debug-acceptance-noqueue-producer-v1"
        || v["status"] != "registered_no_queue_host_rendezvous"
    {
        return Err(Refusal::Shape);
    }
    let o = v["observation"].as_object().ok_or(Refusal::Shape)?;
    // Exact schema from the pinned sibling producer. Unknown fields are not
    // future capabilities, and its process field does not create custody.
    const KEYS: &[&str] = &[
        "process_id",
        "process_entry_snapshot_checked",
        "preactivation_snapshot_checked",
        "proc_snapshot_proves_general_foreign_exclusion",
        "caller_contract",
        "target",
        "wave_width",
        "node",
        "unique_id",
        "gpu_id",
        "device_profile_sha256",
        "artifact_sha256",
        "artifact_bytes",
        "selected_kernel",
        "trap_sha256",
        "trap_bytes",
        "mapped_backing_bytes",
        "metadata_retained_bytes",
        "file_snapshot_rechecked",
        "native_vm_and_mappings_prepared",
        "retention",
        "metadata_version",
        "metadata_published",
        "trap_registered",
        "debug_runtime_enabled",
        "queue_created",
        "kernel_dispatched",
        "gpu_trap_execution_qualified",
        "cleanup_acknowledged",
        "host_process_entry_rendezvous_reached",
        "host_pre_activation_rendezvous_reached",
        "host_post_publication_rendezvous_reached",
        "debugger_acceptance_observed",
        "physical_register_capture",
        "authority",
    ];
    if o.len() != KEYS.len() || o.keys().any(|k| !KEYS.contains(&k.as_str())) {
        return Err(Refusal::Shape);
    }
    let o = &v["observation"];
    if o["process_id"].as_u64() != Some(u64::from(pid))
        || o["target"] != "gfx950:xnack-"
        || o["wave_width"] != 64
        || o["node"].as_u64() != Some(decimal(a[8].as_bytes())?)
        || o["unique_id"] != a[9]
        || o["gpu_id"].as_u64() != Some(decimal(a[10].as_bytes())?)
        || o["device_profile_sha256"] != a[11]
        || o["artifact_sha256"] != a[6]
        || o["artifact_bytes"] != a[5]
        || o["selected_kernel"] != a[7]
        || o["metadata_version"] != 11
    {
        return Err(Refusal::Artifact);
    }
    if o["caller_contract"]
        != "audited_standalone_program_and_supervisor_process_lifetime_exclusion"
        || o["retention"] != "native_resources_retained_until_process_exit"
        || o["authority"] != "registration_facts_only_no_queue_stop_or_launch_authority"
    {
        return Err(Refusal::Shape);
    }
    hash(o["trap_sha256"].as_str().ok_or(Refusal::Shape)?)?;
    if o["trap_bytes"].as_u64().filter(|n| *n > 0).is_none()
        || decimal(
            o["mapped_backing_bytes"]
                .as_str()
                .ok_or(Refusal::Shape)?
                .as_bytes(),
        )? == 0
        || decimal(
            o["metadata_retained_bytes"]
                .as_str()
                .ok_or(Refusal::Shape)?
                .as_bytes(),
        )? == 0
    {
        return Err(Refusal::Bound);
    }
    for key in [
        "process_entry_snapshot_checked",
        "preactivation_snapshot_checked",
        "file_snapshot_rechecked",
        "native_vm_and_mappings_prepared",
        "metadata_published",
        "trap_registered",
        "debug_runtime_enabled",
        "host_process_entry_rendezvous_reached",
        "host_pre_activation_rendezvous_reached",
        "host_post_publication_rendezvous_reached",
    ] {
        if o[key] != true {
            return Err(Refusal::State);
        }
    }
    for key in [
        "proc_snapshot_proves_general_foreign_exclusion",
        "queue_created",
        "kernel_dispatched",
        "gpu_trap_execution_qualified",
        "cleanup_acknowledged",
        "debugger_acceptance_observed",
        "physical_register_capture",
    ] {
        if o[key] != false {
            return Err(Refusal::State);
        }
    }
    Ok(())
}
