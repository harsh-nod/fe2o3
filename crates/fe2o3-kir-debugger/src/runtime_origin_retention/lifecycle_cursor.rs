//! Borrowed same-stop projection. Reverse navigation does not mutate allocation identity.
use super::*;
use crate::runtime_origin_retention::session::{Direction, RejectedBreakpoint};
use crate::{DebugNavigationV1, DebugSessionV1, DebugWatchpointV1};
use fe2o3_kir_sim::{
    SimulationDebugAllocationV1, SimulationDebugCollectionV1, SimulationDebugRecordKindV1,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum State {
    NotYetObserved,
    PreexistingLive,
    CreatedLive,
    Released,
}

/// Absence is explicit; no zero generation or depth-derived activation is invented.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum Unrepresented {
    NotRepresented,
}

pub(in super::super) struct AllocationObservation<'a> {
    pub(in super::super) state: State,
    pub(in super::super) latest: Option<&'a Transition>,
    pub(in super::super) checkpoint: Option<&'a SimulationDebugAllocationV1>,
    pub(in super::super) generation: Unrepresented,
    pub(in super::super) frame_activation: Unrepresented,
    /// The borrow comes from this session; it is not a transport selection token.
    pub(in super::super) record: Option<&'a SimulationDebugRecordV1>,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in super::super) enum QueryError {
    NoCurrent,
    ForeignOwner,
    StaleSelection,
    Gap(Gap),
    Work,
    InvalidSnapshot,
}

/// Process-local borrow, neither serializable nor cloneable; no caller constructor.
/// Borrowing the session prevents safe mutation/movement while this exists.
pub(in super::super) struct Selection<'a> {
    owner: &'a LifecycleSession,
    cursor: Option<usize>,
}

impl LifecycleSession {
    pub(in super::super) fn view(&self) -> &DebugSessionV1 {
        self.observed.view()
    }
    pub(in super::super) fn transitions(&self) -> &[Transition] {
        &self.lifecycle.rows
    }
    pub(in super::super) fn metadata_bytes(&self) -> usize {
        size_of::<Self>() - size_of::<ObservedSession>()
            + self.lifecycle.rows.capacity() * size_of::<Transition>()
            + self.observed.origin_metadata_bytes()
    }
    pub(in super::super) fn selection(&self) -> Selection<'_> {
        Selection {
            owner: self,
            cursor: self.view().cursor_record_index(),
        }
    }
    pub(in super::super) fn allocation_at_selection(
        &self,
        selection: &Selection<'_>,
        allocation: u64,
        work: &mut ReplayWork,
    ) -> Result<AllocationObservation<'_>, QueryError> {
        work.charge(1).map_err(|_| QueryError::Work)?;
        if !std::ptr::eq(self, selection.owner) {
            return Err(QueryError::ForeignOwner);
        }
        if self.view().cursor_record_index() != selection.cursor {
            return Err(QueryError::StaleSelection);
        }
        self.current_allocation(allocation, work)
    }
    pub(in super::super) fn current_allocation(
        &self,
        allocation: u64,
        work: &mut ReplayWork,
    ) -> Result<AllocationObservation<'_>, QueryError> {
        let index = self
            .view()
            .cursor_record_index()
            .ok_or(QueryError::NoCurrent)?;
        let records = self.view().transcript().records();
        if index > records.len() {
            return Err(QueryError::NoCurrent);
        }
        // All scans are prepaid; failure cannot change cursor, counts or sidecar.
        work.charge(
            self.lifecycle
                .rows
                .len()
                .checked_add(1)
                .ok_or(QueryError::Work)?,
        )
        .map_err(|_| QueryError::Work)?;
        if let Some((boundary, reason)) = self.lifecycle.gap
            && index >= boundary
        {
            return Err(QueryError::Gap(reason));
        }
        if index == records.len() {
            if !self.debug_completed {
                return Err(QueryError::Gap(Gap::DebugPrefix));
            }
            if !self.execution_completed {
                return Err(QueryError::Gap(Gap::ExecutionFailure));
            }
        }
        let latest = self
            .lifecycle
            .rows
            .iter()
            .rev()
            .find(|row| row.boundary <= index && row.allocation == allocation);
        let state = match latest.map(|row| row.action) {
            None => State::NotYetObserved,
            Some(Action::Preexisting) => State::PreexistingLive,
            Some(Action::Create) => State::CreatedLive,
            Some(Action::Release) => State::Released,
        };
        let record = records.get(index);
        let checkpoint = match record.map(|record| &record.kind) {
            Some(SimulationDebugRecordKindV1::Checkpoint {
                memory: SimulationDebugCollectionV1::Captured(memory),
                ..
            }) => {
                work.charge(memory.len()).map_err(|_| QueryError::Work)?;
                let found = memory.iter().find(|item| item.allocation == allocation);
                if found.is_some() != matches!(state, State::PreexistingLive | State::CreatedLive) {
                    return Err(QueryError::InvalidSnapshot);
                }
                found
            }
            _ => None,
        };
        if let (Some(row), Some(memory)) = (latest, checkpoint)
            && (row.address_space != memory.address_space || row.bytes != memory.bytes.len())
        {
            return Err(QueryError::InvalidSnapshot);
        }
        Ok(AllocationObservation {
            state,
            latest,
            checkpoint,
            record,
            generation: Unrepresented::NotRepresented,
            frame_activation: Unrepresented::NotRepresented,
        })
    }
    pub(in super::super) fn seek_record(
        &mut self,
        index: usize,
        work: &mut ReplayWork,
    ) -> Result<DebugNavigationV1, SessionError> {
        self.observed.seek_record(index, work)
    }
    pub(in super::super) fn seek_entry(
        &mut self,
        work: &mut ReplayWork,
    ) -> Result<DebugNavigationV1, SessionError> {
        self.observed.seek_entry(work)
    }
    pub(in super::super) fn step_into(
        &mut self,
        direction: Direction,
        focus: SimulationInvocationV1,
        work: &mut ReplayWork,
    ) -> Result<DebugNavigationV1, SessionError> {
        self.observed.step_into(direction, focus, work)
    }
    pub(in super::super) fn continue_to_stop(
        &mut self,
        direction: Direction,
        work: &mut ReplayWork,
    ) -> Result<DebugNavigationV1, SessionError> {
        self.observed.continue_to_stop(direction, work)
    }
    #[expect(
        clippy::result_large_err,
        reason = "Rejected predicates return ownership without allocation, traversal, or drop"
    )]
    pub(in super::super) fn add_breakpoint(
        &mut self,
        breakpoint: crate::DebugBreakpointV1,
        work: &mut ReplayWork,
    ) -> Result<(), RejectedBreakpoint> {
        self.observed.add_breakpoint(breakpoint, work)
    }
    pub(in super::super) fn add_watchpoint(
        &mut self,
        watchpoint: DebugWatchpointV1,
        work: &mut ReplayWork,
    ) -> Result<(), SessionError> {
        self.observed.add_watchpoint(watchpoint, work)
    }
}
