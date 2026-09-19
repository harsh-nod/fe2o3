//! Private replay owner; deliberately no serialized identity or mutable session escape.
use super::*;
use crate::{
    DebugBreakpointV1, DebugNavigationV1, DebugPredicateV1, DebugSessionV1, DebugWatchpointV1,
    DebuggerErrorV1, MAX_DEBUGGER_PREDICATE_NODES_V1,
};
use fe2o3_kir_sim::{
    SimulationDebugCollectionV1 as Collection, SimulationDebugRecordKindV1 as Kind,
};

pub(super) const MAX_FILTERS: usize = 8;
pub(super) const MAX_WORK: usize = 1_000_000_000;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum Direction {
    Forward,
    Reverse,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(super) enum SessionError {
    Origin(Missing),
    WorkLimit,
    FilterLimit,
    Debugger(DebuggerErrorV1),
    NoCurrent,
    WrongPhase,
    FocusMismatch,
    InvalidSequence,
    MissingPair,
    Incomplete,
    Bounds,
}

pub(super) struct ReplayWork {
    remaining: usize,
}
impl ReplayWork {
    pub(super) fn new(units: usize) -> Result<Self, SessionError> {
        (units <= MAX_WORK)
            .then_some(Self { remaining: units })
            .ok_or(SessionError::WorkLimit)
    }
    pub(super) fn remaining(&self) -> usize {
        self.remaining
    }
    pub(super) fn charge(&mut self, units: usize) -> Result<(), SessionError> {
        self.remaining = self
            .remaining
            .checked_sub(units)
            .ok_or(SessionError::WorkLimit)?;
        Ok(())
    }
}

pub(super) struct ObservedSession {
    pub(super) session: DebugSessionV1,
    pub(super) origins: Retention,
    predicate_nodes: [usize; MAX_FILTERS],
}
// Fixed wrapper state is separate from the retained-row byte allowance.
const _: () = assert!(size_of::<ObservedSession>() <= 4096);

impl ObservedTranscript {
    pub(super) fn origin_at(&self, index: usize) -> Result<Observation<'_>, Missing> {
        joined_origin(&self.retained, &self.transcript, index)
    }

    pub(super) fn into_session(self) -> ObservedSession {
        ObservedSession {
            session: DebugSessionV1::new(self.transcript),
            origins: self.retained,
            predicate_nodes: [0; MAX_FILTERS],
        }
    }
}

// Only this leaf can join raw row/record references. Siblings use owning-self
// methods; equal content or numeric tokens are not cross-capture ownership.
fn joined_origin<'a>(
    retained: &'a Retention,
    transcript: &'a DebugTranscriptV1,
    index: usize,
) -> Result<Observation<'a>, Missing> {
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
        Status::Available => Ok(Observation { record, row }),
        Status::RuntimeUnavailable(reason) => Err(Missing::RuntimeUnavailable(reason)),
    }
}

fn nodes(predicate: &DebugPredicateV1) -> usize {
    1 + match predicate {
        DebugPredicateV1::And(children) | DebugPredicateV1::Or(children) => {
            children.iter().map(nodes).sum()
        }
        DebugPredicateV1::Not(child) => nodes(child),
        _ => 0,
    }
}

fn predicate_work(record: &SimulationDebugRecordV1, count: usize) -> Result<usize, SessionError> {
    let frames = match &record.kind {
        Kind::Checkpoint {
            stack: Collection::Captured(frames),
            ..
        } => frames.len(),
        _ => 0,
    };
    // Every scalar leaf may linearly seek a frame, then binary-search values.
    // usize::BITS+1 bounds binary-search comparisons even for a maximal Vec.
    frames
        .checked_add(2 + usize::BITS as usize)
        .and_then(|units| units.checked_mul(count))
        .and_then(|units| units.checked_add(8))
        .ok_or(SessionError::WorkLimit)
}

