//! Bounded, immutable resource observations over actual retained debugger records.
//!
//! This is a library projection, not a trace format or an admission authority.
//! One owner belongs to one backend incarnation. The backend supplies its own
//! configuration identity and revision; these must never come from an unchecked
//! query. The private owner nonce is process-local freshness, not authentication.
//! All allocation generations are zero in this producer profile. Checkpoint
//! presence does not establish allocation lifetime, owning scope, or physical LDS
//! placement. Access history stops at the selected record; no future records,
//! physical registers, hardware timing, or source associations are inferred.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_kernel_ir::{AccessMode, AddressSpace};
use fe2o3_kir_sim::{
    SimulationDebugCheckpointPhaseV1, SimulationDebugCollectionV1, SimulationDebugMemoryAccessV1,
    SimulationDebugRecordKindV1, SimulationDebugRecordV1, SimulationDebugScheduleV1,
    SimulationDebugSiteV1, SimulationDebugUnavailableReasonV1, SimulationInvocationV1,
};

use crate::{
    DebugHierarchyV1, DebugKirIdentityV1, DebugScopeSelectorV1, DebugSessionV1,
    DebugTranscriptCompletenessV1, DebugWaveWidthV1, hierarchy_for_invocation_v1,
};

pub const MAX_RESOURCE_PAGE_ITEMS_V1: u16 = 256;
pub const MAX_RESOURCE_PAGE_SCANS_V1: u16 = 256;

static NEXT_OWNER: AtomicU64 = AtomicU64::new(0);

fn allocate_owner(counter: &AtomicU64) -> Result<u64, ResourceProjectionErrorV1> {
    counter
        .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
            value.checked_add(1)
        })
        .map(|previous| previous + 1)
        .map_err(|_| ResourceProjectionErrorV1::OwnerExhausted)
}

/// Keep exactly one owner per backend session; do not share it across sessions.
pub struct ResourceProjectionOwnerV1 {
    nonce: u64,
}

impl fmt::Debug for ResourceProjectionOwnerV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("ResourceProjectionOwnerV1 { process_local_freshness: opaque }")
    }
}

impl ResourceProjectionOwnerV1 {
    pub fn new() -> Result<Self, ResourceProjectionErrorV1> {
        Ok(Self {
            nonce: allocate_owner(&NEXT_OWNER)?,
        })
    }

