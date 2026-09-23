//! Sealed, bounded runtime observations joined to one immutable legacy transcript.
//! Numeric IDs are run-local; capture_instance additionally distinguishes owners
//! within this process, not across process restarts or backend connections.
use super::{DebugTranscriptV1, SimulationDebugRecordV1};
use fe2o3_kir_sim::{
    SimulationDebugOriginContextV1 as Context,
    SimulationDebugOriginUnavailableV1 as RuntimeMissing, SimulationDebugSinkControlV1 as Control,
};
use std::mem::size_of;

mod allocations;
mod capture;
mod config;
mod frames;
mod navigation;
mod pair;
mod session;
use allocations::AllocationRetention;
pub use allocations::{RuntimeAllocationMissingV1, RuntimeAllocationObservationV1};
pub use capture::{capture_debugger_observed_run_v1, capture_debugger_observed_scheduled_run_v1};
pub use config::*;
use frames::FrameRetention;
pub use frames::{RuntimeFrameMissingV1, RuntimeFrameObservationV1, RuntimeFrameViewV1};
pub use pair::{RuntimeOriginPairErrorV1, RuntimeOriginScanWorkV1};
pub use session::{
    DebugObservedSessionV1, RuntimeNavigationDirectionV1, RuntimeRejectedBreakpointV1,
    RuntimeRejectedBreakpointsV1, RuntimeReplayWorkV1, RuntimeSessionErrorV1,
};

use RuntimeObservationCoverageV1 as Coverage;
use RuntimeObservationCutoffV1 as Cutoff;
use RuntimeOriginMissingV1 as Missing;
const MAX_ROWS: usize = 1_000_000;
const MAX_METADATA: usize = 32 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Limits {
    rows: usize,
    bytes: usize,
}

