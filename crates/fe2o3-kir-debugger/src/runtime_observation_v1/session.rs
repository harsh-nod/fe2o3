//! Sealed replay owner with bounded named controls and no mutable legacy escape.
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

/// Move-only refusal. Formatting is bounded and never traverses the payload.
/// Returning ownership does not bound the caller's eventual recursive Drop.
pub struct RuntimeRejectedBreakpointV1 {
    error: RuntimeSessionErrorV1,
    breakpoint: DebugBreakpointV1,
}
impl std::fmt::Debug for RuntimeRejectedBreakpointV1 {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeRejectedBreakpointV1")
            .field("error", &self.error)
            .field("payload", &"caller-owned; not inspected")
            .finish()
    }
}
impl RuntimeRejectedBreakpointV1 {
    pub fn error(&self) -> &RuntimeSessionErrorV1 {
        &self.error
    }
    pub fn into_parts(self) -> (RuntimeSessionErrorV1, DebugBreakpointV1) {
        (self.error, self.breakpoint)
    }
}

#[path = "batch_registration.rs"]
mod batch_registration;
#[path = "registration.rs"]
mod registration;
pub use batch_registration::RuntimeRejectedBreakpointsV1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RuntimeNavigationDirectionV1 {
    Forward,
    Reverse,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RuntimeSessionErrorV1 {
    Origin(Missing),
    Frames(RuntimeFrameMissingV1),
    NoParent,
    UnsupportedDirection,
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

pub struct RuntimeReplayWorkV1 {
    remaining: usize,
}
impl RuntimeReplayWorkV1 {
    pub fn new(units: usize) -> Result<Self, RuntimeSessionErrorV1> {
        (units <= MAX_WORK)
            .then_some(Self { remaining: units })
            .ok_or(RuntimeSessionErrorV1::WorkLimit)
    }
    pub fn remaining(&self) -> usize {
        self.remaining
    }
    pub fn charge(&mut self, units: usize) -> Result<(), RuntimeSessionErrorV1> {
        self.remaining = self
            .remaining
            .checked_sub(units)
            .ok_or(RuntimeSessionErrorV1::WorkLimit)?;
        Ok(())
    }
}

pub struct DebugObservedSessionV1 {
    pub(super) session: DebugSessionV1,
    pub(super) origins: Retention,
    predicate_nodes: [usize; MAX_FILTERS],
    pub(super) frames: FrameRetention,
    pub(super) allocations: AllocationRetention,
    pub(super) capture_instance: u64,
    pub(super) options: RuntimeObservationOptionsV1,
}
// Fixed wrapper state is separate from the retained-row byte allowance.
const _: () = assert!(size_of::<DebugObservedSessionV1>() <= 4096);

fn nodes(predicate: &DebugPredicateV1) -> usize {
    1 + match predicate {
        DebugPredicateV1::And(children) | DebugPredicateV1::Or(children) => {
            children.iter().map(nodes).sum()
        }
        DebugPredicateV1::Not(child) => nodes(child),
        _ => 0,
    }
}

fn predicate_work(
    record: &SimulationDebugRecordV1,
    count: usize,
) -> Result<usize, RuntimeSessionErrorV1> {
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
        .ok_or(RuntimeSessionErrorV1::WorkLimit)
}

impl DebugObservedSessionV1 {
    pub fn legacy(&self) -> &DebugSessionV1 {
        &self.session
    }
    pub fn capture_instance(&self) -> u64 {
        self.capture_instance
    }
    pub fn options(&self) -> RuntimeObservationOptionsV1 {
        self.options
    }
    pub fn origin_coverage(&self) -> RuntimeOriginCoverageV1 {
        self.origins.coverage
    }
    pub fn origin_metadata_usage(&self) -> RuntimeOriginUsageV1 {
        self.origins.usage()
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
    /// Fixed wrapper state excludes legacy owner and separately charged retention headers.
    pub fn fixed_owner_metadata_bytes(&self) -> usize {
        size_of::<Self>()
            - size_of::<DebugSessionV1>()
            - size_of::<Retention>()
            - size_of::<FrameRetention>()
            - size_of::<AllocationRetention>()
    }
    pub fn frames_at(
        &self,
        index: usize,
    ) -> Result<RuntimeFrameObservationV1<'_>, RuntimeFrameMissingV1> {
        self.frames.at(self.session.transcript(), index)
    }
    pub fn current_frames(&self) -> Result<RuntimeFrameObservationV1<'_>, RuntimeFrameMissingV1> {
        let index = self
            .session
            .cursor_record_index()
            .filter(|index| *index < self.session.transcript().records().len())
            .ok_or(RuntimeFrameMissingV1::NoCurrentRecord)?;
        self.frames_at(index)
    }
    pub fn allocations_at(
        &self,
        index: usize,
    ) -> Result<RuntimeAllocationObservationV1<'_>, RuntimeAllocationMissingV1> {
        self.allocations.at(self.session.transcript(), index)
    }
    pub fn current_allocations(
        &self,
    ) -> Result<RuntimeAllocationObservationV1<'_>, RuntimeAllocationMissingV1> {
        let index = self
            .session
            .cursor_record_index()
            .filter(|index| *index < self.session.transcript().records().len())
            .ok_or(RuntimeAllocationMissingV1::NoCurrentRecord)?;
        self.allocations_at(index)
    }
    pub fn bind_source_catalog(
        &mut self,
        module: &fe2o3_kir_sim::AdmittedSimulationModuleV1,
        catalog: crate::DebugSourceCatalogV1,
    ) -> Result<(), DebuggerErrorV1> {
        self.session.bind_source_catalog(module, catalog)
    }
    pub fn origin_at(&self, index: usize) -> Result<RuntimeOriginObservationV1<'_>, Missing> {
        joined_origin(&self.origins, self.session.transcript(), index)
    }
    pub fn current_origin(&self) -> Result<RuntimeOriginObservationV1<'_>, Missing> {
        let index = self
            .session
            .cursor_record_index()
            .filter(|index| *index < self.session.transcript().records().len())
            .ok_or(Missing::NoCurrentRecord)?;
        self.origin_at(index)
    }
    pub(super) fn filter_work(
        &self,
        record: &SimulationDebugRecordV1,
    ) -> Result<usize, RuntimeSessionErrorV1> {
        self.predicate_nodes[..self.session.breakpoints.len()]
            .iter()
            .try_fold(1 + 8 * self.session.watchpoints.len(), |total, count| {
                total
                    .checked_add(predicate_work(record, *count)?)
                    .ok_or(RuntimeSessionErrorV1::WorkLimit)
            })
    }
    pub(super) fn charge_seek(
        &self,
        target: Option<usize>,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<(), RuntimeSessionErrorV1> {
        let len = self.session.transcript().records().len();
        if target.is_some_and(|index| index > len) {
            return Err(RuntimeSessionErrorV1::Bounds);
        }
        work.charge(1)?;
        let current = self.session.cursor_prefix_len();
        let next = target.map_or(0, |index| index.saturating_add(1).min(len));
        for record in &self.session.transcript().records()[current.min(next)..current.max(next)] {
            work.charge(self.filter_work(record)?)?;
        }
        Ok(())
    }
    pub fn seek_record(
        &mut self,
        index: usize,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<DebugNavigationV1, RuntimeSessionErrorV1> {
        work.charge(1)?;
        self.charge_seek(Some(index), work)?;
        Ok(self.session.seek_record_index(index))
    }
    pub fn seek_entry(
        &mut self,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<DebugNavigationV1, RuntimeSessionErrorV1> {
        self.charge_seek(None, work)?;
        Ok(self.session.seek_entry())
    }
    pub fn add_watchpoint(
        &mut self,
        watchpoint: DebugWatchpointV1,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<(), RuntimeSessionErrorV1> {
        if self.session.watchpoints.len() == MAX_FILTERS {
            return Err(RuntimeSessionErrorV1::FilterLimit);
        }
        work.charge(MAX_FILTERS + 8 * self.session.cursor_prefix_len())?;
        self.session
            .add_watchpoint(watchpoint)
            .map_err(RuntimeSessionErrorV1::Debugger)
    }
    pub fn remove_breakpoint(
        &mut self,
        id: u64,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<bool, RuntimeSessionErrorV1> {
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
    pub fn remove_watchpoint(
        &mut self,
        id: u64,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<bool, RuntimeSessionErrorV1> {
        work.charge(3 * MAX_FILTERS)?;
        Ok(self.session.remove_watchpoint(id))
    }
}

impl DebugObservedTranscriptV1 {
    pub fn into_session(self) -> DebugObservedSessionV1 {
        DebugObservedSessionV1 {
            session: DebugSessionV1::new(self.transcript),
            origins: self.retained,
            frames: self.frames,
            allocations: self.allocations,
            capture_instance: self.capture_instance,
            options: self.options,
            predicate_nodes: [0; MAX_FILTERS],
        }
    }
}
