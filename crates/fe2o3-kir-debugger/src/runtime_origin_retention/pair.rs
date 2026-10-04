use super::*;
use fe2o3_kir_sim::{
    SimulationDebugCheckpointPhaseV1 as Phase, SimulationDebugRecordKindV1 as Kind,
};

#[derive(Debug)]
pub(super) struct ScanBudget {
    remaining: usize,
}

impl ScanBudget {
    pub(super) fn new(records: usize) -> Result<Self, &'static str> {
        if records > MAX_ROWS {
            return Err("origin scan budget");
        }
        Ok(Self { remaining: records })
    }

    pub(super) fn remaining(&self) -> usize {
        self.remaining
    }

    fn charge(&mut self) -> Result<(), PairError> {
        self.remaining = self.remaining.checked_sub(1).ok_or(PairError::WorkLimit)?;
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum PairError {
    Origin(Missing),
    NotBefore,
    InvalidSequence,
    MissingAfter,
    Incomplete,
    WorkLimit,
}

impl ObservedTranscript {
    /// Exact internal record pairing, not a cursor mutation or frame handle.
    pub(super) fn paired_after(
        &self,
        index: usize,
        budget: &mut ScanBudget,
    ) -> Result<usize, PairError> {
        budget.charge()?;
        let baseline = self.origin_at(index).map_err(PairError::Origin)?;
        if !matches!(
            baseline.record.kind,
            Kind::Checkpoint {
                phase: Phase::BeforeOperation,
                ..
            }
        ) {
            return Err(PairError::NotBefore);
        }
        for next in index + 1..self.transcript.records().len() {
            budget.charge()?;
            let record = &self.transcript.records()[next];
            if record.invocation != baseline.record.invocation {
                continue;
            }
            let observed = match self.origin_at(next) {
                Err(Missing::RuntimeUnavailable(RuntimeMissing::AggregateRecord)) => continue,
                result => result.map_err(PairError::Origin)?,
            };
            if observed.row.activation != baseline.row.activation {
                continue;
            }
            if observed.row.attempt != baseline.row.attempt || record.site != baseline.record.site {
                return Err(PairError::InvalidSequence);
            }
            match record.kind {
                Kind::Checkpoint {
                    phase: Phase::AfterOperation,
                    ..
                } => return Ok(next),
                Kind::Checkpoint {
                    phase: Phase::BeforeOperation,
                    ..
                } => return Err(PairError::InvalidSequence),
                _ => {}
            }
        }
        if self.retained.coverage != Coverage::Complete
            || self.transcript.completeness() != crate::DebugTranscriptCompletenessV1::Complete
        {
            Err(PairError::Incomplete)
        } else {
            Err(PairError::MissingAfter)
        }
    }
}