impl Limits {
    fn new(rows: usize, bytes: usize) -> Result<Self, &'static str> {
        if !(1..=MAX_ROWS).contains(&rows)
            || !(size_of::<Retention>() + size_of::<Row>()..=MAX_METADATA).contains(&bytes)
        {
            return Err("origin metadata limits");
        }
        Ok(Self { rows, bytes })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeObservationCutoffV1 {
    RowLimit,
    FrameRowLimit,
    TransitionLimit,
    ValidationWorkLimit,
    ByteLimit,
    AllocationFailure,
    InvalidCapacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeObservationCoverageV1 {
    Disabled,
    Complete,
    PrefixTruncated(Cutoff),
    InvalidJoin,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Status {
    Available,
    RuntimeUnavailable(RuntimeMissing),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Row {
    activation: u64,
    attempt: u64,
    status: Status,
}
const _: () = assert!(size_of::<Row>() <= 32);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeOriginMissingV1 {
    NoSuchRecord,
    NoCurrentRecord,
    Disabled,
    PrefixTruncated(Cutoff),
    RuntimeUnavailable(RuntimeMissing),
    InvalidJoin,
}

struct Pending {
    index: usize,
    row: Result<Row, Coverage>,
}

struct Retention {
    rows: Vec<Row>,
    limits: Option<Limits>,
    capacity_limit: usize,
    coverage: Coverage,
}

impl Retention {
    fn new(limits: Option<Limits>) -> Self {
        Self::with_reservation(limits, |count| {
            let mut rows = Vec::new();
            rows.try_reserve_exact(count).map_err(|_| ())?;
            Ok(rows)
        })
    }

    fn with_reservation(
        limits: Option<Limits>,
        reserve: impl FnOnce(usize) -> Result<Vec<Row>, ()>,
    ) -> Self {
        let mut retained = Self {
            rows: Vec::new(),
            limits,
            capacity_limit: 0,
            coverage: Coverage::Disabled,
        };
        let Some(limits) = limits else {
            return retained;
        };
        let count = limits
            .rows
            .min((limits.bytes - size_of::<Self>()) / size_of::<Row>());
        retained.capacity_limit = count;
        retained.coverage = Coverage::Complete;
        match reserve(count) {
            Err(()) => retained.coverage = Coverage::PrefixTruncated(Cutoff::AllocationFailure),
            Ok(rows) => {
                let bytes = rows
                    .capacity()
                    .checked_mul(size_of::<Row>())
                    .and_then(|bytes| bytes.checked_add(size_of::<Self>()));
                if !rows.is_empty()
                    || rows.capacity() < count
                    || bytes.is_none_or(|bytes| bytes > limits.bytes)
                {
                    retained.coverage = Coverage::PrefixTruncated(Cutoff::InvalidCapacity);
                } else {
                    retained.rows = rows;
                }
            }
        }
        retained
    }

    fn metadata_bytes(&self) -> usize {
        // Construction validated the multiplication and no later growth occurs.
        size_of::<Self>() + self.rows.capacity() * size_of::<Row>()
    }

    fn prepare(&self, index: usize, record: &SimulationDebugRecordV1, context: Context) -> Pending {
        let row = if self.coverage != Coverage::Complete {
            Err(self.coverage)
        } else if index != self.rows.len() || u64::try_from(index) != Ok(record.ordinal) {
            Err(Coverage::InvalidJoin)
        } else if self.rows.len() == self.capacity_limit {
            let cutoff = if self.rows.len() == self.limits.unwrap().rows {
                Cutoff::RowLimit
            } else {
                Cutoff::ByteLimit
            };
            Err(Coverage::PrefixTruncated(cutoff))
        } else {
            match context {
                Context::Available(origin)
                    if origin.invocation() == record.invocation
                        && origin.site() == record.site
                        && origin.activation() != 0
                        && origin.attempt() != 0 =>
                {
                    Ok(Row {
                        activation: origin.activation(),
                        attempt: origin.attempt(),
                        status: Status::Available,
                    })
                }
                Context::Available(_) => Err(Coverage::InvalidJoin),
                Context::Unavailable(reason) => Ok(Row {
                    activation: 0,
                    attempt: 0,
                    status: Status::RuntimeUnavailable(reason),
                }),
            }
        };
        Pending { index, row }
    }

    fn commit(&mut self, pending: Pending, control: Control, accepted_len: usize) {
        let accepted = matches!(control, Control::Continue | Control::Stop);
        if pending.index.checked_add(usize::from(accepted)) != Some(accepted_len) {
            self.coverage = Coverage::InvalidJoin;
            return;
        }
        if !accepted || self.coverage != Coverage::Complete {
            return;
        }
        match pending.row {
            Ok(row) => {
                if pending.index != self.rows.len() || self.rows.len() >= self.capacity_limit {
                    self.coverage = Coverage::InvalidJoin;
                    return;
                }
                self.rows.push(row); // Capacity was pre-reserved and checked.
            }
            Err(coverage) => self.coverage = coverage,
        }
    }
}

/// Distinct completion domains: legacy completeness, metadata coverage and each
/// record's runtime availability must be interpreted independently.
pub type RuntimeOriginCoverageV1 = RuntimeObservationCoverageV1;
pub type RuntimeFrameCoverageV1 = RuntimeObservationCoverageV1;
pub type RuntimeAllocationCoverageV1 = RuntimeObservationCoverageV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeOriginUsageV1 {
    pub retained_rows: usize,
    pub capacity_rows: usize,
    pub metadata_bytes: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeFrameUsageV1 {
    pub retained_records: usize,
    pub retained_frames: usize,
    pub record_capacity: usize,
    pub frame_capacity: usize,
    pub metadata_bytes: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RuntimeAllocationUsageV1 {
    pub retained_records: usize,
    pub retained_transitions: usize,
    pub record_capacity: usize,
    pub transition_capacity: usize,
    pub metadata_bytes: usize,
    pub validation_work_limit: usize,
    pub validation_work_used: usize,
}

/// No Clone, Deserialize, public constructor, attach, replace or mutable escape.
pub struct DebugObservedTranscriptV1 {
    transcript: DebugTranscriptV1,
    retained: Retention,
    frames: FrameRetention,
    allocations: AllocationRetention,
    capture_instance: u64,
    options: RuntimeObservationOptionsV1,
}
pub struct DebugObservedRunV1 {
    execution: Result<fe2o3_kir_sim::SimulationExecutionV1, fe2o3_kir_sim::SimulationErrorV1>,
    transcript: DebugObservedTranscriptV1,
}
impl DebugObservedRunV1 {
    pub fn execution(
        &self,
    ) -> &Result<fe2o3_kir_sim::SimulationExecutionV1, fe2o3_kir_sim::SimulationErrorV1> {
        &self.execution
    }
    pub fn transcript(&self) -> &DebugObservedTranscriptV1 {
        &self.transcript
    }
    pub fn into_parts(
        self,
    ) -> (
        Result<fe2o3_kir_sim::SimulationExecutionV1, fe2o3_kir_sim::SimulationErrorV1>,
        DebugObservedTranscriptV1,
    ) {
        (self.execution, self.transcript)
    }
}

#[derive(Clone, Copy, Debug)]
pub struct RuntimeOriginObservationV1<'capture> {
    record: &'capture SimulationDebugRecordV1,
    row: &'capture Row,
}
impl RuntimeOriginObservationV1<'_> {
    pub fn record(&self) -> &SimulationDebugRecordV1 {
        self.record
    }
    pub fn activation(&self) -> u64 {
        self.row.activation
    }
    pub fn attempt(&self) -> u64 {
        self.row.attempt
    }
    pub fn invocation(&self) -> fe2o3_kir_sim::SimulationInvocationV1 {
        self.record.invocation
    }
    pub fn site(&self) -> fe2o3_kir_sim::SimulationDebugSiteV1 {
        self.record.site
    }
}
impl DebugObservedTranscriptV1 {
    pub fn legacy(&self) -> &DebugTranscriptV1 {
        &self.transcript
    }
    pub const fn capture_instance(&self) -> u64 {
        self.capture_instance
    }
    pub const fn options(&self) -> RuntimeObservationOptionsV1 {
        self.options
    }
    pub fn fixed_owner_metadata_bytes(&self) -> usize {
        size_of::<Self>()
            - size_of::<DebugTranscriptV1>()
            - size_of::<Retention>()
            - size_of::<FrameRetention>()
            - size_of::<AllocationRetention>()
    }
    pub fn origin_coverage(&self) -> RuntimeOriginCoverageV1 {
        self.retained.coverage
    }
    pub fn origin_metadata_usage(&self) -> RuntimeOriginUsageV1 {
        self.retained.usage()
    }
    pub fn frame_coverage(&self) -> RuntimeFrameCoverageV1 {
        self.frames.coverage
    }
    pub fn frame_metadata_usage(&self) -> RuntimeFrameUsageV1 {
        self.frames.usage()
    }
    pub fn allocation_coverage(&self) -> RuntimeAllocationCoverageV1 {
        self.allocations.coverage()
    }
    pub fn allocation_metadata_usage(&self) -> RuntimeAllocationUsageV1 {
        self.allocations.usage()
    }
    pub fn origin_at(
        &self,
        index: usize,
    ) -> Result<RuntimeOriginObservationV1<'_>, RuntimeOriginMissingV1> {
        joined_origin(&self.retained, &self.transcript, index)
    }
    pub fn frames_at(
        &self,
        index: usize,
    ) -> Result<RuntimeFrameObservationV1<'_>, RuntimeFrameMissingV1> {
        self.frames.at(&self.transcript, index)
    }
    pub fn allocations_at(
        &self,
        index: usize,
    ) -> Result<RuntimeAllocationObservationV1<'_>, RuntimeAllocationMissingV1> {
        self.allocations.at(&self.transcript, index)
    }
}
impl Retention {
    fn usage(&self) -> RuntimeOriginUsageV1 {
        RuntimeOriginUsageV1 {
            retained_rows: self.rows.len(),
            capacity_rows: self.rows.capacity(),
            metadata_bytes: self.metadata_bytes(),
        }
    }
    fn validate_seal(&mut self, len: usize) {
        if self.rows.len() > len || (self.coverage == Coverage::Complete && self.rows.len() != len)
        {
            self.coverage = Coverage::InvalidJoin;
        }
    }
}
fn joined_origin<'a>(
    retained: &'a Retention,
    transcript: &'a DebugTranscriptV1,
    index: usize,
) -> Result<RuntimeOriginObservationV1<'a>, Missing> {
    let record = transcript
        .records()
        .get(index)
        .ok_or(Missing::NoSuchRecord)?;
    if retained.coverage == Coverage::InvalidJoin {
        return Err(Missing::InvalidJoin);
    }
    let Some(row) = retained.rows.get(index) else {
        return Err(match retained.coverage {
            Coverage::Disabled => Missing::Disabled,
            Coverage::PrefixTruncated(reason) => Missing::PrefixTruncated(reason),
            Coverage::Complete | Coverage::InvalidJoin => Missing::InvalidJoin,
        });
    };
    match row.status {
        Status::Available => Ok(RuntimeOriginObservationV1 { record, row }),
        Status::RuntimeUnavailable(reason) => Err(Missing::RuntimeUnavailable(reason)),
    }
}

#[cfg(test)]
mod tests;
