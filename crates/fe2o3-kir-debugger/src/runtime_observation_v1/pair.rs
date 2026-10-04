use super::*;
use fe2o3_kir_sim::{
    SimulationDebugCheckpointPhaseV1 as Phase, SimulationDebugRecordKindV1 as Kind,
};

#[derive(Debug)]
pub struct RuntimeOriginScanWorkV1 {
    remaining: usize,
}

impl RuntimeOriginScanWorkV1 {
    pub fn new(records: usize) -> Result<Self, &'static str> {
        if records > MAX_ROWS {
            return Err("origin scan budget");
        }
        Ok(Self { remaining: records })
    }

    pub fn remaining(&self) -> usize {
        self.remaining
    }

    fn charge(&mut self) -> Result<(), RuntimeOriginPairErrorV1> {
        self.remaining = self
            .remaining
            .checked_sub(1)
            .ok_or(RuntimeOriginPairErrorV1::WorkLimit)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeOriginPairErrorV1 {
    Origin(Missing),
    NotBefore,
    InvalidSequence,
    MissingAfter,
    Incomplete,
    WorkLimit,
}

impl DebugObservedTranscriptV1 {
    /// Exact internal record pairing, not a cursor mutation or frame handle.
    pub fn paired_after(
        &self,
        index: usize,
        budget: &mut RuntimeOriginScanWorkV1,
    ) -> Result<usize, RuntimeOriginPairErrorV1> {
        budget.charge()?;
        let baseline = self
            .origin_at(index)
            .map_err(RuntimeOriginPairErrorV1::Origin)?;
        if !matches!(
            baseline.record.kind,
            Kind::Checkpoint {
                phase: Phase::BeforeOperation,
                ..
            }
        ) {
            return Err(RuntimeOriginPairErrorV1::NotBefore);
        }
        for next in index + 1..self.transcript.records().len() {
            budget.charge()?;
            let record = &self.transcript.records()[next];
            if record.invocation != baseline.record.invocation {
                continue;
            }
            let observed = match self.origin_at(next) {
                Err(Missing::RuntimeUnavailable(RuntimeMissing::AggregateRecord)) => continue,
                result => result.map_err(RuntimeOriginPairErrorV1::Origin)?,
            };
            if observed.row.activation != baseline.row.activation {
                continue;
            }
            if observed.row.attempt != baseline.row.attempt || record.site != baseline.record.site {
                return Err(RuntimeOriginPairErrorV1::InvalidSequence);
            }
            match record.kind {
                Kind::Checkpoint {
                    phase: Phase::AfterOperation,
                    ..
                } => return Ok(next),
                Kind::Checkpoint {
                    phase: Phase::BeforeOperation,
                    ..
                } => return Err(RuntimeOriginPairErrorV1::InvalidSequence),
                _ => {}
            }
        }
        if self.retained.coverage != Coverage::Complete
            || self.transcript.completeness() != crate::DebugTranscriptCompletenessV1::Complete
        {
            Err(RuntimeOriginPairErrorV1::Incomplete)
        } else {
            Err(RuntimeOriginPairErrorV1::MissingAfter)
        }
    }
}
