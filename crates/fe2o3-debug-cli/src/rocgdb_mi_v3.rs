//! Structured debugger-side ROCgdb GDB/MI adapter.
//!
//! ROCgdb complements direct KFD with stopped-state inspection and authorized
//! execution control. It is never runtime, queue, exception, or artifact
//! authority. Native MI identifiers, paths, addresses, and process handles are
//! kept private and are mapped to sanitized protocol records.
//!
//! ROCgdb and a direct-KFD debug-trap session must not simultaneously own stop
//! control for one process. A coordinator selects one stop-control backend;
//! direct-KFD runtime and queue telemetry remain usable when no second
//! debug-trap session claims that exclusive authority.
//!
//! This low-level substrate binds structured MI thread identifiers for process
//! control. It does not classify those threads as GPU waves from human-oriented
//! thread metadata. Stopped GPU semantics remain unavailable until a separate
//! trusted correlation source authenticates a GPU thread binding.

mod process;

use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::fmt;
use std::os::unix::ffi::OsStrExt;

use fe2o3_debug_protocol::*;
use sha2::{Digest, Sha256};

use crate::rocgdb_mi_parser_v3::{
    MAX_MI_LINE_BYTES_V3, MiAsyncKindV3, MiListV3, MiParseErrorV3, MiParserLimitsV3, MiRecordV3,
    MiResultsV3, MiValueV3, parse_mi_record_v3,
};

pub use process::RocgdbMiProcessV3;

const MAX_RECORDS_PER_COMMAND_V3: usize = 4_096;
const MAX_PENDING_EVENTS_V3: usize = 256;
const MAX_PRIVATE_BINDINGS_V3: usize = 4_096;
const MAX_NATIVE_TOKEN_BYTES_V3: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RocgdbMiAdapterLimitsV3 {
    pub max_line_bytes: usize,
    pub max_records_per_command: usize,
    pub max_pending_events: usize,
    pub max_private_bindings: usize,
}

impl Default for RocgdbMiAdapterLimitsV3 {
    fn default() -> Self {
        Self {
            max_line_bytes: MAX_MI_LINE_BYTES_V3,
            max_records_per_command: MAX_RECORDS_PER_COMMAND_V3,
            max_pending_events: MAX_PENDING_EVENTS_V3,
            max_private_bindings: MAX_PRIVATE_BINDINGS_V3,
        }
    }
}

impl RocgdbMiAdapterLimitsV3 {
    pub fn validate(self) -> Result<(), RocgdbMiAdapterErrorV3> {
        if self.max_line_bytes == 0 || self.max_line_bytes > MAX_MI_LINE_BYTES_V3 {
            return Err(RocgdbMiAdapterErrorV3::LimitOutOfRange("line"));
        }
        if self.max_records_per_command == 0
            || self.max_records_per_command > MAX_RECORDS_PER_COMMAND_V3
        {
            return Err(RocgdbMiAdapterErrorV3::LimitOutOfRange("records"));
        }
        if self.max_pending_events == 0 || self.max_pending_events > MAX_PENDING_EVENTS_V3 {
            return Err(RocgdbMiAdapterErrorV3::LimitOutOfRange("events"));
        }
        if self.max_private_bindings == 0 || self.max_private_bindings > MAX_PRIVATE_BINDINGS_V3 {
            return Err(RocgdbMiAdapterErrorV3::LimitOutOfRange("bindings"));
        }
        Ok(())
    }

