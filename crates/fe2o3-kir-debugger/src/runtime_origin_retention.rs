//! Private opt-in retention prototype. No protocol, CLI or resource authority.
//! The primary enables this module only under cfg(test) until owner review.
use super::{DebugTranscriptV1, SimulationDebugRecordV1};
use fe2o3_kir_sim::{
    SimulationDebugOriginContextV1 as Context,
    SimulationDebugOriginUnavailableV1 as RuntimeMissing, SimulationDebugSinkControlV1 as Control,
};
use std::mem::size_of;

#[path = "runtime_origin_retention/capture.rs"]
mod capture;
#[path = "runtime_origin_retention/pair.rs"]
mod pair;
use capture::OriginCollector;
use pair::{PairError, ScanBudget};
#[path = "runtime_origin_retention/collector_tests.rs"]
mod collector_tests;
#[path = "runtime_origin_retention/fixtures_tests.rs"]
mod fixtures;
#[path = "runtime_origin_retention/runtime_tests.rs"]
mod runtime_tests;

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
enum Cutoff {
    RowLimit,
    ByteLimit,
    AllocationFailure,
    InvalidCapacity,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Coverage {
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
enum Missing {
    NoSuchRecord,
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

    fn seal(mut self, transcript: DebugTranscriptV1) -> ObservedTranscript {
        if self.rows.len() > transcript.records().len()
            || (self.coverage == Coverage::Complete
                && self.rows.len() != transcript.records().len())
        {
            self.coverage = Coverage::InvalidJoin;
        }
        ObservedTranscript {
            transcript,
            retained: self,
        }
    }
}

// No Clone/Deserialize and no public attach/replace constructor.
struct ObservedTranscript {
    transcript: DebugTranscriptV1,
    retained: Retention,
}

#[derive(Clone, Copy, Debug)]
struct Observation<'capture> {
    record: &'capture SimulationDebugRecordV1,
    row: &'capture Row,
}

impl ObservedTranscript {
    fn origin_at(&self, index: usize) -> Result<Observation<'_>, Missing> {
        let record = self
            .transcript
            .records()
            .get(index)
            .ok_or(Missing::NoSuchRecord)?;
        if self.retained.coverage == Coverage::InvalidJoin {
            return Err(Missing::InvalidJoin);
        }
        let Some(row) = self.retained.rows.get(index) else {
            return Err(match self.retained.coverage {
                Coverage::Disabled => Missing::Disabled,
                Coverage::PrefixTruncated(reason) => Missing::PrefixTruncated(reason),
                Coverage::Complete | Coverage::InvalidJoin => Missing::InvalidJoin,
            });
        };
        match row.status {
            Status::Available => Ok(Observation { record, row }),
            Status::RuntimeUnavailable(reason) => Err(Missing::RuntimeUnavailable(reason)),
        }
    }
}