    pub fn bind<'a>(
        &'a self,
        session: &'a DebugSessionV1,
        configuration_identity: [u8; 32],
        state_revision: u64,
    ) -> Result<ResourceProjectionV1<'a>, ResourceProjectionErrorV1> {
        if configuration_identity == [0; 32] {
            return Err(ResourceProjectionErrorV1::ZeroConfigurationIdentity);
        }
        let record = session
            .current()
            .map(|record| record_anchor(record, session.transcript().wave_width()))
            .transpose()?;
        Ok(ResourceProjectionV1 {
            session,
            selection: ResourceSelectionV1 {
                owner: self.nonce,
                configuration_identity,
                state_revision,
                kir: session.transcript().identity(),
                wave_width: session.transcript().wave_width(),
                record_index: session.cursor_record_index(),
                record,
            },
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub struct ResourceSelectionV1 {
    owner: u64,
    pub configuration_identity: [u8; 32],
    pub state_revision: u64,
    pub kir: DebugKirIdentityV1,
    pub wave_width: DebugWaveWidthV1,
    pub record_index: Option<usize>,
    pub record: Option<ResourceRecordAnchorV1>,
}

impl fmt::Debug for ResourceSelectionV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ResourceSelectionV1")
            .field("configuration_identity", &self.configuration_identity)
            .field("state_revision", &self.state_revision)
            .field("kir", &self.kir)
            .field("wave_width", &self.wave_width)
            .field("record_index", &self.record_index)
            .field("record", &self.record)
            .finish_non_exhaustive()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceRecordAnchorV1 {
    pub record_ordinal: u64,
    /// The existing debugger event projection uses retained ordinal + 1.
    pub event_sequence: u64,
    pub invocation: SimulationInvocationV1,
    pub hierarchy: DebugHierarchyV1,
    pub site: SimulationDebugSiteV1,
    pub schedule: SimulationDebugScheduleV1,
    pub checkpoint_phase: Option<SimulationDebugCheckpointPhaseV1>,
}

fn record_anchor(
    record: &SimulationDebugRecordV1,
    width: DebugWaveWidthV1,
) -> Result<ResourceRecordAnchorV1, ResourceProjectionErrorV1> {
    Ok(ResourceRecordAnchorV1 {
        record_ordinal: record.ordinal,
        event_sequence: record
            .ordinal
            .checked_add(1)
            .ok_or(ResourceProjectionErrorV1::RangeOverflow)?,
        invocation: record.invocation,
        hierarchy: hierarchy_for_invocation_v1(record.invocation, width),
        site: record.site,
        schedule: record.schedule,
        checkpoint_phase: match record.kind {
            SimulationDebugRecordKindV1::Checkpoint { phase, .. } => Some(phase),
            _ => None,
        },
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceAllocationIdentityV1 {
    pub ordinal: u64,
    pub generation: u64,
}

impl ResourceAllocationIdentityV1 {
    fn validate(self) -> Result<(), ResourceProjectionErrorV1> {
        if self.ordinal == 0 {
            return Err(ResourceProjectionErrorV1::ZeroAllocationIdentity);
        }
        if self.generation != 0 {
            return Err(ResourceProjectionErrorV1::UnsupportedAllocationGeneration);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourceByteRangeV1 {
    pub byte_offset: u64,
    pub byte_len: u64,
}

impl ResourceByteRangeV1 {
    fn end(self) -> Result<u64, ResourceProjectionErrorV1> {
        if self.byte_len == 0 {
            return Err(ResourceProjectionErrorV1::ZeroLengthRange);
        }
        self.byte_offset
            .checked_add(self.byte_len)
            .ok_or(ResourceProjectionErrorV1::RangeOverflow)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceUnavailableFactV1 {
    NotRepresented,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceAllocationObservationV1 {
    pub allocation: ResourceAllocationIdentityV1,
    pub address_space: AddressSpace,
    pub access: AccessMode,
    pub alignment: u32,
    pub capacity_bytes: u64,
    /// Bytes and initialization are captured, but queried separately by range.
    pub snapshot_bytes_available: bool,
    pub initialization_available: bool,
    pub owning_scope: ResourceUnavailableFactV1,
    pub lifetime: ResourceUnavailableFactV1,
    pub physical_base: ResourceUnavailableFactV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceAccessObservationV1 {
    /// Exact retained access occurrence, not a guessed physical transaction.
    pub occurrence: ResourceRecordAnchorV1,
    pub allocation: ResourceAllocationIdentityV1,
    pub range: ResourceByteRangeV1,
    pub address_space: AddressSpace,
    pub access: SimulationDebugMemoryAccessV1,
    pub call_frame: ResourceUnavailableFactV1,
    pub operation_occurrence: ResourceUnavailableFactV1,
    pub source_association: ResourceUnavailableFactV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourceAccessFilterV1 {
    pub scope: DebugScopeSelectorV1,
    pub allocation: Option<ResourceAllocationIdentityV1>,
    pub address_space: Option<AddressSpace>,
    pub access: Option<SimulationDebugMemoryAccessV1>,
    pub range: Option<ResourceByteRangeV1>,
}

impl Default for ResourceAccessFilterV1 {
    fn default() -> Self {
        Self {
            scope: DebugScopeSelectorV1::Dispatch,
            allocation: None,
            address_space: None,
            access: None,
            range: None,
        }
    }
}

impl ResourceAccessFilterV1 {
    fn validate(&self, width: DebugWaveWidthV1) -> Result<(), ResourceProjectionErrorV1> {
        if let Some(allocation) = self.allocation {
            allocation.validate()?;
        }
        if let Some(range) = self.range {
            range.end()?;
        }
        if matches!(self.scope, DebugScopeSelectorV1::Lane { lane, .. } if lane >= width.lanes()) {
            return Err(ResourceProjectionErrorV1::InvalidLane);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ResourcePageLimitsV1 {
    pub max_items: u16,
    pub max_scanned: u16,
}

impl Default for ResourcePageLimitsV1 {
    fn default() -> Self {
        Self {
            max_items: 64,
            max_scanned: 256,
        }
    }
}

impl ResourcePageLimitsV1 {
    fn validate(self) -> Result<(), ResourceProjectionErrorV1> {
        if self.max_items == 0
            || self.max_items > MAX_RESOURCE_PAGE_ITEMS_V1
            || self.max_scanned == 0
            || self.max_scanned > MAX_RESOURCE_PAGE_SCANS_V1
        {
            return Err(ResourceProjectionErrorV1::InvalidPageLimits);
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ResourceQueryV1 {
    Allocations { address_space: Option<AddressSpace> },
    Accesses(ResourceAccessFilterV1),
}

/// Opaque process-local continuation, minted by this library; not a wire token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcePageCursorV1 {
    selection: ResourceSelectionV1,
    query: ResourceQueryV1,
    limits: ResourcePageLimitsV1,
    next_index: usize,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ResourcePageRequestV1 {
    pub limits: ResourcePageLimitsV1,
    pub cursor: Option<ResourcePageCursorV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceProjectionUnavailableV1 {
    NoSelectedRecord,
    NotCheckpoint,
    MemoryCapture(SimulationDebugUnavailableReasonV1),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ResourcePageDataV1<T> {
    Available(Vec<T>),
    Unavailable {
        reason: ResourceProjectionUnavailableV1,
        required: Option<u64>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ResourcePageV1<T> {
    pub selection: ResourceSelectionV1,
    pub completeness: DebugTranscriptCompletenessV1,
    /// Raw entries in the selected source prefix, not the number matching filters.
    pub source_count: usize,
    pub scanned: u16,
    pub data: ResourcePageDataV1<T>,
    pub next_cursor: Option<ResourcePageCursorV1>,
    pub physical_registers: ResourceUnavailableFactV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResourceProjectionErrorV1 {
    OwnerExhausted,
    ZeroConfigurationIdentity,
    SelectionMismatch,
    CursorMismatch,
    InvalidPageLimits,
    ZeroAllocationIdentity,
    UnsupportedAllocationGeneration,
    InvalidLane,
    ZeroLengthRange,
    RangeOverflow,
    InvalidCapturedAllocation,
    AllocationFailure,
}

impl fmt::Display for ResourceProjectionErrorV1 {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "resource projection rejected: {self:?}")
    }
}

impl std::error::Error for ResourceProjectionErrorV1 {}

pub struct ResourceProjectionV1<'a> {
    session: &'a DebugSessionV1,
    selection: ResourceSelectionV1,
}

impl ResourceProjectionV1<'_> {
    pub const fn selection(&self) -> ResourceSelectionV1 {
        self.selection
    }

    fn start(
        &self,
        selected: &ResourceSelectionV1,
        query: &ResourceQueryV1,
        request: &ResourcePageRequestV1,
    ) -> Result<usize, ResourceProjectionErrorV1> {
        request.limits.validate()?;
        if *selected != self.selection {
            return Err(ResourceProjectionErrorV1::SelectionMismatch);
        }
        match &request.cursor {
            Some(cursor)
                if cursor.selection == self.selection
                    && cursor.query == *query
                    && cursor.limits == request.limits =>
            {
                Ok(cursor.next_index)
            }
            Some(_) => Err(ResourceProjectionErrorV1::CursorMismatch),
            None => Ok(0),
        }
    }

    fn unavailable<T>(
        &self,
        reason: ResourceProjectionUnavailableV1,
        required: Option<u64>,
    ) -> ResourcePageV1<T> {
        ResourcePageV1 {
            selection: self.selection,
            completeness: self.session.transcript().completeness(),
            source_count: 0,
            scanned: 0,
            data: ResourcePageDataV1::Unavailable { reason, required },
            next_cursor: None,
            physical_registers: ResourceUnavailableFactV1::NotRepresented,
        }
    }

    fn page<T>(
        &self,
        query: ResourceQueryV1,
        limits: ResourcePageLimitsV1,
        source_count: usize,
        start: usize,
        next: usize,
        items: Vec<T>,
    ) -> ResourcePageV1<T> {
        ResourcePageV1 {
            selection: self.selection,
            completeness: self.session.transcript().completeness(),
            source_count,
            scanned: u16::try_from(next - start).expect("scan count is bounded by u16 request"),
            data: ResourcePageDataV1::Available(items),
            next_cursor: (next < source_count).then_some(ResourcePageCursorV1 {
                selection: self.selection,
                query,
                limits,
                next_index: next,
            }),
            physical_registers: ResourceUnavailableFactV1::NotRepresented,
        }
    }

    pub fn allocations(
        &self,
        selected: &ResourceSelectionV1,
        address_space: Option<AddressSpace>,
        request: ResourcePageRequestV1,
    ) -> Result<ResourcePageV1<ResourceAllocationObservationV1>, ResourceProjectionErrorV1> {
        let query = ResourceQueryV1::Allocations { address_space };
        let start = self.start(selected, &query, &request)?;
        let Some(record) = self.session.current() else {
            return Ok(self.unavailable(ResourceProjectionUnavailableV1::NoSelectedRecord, None));
        };
        let SimulationDebugRecordKindV1::Checkpoint { memory, .. } = &record.kind else {
            return Ok(self.unavailable(ResourceProjectionUnavailableV1::NotCheckpoint, None));
        };
        let allocations = match memory {
            SimulationDebugCollectionV1::Captured(allocations) => allocations,
            SimulationDebugCollectionV1::Unavailable { reason, required } => {
                return Ok(self.unavailable(
                    ResourceProjectionUnavailableV1::MemoryCapture(*reason),
                    Some(*required),
                ));
            }
        };
        if start > allocations.len() {
            return Err(ResourceProjectionErrorV1::CursorMismatch);
        }
        let mut items = Vec::new();
        items
            .try_reserve_exact(usize::from(request.limits.max_items).min(allocations.len() - start))
            .map_err(|_| ResourceProjectionErrorV1::AllocationFailure)?;
        let mut next = start;
        while next < allocations.len()
            && next - start < usize::from(request.limits.max_scanned)
            && items.len() < usize::from(request.limits.max_items)
        {
            let allocation = &allocations[next];
            next += 1;
            if allocation.allocation == 0
                || !allocation.alignment.is_power_of_two()
                || allocation.bytes.len() != allocation.initialized.len()
            {
                return Err(ResourceProjectionErrorV1::InvalidCapturedAllocation);
            }
            if address_space.is_some_and(|space| space != allocation.address_space) {
                continue;
            }
            items.push(ResourceAllocationObservationV1 {
                allocation: ResourceAllocationIdentityV1 {
                    ordinal: allocation.allocation,
                    generation: 0,
                },
                address_space: allocation.address_space,
                access: allocation.access,
                alignment: allocation.alignment,
                capacity_bytes: u64::try_from(allocation.bytes.len())
                    .map_err(|_| ResourceProjectionErrorV1::RangeOverflow)?,
                snapshot_bytes_available: true,
                initialization_available: true,
                owning_scope: ResourceUnavailableFactV1::NotRepresented,
                lifetime: ResourceUnavailableFactV1::NotRepresented,
                physical_base: ResourceUnavailableFactV1::NotRepresented,
            });
        }
        Ok(self.page(query, request.limits, allocations.len(), start, next, items))
    }

    pub fn accesses(
        &self,
        selected: &ResourceSelectionV1,
        filter: ResourceAccessFilterV1,
        request: ResourcePageRequestV1,
    ) -> Result<ResourcePageV1<ResourceAccessObservationV1>, ResourceProjectionErrorV1> {
        let query = ResourceQueryV1::Accesses(filter.clone());
        let start = self.start(selected, &query, &request)?;
        filter.validate(self.selection.wave_width)?;
        let (Some(selected_index), Some(_)) = (self.selection.record_index, self.selection.record)
        else {
            return Ok(self.unavailable(ResourceProjectionUnavailableV1::NoSelectedRecord, None));
        };
        let source_count = selected_index
            .checked_add(1)
            .ok_or(ResourceProjectionErrorV1::RangeOverflow)?;
        let records = self.session.transcript().records();
        if source_count > records.len() || start > source_count {
            return Err(ResourceProjectionErrorV1::CursorMismatch);
        }
        let mut items = Vec::new();
        items
            .try_reserve_exact(usize::from(request.limits.max_items).min(source_count - start))
            .map_err(|_| ResourceProjectionErrorV1::AllocationFailure)?;
        let mut next = start;
        while next < source_count
            && next - start < usize::from(request.limits.max_scanned)
            && items.len() < usize::from(request.limits.max_items)
        {
            let record = &records[next];
            next += 1;
            let SimulationDebugRecordKindV1::Memory {
                access,
                allocation,
                byte_offset,
                byte_len,
                address_space,
                ..
            } = &record.kind
            else {
                continue;
            };
            byte_offset
                .checked_add(*byte_len)
                .ok_or(ResourceProjectionErrorV1::RangeOverflow)?;
            let allocation = ResourceAllocationIdentityV1 {
                ordinal: *allocation,
                generation: 0,
            };
            allocation.validate()?;
            let range = ResourceByteRangeV1 {
                byte_offset: u64::try_from(*byte_offset)
                    .map_err(|_| ResourceProjectionErrorV1::RangeOverflow)?,
                byte_len: u64::try_from(*byte_len)
                    .map_err(|_| ResourceProjectionErrorV1::RangeOverflow)?,
            };
            let end = range.end()?;
            if !filter
                .scope
                .matches(record.invocation, self.selection.wave_width)
                || filter
                    .allocation
                    .is_some_and(|expected| expected != allocation)
                || filter
                    .address_space
                    .is_some_and(|expected| expected != *address_space)
                || filter.access.is_some_and(|expected| expected != *access)
            {
                continue;
            }
            if let Some(selected_range) = filter.range
                && (range.byte_offset >= selected_range.end()? || selected_range.byte_offset >= end)
            {
                continue;
            }
            items.push(ResourceAccessObservationV1 {
                occurrence: record_anchor(record, self.selection.wave_width)?,
                allocation,
                range,
                address_space: *address_space,
                access: *access,
                call_frame: ResourceUnavailableFactV1::NotRepresented,
                operation_occurrence: ResourceUnavailableFactV1::NotRepresented,
                source_association: ResourceUnavailableFactV1::NotRepresented,
            });
        }
        Ok(self.page(query, request.limits, source_count, start, next, items))
    }
}

#[cfg(test)]
mod tests;
