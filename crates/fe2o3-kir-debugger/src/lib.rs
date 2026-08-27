#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

use std::error::Error;
use std::fmt;

use fe2o3_kernel_ir::{BlockId, ValueId};
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, ScalarBitsV1, SimulationDebugCaptureLimitsV1,
    SimulationDebugCheckpointPhaseV1, SimulationDebugCollectionV1, SimulationDebugFrameV1,
    SimulationDebugMemoryAccessV1, SimulationDebugRecordKindV1, SimulationDebugRecordV1,
    SimulationDebugSinkControlV1, SimulationDebugSinkV1, SimulationDebugSiteV1,
    SimulationDebugUnavailableReasonV1, SimulationDebugValueV1, SimulationErrorV1,
    SimulationExecutionErrorKindV1, SimulationExecutionV1, SimulationInvocationV1,
    SimulationLimitsV1, SimulationRequestV1, SimulationSiteV1, SimulationTargetV1,
};

pub const MAX_DEBUGGER_RECORDS_V1: usize = 1_000_000;
pub const MAX_DEBUGGER_RETAINED_VALUES_V1: usize = 16_000_000;
pub const MAX_DEBUGGER_RETAINED_MEMORY_BYTES_V1: usize = 256 * 1024 * 1024;
pub const MAX_DEBUGGER_PREDICATE_DEPTH_V1: usize = 32;
pub const MAX_DEBUGGER_PREDICATE_NODES_V1: usize = 1_024;
pub const MAX_DEBUGGER_SOURCE_FILES_V1: usize = 65_536;
pub const MAX_DEBUGGER_SOURCE_SITES_V1: usize = 1_000_000;
pub const MAX_DEBUGGER_SOURCE_SPANS_V1: usize = 4_000_000;
pub const MAX_DEBUGGER_SOURCE_PATH_BYTES_V1: usize = 4_096;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebuggerLimitsV1 {
    max_records: usize,
    max_retained_values: usize,
    max_retained_memory_bytes: usize,
}

impl DebuggerLimitsV1 {
    pub fn new(
        max_records: usize,
        max_retained_values: usize,
        max_retained_memory_bytes: usize,
    ) -> Result<Self, DebuggerErrorV1> {
        let limits = Self {
            max_records,
            max_retained_values,
            max_retained_memory_bytes,
        };
        for (actual, maximum, field) in [
            (max_records, MAX_DEBUGGER_RECORDS_V1, "records"),
            (
                max_retained_values,
                MAX_DEBUGGER_RETAINED_VALUES_V1,
                "values",
            ),
            (
                max_retained_memory_bytes,
                MAX_DEBUGGER_RETAINED_MEMORY_BYTES_V1,
                "memory bytes",
            ),
        ] {
            if actual == 0 || actual > maximum {
                return Err(DebuggerErrorV1::LimitOutOfRange {
                    field,
                    actual,
                    maximum,
                });
            }
        }
        Ok(limits)
    }

    pub const fn max_records(self) -> usize {
        self.max_records
    }

    pub const fn max_retained_values(self) -> usize {
        self.max_retained_values
    }