    fn parser(self) -> MiParserLimitsV3 {
        MiParserLimitsV3 {
            max_line_bytes: self.max_line_bytes,
            ..MiParserLimitsV3::default()
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RocgdbMiAdapterErrorV3 {
    LimitOutOfRange(&'static str),
    InvalidWaveWidth,
    InvalidTimeout,
    InvalidCommand,
    InvalidMiRecord,
    UnexpectedMiRecord,
    MissingField(&'static str),
    InvalidField(&'static str),
    CountOutOfRange(&'static str),
    DuplicateBinding,
    IdentityCollision,
    UnknownThread,
    UnknownAllocation,
    UnknownCodeObject,
    UnknownSource,
    StaleRevision,
    SessionNotStopped,
    GpuClassificationUnavailable,
    ProcessSpawn,
    ProcessIo,
    ProcessExited,
    Timeout,
    ResponseBudgetExhausted,
    EventBudgetExhausted,
    BackendRejected,
    ProtocolRecordRejected,
}

impl fmt::Display for RocgdbMiAdapterErrorV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "ROCgdb structured MI adapter failed: {self:?}")
    }
}

impl std::error::Error for RocgdbMiAdapterErrorV3 {}

impl From<MiParseErrorV3> for RocgdbMiAdapterErrorV3 {
    fn from(_: MiParseErrorV3) -> Self {
        Self::InvalidMiRecord
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ExecutionStateV3 {
    Unknown,
    Running,
    Stopped,
    Exited,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct CodeObjectAuthorityV3 {
    content: LiveGpuContentIdentityV3,
    load_base: u64,
    byte_len: u64,
    kernel_entry: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct AllocationAuthorityV3 {
    base: u64,
    byte_len: u64,
    space: LiveGpuMemorySpaceV3,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SourceAuthorityV3 {
    path: Vec<u8>,
    line: u64,
    span: LiveGpuSourceSpanV3,
}

/// Stateful sanitizer for structured MI records.
pub struct RocgdbMiObservationAdapterV3 {
    session_identity: OpaqueIdentityV1,
    authorization_identity: OpaqueIdentityV1,
    wave_width: u16,
    revision: u64,
    state: ExecutionStateV3,
    current_stop: Option<OpaqueIdentityV1>,
    limits: RocgdbMiAdapterLimitsV3,
    raw_threads: BTreeMap<Vec<u8>, RocgdbMiThreadIdentityV3>,
    logical_threads: BTreeMap<RocgdbMiThreadIdentityV3, Vec<u8>>,
    authenticated_gpu_threads: BTreeMap<RocgdbMiThreadIdentityV3, ()>,
    code_objects: Vec<CodeObjectAuthorityV3>,
    allocations: Vec<(AllocationIdentityV1, AllocationAuthorityV3)>,
    sources: Vec<SourceAuthorityV3>,
    raw_breakpoints: BTreeMap<Vec<u8>, RocgdbMiBreakpointIdentityV3>,
    logical_breakpoints: BTreeMap<RocgdbMiBreakpointIdentityV3, Vec<u8>>,
}

impl fmt::Debug for RocgdbMiObservationAdapterV3 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RocgdbMiObservationAdapterV3")
            .field("session_identity", &self.session_identity)
            .field("wave_width", &self.wave_width)
            .field("revision", &self.revision)
            .field("state", &self.state)
            .field("thread_bindings", &self.raw_threads.len())
            .field(
                "authenticated_gpu_thread_bindings",
                &self.authenticated_gpu_threads.len(),
            )
            .field("code_object_bindings", &self.code_objects.len())
            .field("allocation_bindings", &self.allocations.len())
            .field("source_bindings", &self.sources.len())
            .field("breakpoint_bindings", &self.raw_breakpoints.len())
            .field("native_authority", &"REDACTED")
            .finish()
    }
}

impl RocgdbMiObservationAdapterV3 {
    pub fn new(
        session_identity: OpaqueIdentityV1,
        authorization_identity: OpaqueIdentityV1,
        wave_width: u16,
        limits: RocgdbMiAdapterLimitsV3,
    ) -> Result<Self, RocgdbMiAdapterErrorV3> {
        limits.validate()?;
        if !matches!(wave_width, 32 | 64) {
            return Err(RocgdbMiAdapterErrorV3::InvalidWaveWidth);
        }
        Ok(Self {
            session_identity,
            authorization_identity,
            wave_width,
            revision: 0,
            state: ExecutionStateV3::Unknown,
            current_stop: None,
            limits,
            raw_threads: BTreeMap::new(),
            logical_threads: BTreeMap::new(),
            authenticated_gpu_threads: BTreeMap::new(),
            code_objects: Vec::new(),
            allocations: Vec::new(),
            sources: Vec::new(),
            raw_breakpoints: BTreeMap::new(),
            logical_breakpoints: BTreeMap::new(),
        })
    }

    pub const fn revision(&self) -> u64 {
        self.revision
    }

    pub const fn session_identity(&self) -> OpaqueIdentityV1 {
        self.session_identity
    }

    pub fn bind_code_object(
        &mut self,
        content: LiveGpuContentIdentityV3,
        load_base: u64,
        byte_len: u64,
        kernel_entry: u64,
    ) -> Result<(), RocgdbMiAdapterErrorV3> {
        let end = load_base
            .checked_add(byte_len)
            .ok_or(RocgdbMiAdapterErrorV3::InvalidField("code object"))?;
        if byte_len == 0 || kernel_entry < load_base || kernel_entry >= end {
            return Err(RocgdbMiAdapterErrorV3::InvalidField("code object"));
        }
        self.check_binding_budget(self.code_objects.len())?;
        if self.code_objects.iter().any(|item| item.content == content) {
            return Err(RocgdbMiAdapterErrorV3::DuplicateBinding);
        }
        self.code_objects.push(CodeObjectAuthorityV3 {
            content,
            load_base,
            byte_len,
            kernel_entry,
        });
        Ok(())
    }

    pub fn bind_allocation(
        &mut self,
        allocation: AllocationIdentityV1,
        base: u64,
        byte_len: u64,
        space: LiveGpuMemorySpaceV3,
    ) -> Result<(), RocgdbMiAdapterErrorV3> {
        if allocation.ordinal == 0 || byte_len == 0 || base.checked_add(byte_len).is_none() {
            return Err(RocgdbMiAdapterErrorV3::InvalidField("allocation"));
        }
        self.check_binding_budget(self.allocations.len())?;
        if self
            .allocations
            .iter()
            .any(|(identity, _)| *identity == allocation)
        {
            return Err(RocgdbMiAdapterErrorV3::DuplicateBinding);
        }
        self.allocations.push((
            allocation,
            AllocationAuthorityV3 {
                base,
                byte_len,
                space,
            },
        ));
        Ok(())
    }

    pub fn bind_source_line(
        &mut self,
        path: &OsStr,
        line: u64,
        span: LiveGpuSourceSpanV3,
    ) -> Result<(), RocgdbMiAdapterErrorV3> {
        let path = path.as_bytes();
        if path.is_empty() || line == 0 || span.byte_start >= span.byte_end {
            return Err(RocgdbMiAdapterErrorV3::InvalidField("source"));
        }
        self.check_binding_budget(self.sources.len())?;
        if self
            .sources
            .iter()
            .any(|item| item.path == path && item.line == line)
        {
            return Err(RocgdbMiAdapterErrorV3::DuplicateBinding);
        }
        self.sources.push(SourceAuthorityV3 {
            path: path.to_vec(),
            line,
            span,
        });
        Ok(())
    }

    /// Admits caller-selected generic thread ordinals from one structured MI
    /// result. No GPU classification is inferred from ROCgdb's human-oriented
    /// `target-id`, `details`, names, or console stream.
    pub fn admit_threads_from_thread_info(
        &mut self,
        line: &[u8],
        ordinals: &[u16],
    ) -> Result<Vec<RocgdbMiThreadAdmissionV3>, RocgdbMiAdapterErrorV3> {
        let record = parse_mi_record_v3(line, self.limits.parser())?;
        let MiRecordV3::Result { class, results, .. } = record else {
            return Err(RocgdbMiAdapterErrorV3::UnexpectedMiRecord);
        };
        if class != "done" {
            return Err(RocgdbMiAdapterErrorV3::BackendRejected);
        }
        self.admit_thread_results(&results, ordinals, line)
    }

    fn admit_thread_results(
        &mut self,
        results: &MiResultsV3,
        ordinals: &[u16],
        evidence: &[u8],
    ) -> Result<Vec<RocgdbMiThreadAdmissionV3>, RocgdbMiAdapterErrorV3> {
        if ordinals.is_empty() || ordinals.len() > MAX_ROCGDB_MI_THREADS_V3 {
            return Err(RocgdbMiAdapterErrorV3::CountOutOfRange("thread admissions"));
        }
        let mut unique = ordinals.to_vec();
        unique.sort_unstable();
        unique.dedup();
        if unique.len() != ordinals.len() {
            return Err(RocgdbMiAdapterErrorV3::DuplicateBinding);
        }
        let threads = tuple_values(
            results
                .get("threads")
                .ok_or(RocgdbMiAdapterErrorV3::MissingField("threads"))?,
        )?;
        let record_identity = self.derive_identity(b"thread-info-record", evidence)?;
        let mut admitted = Vec::with_capacity(ordinals.len());
        for ordinal in ordinals {
            let tuple = threads
                .get(usize::from(*ordinal))
                .ok_or(RocgdbMiAdapterErrorV3::InvalidField("thread ordinal"))?;
            let raw = required_const(tuple, "id")?;
            let thread = self.admit_thread(raw)?;
            admitted.push(RocgdbMiThreadAdmissionV3 {
                thread_info_record_identity: record_identity,
                thread_ordinal: *ordinal,
                thread,
            });
        }
        Ok(admitted)
    }

    /// Accepts exactly one newline-terminated MI record; unprefixed prose fails.
    pub fn ingest_line(
        &mut self,
        line: &[u8],
    ) -> Result<Option<RocgdbMiExecutionEventV3>, RocgdbMiAdapterErrorV3> {
        let record = parse_mi_record_v3(line, self.limits.parser())?;
        self.ingest_record(record, line)
    }

    fn ingest_record(
        &mut self,
        record: MiRecordV3,
        evidence_bytes: &[u8],
    ) -> Result<Option<RocgdbMiExecutionEventV3>, RocgdbMiAdapterErrorV3> {
        match record {
            MiRecordV3::Async {
                kind: MiAsyncKindV3::Exec,
                class,
                ..
            } if class == "running" => {
                if self.state != ExecutionStateV3::Running {
                    self.bump_revision()?;
                }
                self.state = ExecutionStateV3::Running;
                self.current_stop = None;
                Ok(Some(RocgdbMiExecutionEventV3::Running {
                    revision: self.revision,
                }))
            }
            MiRecordV3::Async {
                kind: MiAsyncKindV3::Exec,
                class,
                results,
                ..
            } if class == "stopped" => self.map_stopped(results, evidence_bytes).map(Some),
            MiRecordV3::Async {
                kind: MiAsyncKindV3::Notify,
                class,
                results,
                ..
            } if class == "thread-exited" => {
                if let Some(raw) = optional_const(&results, "id")
                    && let Some(logical) = self.raw_threads.remove(raw)
                {
                    self.logical_threads.remove(&logical);
                    self.authenticated_gpu_threads.remove(&logical);
                }
                Ok(None)
            }
            MiRecordV3::Result { .. }
            | MiRecordV3::Async { .. }
            | MiRecordV3::Stream { .. }
            | MiRecordV3::Prompt => Ok(None),
        }
    }

    fn map_stopped(
        &mut self,
        results: MiResultsV3,
        evidence_bytes: &[u8],
    ) -> Result<RocgdbMiExecutionEventV3, RocgdbMiAdapterErrorV3> {
        let reason_text = optional_const(&results, "reason");
        if matches!(
            reason_text,
            Some(b"exited" | b"exited-normally" | b"exited-signalled")
        ) {
            self.bump_revision()?;
            self.state = ExecutionStateV3::Exited;
            self.current_stop = None;
            return Ok(RocgdbMiExecutionEventV3::Exited {
                revision: self.revision,
            });
        }

        let raw_thread = required_const(&results, "thread-id")?;
        let Some(thread) = self.raw_threads.get(raw_thread).copied() else {
            self.bump_revision()?;
            self.state = ExecutionStateV3::Stopped;
            self.current_stop = None;
            return Ok(RocgdbMiExecutionEventV3::Unavailable {
                revision: self.revision,
                reason: LiveGpuUnavailableReasonV3::OutsideCaptureScope,
            });
        };
        if !self.authenticated_gpu_threads.contains_key(&thread) {
            self.bump_revision()?;
            self.state = ExecutionStateV3::Stopped;
            self.current_stop = None;
            return Ok(RocgdbMiExecutionEventV3::Unavailable {
                revision: self.revision,
                reason: LiveGpuUnavailableReasonV3::Unsupported,
            });
        }
        let wave = RocgdbMiWaveIdentityV3 {
            identity: self.derive_identity(b"wave", raw_thread)?,
            thread,
        };
        let truth = observed_truth(self.derive_identity(b"observation", evidence_bytes)?);
        let frame = results.get("frame").and_then(MiValueV3::as_tuple);
        let relative_pc = self.map_relative_pc(frame, &truth);
        let source = self.map_source(frame, &truth);
        let mut lanes = Vec::with_capacity(usize::from(self.wave_width));
        for lane in 0..self.wave_width {
            let mut material = raw_thread.to_vec();
            material.extend_from_slice(&lane.to_le_bytes());
            lanes.push(RocgdbMiLaneObservationV3 {
                lane: RocgdbMiLaneIdentityV3 {
                    identity: self.derive_identity(b"lane", &material)?,
                    wave,
                    lane,
                },
                active: unavailable(LiveGpuUnavailableReasonV3::NotCaptured),
            });
        }

        self.bump_revision()?;
        self.state = ExecutionStateV3::Stopped;
        let mut stop_material = evidence_bytes.to_vec();
        stop_material.extend_from_slice(&self.revision.to_le_bytes());
        let stop_identity = self.derive_identity(b"stop", &stop_material)?;
        self.current_stop = Some(stop_identity);
        let snapshot_identity = self.derive_identity(b"snapshot", &stop_identity.as_bytes())?;
        let reason = match reason_text {
            Some(b"breakpoint-hit") => RocgdbMiStopReasonV3::Breakpoint,
            Some(b"end-stepping-range" | b"function-finished") => RocgdbMiStopReasonV3::Step,
            Some(b"signal-received") => RocgdbMiStopReasonV3::Signal,
            _ => RocgdbMiStopReasonV3::Unknown,
        };
        let breakpoint = if reason == RocgdbMiStopReasonV3::Breakpoint {
            Some(self.bind_breakpoint(required_const(&results, "bkptno")?)?)
        } else {
            None
        };
        let snapshot = RocgdbMiStoppedSnapshotV3 {
            snapshot_identity,
            stop_identity,
            revision: self.revision,
            reason,
            breakpoint,
            threads: vec![RocgdbMiStoppedThreadV3 {
                thread,
                wave,
                wave_width: self.wave_width,
                lanes,
                relative_pc,
                source,
            }],
        };
        snapshot
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        Ok(RocgdbMiExecutionEventV3::Stopped { snapshot })
    }

    fn map_relative_pc(
        &self,
        frame: Option<&MiResultsV3>,
        truth: &LiveGpuTruthV3,
    ) -> LiveGpuAvailabilityV3<LiveGpuRelativePcV3> {
        let Some(address) = frame
            .and_then(|frame| optional_const(frame, "addr"))
            .and_then(parse_hex_u64)
        else {
            return unavailable(LiveGpuUnavailableReasonV3::NotCaptured);
        };
        let Some(binding) = self.code_objects.iter().find(|binding| {
            address >= binding.kernel_entry
                && address < binding.load_base.saturating_add(binding.byte_len)
        }) else {
            return unavailable(LiveGpuUnavailableReasonV3::OutsideCaptureScope);
        };
        LiveGpuAvailabilityV3::Available {
            value: LiveGpuRelativePcV3 {
                kernel_entry_byte_offset: address - binding.kernel_entry,
            },
            truth: truth.clone(),
        }
    }

    fn map_source(
        &self,
        frame: Option<&MiResultsV3>,
        truth: &LiveGpuTruthV3,
    ) -> LiveGpuAvailabilityV3<LiveGpuSourceSpanV3> {
        let Some(frame) = frame else {
            return unavailable(LiveGpuUnavailableReasonV3::NotCaptured);
        };
        let path = optional_const(frame, "fullname").or_else(|| optional_const(frame, "file"));
        let line = optional_const(frame, "line").and_then(parse_decimal_u64);
        let Some(binding) = path.zip(line).and_then(|(path, line)| {
            self.sources
                .iter()
                .find(|binding| binding.path == path && binding.line == line)
        }) else {
            return unavailable(LiveGpuUnavailableReasonV3::NotCaptured);
        };
        LiveGpuAvailabilityV3::Available {
            value: binding.span,
            truth: truth.clone(),
        }
    }

    fn admit_thread(
        &mut self,
        raw: &[u8],
    ) -> Result<RocgdbMiThreadIdentityV3, RocgdbMiAdapterErrorV3> {
        if let Some(identity) = self.raw_threads.get(raw).copied() {
            return Ok(identity);
        }
        validate_native_token(raw, "thread")?;
        self.check_binding_budget(self.raw_threads.len())?;
        let identity = RocgdbMiThreadIdentityV3 {
            identity: self.derive_identity(b"thread", raw)?,
        };
        if self
            .logical_threads
            .insert(identity, raw.to_vec())
            .is_some()
        {
            return Err(RocgdbMiAdapterErrorV3::IdentityCollision);
        }
        self.raw_threads.insert(raw.to_vec(), identity);
        Ok(identity)
    }

    pub fn apply_exec_mask(
        &self,
        snapshot: &mut RocgdbMiStoppedSnapshotV3,
        registers: &RocgdbMiRegisterSnapshotV3,
    ) -> Result<(), RocgdbMiAdapterErrorV3> {
        registers
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)?;
        let thread = snapshot
            .threads
            .iter_mut()
            .find(|thread| thread.wave == registers.scope.wave)
            .ok_or(RocgdbMiAdapterErrorV3::UnknownThread)?;
        let Some(register) = registers.registers.iter().find(|item| item.name == "exec") else {
            return Ok(());
        };
        let LiveGpuAvailabilityV3::Available { value, truth } = &register.value else {
            return Ok(());
        };
        let LiveGpuValueEncodingV3::Bits { bit_width, bits } = value else {
            return Ok(());
        };
        if *bit_width > 64 || bits.len() > 16 {
            return Ok(());
        }
        let Ok(mask) = u64::from_str_radix(bits, 16) else {
            return Ok(());
        };
        if thread.wave_width == 32 && mask > u64::from(u32::MAX) {
            return Ok(());
        }
        for lane in &mut thread.lanes {
            lane.active = LiveGpuAvailabilityV3::Available {
                value: mask & (1_u64 << lane.lane.lane) != 0,
                truth: truth.clone(),
            };
        }
        snapshot
            .validate()
            .map_err(|_| RocgdbMiAdapterErrorV3::ProtocolRecordRejected)
    }

    fn bind_breakpoint(
        &mut self,
        raw: &[u8],
    ) -> Result<RocgdbMiBreakpointIdentityV3, RocgdbMiAdapterErrorV3> {
        if let Some(identity) = self.raw_breakpoints.get(raw).copied() {
            return Ok(identity);
        }
        validate_native_token(raw, "breakpoint")?;
        self.check_binding_budget(self.raw_breakpoints.len())?;
        let identity = RocgdbMiBreakpointIdentityV3 {
            identity: self.derive_identity(b"breakpoint", raw)?,
        };
        if self
            .logical_breakpoints
            .insert(identity, raw.to_vec())
            .is_some()
        {
            return Err(RocgdbMiAdapterErrorV3::IdentityCollision);
        }
        self.raw_breakpoints.insert(raw.to_vec(), identity);
        Ok(identity)
    }

    fn derive_identity(
        &self,
        domain: &[u8],
        native: &[u8],
    ) -> Result<OpaqueIdentityV1, RocgdbMiAdapterErrorV3> {
        let mut digest = Sha256::new();
        digest.update(b"fe2o3-rocgdb-mi-v3\0");
        digest.update(domain);
        digest.update([0]);
        digest.update(self.session_identity.as_bytes());
        digest.update(
            u64::try_from(native.len())
                .unwrap_or(u64::MAX)
                .to_le_bytes(),
        );
        digest.update(native);
        OpaqueIdentityV1::new(digest.finalize().into())
            .map_err(|_| RocgdbMiAdapterErrorV3::IdentityCollision)
    }

    fn check_binding_budget(&self, count: usize) -> Result<(), RocgdbMiAdapterErrorV3> {
        if count >= self.limits.max_private_bindings {
            Err(RocgdbMiAdapterErrorV3::CountOutOfRange("bindings"))
        } else {
            Ok(())
        }
    }

    fn bump_revision(&mut self) -> Result<(), RocgdbMiAdapterErrorV3> {
        self.revision = self
            .revision
            .checked_add(1)
            .ok_or(RocgdbMiAdapterErrorV3::CountOutOfRange("revision"))?;
        Ok(())
    }
}

fn observed_truth(identity: OpaqueIdentityV1) -> LiveGpuTruthV3 {
    LiveGpuTruthV3 {
        origin: LiveGpuTruthOriginV3::Observed,
        evidence: vec![LiveGpuEvidenceRefV3 {
            kind: LiveGpuEvidenceKindV3::RuntimeObservation,
            identity,
        }],
    }
}

fn unavailable<T>(reason: LiveGpuUnavailableReasonV3) -> LiveGpuAvailabilityV3<T> {
    LiveGpuAvailabilityV3::Unavailable {
        reason,
        truth: LiveGpuTruthV3 {
            origin: LiveGpuTruthOriginV3::Unavailable,
            evidence: Vec::new(),
        },
    }
}

fn required_const<'a>(
    results: &'a MiResultsV3,
    name: &'static str,
) -> Result<&'a [u8], RocgdbMiAdapterErrorV3> {
    results
        .get(name)
        .and_then(MiValueV3::as_const)
        .ok_or(RocgdbMiAdapterErrorV3::MissingField(name))
}

fn optional_const<'a>(results: &'a MiResultsV3, name: &str) -> Option<&'a [u8]> {
    results.get(name).and_then(MiValueV3::as_const)
}

fn tuple_values(value: &MiValueV3) -> Result<Vec<&MiResultsV3>, RocgdbMiAdapterErrorV3> {
    match value {
        MiValueV3::List(MiListV3::Values(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_tuple()
                    .ok_or(RocgdbMiAdapterErrorV3::InvalidField("thread tuple"))
            })
            .collect(),
        MiValueV3::List(MiListV3::Results(results)) => results
            .iter()
            .map(|(_, value)| {
                value
                    .as_tuple()
                    .ok_or(RocgdbMiAdapterErrorV3::InvalidField("thread tuple"))
            })
            .collect(),
        MiValueV3::Const(_) | MiValueV3::Tuple(_) => {
            Err(RocgdbMiAdapterErrorV3::InvalidField("threads"))
        }
    }
}

fn parse_decimal_u64(bytes: &[u8]) -> Option<u64> {
    if bytes.is_empty() || !bytes.iter().all(u8::is_ascii_digit) {
        return None;
    }
    std::str::from_utf8(bytes).ok()?.parse().ok()
}

fn parse_hex_u64(bytes: &[u8]) -> Option<u64> {
    let digits = bytes.strip_prefix(b"0x")?;
    if digits.is_empty() || !digits.iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    u64::from_str_radix(std::str::from_utf8(digits).ok()?, 16).ok()
}

fn validate_native_token(token: &[u8], field: &'static str) -> Result<(), RocgdbMiAdapterErrorV3> {
    if token.is_empty()
        || token.len() > MAX_NATIVE_TOKEN_BYTES_V3
        || !token
            .iter()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
    {
        return Err(RocgdbMiAdapterErrorV3::InvalidField(field));
    }
    Ok(())
}
