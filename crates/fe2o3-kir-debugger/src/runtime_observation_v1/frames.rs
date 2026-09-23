//! All-or-none bounded frame rosters; no guessed depth-to-activation mapping.
use super::*;
use fe2o3_kir_sim::{
    SimulationDebugCheckpointFramesV1 as ContextFrames, SimulationDebugCollectionV1 as Collection,
    SimulationDebugFrameOperationV1, SimulationDebugFrameOriginUnavailableV1 as FrameUnavailable,
    SimulationDebugFrameOriginV1, SimulationDebugFrameOriginsV1 as LiveFrames,
    SimulationDebugFrameParentV1, SimulationDebugFrameV1, SimulationDebugRecordKindV1 as Kind,
    SimulationDebugUnavailableReasonV1 as LegacyUnavailable,
};

#[derive(Clone, Copy, Debug)]
struct FrameRow {
    activation: u64,
    operation_state: SimulationDebugFrameOperationV1,
    parent: SimulationDebugFrameParentV1,
}
impl From<SimulationDebugFrameOriginV1> for FrameRow {
    fn from(value: SimulationDebugFrameOriginV1) -> Self {
        Self {
            activation: value.activation(),
            operation_state: value.operation_state(),
            parent: value.parent(),
        }
    }
}
#[derive(Clone, Copy, Debug)]
struct FrameIndex {
    start: u32,
    count: u32,
    status: u8,
    reason: u8,
    required: u64,
}
const _: () = assert!(size_of::<FrameRow>() <= 128);
const _: () = assert!(size_of::<FrameIndex>() <= 32);

pub(super) struct FrameRetention {
    rows: Vec<FrameRow>,
    indexes: Vec<FrameIndex>,
    limits: Option<RuntimeFrameCaptureLimitsV1>,
    row_limit: usize,
    index_limit: usize,
    pub(super) coverage: Coverage,
}
pub(super) fn minimum_metadata_bytes() -> usize {
    size_of::<FrameRetention>() + size_of::<FrameRow>() + size_of::<FrameIndex>()
}
pub(super) struct PendingFrames<'a> {
    index: usize,
    result: Result<(FrameIndex, Option<LiveFrames<'a>>), Coverage>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeFrameMissingV1 {
    NoSuchRecord,
    NoCurrentRecord,
    Disabled,
    PrefixTruncated(Cutoff),
    RuntimeUnavailable(FrameUnavailable),
    InvalidJoin,
}
#[derive(Clone, Copy, Debug)]
pub struct RuntimeFrameObservationV1<'a> {
    record: &'a SimulationDebugRecordV1,
    legacy: &'a [SimulationDebugFrameV1],
    rows: &'a [FrameRow],
}
#[derive(Clone, Copy, Debug)]
pub struct RuntimeFrameViewV1<'a> {
    legacy: &'a SimulationDebugFrameV1,
    row: &'a FrameRow,
}
impl<'a> RuntimeFrameObservationV1<'a> {
    pub fn record(self) -> &'a SimulationDebugRecordV1 {
        self.record
    }
    pub fn len(self) -> usize {
        self.rows.len()
    }
    pub fn is_empty(self) -> bool {
        self.rows.is_empty()
    }
    pub fn get(self, index: usize) -> Option<RuntimeFrameViewV1<'a>> {
        Some(RuntimeFrameViewV1 {
            legacy: self.legacy.get(index)?,
            row: self.rows.get(index)?,
        })
    }
    pub fn by_activation(self, activation: u64) -> Option<RuntimeFrameViewV1<'a>> {
        self.rows
            .iter()
            .position(|row| row.activation == activation)
            .and_then(|index| self.get(index))
    }
}
impl<'a> RuntimeFrameViewV1<'a> {
    pub fn legacy(self) -> &'a SimulationDebugFrameV1 {
        self.legacy
    }
    pub fn activation(self) -> u64 {
        self.row.activation
    }
    pub fn operation_state(self) -> SimulationDebugFrameOperationV1 {
        self.row.operation_state
    }
    pub fn parent(self) -> SimulationDebugFrameParentV1 {
        self.row.parent
    }
}

