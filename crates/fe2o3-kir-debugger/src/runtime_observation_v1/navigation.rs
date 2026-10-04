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

impl RuntimeNavigationDirectionV1 {
    fn next(self, index: Option<usize>, len: usize) -> Option<usize> {
        match self {
            Self::Forward => index
                .map_or(Some(0), |i| i.checked_add(1))
                .filter(|i| *i < len),
            Self::Reverse => index.and_then(|i| i.checked_sub(1)).filter(|i| *i < len),
        }
    }
}

impl DebugObservedSessionV1 {
    fn boundary(
        &mut self,
        direction: RuntimeNavigationDirectionV1,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<DebugNavigationV1, RuntimeSessionErrorV1> {
        if direction == RuntimeNavigationDirectionV1::Reverse {
            return self.seek_entry(work);
        }
        if self.origins.coverage != Coverage::Complete
            || self.session.transcript().completeness() != DebugTranscriptCompletenessV1::Complete
        {
            return Err(RuntimeSessionErrorV1::Incomplete);
        }
        let end = self.session.transcript().records().len();
        self.charge_seek(Some(end), work)?;
        Ok(self.session.forward_end())
    }

    pub fn step_into(
        &mut self,
        direction: RuntimeNavigationDirectionV1,
        focus: SimulationInvocationV1,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<DebugNavigationV1, RuntimeSessionErrorV1> {
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

    pub fn step_over(
        &mut self,
        direction: RuntimeNavigationDirectionV1,
        focus: SimulationInvocationV1,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<DebugNavigationV1, RuntimeSessionErrorV1> {
        work.charge(1)?;
        let baseline = self
            .current_origin()
            .map_err(RuntimeSessionErrorV1::Origin)?;
        if baseline.record.invocation != focus {
            return Err(RuntimeSessionErrorV1::FocusMismatch);
        }
        let (start_phase, end_phase) = match direction {
            RuntimeNavigationDirectionV1::Forward => {
                (Phase::BeforeOperation, Phase::AfterOperation)
            }
            RuntimeNavigationDirectionV1::Reverse => {
                (Phase::AfterOperation, Phase::BeforeOperation)
            }
        };
        if phase(baseline.record) != Some(start_phase) {
            return Err(RuntimeSessionErrorV1::WrongPhase);
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
                result => result.map_err(RuntimeSessionErrorV1::Origin)?,
            };
            if observed.row.activation != origin.activation {
                continue;
            }
            if observed.row.attempt != origin.attempt || record.site != site {
                return Err(RuntimeSessionErrorV1::InvalidSequence);
            }
            if phase(record) == Some(end_phase) {
                return self.seek_record(index, work);
            }
            if phase(record) == Some(start_phase) {
                return Err(RuntimeSessionErrorV1::InvalidSequence);
            }
        }
        if self.origins.coverage != Coverage::Complete
            || self.session.transcript().completeness() != DebugTranscriptCompletenessV1::Complete
        {
            Err(RuntimeSessionErrorV1::Incomplete)
        } else {
            Err(RuntimeSessionErrorV1::MissingPair)
        }
    }

    /// Step to the actual parent call, never to a frame with merely equal depth.
    pub fn step_out(
        &mut self,
        direction: RuntimeNavigationDirectionV1,
        focus: SimulationInvocationV1,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<DebugNavigationV1, RuntimeSessionErrorV1> {
        work.charge(1)?;
        let frames = self
            .current_frames()
            .map_err(RuntimeSessionErrorV1::Frames)?;
        if frames.record().invocation != focus {
            return Err(RuntimeSessionErrorV1::FocusMismatch);
        }
        let top = frames
            .get(
                frames
                    .len()
                    .checked_sub(1)
                    .ok_or(RuntimeSessionErrorV1::NoCurrent)?,
            )
            .ok_or(RuntimeSessionErrorV1::InvalidSequence)?;
        let fe2o3_kir_sim::SimulationDebugFrameParentV1::Caller {
            activation,
            attempt,
            call_site,
        } = top.parent()
        else {
            return Err(RuntimeSessionErrorV1::NoParent);
        };
        let target_phase = match direction {
            RuntimeNavigationDirectionV1::Forward => Phase::AfterOperation,
            RuntimeNavigationDirectionV1::Reverse => Phase::BeforeOperation,
        };
        let len = self.session.transcript().records().len();
        let mut position = self.session.cursor_record_index();
        while let Some(index) = direction.next(position, len) {
            work.charge(1)?;
            position = Some(index);
            let record = &self.session.transcript().records()[index];
            if record.invocation != focus {
                continue;
            }
            let observed = match self.origin_at(index) {
                Err(Missing::RuntimeUnavailable(RuntimeMissing::AggregateRecord)) => continue,
                result => result.map_err(RuntimeSessionErrorV1::Origin)?,
            };
            if observed.activation() != activation {
                continue;
            }
            if observed.attempt() != attempt || observed.site() != call_site {
                return Err(RuntimeSessionErrorV1::InvalidSequence);
            }
            if phase(record) == Some(target_phase) {
                return self.seek_record(index, work);
            }
        }
        if self.origins.coverage != Coverage::Complete
            || self.session.transcript().completeness() != DebugTranscriptCompletenessV1::Complete
        {
            Err(RuntimeSessionErrorV1::Incomplete)
        } else {
            Err(RuntimeSessionErrorV1::MissingPair)
        }
    }

    fn budget_stop(&self) -> DebugNavigationV1 {
        match self.session.cursor_record_index() {
            None => DebugNavigationV1::Beginning,
            Some(index) => match self.session.transcript().records().get(index) {
                Some(record) => DebugNavigationV1::BudgetExhausted(crate::DebugStopV1 {
                    record_index: index,
                    record_ordinal: record.ordinal,
                    reason: DebugStopReasonV1::Step,
                }),
                None => DebugNavigationV1::End,
            },
        }
    }

    pub fn continue_to_stop(
        &mut self,
        direction: RuntimeNavigationDirectionV1,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<DebugNavigationV1, RuntimeSessionErrorV1> {
        self.continue_to_stop_bounded(direction, MAX_ROWS, work)
    }

    /// Existing matchers/hit counters remain authoritative. A budget error
    /// preserves the last committed cursor, not an invented semantic stop.
    pub fn continue_to_stop_bounded(
        &mut self,
        direction: RuntimeNavigationDirectionV1,
        max_records: usize,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<DebugNavigationV1, RuntimeSessionErrorV1> {
        if max_records > MAX_ROWS {
            return Err(RuntimeSessionErrorV1::Bounds);
        }
        let len = self.session.transcript().records().len();
        let mut examined = 0;
        while let Some(index) = direction.next(self.session.cursor_record_index(), len) {
            if examined == max_records {
                return Ok(self.budget_stop());
            }
            examined += 1;
            work.charge(1)?;
            let record = &self.session.transcript().records()[index];
            // Charge both aggregate eligibility and the actual stop query.
            work.charge(
                self.filter_work(record)?
                    .checked_mul(2)
                    .ok_or(RuntimeSessionErrorV1::WorkLimit)?,
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
                        return Err(RuntimeSessionErrorV1::Origin(Missing::RuntimeUnavailable(
                            RuntimeMissing::AggregateRecord,
                        )));
                    }
                }
                Err(reason) => return Err(RuntimeSessionErrorV1::Origin(reason)),
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