    pub const fn max_retained_memory_bytes(self) -> usize {
        self.max_retained_memory_bytes
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugWaveWidthV1 {
    Wave32,
    Wave64,
}

impl DebugWaveWidthV1 {
    pub const fn lanes(self) -> u16 {
        match self {
            Self::Wave32 => 32,
            Self::Wave64 => 64,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugKirIdentityV1 {
    pub digest: [u8; 32],
    pub canonical_len: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugTranscriptTruncationV1 {
    RecordLimit,
    ValueLimit,
    MemoryByteLimit,
    AllocationFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugTranscriptCompletenessV1 {
    Complete,
    Truncated(DebugTranscriptTruncationV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugTranscriptV1 {
    identity: DebugKirIdentityV1,
    wave_width: DebugWaveWidthV1,
    records: Vec<SimulationDebugRecordV1>,
    terminal_fault: Option<DebugTerminalFaultV1>,
    completeness: DebugTranscriptCompletenessV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugTerminalFaultV1 {
    pub ordinal: u64,
    pub invocation: Option<SimulationInvocationV1>,
    pub site: Option<SimulationSiteV1>,
    pub kind: SimulationExecutionErrorKindV1,
}

impl DebugTranscriptV1 {
    pub const fn identity(&self) -> DebugKirIdentityV1 {
        self.identity
    }

    pub const fn wave_width(&self) -> DebugWaveWidthV1 {
        self.wave_width
    }

    pub fn records(&self) -> &[SimulationDebugRecordV1] {
        &self.records
    }

    pub const fn completeness(&self) -> DebugTranscriptCompletenessV1 {
        self.completeness
    }

    pub const fn terminal_fault(&self) -> Option<&DebugTerminalFaultV1> {
        self.terminal_fault.as_ref()
    }
}

#[derive(Debug)]
pub struct DebuggerRunV1 {
    pub execution: Result<SimulationExecutionV1, SimulationErrorV1>,
    pub transcript: DebugTranscriptV1,
}

struct TranscriptCollectorV1 {
    limits: DebuggerLimitsV1,
    records: Vec<SimulationDebugRecordV1>,
    retained_values: usize,
    retained_memory_bytes: usize,
    truncation: Option<DebugTranscriptTruncationV1>,
}

impl TranscriptCollectorV1 {
    fn new(limits: DebuggerLimitsV1) -> Self {
        Self {
            limits,
            records: Vec::new(),
            retained_values: 0,
            retained_memory_bytes: 0,
            truncation: None,
        }
    }

    fn into_transcript(
        self,
        identity: DebugKirIdentityV1,
        wave_width: DebugWaveWidthV1,
        terminal_fault: Option<DebugTerminalFaultV1>,
    ) -> DebugTranscriptV1 {
        DebugTranscriptV1 {
            identity,
            wave_width,
            records: self.records,
            terminal_fault,
            completeness: self
                .truncation
                .map(DebugTranscriptCompletenessV1::Truncated)
                .unwrap_or(DebugTranscriptCompletenessV1::Complete),
        }
    }
}

impl SimulationDebugSinkV1 for TranscriptCollectorV1 {
    fn record(&mut self, record: SimulationDebugRecordV1) -> SimulationDebugSinkControlV1 {
        let (values, memory_bytes) = record_cost(&record);
        let Some(next_values) = self.retained_values.checked_add(values) else {
            self.truncation = Some(DebugTranscriptTruncationV1::ValueLimit);
            return SimulationDebugSinkControlV1::DropAndStop;
        };
        let Some(next_memory) = self.retained_memory_bytes.checked_add(memory_bytes) else {
            self.truncation = Some(DebugTranscriptTruncationV1::MemoryByteLimit);
            return SimulationDebugSinkControlV1::DropAndStop;
        };
        let truncation = if self.records.len() == self.limits.max_records {
            Some(DebugTranscriptTruncationV1::RecordLimit)
        } else if next_values > self.limits.max_retained_values {
            Some(DebugTranscriptTruncationV1::ValueLimit)
        } else if next_memory > self.limits.max_retained_memory_bytes {
            Some(DebugTranscriptTruncationV1::MemoryByteLimit)
        } else if self.records.try_reserve(1).is_err() {
            Some(DebugTranscriptTruncationV1::AllocationFailure)
        } else {
            None
        };
        if let Some(truncation) = truncation {
            self.truncation = Some(truncation);
            return SimulationDebugSinkControlV1::DropAndStop;
        }
        self.retained_values = next_values;
        self.retained_memory_bytes = next_memory;
        self.records.push(record);
        SimulationDebugSinkControlV1::Continue
    }
}

fn record_cost(record: &SimulationDebugRecordV1) -> (usize, usize) {
    match &record.kind {
        SimulationDebugRecordKindV1::Checkpoint { stack, memory, .. } => {
            let values = match stack {
                SimulationDebugCollectionV1::Captured(frames) => frames
                    .iter()
                    .map(|frame| match &frame.values {
                        SimulationDebugCollectionV1::Captured(values) => values.len(),
                        SimulationDebugCollectionV1::Unavailable { .. } => 0,
                    })
                    .fold(0_usize, usize::saturating_add),
                SimulationDebugCollectionV1::Unavailable { .. } => 0,
            };
            let memory = match memory {
                SimulationDebugCollectionV1::Captured(allocations) => allocations
                    .iter()
                    .map(|allocation| {
                        allocation
                            .bytes
                            .len()
                            .saturating_add(allocation.initialized.len())
                    })
                    .fold(0_usize, usize::saturating_add),
                SimulationDebugCollectionV1::Unavailable { .. } => 0,
            };
            (values, memory)
        }
        SimulationDebugRecordKindV1::Memory { .. } => (1, 0),
        SimulationDebugRecordKindV1::WorkgroupBarrier { .. }
        | SimulationDebugRecordKindV1::Fence { .. } => (0, 0),
    }
}

pub fn capture_debugger_run_v1(
    module: &AdmittedSimulationModuleV1,
    request: &SimulationRequestV1,
    target: SimulationTargetV1,
    simulation_limits: SimulationLimitsV1,
    capture_limits: SimulationDebugCaptureLimitsV1,
    debugger_limits: DebuggerLimitsV1,
    wave_width: DebugWaveWidthV1,
) -> DebuggerRunV1 {
    let identity = DebugKirIdentityV1 {
        digest: *module.identity().digest(),
        canonical_len: module.identity().canonical_length(),
    };
    let mut collector = TranscriptCollectorV1::new(debugger_limits);
    let execution = module.simulate_debugged_with_sink(
        request,
        target,
        simulation_limits,
        capture_limits,
        &mut collector,
    );
    let terminal_fault = match &execution {
        Err(SimulationErrorV1::Execution(error)) => Some(DebugTerminalFaultV1 {
            ordinal: collector
                .records
                .last()
                .map_or(0, |record| record.ordinal.saturating_add(1)),
            invocation: error.invocation,
            site: error.site.clone(),
            kind: error.kind.clone(),
        }),
        _ => None,
    };
    DebuggerRunV1 {
        execution,
        transcript: collector.into_transcript(identity, wave_width, terminal_fault),
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugHierarchyV1 {
    pub workgroup: [u64; 3],
    pub wave: u32,
    pub lane: u16,
    pub active_mask: u64,
    pub local: [u32; 3],
    pub global: [u64; 3],
}

pub fn hierarchy_for_invocation_v1(
    invocation: SimulationInvocationV1,
    width: DebugWaveWidthV1,
) -> DebugHierarchyV1 {
    let size = invocation.workgroup_size;
    let linear = u64::from(invocation.local[2]) * u64::from(size[1]) * u64::from(size[0])
        + u64::from(invocation.local[1]) * u64::from(size[0])
        + u64::from(invocation.local[0]);
    let lanes = u64::from(width.lanes());
    let wave = u32::try_from(linear / lanes).unwrap_or(u32::MAX);
    let lane = u16::try_from(linear % lanes).unwrap_or(u16::MAX);
    let first = u64::from(wave) * lanes;
    let volume = u64::from(size[0]) * u64::from(size[1]) * u64::from(size[2]);
    let mut active_mask = 0_u64;
    for candidate in 0..lanes {
        let candidate_linear = first + candidate;
        if candidate_linear >= volume {
            break;
        }
        let x = candidate_linear % u64::from(size[0]);
        let yz = candidate_linear / u64::from(size[0]);
        let y = yz % u64::from(size[1]);
        let z = yz / u64::from(size[1]);
        let active = [x, y, z].into_iter().enumerate().all(|(axis, local)| {
            invocation.workgroup[axis] * u64::from(size[axis]) + local
                < invocation.launch_extent[axis]
        });
        if active {
            active_mask |= 1_u64 << candidate;
        }
    }
    DebugHierarchyV1 {
        workgroup: invocation.workgroup,
        wave,
        lane,
        active_mask,
        local: invocation.local,
        global: invocation.global,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DebugScopeSelectorV1 {
    Dispatch,
    Workgroup([u64; 3]),
    Wave {
        workgroup: [u64; 3],
        wave: u32,
    },
    Lane {
        workgroup: [u64; 3],
        wave: u32,
        lane: u16,
    },
    GlobalWorkitem([u64; 3]),
}

impl DebugScopeSelectorV1 {
    pub fn matches(&self, invocation: SimulationInvocationV1, width: DebugWaveWidthV1) -> bool {
        let hierarchy = hierarchy_for_invocation_v1(invocation, width);
        match self {
            Self::Dispatch => true,
            Self::Workgroup(workgroup) => hierarchy.workgroup == *workgroup,
            Self::Wave { workgroup, wave } => {
                hierarchy.workgroup == *workgroup && hierarchy.wave == *wave
            }
            Self::Lane {
                workgroup,
                wave,
                lane,
            } => {
                hierarchy.workgroup == *workgroup
                    && hierarchy.wave == *wave
                    && hierarchy.lane == *lane
            }
            Self::GlobalWorkitem(global) => hierarchy.global == *global,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugSiteSelectorV1 {
    pub function_ordinal: Option<usize>,
    pub block: Option<BlockId>,
    pub operation: Option<u32>,
    pub phase: Option<SimulationDebugCheckpointPhaseV1>,
}

impl DebugSiteSelectorV1 {
    fn matches(&self, record: &SimulationDebugRecordV1) -> bool {
        self.function_ordinal
            .is_none_or(|value| value == record.site.function_ordinal)
            && self.block.is_none_or(|value| value == record.site.block)
            && self
                .operation
                .is_none_or(|value| value == record.site.operation)
            && self.phase.is_none_or(|phase| {
                matches!(
                    &record.kind,
                    SimulationDebugRecordKindV1::Checkpoint {
                        phase: actual,
                        ..
                    } if *actual == phase
                )
            })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DebugPredicateV1 {
    True,
    ScalarEquals {
        frame_depth: u32,
        value: ValueId,
        expected: ScalarBitsV1,
    },
    ScalarNotEquals {
        frame_depth: u32,
        value: ValueId,
        expected: ScalarBitsV1,
    },
    And(Vec<DebugPredicateV1>),
    Or(Vec<DebugPredicateV1>),
    Not(Box<DebugPredicateV1>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PredicateResultV1 {
    True,
    False,
    Unavailable,
}

impl DebugPredicateV1 {
    fn validate(&self) -> Result<(), DebuggerErrorV1> {
        fn visit(
            predicate: &DebugPredicateV1,
            depth: usize,
            nodes: &mut usize,
        ) -> Result<(), DebuggerErrorV1> {
            *nodes = nodes.saturating_add(1);
            if depth > MAX_DEBUGGER_PREDICATE_DEPTH_V1 || *nodes > MAX_DEBUGGER_PREDICATE_NODES_V1 {
                return Err(DebuggerErrorV1::PredicateLimit);
            }
            match predicate {
                DebugPredicateV1::And(children) | DebugPredicateV1::Or(children) => {
                    for child in children {
                        visit(child, depth + 1, nodes)?;
                    }
                }
                DebugPredicateV1::Not(child) => visit(child, depth + 1, nodes)?,
                _ => {}
            }
            Ok(())
        }
        let mut nodes = 0;
        visit(self, 1, &mut nodes)
    }

    fn evaluate(&self, record: &SimulationDebugRecordV1) -> PredicateResultV1 {
        match self {
            Self::True => PredicateResultV1::True,
            Self::ScalarEquals {
                frame_depth,
                value,
                expected,
            } => scalar_binding(record, *frame_depth, *value)
                .map(|actual| {
                    if actual == *expected {
                        PredicateResultV1::True
                    } else {
                        PredicateResultV1::False
                    }
                })
                .unwrap_or(PredicateResultV1::Unavailable),
            Self::ScalarNotEquals {
                frame_depth,
                value,
                expected,
            } => scalar_binding(record, *frame_depth, *value)
                .map(|actual| {
                    if actual != *expected {
                        PredicateResultV1::True
                    } else {
                        PredicateResultV1::False
                    }
                })
                .unwrap_or(PredicateResultV1::Unavailable),
            Self::And(children) => {
                let mut unavailable = false;
                for child in children {
                    match child.evaluate(record) {
                        PredicateResultV1::False => return PredicateResultV1::False,
                        PredicateResultV1::Unavailable => unavailable = true,
                        PredicateResultV1::True => {}
                    }
                }
                if unavailable {
                    PredicateResultV1::Unavailable
                } else {
                    PredicateResultV1::True
                }
            }
            Self::Or(children) => {
                let mut unavailable = false;
                for child in children {
                    match child.evaluate(record) {
                        PredicateResultV1::True => return PredicateResultV1::True,
                        PredicateResultV1::Unavailable => unavailable = true,
                        PredicateResultV1::False => {}
                    }
                }
                if unavailable {
                    PredicateResultV1::Unavailable
                } else {
                    PredicateResultV1::False
                }
            }
            Self::Not(child) => match child.evaluate(record) {
                PredicateResultV1::True => PredicateResultV1::False,
                PredicateResultV1::False => PredicateResultV1::True,
                PredicateResultV1::Unavailable => PredicateResultV1::Unavailable,
            },
        }
    }
}

fn scalar_binding(
    record: &SimulationDebugRecordV1,
    frame_depth: u32,
    value: ValueId,
) -> Option<ScalarBitsV1> {
    let SimulationDebugRecordKindV1::Checkpoint { stack, .. } = &record.kind else {
        return None;
    };
    let SimulationDebugCollectionV1::Captured(frames) = stack else {
        return None;
    };
    let frame = frames.iter().find(|frame| frame.depth == frame_depth)?;
    let SimulationDebugCollectionV1::Captured(values) = &frame.values else {
        return None;
    };
    values
        .binary_search_by_key(&value, |binding| binding.value)
        .ok()
        .and_then(|index| match &values.get(index)?.observed {
            SimulationDebugValueV1::Scalar(value) => Some(*value),
            _ => None,
        })
}

fn checkpoint_depth(record: &SimulationDebugRecordV1) -> Result<u32, DebugInspectionUnavailableV1> {
    let SimulationDebugRecordKindV1::Checkpoint { stack, .. } = &record.kind else {
        return Err(DebugInspectionUnavailableV1::NotCheckpoint);
    };
    match stack {
        SimulationDebugCollectionV1::Captured(frames) => frames
            .last()
            .map(|frame| frame.depth)
            .ok_or(DebugInspectionUnavailableV1::UnknownFrame),
        SimulationDebugCollectionV1::Unavailable { reason, .. } => {
            Err(DebugInspectionUnavailableV1::Stack(*reason))
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugBreakpointV1 {
    pub id: u64,
    pub site: DebugSiteSelectorV1,
    pub scope: DebugScopeSelectorV1,
    pub predicate: DebugPredicateV1,
    pub hit_condition: Option<DebugHitConditionV1>,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugHitConditionV1 {
    Equal(u64),
    AtLeast(u64),
    Multiple(u64),
}

impl DebugHitConditionV1 {
    fn matches(self, hits: u64) -> bool {
        match self {
            Self::Equal(expected) => hits == expected,
            Self::AtLeast(minimum) => hits >= minimum,
            Self::Multiple(divisor) => hits.is_multiple_of(divisor),
        }
    }

    fn is_valid(self) -> bool {
        match self {
            Self::Equal(value) | Self::AtLeast(value) | Self::Multiple(value) => value != 0,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugWatchAccessV1 {
    Read,
    Write,
    Atomic,
    ReadWrite,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugWatchpointV1 {
    pub id: u64,
    pub allocation: u64,
    pub byte_offset: usize,
    pub byte_len: usize,
    pub access: DebugWatchAccessV1,
    pub scope: DebugScopeSelectorV1,
    pub value_equals: Option<ScalarBitsV1>,
    pub enabled: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugStopReasonV1 {
    Step,
    Breakpoint(u64),
    Watchpoint(u64),
    Fault,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugStopV1 {
    pub record_index: usize,
    pub record_ordinal: u64,
    pub reason: DebugStopReasonV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugNavigationV1 {
    Stopped(DebugStopV1),
    Beginning,
    End,
    BudgetExhausted(DebugStopV1),
    TranscriptTruncated(DebugTranscriptTruncationV1),
    Unavailable(DebugInspectionUnavailableV1),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugInspectionUnavailableV1 {
    NoCurrentRecord,
    NotCheckpoint,
    Stack(SimulationDebugUnavailableReasonV1),
    Values(SimulationDebugUnavailableReasonV1),
    Memory(SimulationDebugUnavailableReasonV1),
    UnknownFrame,
    UnknownValue,
    NonScalarValue,
    UnknownAllocation,
    RangeOverflow,
    OutOfBounds,
    SourceNotBound,
    SourceUnavailable,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DebugInspectionV1<T> {
    Available(T),
    Unavailable(DebugInspectionUnavailableV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugMemorySliceV1 {
    pub allocation: u64,
    pub byte_offset: usize,
    pub address_space: fe2o3_kernel_ir::AddressSpace,
    pub bytes: Vec<u8>,
    pub initialized: Vec<bool>,
}

pub struct DebugSessionV1 {
    transcript: DebugTranscriptV1,
    cursor: Option<usize>,
    breakpoints: Vec<DebugBreakpointV1>,
    breakpoint_hits: Vec<u64>,
    watchpoints: Vec<DebugWatchpointV1>,
    watchpoint_hits: Vec<u64>,
    source_catalog: Option<DebugSourceCatalogV1>,
}

impl DebugSessionV1 {
    pub const fn new(transcript: DebugTranscriptV1) -> Self {
        Self {
            transcript,
            cursor: None,
            breakpoints: Vec::new(),
            breakpoint_hits: Vec::new(),
            watchpoints: Vec::new(),
            watchpoint_hits: Vec::new(),
            source_catalog: None,
        }
    }

    pub const fn transcript(&self) -> &DebugTranscriptV1 {
        &self.transcript
    }

    pub fn current(&self) -> Option<&SimulationDebugRecordV1> {
        self.cursor
            .and_then(|cursor| self.transcript.records.get(cursor))
    }

    pub fn current_fault(&self) -> Option<&DebugTerminalFaultV1> {
        (self.cursor == Some(self.transcript.records.len()))
            .then_some(self.transcript.terminal_fault.as_ref())
            .flatten()
    }

    pub fn current_hierarchy(&self) -> Option<DebugHierarchyV1> {
        self.current().map(|record| {
            hierarchy_for_invocation_v1(record.invocation, self.transcript.wave_width)
        })
    }

    /// Returns the replay cursor index. `None` is the entry position and an
    /// index equal to the record count is the terminal completion/fault slot.
    pub const fn cursor_record_index(&self) -> Option<usize> {
        self.cursor
    }

    pub fn seek_entry(&mut self) -> DebugNavigationV1 {
        self.set_cursor(None);
        DebugNavigationV1::Beginning
    }

    pub fn seek_record_index(&mut self, index: usize) -> DebugNavigationV1 {
        if index < self.transcript.records.len() {
            return self.stop(index, DebugStopReasonV1::Step);
        }
        if index == self.transcript.records.len() {
            if let DebugTranscriptCompletenessV1::Truncated(reason) = self.transcript.completeness {
                return DebugNavigationV1::TranscriptTruncated(reason);
            }
            self.set_cursor(Some(index));
            if let Some(fault) = &self.transcript.terminal_fault {
                return DebugNavigationV1::Stopped(DebugStopV1 {
                    record_index: index,
                    record_ordinal: fault.ordinal,
                    reason: DebugStopReasonV1::Fault,
                });
            }
            return DebugNavigationV1::End;
        }
        DebugNavigationV1::Unavailable(DebugInspectionUnavailableV1::OutOfBounds)
    }

    pub fn add_breakpoint(&mut self, breakpoint: DebugBreakpointV1) -> Result<(), DebuggerErrorV1> {
        if breakpoint.id == 0
            || self
                .breakpoints
                .iter()
                .any(|value| value.id == breakpoint.id)
        {
            return Err(DebuggerErrorV1::InvalidOrDuplicateIdentity);
        }
        breakpoint.predicate.validate()?;
        if breakpoint
            .hit_condition
            .is_some_and(|value| !value.is_valid())
        {
            return Err(DebuggerErrorV1::InvalidHitCondition);
        }
        self.breakpoints
            .try_reserve(1)
            .map_err(|_| DebuggerErrorV1::AllocationFailure)?;
        self.breakpoint_hits
            .try_reserve(1)
            .map_err(|_| DebuggerErrorV1::AllocationFailure)?;
        let hits = self.breakpoint_hits_through_cursor(&breakpoint);
        self.breakpoints.push(breakpoint);
        self.breakpoint_hits.push(hits);
        Ok(())
    }

    pub fn add_breakpoints_atomic(
        &mut self,
        breakpoints: Vec<DebugBreakpointV1>,
    ) -> Result<(), DebuggerErrorV1> {
        for (index, breakpoint) in breakpoints.iter().enumerate() {
            if breakpoint.id == 0
                || self
                    .breakpoints
                    .iter()
                    .any(|value| value.id == breakpoint.id)
                || breakpoints[..index]
                    .iter()
                    .any(|value| value.id == breakpoint.id)
            {
                return Err(DebuggerErrorV1::InvalidOrDuplicateIdentity);
            }
            breakpoint.predicate.validate()?;
            if breakpoint
                .hit_condition
                .is_some_and(|value| !value.is_valid())
            {
                return Err(DebuggerErrorV1::InvalidHitCondition);
            }
        }
        self.breakpoints
            .try_reserve(breakpoints.len())
            .map_err(|_| DebuggerErrorV1::AllocationFailure)?;
        self.breakpoint_hits
            .try_reserve(breakpoints.len())
            .map_err(|_| DebuggerErrorV1::AllocationFailure)?;
        let mut hits = Vec::new();
        hits.try_reserve_exact(breakpoints.len())
            .map_err(|_| DebuggerErrorV1::AllocationFailure)?;
        hits.extend(
            breakpoints
                .iter()
                .map(|breakpoint| self.breakpoint_hits_through_cursor(breakpoint)),
        );
        self.breakpoints.extend(breakpoints);
        self.breakpoint_hits.extend(hits);
        Ok(())
    }

    pub fn breakpoints(&self) -> &[DebugBreakpointV1] {
        &self.breakpoints
    }

    pub fn remove_breakpoint(&mut self, id: u64) -> bool {
        let Some(index) = self.breakpoints.iter().position(|value| value.id == id) else {
            return false;
        };
        self.breakpoints.remove(index);
        self.breakpoint_hits.remove(index);
        true
    }

    pub fn breakpoint_hit_count(&self, id: u64) -> Option<u64> {
        let index = self.breakpoints.iter().position(|value| value.id == id)?;
        self.breakpoint_hits.get(index).copied()
    }

    pub fn add_watchpoint(&mut self, watchpoint: DebugWatchpointV1) -> Result<(), DebuggerErrorV1> {
        if watchpoint.id == 0
            || watchpoint.byte_len == 0
            || watchpoint
                .byte_offset
                .checked_add(watchpoint.byte_len)
                .is_none()
            || self
                .watchpoints
                .iter()
                .any(|value| value.id == watchpoint.id)
        {
            return Err(DebuggerErrorV1::InvalidOrDuplicateIdentity);
        }
        self.watchpoints
            .try_reserve(1)
            .map_err(|_| DebuggerErrorV1::AllocationFailure)?;
        self.watchpoint_hits
            .try_reserve(1)
            .map_err(|_| DebuggerErrorV1::AllocationFailure)?;
        let hits = self.watchpoint_hits_through_cursor(&watchpoint);
        self.watchpoints.push(watchpoint);
        self.watchpoint_hits.push(hits);
        Ok(())
    }

    pub fn add_watchpoints_atomic(
        &mut self,
        watchpoints: Vec<DebugWatchpointV1>,
    ) -> Result<(), DebuggerErrorV1> {
        for (index, watchpoint) in watchpoints.iter().enumerate() {
            if watchpoint.id == 0
                || watchpoint.byte_len == 0
                || watchpoint
                    .byte_offset
                    .checked_add(watchpoint.byte_len)
                    .is_none()
                || self
                    .watchpoints
                    .iter()
                    .any(|value| value.id == watchpoint.id)
                || watchpoints[..index]
                    .iter()
                    .any(|value| value.id == watchpoint.id)
            {
                return Err(DebuggerErrorV1::InvalidOrDuplicateIdentity);
            }
        }
        self.watchpoints
            .try_reserve(watchpoints.len())
            .map_err(|_| DebuggerErrorV1::AllocationFailure)?;
        self.watchpoint_hits
            .try_reserve(watchpoints.len())
            .map_err(|_| DebuggerErrorV1::AllocationFailure)?;
        let mut hits = Vec::new();
        hits.try_reserve_exact(watchpoints.len())
            .map_err(|_| DebuggerErrorV1::AllocationFailure)?;
        hits.extend(
            watchpoints
                .iter()
                .map(|watchpoint| self.watchpoint_hits_through_cursor(watchpoint)),
        );
        self.watchpoints.extend(watchpoints);
        self.watchpoint_hits.extend(hits);
        Ok(())
    }

    pub fn watchpoints(&self) -> &[DebugWatchpointV1] {
        &self.watchpoints
    }

    pub fn remove_watchpoint(&mut self, id: u64) -> bool {
        let Some(index) = self.watchpoints.iter().position(|value| value.id == id) else {
            return false;
        };
        self.watchpoints.remove(index);
        self.watchpoint_hits.remove(index);
        true
    }

    pub fn watchpoint_hit_count(&self, id: u64) -> Option<u64> {
        let index = self.watchpoints.iter().position(|value| value.id == id)?;
        self.watchpoint_hits.get(index).copied()
    }

    pub fn forward_step(&mut self, scope: &DebugScopeSelectorV1) -> DebugNavigationV1 {
        let start = self.cursor.map_or(0, |cursor| cursor.saturating_add(1));
        let width = self.transcript.wave_width;
        self.find_forward(
            start,
            |record| {
                scope.matches(record.invocation, width)
                    && matches!(&record.kind, SimulationDebugRecordKindV1::Checkpoint { .. })
            },
            DebugStopReasonV1::Step,
        )
    }

    pub fn reverse_step(&mut self, scope: &DebugScopeSelectorV1) -> DebugNavigationV1 {
        let Some(start) = self.cursor.and_then(|cursor| cursor.checked_sub(1)) else {
            self.set_cursor(None);
            return DebugNavigationV1::Beginning;
        };
        let width = self.transcript.wave_width;
        self.find_reverse(
            start,
            |record| {
                scope.matches(record.invocation, width)
                    && matches!(&record.kind, SimulationDebugRecordKindV1::Checkpoint { .. })
            },
            DebugStopReasonV1::Step,
        )
    }

    pub fn step_over(&mut self, scope: &DebugScopeSelectorV1) -> DebugNavigationV1 {
        let Some(record) = self.current() else {
            return self.forward_step(scope);
        };
        let depth = match checkpoint_depth(record) {
            Ok(depth) => depth,
            Err(reason) => return DebugNavigationV1::Unavailable(reason),
        };
        let start = self.cursor.map_or(0, |cursor| cursor.saturating_add(1));
        let width = self.transcript.wave_width;
        self.find_forward(
            start,
            |record| {
                scope.matches(record.invocation, width)
                    && checkpoint_depth(record).is_ok_and(|actual| actual <= depth)
            },
            DebugStopReasonV1::Step,
        )
    }

    pub fn step_out(&mut self, scope: &DebugScopeSelectorV1) -> DebugNavigationV1 {
        let Some(record) = self.current() else {
            return DebugNavigationV1::Unavailable(DebugInspectionUnavailableV1::NoCurrentRecord);
        };
        let depth = match checkpoint_depth(record) {
            Ok(depth) => depth,
            Err(reason) => return DebugNavigationV1::Unavailable(reason),
        };
        let start = self.cursor.map_or(0, |cursor| cursor.saturating_add(1));
        let width = self.transcript.wave_width;
        self.find_forward(
            start,
            |record| {
                scope.matches(record.invocation, width)
                    && checkpoint_depth(record).is_ok_and(|actual| actual < depth)
            },
            DebugStopReasonV1::Step,
        )
    }

    pub fn continue_forward(&mut self) -> DebugNavigationV1 {
        let start = self.cursor.map_or(0, |cursor| cursor.saturating_add(1));
        for index in start..self.transcript.records.len() {
            self.set_cursor(Some(index));
            if let Some(reason) = self.stop_reason(&self.transcript.records[index]) {
                return self.current_stop(reason);
            }
        }
        self.forward_end()
    }

    pub fn continue_forward_bounded(&mut self, max_events: usize) -> DebugNavigationV1 {
        if max_events == 0 {
            return self.cursor.map_or(DebugNavigationV1::Beginning, |index| {
                if let Some(record) = self.transcript.records.get(index) {
                    DebugNavigationV1::BudgetExhausted(DebugStopV1 {
                        record_index: index,
                        record_ordinal: record.ordinal,
                        reason: DebugStopReasonV1::Step,
                    })
                } else {
                    DebugNavigationV1::End
                }
            });
        }
        let start = self.cursor.map_or(0, |cursor| cursor.saturating_add(1));
        let end = start
            .saturating_add(max_events)
            .min(self.transcript.records.len());
        for index in start..end {
            self.set_cursor(Some(index));
            if let Some(reason) = self.stop_reason(&self.transcript.records[index]) {
                return self.current_stop(reason);
            }
        }
        if end < self.transcript.records.len() {
            let record = &self.transcript.records[end - 1];
            return DebugNavigationV1::BudgetExhausted(DebugStopV1 {
                record_index: end - 1,
                record_ordinal: record.ordinal,
                reason: DebugStopReasonV1::Step,
            });
        }
        self.forward_end()
    }

    pub fn continue_reverse(&mut self) -> DebugNavigationV1 {
        let Some(start) = self.cursor.and_then(|cursor| cursor.checked_sub(1)) else {
            self.set_cursor(None);
            return DebugNavigationV1::Beginning;
        };
        for index in (0..=start).rev() {
            self.set_cursor(Some(index));
            if let Some(reason) = self.stop_reason(&self.transcript.records[index]) {
                return self.current_stop(reason);
            }
        }
        self.set_cursor(None);
        DebugNavigationV1::Beginning
    }

    fn find_forward(
        &mut self,
        start: usize,
        predicate: impl Fn(&SimulationDebugRecordV1) -> bool,
        reason: DebugStopReasonV1,
    ) -> DebugNavigationV1 {
        for index in start..self.transcript.records.len() {
            if predicate(&self.transcript.records[index]) {
                return self.stop(index, reason);
            }
        }
        self.forward_end()
    }

    fn find_reverse(
        &mut self,
        start: usize,
        predicate: impl Fn(&SimulationDebugRecordV1) -> bool,
        reason: DebugStopReasonV1,
    ) -> DebugNavigationV1 {
        for index in (0..=start).rev() {
            if predicate(&self.transcript.records[index]) {
                return self.stop(index, reason);
            }
        }
        self.set_cursor(None);
        DebugNavigationV1::Beginning
    }

    fn stop_reason(&self, record: &SimulationDebugRecordV1) -> Option<DebugStopReasonV1> {
        for (breakpoint, hits) in self.breakpoints.iter().zip(&self.breakpoint_hits) {
            if breakpoint_base_matches(breakpoint, record, self.transcript.wave_width)
                && breakpoint
                    .hit_condition
                    .is_none_or(|condition| condition.matches(*hits))
            {
                return Some(DebugStopReasonV1::Breakpoint(breakpoint.id));
            }
        }
        for watchpoint in &self.watchpoints {
            if watchpoint.enabled
                && watchpoint_matches(watchpoint, record, self.transcript.wave_width)
            {
                return Some(DebugStopReasonV1::Watchpoint(watchpoint.id));
            }
        }
        None
    }

    fn breakpoint_hits_through_cursor(&self, breakpoint: &DebugBreakpointV1) -> u64 {
        self.transcript.records[..self.cursor_prefix_len()]
            .iter()
            .filter(|record| {
                breakpoint_base_matches(breakpoint, record, self.transcript.wave_width)
            })
            .count()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    fn watchpoint_hits_through_cursor(&self, watchpoint: &DebugWatchpointV1) -> u64 {
        self.transcript.records[..self.cursor_prefix_len()]
            .iter()
            .filter(|record| {
                watchpoint.enabled
                    && watchpoint_matches(watchpoint, record, self.transcript.wave_width)
            })
            .count()
            .try_into()
            .unwrap_or(u64::MAX)
    }

    fn cursor_prefix_len(&self) -> usize {
        self.cursor.map_or(0, |cursor| {
            cursor.saturating_add(1).min(self.transcript.records.len())
        })
    }

    fn set_cursor(&mut self, cursor: Option<usize>) {
        let current = self.cursor_prefix_len();
        let next = cursor.map_or(0, |index| {
            index.saturating_add(1).min(self.transcript.records.len())
        });
        let width = self.transcript.wave_width;
        if next > current {
            for record in &self.transcript.records[current..next] {
                for (breakpoint, hits) in self.breakpoints.iter().zip(&mut self.breakpoint_hits) {
                    if breakpoint_base_matches(breakpoint, record, width) {
                        *hits = hits.saturating_add(1);
                    }
                }
                for (watchpoint, hits) in self.watchpoints.iter().zip(&mut self.watchpoint_hits) {
                    if watchpoint.enabled && watchpoint_matches(watchpoint, record, width) {
                        *hits = hits.saturating_add(1);
                    }
                }
            }
        } else if next < current {
            for record in &self.transcript.records[next..current] {
                for (breakpoint, hits) in self.breakpoints.iter().zip(&mut self.breakpoint_hits) {
                    if breakpoint_base_matches(breakpoint, record, width) {
                        *hits = hits.saturating_sub(1);
                    }
                }
                for (watchpoint, hits) in self.watchpoints.iter().zip(&mut self.watchpoint_hits) {
                    if watchpoint.enabled && watchpoint_matches(watchpoint, record, width) {
                        *hits = hits.saturating_sub(1);
                    }
                }
            }
        }
        self.cursor = cursor;
    }

    fn stop(&mut self, index: usize, reason: DebugStopReasonV1) -> DebugNavigationV1 {
        self.set_cursor(Some(index));
        self.current_stop(reason)
    }

    fn current_stop(&self, reason: DebugStopReasonV1) -> DebugNavigationV1 {
        let index = self.cursor.expect("stopped replay cursor");
        DebugNavigationV1::Stopped(DebugStopV1 {
            record_index: index,
            record_ordinal: self.transcript.records[index].ordinal,
            reason,
        })
    }

    fn forward_end(&mut self) -> DebugNavigationV1 {
        match self.transcript.completeness {
            DebugTranscriptCompletenessV1::Complete
                if self.cursor != Some(self.transcript.records.len())
                    && self.transcript.terminal_fault.is_some() =>
            {
                let index = self.transcript.records.len();
                self.set_cursor(Some(index));
                let fault = self
                    .transcript
                    .terminal_fault
                    .as_ref()
                    .expect("guarded terminal fault");
                DebugNavigationV1::Stopped(DebugStopV1 {
                    record_index: index,
                    record_ordinal: fault.ordinal,
                    reason: DebugStopReasonV1::Fault,
                })
            }
            DebugTranscriptCompletenessV1::Complete => {
                self.set_cursor(Some(self.transcript.records.len()));
                DebugNavigationV1::End
            }
            DebugTranscriptCompletenessV1::Truncated(reason) => {
                DebugNavigationV1::TranscriptTruncated(reason)
            }
        }
    }

    pub fn stack(&self) -> DebugInspectionV1<&[SimulationDebugFrameV1]> {
        let Some(record) = self.current() else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::NoCurrentRecord);
        };
        let SimulationDebugRecordKindV1::Checkpoint { stack, .. } = &record.kind else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::NotCheckpoint);
        };
        match stack {
            SimulationDebugCollectionV1::Captured(frames) => DebugInspectionV1::Available(frames),
            SimulationDebugCollectionV1::Unavailable { reason, .. } => {
                DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::Stack(*reason))
            }
        }
    }

    pub fn scalar(&self, frame_depth: u32, value: ValueId) -> DebugInspectionV1<ScalarBitsV1> {
        let Some(record) = self.current() else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::NoCurrentRecord);
        };
        let SimulationDebugRecordKindV1::Checkpoint { stack, .. } = &record.kind else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::NotCheckpoint);
        };
        let SimulationDebugCollectionV1::Captured(frames) = stack else {
            let SimulationDebugCollectionV1::Unavailable { reason, .. } = stack else {
                unreachable!()
            };
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::Stack(*reason));
        };
        let Some(frame) = frames.iter().find(|frame| frame.depth == frame_depth) else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::UnknownFrame);
        };
        let SimulationDebugCollectionV1::Captured(values) = &frame.values else {
            let SimulationDebugCollectionV1::Unavailable { reason, .. } = &frame.values else {
                unreachable!()
            };
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::Values(*reason));
        };
        let Ok(index) = values.binary_search_by_key(&value, |binding| binding.value) else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::UnknownValue);
        };
        match &values[index].observed {
            SimulationDebugValueV1::Scalar(value) => DebugInspectionV1::Available(*value),
            _ => DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::NonScalarValue),
        }
    }

    pub fn memory(
        &self,
        allocation: u64,
        byte_offset: usize,
        byte_len: usize,
    ) -> DebugInspectionV1<DebugMemorySliceV1> {
        let Some(record) = self.current() else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::NoCurrentRecord);
        };
        let SimulationDebugRecordKindV1::Checkpoint { memory, .. } = &record.kind else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::NotCheckpoint);
        };
        let SimulationDebugCollectionV1::Captured(allocations) = memory else {
            let SimulationDebugCollectionV1::Unavailable { reason, .. } = memory else {
                unreachable!()
            };
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::Memory(*reason));
        };
        let Some(allocation) = allocations
            .iter()
            .find(|value| value.allocation == allocation)
        else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::UnknownAllocation);
        };
        let Some(end) = byte_offset.checked_add(byte_len) else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::RangeOverflow);
        };
        let (Some(bytes), Some(initialized)) = (
            allocation.bytes.get(byte_offset..end),
            allocation.initialized.get(byte_offset..end),
        ) else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::OutOfBounds);
        };
        DebugInspectionV1::Available(DebugMemorySliceV1 {
            allocation: allocation.allocation,
            byte_offset,
            address_space: allocation.address_space,
            bytes: bytes.to_vec(),
            initialized: initialized.to_vec(),
        })
    }

    pub fn bind_source_catalog(
        &mut self,
        module: &AdmittedSimulationModuleV1,
        catalog: DebugSourceCatalogV1,
    ) -> Result<(), DebuggerErrorV1> {
        if catalog.identity != self.transcript.identity {
            return Err(DebuggerErrorV1::SourceIdentityMismatch);
        }
        catalog.validate_sites(module)?;
        self.source_catalog = Some(catalog);
        Ok(())
    }

    pub fn source_spans(&self) -> DebugInspectionV1<&[DebugSourceSpanV1]> {
        let Some(record) = self.current() else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::NoCurrentRecord);
        };
        let Some(catalog) = &self.source_catalog else {
            return DebugInspectionV1::Unavailable(DebugInspectionUnavailableV1::SourceNotBound);
        };
        catalog
            .sites
            .binary_search_by_key(&site_key(record.site), |entry| site_key(entry.site))
            .ok()
            .and_then(|index| catalog.sites.get(index))
            .map(|entry| DebugInspectionV1::Available(entry.spans.as_slice()))
            .unwrap_or(DebugInspectionV1::Unavailable(
                DebugInspectionUnavailableV1::SourceUnavailable,
            ))
    }

    pub fn resolve_source_site(
        &self,
        site: SimulationDebugSiteV1,
    ) -> DebugInspectionV1<DebugSourceResolutionV1> {
        self.source_catalog
            .as_ref()
            .map(|catalog| DebugInspectionV1::Available(catalog.resolve_site(site)))
            .unwrap_or(DebugInspectionV1::Unavailable(
                DebugInspectionUnavailableV1::SourceNotBound,
            ))
    }

    pub fn resolve_source_location(
        &self,
        file: [u8; 32],
        byte_start: u64,
        byte_end: u64,
    ) -> DebugInspectionV1<DebugSourceResolutionV1> {
        self.source_catalog
            .as_ref()
            .map(|catalog| {
                DebugInspectionV1::Available(catalog.resolve_source(file, byte_start, byte_end))
            })
            .unwrap_or(DebugInspectionV1::Unavailable(
                DebugInspectionUnavailableV1::SourceNotBound,
            ))
    }
}

fn breakpoint_base_matches(
    breakpoint: &DebugBreakpointV1,
    record: &SimulationDebugRecordV1,
    width: DebugWaveWidthV1,
) -> bool {
    breakpoint.enabled
        && breakpoint.site.matches(record)
        && breakpoint.scope.matches(record.invocation, width)
        && breakpoint.predicate.evaluate(record) == PredicateResultV1::True
}

fn watchpoint_matches(
    watchpoint: &DebugWatchpointV1,
    record: &SimulationDebugRecordV1,
    width: DebugWaveWidthV1,
) -> bool {
    let SimulationDebugRecordKindV1::Memory {
        access,
        allocation,
        byte_offset,
        byte_len,
        value,
        ..
    } = &record.kind
    else {
        return false;
    };
    let access_matches = matches!(
        (watchpoint.access, access),
        (
            DebugWatchAccessV1::Read,
            SimulationDebugMemoryAccessV1::Read
                | SimulationDebugMemoryAccessV1::AtomicRead
                | SimulationDebugMemoryAccessV1::AtomicReadWriteCommitted
        ) | (
            DebugWatchAccessV1::Write,
            SimulationDebugMemoryAccessV1::WriteCommitted
                | SimulationDebugMemoryAccessV1::AtomicWriteCommitted
                | SimulationDebugMemoryAccessV1::AtomicReadWriteCommitted
        ) | (
            DebugWatchAccessV1::Atomic,
            SimulationDebugMemoryAccessV1::AtomicRead
                | SimulationDebugMemoryAccessV1::AtomicWriteCommitted
                | SimulationDebugMemoryAccessV1::AtomicReadWriteCommitted
        ) | (DebugWatchAccessV1::ReadWrite, _)
    );
    let ranges_overlap = byte_offset
        .checked_add(*byte_len)
        .zip(watchpoint.byte_offset.checked_add(watchpoint.byte_len))
        .is_some_and(|(event_end, watched_end)| {
            *byte_offset < watched_end && watchpoint.byte_offset < event_end
        });
    let value_matches = watchpoint.value_equals.is_none_or(
        |expected| matches!(value, SimulationDebugValueV1::Scalar(actual) if *actual == expected),
    );
    access_matches
        && *allocation == watchpoint.allocation
        && ranges_overlap
        && value_matches
        && watchpoint.scope.matches(record.invocation, width)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugSourceFileV1 {
    pub identity: [u8; 32],
    pub byte_len: u64,
    pub display_path: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DebugSourceSpanV1 {
    pub file: [u8; 32],
    pub byte_start: u64,
    pub byte_end: u64,
    pub line: u32,
    pub column: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugSourceSiteV1 {
    pub site: SimulationDebugSiteV1,
    pub spans: Vec<DebugSourceSpanV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebugSourceResolutionV1 {
    Resolved {
        site: SimulationDebugSiteV1,
        span: DebugSourceSpanV1,
    },
    Absent,
    Eliminated,
    ManyToOne,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebugSourceCatalogV1 {
    identity: DebugKirIdentityV1,
    files: Vec<DebugSourceFileV1>,
    sites: Vec<DebugSourceSiteV1>,
    eliminated: Vec<DebugSourceSpanV1>,
}

impl DebugSourceCatalogV1 {
    pub fn new(
        identity: DebugKirIdentityV1,
        files: Vec<DebugSourceFileV1>,
        sites: Vec<DebugSourceSiteV1>,
    ) -> Result<Self, DebuggerErrorV1> {
        Self::new_with_eliminated(identity, files, sites, Vec::new())
    }

    pub fn new_with_eliminated(
        identity: DebugKirIdentityV1,
        mut files: Vec<DebugSourceFileV1>,
        mut sites: Vec<DebugSourceSiteV1>,
        mut eliminated: Vec<DebugSourceSpanV1>,
    ) -> Result<Self, DebuggerErrorV1> {
        if identity.digest == [0; 32]
            || identity.canonical_len == 0
            || files.len() > MAX_DEBUGGER_SOURCE_FILES_V1
            || sites.len() > MAX_DEBUGGER_SOURCE_SITES_V1
        {
            return Err(DebuggerErrorV1::InvalidSourceCatalog);
        }
        files.sort_unstable_by_key(|file| file.identity);
        if files
            .windows(2)
            .any(|pair| pair[0].identity == pair[1].identity)
            || files.iter().any(|file| {
                file.identity == [0; 32]
                    || file.byte_len == 0
                    || file.display_path.is_empty()
                    || file.display_path.len() > MAX_DEBUGGER_SOURCE_PATH_BYTES_V1
                    || file.display_path.contains('\0')
            })
        {
            return Err(DebuggerErrorV1::InvalidSourceCatalog);
        }
        sites.sort_unstable_by_key(|entry| site_key(entry.site));
        if sites
            .windows(2)
            .any(|pair| site_key(pair[0].site) == site_key(pair[1].site))
        {
            return Err(DebuggerErrorV1::InvalidSourceCatalog);
        }
        for site in &mut sites {
            site.spans.sort_unstable_by_key(source_span_key);
            if site.spans.windows(2).any(|pair| pair[0] == pair[1]) {
                return Err(DebuggerErrorV1::InvalidSourceCatalog);
            }
        }
        eliminated.sort_unstable_by_key(source_span_key);
        if eliminated.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(DebuggerErrorV1::InvalidSourceCatalog);
        }
        let Some(span_count) = sites
            .iter()
            .try_fold(0_usize, |count, site| count.checked_add(site.spans.len()))
            .and_then(|count| count.checked_add(eliminated.len()))
        else {
            return Err(DebuggerErrorV1::InvalidSourceCatalog);
        };
        if span_count > MAX_DEBUGGER_SOURCE_SPANS_V1 {
            return Err(DebuggerErrorV1::InvalidSourceCatalog);
        }
        for site in &sites {
            for span in &site.spans {
                if !source_span_valid(&files, *span, false) {
                    return Err(DebuggerErrorV1::InvalidSourceCatalog);
                }
            }
        }
        if eliminated
            .iter()
            .any(|span| !source_span_valid(&files, *span, true))
        {
            return Err(DebuggerErrorV1::InvalidSourceCatalog);
        }
        Ok(Self {
            identity,
            files,
            sites,
            eliminated,
        })
    }

    pub const fn identity(&self) -> DebugKirIdentityV1 {
        self.identity
    }

    pub fn files(&self) -> &[DebugSourceFileV1] {
        &self.files
    }

    pub fn sites(&self) -> &[DebugSourceSiteV1] {
        &self.sites
    }

    pub fn eliminated(&self) -> &[DebugSourceSpanV1] {
        &self.eliminated
    }

    pub fn resolve_site(&self, site: SimulationDebugSiteV1) -> DebugSourceResolutionV1 {
        let Ok(index) = self
            .sites
            .binary_search_by_key(&site_key(site), |entry| site_key(entry.site))
        else {
            return DebugSourceResolutionV1::Absent;
        };
        match self.sites[index].spans.as_slice() {
            [span] => DebugSourceResolutionV1::Resolved { site, span: *span },
            [] => DebugSourceResolutionV1::Absent,
            _ => DebugSourceResolutionV1::ManyToOne,
        }
    }

    pub fn resolve_source(
        &self,
        file: [u8; 32],
        byte_start: u64,
        byte_end: u64,
    ) -> DebugSourceResolutionV1 {
        let Ok(file_index) = self
            .files
            .binary_search_by_key(&file, |entry| entry.identity)
        else {
            return DebugSourceResolutionV1::Absent;
        };
        if byte_start >= byte_end || byte_end > self.files[file_index].byte_len {
            return DebugSourceResolutionV1::Absent;
        }
        let overlaps = |span: &DebugSourceSpanV1| {
            span.file == file && span.byte_start < byte_end && byte_start < span.byte_end
        };
        let mut found = None;
        for entry in &self.sites {
            for span in entry.spans.iter().filter(|span| overlaps(span)) {
                if found.is_some() {
                    return DebugSourceResolutionV1::ManyToOne;
                }
                found = Some((entry.site, *span));
            }
        }
        if let Some((site, span)) = found {
            return DebugSourceResolutionV1::Resolved { site, span };
        }
        if self.eliminated.iter().any(overlaps) {
            DebugSourceResolutionV1::Eliminated
        } else {
            DebugSourceResolutionV1::Absent
        }
    }

    fn validate_sites(&self, module: &AdmittedSimulationModuleV1) -> Result<(), DebuggerErrorV1> {
        let actual = DebugKirIdentityV1 {
            digest: *module.identity().digest(),
            canonical_len: module.identity().canonical_length(),
        };
        if actual != self.identity {
            return Err(DebuggerErrorV1::SourceIdentityMismatch);
        }
        for entry in &self.sites {
            let Some(function) = module.module().functions.get(entry.site.function_ordinal) else {
                return Err(DebuggerErrorV1::InvalidSourceCatalogSite(entry.site));
            };
            let Some(block) = function.body.as_ref().and_then(|body| {
                body.blocks
                    .iter()
                    .find(|block| block.id == entry.site.block)
            }) else {
                return Err(DebuggerErrorV1::InvalidSourceCatalogSite(entry.site));
            };
            if block
                .operations
                .get(entry.site.operation as usize)
                .is_none()
            {
                return Err(DebuggerErrorV1::InvalidSourceCatalogSite(entry.site));
            }
        }
        Ok(())
    }
}

fn source_span_key(span: &DebugSourceSpanV1) -> ([u8; 32], u64, u64, u32, u32) {
    (
        span.file,
        span.byte_start,
        span.byte_end,
        span.line,
        span.column,
    )
}

fn source_span_valid(
    files: &[DebugSourceFileV1],
    span: DebugSourceSpanV1,
    allow_empty: bool,
) -> bool {
    files
        .binary_search_by_key(&span.file, |file| file.identity)
        .ok()
        .is_some_and(|index| {
            span.byte_start <= span.byte_end
                && (allow_empty || span.byte_start < span.byte_end)
                && span.byte_end <= files[index].byte_len
                && span.line > 0
                && span.column > 0
        })
}

fn site_key(site: SimulationDebugSiteV1) -> (usize, u32, u32) {
    (site.function_ordinal, site.block.0, site.operation)
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DebuggerErrorV1 {
    LimitOutOfRange {
        field: &'static str,
        actual: usize,
        maximum: usize,
    },
    AllocationFailure,
    InvalidOrDuplicateIdentity,
    PredicateLimit,
    InvalidHitCondition,
    InvalidSourceCatalog,
    InvalidSourceCatalogSite(SimulationDebugSiteV1),
    SourceIdentityMismatch,
}

impl fmt::Display for DebuggerErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "fe2o3 KIR debugger failed: {self:?}")
    }
}

impl Error for DebuggerErrorV1 {}