impl FrameRetention {
    pub(super) fn new(limits: Option<RuntimeFrameCaptureLimitsV1>) -> Self {
        let mut retained = Self {
            rows: Vec::new(),
            indexes: Vec::new(),
            limits,
            row_limit: 0,
            index_limit: 0,
            coverage: Coverage::Disabled,
        };
        let Some(limits) = limits else {
            return retained;
        };
        let available = limits.bytes - size_of::<Self>();
        retained.index_limit = limits
            .records
            .min((available - size_of::<FrameRow>()) / size_of::<FrameIndex>());
        let remaining = available - retained.index_limit * size_of::<FrameIndex>();
        retained.row_limit = limits.rows.min(remaining / size_of::<FrameRow>());
        retained.coverage = Coverage::Complete;
        if retained
            .indexes
            .try_reserve_exact(retained.index_limit)
            .is_err()
            || retained.rows.try_reserve_exact(retained.row_limit).is_err()
        {
            retained.indexes = Vec::new();
            retained.rows = Vec::new();
            retained.coverage = Coverage::PrefixTruncated(Cutoff::AllocationFailure);
        } else if retained
            .checked_bytes()
            .is_none_or(|bytes| bytes > limits.bytes)
            || retained.indexes.capacity() < retained.index_limit
            || retained.rows.capacity() < retained.row_limit
        {
            retained.indexes = Vec::new();
            retained.rows = Vec::new();
            retained.coverage = Coverage::PrefixTruncated(Cutoff::InvalidCapacity);
        }
        retained
    }
    fn checked_bytes(&self) -> Option<usize> {
        self.indexes
            .capacity()
            .checked_mul(size_of::<FrameIndex>())?
            .checked_add(self.rows.capacity().checked_mul(size_of::<FrameRow>())?)?
            .checked_add(size_of::<Self>())
    }
    pub(super) fn usage(&self) -> RuntimeFrameUsageV1 {
        RuntimeFrameUsageV1 {
            retained_records: self.indexes.len(),
            retained_frames: self.rows.len(),
            record_capacity: self.indexes.capacity(),
            frame_capacity: self.rows.capacity(),
            metadata_bytes: self
                .checked_bytes()
                .expect("validated immutable capacities"),
        }
    }
    pub(super) fn prepare<'a>(
        &self,
        index: usize,
        record: &SimulationDebugRecordV1,
        frames: ContextFrames<'a>,
    ) -> PendingFrames<'a> {
        let result = self.prepare_inner(index, record, frames);
        PendingFrames { index, result }
    }
    fn prepare_inner<'a>(
        &self,
        index: usize,
        record: &SimulationDebugRecordV1,
        frames: ContextFrames<'a>,
    ) -> Result<(FrameIndex, Option<LiveFrames<'a>>), Coverage> {
        if self.coverage != Coverage::Complete {
            return Err(self.coverage);
        }
        if index != self.indexes.len() || u64::try_from(index) != Ok(record.ordinal) {
            return Err(Coverage::InvalidJoin);
        }
        let limits = self.limits.expect("enabled");
        if self.indexes.len() == self.index_limit {
            return Err(Coverage::PrefixTruncated(
                if self.index_limit == limits.records {
                    Cutoff::RowLimit
                } else {
                    Cutoff::ByteLimit
                },
            ));
        }
        let mut row = FrameIndex {
            start: self.rows.len() as u32,
            count: 0,
            status: 0,
            reason: 0,
            required: 0,
        };
        match frames {
            ContextFrames::Unavailable(reason) => {
                match reason {
                    FrameUnavailable::NotRequested => row.status = 1,
                    FrameUnavailable::NotCheckpoint => {
                        if matches!(record.kind, Kind::Checkpoint { .. }) {
                            return Err(Coverage::InvalidJoin);
                        }
                        row.status = 2;
                    }
                    FrameUnavailable::LegacyStackUnavailable { reason, required } => {
                        if !matches!(&record.kind, Kind::Checkpoint { stack: Collection::Unavailable {
                            reason: actual, required: count }, .. } if *actual == reason && *count == required)
                        {
                            return Err(Coverage::InvalidJoin);
                        }
                        row.status = 3;
                        row.reason = encode_legacy_reason(reason);
                        row.required = required;
                    }
                    FrameUnavailable::IdentityInvariant => row.status = 4,
                }
                Ok((row, None))
            }
            ContextFrames::Captured(live) => {
                let Kind::Checkpoint {
                    stack: Collection::Captured(legacy),
                    ..
                } = &record.kind
                else {
                    return Err(Coverage::InvalidJoin);
                };
                if live.is_empty()
                    || live.len() != legacy.len()
                    || live.len() > fe2o3_kir_sim::MAX_DEBUG_FRAMES_PER_CHECKPOINT_V1
                {
                    return Err(Coverage::InvalidJoin);
                }
                if live.len() > self.row_limit.saturating_sub(self.rows.len()) {
                    return Err(Coverage::PrefixTruncated(
                        if self.row_limit == limits.rows {
                            Cutoff::FrameRowLimit
                        } else {
                            Cutoff::ByteLimit
                        },
                    ));
                }
                let mut previous: Option<SimulationDebugFrameOriginV1> = None;
                for (depth, frame) in legacy.iter().enumerate() {
                    let live_row = live.get(depth).ok_or(Coverage::InvalidJoin)?;
                    if live_row.legacy_depth() != frame.depth
                        || live_row.function_ordinal() != frame.function_ordinal
                        || live_row.block() != frame.block
                        || live_row.next_operation() != frame.next_operation
                        || live_row.activation() == 0
                    {
                        return Err(Coverage::InvalidJoin);
                    }
                    match (previous, live_row.parent()) {
                        (None, SimulationDebugFrameParentV1::Root) => {}
                        (
                            Some(caller),
                            SimulationDebugFrameParentV1::Caller {
                                activation,
                                attempt,
                                call_site,
                            },
                        ) if live_row.activation() > caller.activation()
                            && activation == caller.activation()
                            && matches!(caller.operation_state(),
                                    SimulationDebugFrameOperationV1::Suspended { attempt: pending, site }
                                    if pending == attempt && site == call_site) => {}
                        _ => return Err(Coverage::InvalidJoin),
                    }
                    previous = Some(live_row);
                }
                row.count = live.len() as u32;
                Ok((row, Some(live)))
            }
        }
    }
    pub(super) fn commit(
        &mut self,
        pending: PendingFrames<'_>,
        control: Control,
        accepted_len: usize,
    ) {
        let accepted = matches!(control, Control::Continue | Control::Stop);
        if pending.index.checked_add(usize::from(accepted)) != Some(accepted_len) {
            self.coverage = Coverage::InvalidJoin;
            return;
        }
        if !accepted || self.coverage != Coverage::Complete {
            return;
        }
        match pending.result {
            Err(coverage) => self.coverage = coverage,
            Ok((row, live)) => {
                if pending.index != self.indexes.len() || self.indexes.len() >= self.index_limit {
                    self.coverage = Coverage::InvalidJoin;
                    return;
                }
                if let Some(live) = live {
                    let old_len = self.rows.len();
                    for index in 0..live.len() {
                        let Some(value) = live.get(index) else {
                            self.rows.truncate(old_len);
                            self.coverage = Coverage::InvalidJoin;
                            return;
                        };
                        self.rows.push(value.into());
                    }
                }
                self.indexes.push(row);
            }
        }
    }
    pub(super) fn validate_seal(&mut self, len: usize) {
        if self.indexes.len() > len
            || (self.coverage == Coverage::Complete && self.indexes.len() != len)
        {
            self.coverage = Coverage::InvalidJoin;
        }
    }
    pub(super) fn at<'a>(
        &'a self,
        transcript: &'a DebugTranscriptV1,
        index: usize,
    ) -> Result<RuntimeFrameObservationV1<'a>, RuntimeFrameMissingV1> {
        use RuntimeFrameMissingV1 as Error;
        let record = transcript.records().get(index).ok_or(Error::NoSuchRecord)?;
        if self.coverage == Coverage::InvalidJoin {
            return Err(Error::InvalidJoin);
        }
        let Some(row) = self.indexes.get(index) else {
            return Err(match self.coverage {
                Coverage::Disabled => Error::Disabled,
                Coverage::PrefixTruncated(reason) => Error::PrefixTruncated(reason),
                _ => Error::InvalidJoin,
            });
        };
        let unavailable = match row.status {
            0 => None,
            1 => Some(FrameUnavailable::NotRequested),
            2 => Some(FrameUnavailable::NotCheckpoint),
            3 => Some(FrameUnavailable::LegacyStackUnavailable {
                reason: decode_legacy_reason(row.reason).ok_or(Error::InvalidJoin)?,
                required: row.required,
            }),
            4 => Some(FrameUnavailable::IdentityInvariant),
            _ => return Err(Error::InvalidJoin),
        };
        if let Some(reason) = unavailable {
            return Err(Error::RuntimeUnavailable(reason));
        }
        let Kind::Checkpoint {
            stack: Collection::Captured(legacy),
            ..
        } = &record.kind
        else {
            return Err(Error::InvalidJoin);
        };
        let start = row.start as usize;
        let end = start
            .checked_add(row.count as usize)
            .ok_or(Error::InvalidJoin)?;
        let rows = self.rows.get(start..end).ok_or(Error::InvalidJoin)?;
        if rows.len() != legacy.len() {
            return Err(Error::InvalidJoin);
        }
        Ok(RuntimeFrameObservationV1 {
            record,
            legacy,
            rows,
        })
    }
}
fn encode_legacy_reason(reason: LegacyUnavailable) -> u8 {
    match reason {
        LegacyUnavailable::FrameLimit => 0,
        LegacyUnavailable::ValueLimit => 1,
        LegacyUnavailable::AllocationLimit => 2,
        LegacyUnavailable::MemoryByteLimit => 3,
        LegacyUnavailable::AllocationFailure => 4,
        LegacyUnavailable::NotCaptured => 5,
    }
}
fn decode_legacy_reason(reason: u8) -> Option<LegacyUnavailable> {
    Some(match reason {
        0 => LegacyUnavailable::FrameLimit,
        1 => LegacyUnavailable::ValueLimit,
        2 => LegacyUnavailable::AllocationLimit,
        3 => LegacyUnavailable::MemoryByteLimit,
        4 => LegacyUnavailable::AllocationFailure,
        5 => LegacyUnavailable::NotCaptured,
        _ => return None,
    })
}