impl ObservedSession {
    pub(super) fn view(&self) -> &DebugSessionV1 {
        &self.session
    }
    pub(super) fn origin_metadata_bytes(&self) -> usize {
        size_of::<Self>() - size_of::<DebugSessionV1>()
            + self.origins.rows.capacity() * size_of::<Row>()
    }
    pub(super) fn origin_at(&self, index: usize) -> Result<Observation<'_>, Missing> {
        joined_origin(&self.origins, self.session.transcript(), index)
    }
    pub(super) fn current_origin(&self) -> Result<Observation<'_>, SessionError> {
        self.origin_at(
            self.session
                .cursor_record_index()
                .ok_or(SessionError::NoCurrent)?,
        )
        .map_err(SessionError::Origin)
    }
    pub(super) fn filter_work(
        &self,
        record: &SimulationDebugRecordV1,
    ) -> Result<usize, SessionError> {
        self.predicate_nodes[..self.session.breakpoints.len()]
            .iter()
            .try_fold(1 + 8 * self.session.watchpoints.len(), |total, count| {
                total
                    .checked_add(predicate_work(record, *count)?)
                    .ok_or(SessionError::WorkLimit)
            })
    }
    pub(super) fn charge_seek(
        &self,
        target: Option<usize>,
        work: &mut ReplayWork,
    ) -> Result<(), SessionError> {
        let len = self.session.transcript().records().len();
        if target.is_some_and(|index| index > len) {
            return Err(SessionError::Bounds);
        }
        work.charge(1)?;
        let current = self.session.cursor_prefix_len();
        let next = target.map_or(0, |index| index.saturating_add(1).min(len));
        for record in &self.session.transcript().records()[current.min(next)..current.max(next)] {
            work.charge(self.filter_work(record)?)?;
        }
        Ok(())
    }
    pub(super) fn seek_record(
        &mut self,
        index: usize,
        work: &mut ReplayWork,
    ) -> Result<DebugNavigationV1, SessionError> {
        work.charge(1)?;
        self.origin_at(index).map_err(SessionError::Origin)?;
        self.charge_seek(Some(index), work)?;
        Ok(self.session.seek_record_index(index))
    }
    pub(super) fn seek_entry(
        &mut self,
        work: &mut ReplayWork,
    ) -> Result<DebugNavigationV1, SessionError> {
        self.charge_seek(None, work)?;
        Ok(self.session.seek_entry())
    }
    pub(super) fn add_breakpoint(
        &mut self,
        breakpoint: DebugBreakpointV1,
        work: &mut ReplayWork,
    ) -> Result<(), SessionError> {
        if self.session.breakpoints.len() == MAX_FILTERS {
            return Err(SessionError::FilterLimit);
        }
        // Two validations and one bounded count walk; invalid input cannot
        // trigger the count recursion until the legacy validator accepts it.
        work.charge(3 * (MAX_DEBUGGER_PREDICATE_NODES_V1 + 1) + MAX_FILTERS)?;
        breakpoint
            .predicate
            .validate()
            .map_err(SessionError::Debugger)?;
        let count = nodes(&breakpoint.predicate);
        for record in &self.session.transcript.records[..self.session.cursor_prefix_len()] {
            work.charge(predicate_work(record, count)?)?;
        }
        let index = self.session.breakpoints.len();
        self.session
            .add_breakpoint(breakpoint)
            .map_err(SessionError::Debugger)?;
        self.predicate_nodes[index] = count;
        Ok(())
    }
    pub(super) fn add_watchpoint(
        &mut self,
        watchpoint: DebugWatchpointV1,
        work: &mut ReplayWork,
    ) -> Result<(), SessionError> {
        if self.session.watchpoints.len() == MAX_FILTERS {
            return Err(SessionError::FilterLimit);
        }
        work.charge(MAX_FILTERS + 8 * self.session.cursor_prefix_len())?;
        self.session
            .add_watchpoint(watchpoint)
            .map_err(SessionError::Debugger)
    }
    pub(super) fn remove_breakpoint(
        &mut self,
        id: u64,
        work: &mut ReplayWork,
    ) -> Result<bool, SessionError> {
        work.charge(3 * MAX_FILTERS + MAX_DEBUGGER_PREDICATE_NODES_V1)?;
        let Some(index) = self
            .session
            .breakpoints
            .iter()
            .position(|value| value.id == id)
        else {
            return Ok(false);
        };
        self.predicate_nodes
            .copy_within(index + 1..MAX_FILTERS, index);
        self.predicate_nodes[MAX_FILTERS - 1] = 0;
        Ok(self.session.remove_breakpoint(id))
    }
    pub(super) fn remove_watchpoint(
        &mut self,
        id: u64,
        work: &mut ReplayWork,
    ) -> Result<bool, SessionError> {
        work.charge(3 * MAX_FILTERS)?;
        Ok(self.session.remove_watchpoint(id))
    }
}
