//! Budgeted replay over actual retained contexts; no caller-frame reconstruction.
use super::session::*;
use super::*;
use crate::{DebugNavigationV1, DebugStopReasonV1, DebugTranscriptCompletenessV1};
use fe2o3_kir_sim::{
    SimulationDebugCheckpointPhaseV1 as Phase, SimulationDebugRecordKindV1 as Kind,
    SimulationInvocationV1,
};

fn phase(record: &SimulationDebugRecordV1) -> Option<Phase> {
    match record.kind {
        Kind::Checkpoint { phase, .. } => Some(phase),
        _ => None,
    }
}

impl Direction {
    fn next(self, index: Option<usize>, len: usize) -> Option<usize> {
        match self {
            Self::Forward => index
                .map_or(Some(0), |i| i.checked_add(1))
                .filter(|i| *i < len),
            Self::Reverse => index.and_then(|i| i.checked_sub(1)).filter(|i| *i < len),
        }
    }
}

impl ObservedSession {
    fn boundary(
        &mut self,
        direction: Direction,
        work: &mut ReplayWork,
    ) -> Result<DebugNavigationV1, SessionError> {
        if direction == Direction::Reverse {
            return self.seek_entry(work);
        }
        if self.origins.coverage != Coverage::Complete
            || self.session.transcript().completeness() != DebugTranscriptCompletenessV1::Complete
        {
            return Err(SessionError::Incomplete);
        }
        let end = self.session.transcript().records().len();
        self.charge_seek(Some(end), work)?;
        Ok(self.session.forward_end())
    }

    pub(super) fn step_into(
        &mut self,
        direction: Direction,
        focus: SimulationInvocationV1,
        work: &mut ReplayWork,
    ) -> Result<DebugNavigationV1, SessionError> {
        let mut position = self.session.cursor_record_index();
        let len = self.session.transcript().records().len();
        while let Some(index) = direction.next(position, len) {
            work.charge(1)?;
            let record = &self.session.transcript().records()[index];
            if record.invocation == focus && phase(record).is_some() {
                return self.seek_record(index, work);
            }
            position = Some(index);
        }
        self.boundary(direction, work)
    }

    pub(super) fn step_over(
        &mut self,
        direction: Direction,
        focus: SimulationInvocationV1,
        work: &mut ReplayWork,
    ) -> Result<DebugNavigationV1, SessionError> {
        work.charge(1)?;
        let baseline = self.current_origin()?;
        if baseline.record.invocation != focus {
            return Err(SessionError::FocusMismatch);
        }
        let (start_phase, end_phase) = match direction {
            Direction::Forward => (Phase::BeforeOperation, Phase::AfterOperation),
            Direction::Reverse => (Phase::AfterOperation, Phase::BeforeOperation),
        };
        if phase(baseline.record) != Some(start_phase) {
            return Err(SessionError::WrongPhase);
        }
        let origin = *baseline.row;
        let site = baseline.record.site;
        let len = self.session.transcript().records().len();
        let mut position = self.session.cursor_record_index();
        while let Some(index) = direction.next(position, len) {
            work.charge(1)?;
            let record = &self.session.transcript().records()[index];
            position = Some(index);
            if record.invocation != focus {
                continue;
            }
            let observed = match self.origin_at(index) {
                Err(Missing::RuntimeUnavailable(RuntimeMissing::AggregateRecord)) => continue,
                result => result.map_err(SessionError::Origin)?,
            };
            if observed.row.activation != origin.activation {
                continue;
            }
            if observed.row.attempt != origin.attempt || record.site != site {
                return Err(SessionError::InvalidSequence);
            }
            if phase(record) == Some(end_phase) {
                return self.seek_record(index, work);
            }
            if phase(record) == Some(start_phase) {
                return Err(SessionError::InvalidSequence);
            }
        }
        if self.origins.coverage != Coverage::Complete
            || self.session.transcript().completeness() != DebugTranscriptCompletenessV1::Complete
        {
            Err(SessionError::Incomplete)
        } else {
            Err(SessionError::MissingPair)
        }
    }

    /// Existing matchers/hit counters remain authoritative. A budget error
    /// preserves the last committed cursor, not an invented semantic stop.
    pub(super) fn continue_to_stop(
        &mut self,
        direction: Direction,
        work: &mut ReplayWork,
    ) -> Result<DebugNavigationV1, SessionError> {
        let len = self.session.transcript().records().len();
        while let Some(index) = direction.next(self.session.cursor_record_index(), len) {
            work.charge(1)?;
            let record = &self.session.transcript().records()[index];
            // Charge both aggregate eligibility and the actual stop query.
            work.charge(
                self.filter_work(record)?
                    .checked_mul(2)
                    .ok_or(SessionError::WorkLimit)?,
            )?;
            match self.origin_at(index) {
                Ok(_) => {}
                Err(Missing::RuntimeUnavailable(RuntimeMissing::AggregateRecord)) => {
                    if self.session.breakpoints.iter().any(|bp| {
                        crate::breakpoint_base_matches(
                            bp,
                            record,
                            self.session.transcript.wave_width,
                        )
                    }) || self.session.watchpoints.iter().any(|wp| {
                        wp.enabled
                            && crate::watchpoint_matches(
                                wp,
                                record,
                                self.session.transcript.wave_width,
                            )
                    }) {
                        return Err(SessionError::Origin(Missing::RuntimeUnavailable(
                            RuntimeMissing::AggregateRecord,
                        )));
                    }
                }
                Err(reason) => return Err(SessionError::Origin(reason)),
            }
            self.charge_seek(Some(index), work)?;
            self.session.set_cursor(Some(index));
            if let Some(reason) = self
                .session
                .stop_reason(&self.session.transcript.records[index])
            {
                debug_assert!(matches!(
                    reason,
                    DebugStopReasonV1::Breakpoint(_) | DebugStopReasonV1::Watchpoint(_)
                ));
                return Ok(self.session.current_stop(reason));
            }
        }
        self.boundary(direction, work)
    }
}
