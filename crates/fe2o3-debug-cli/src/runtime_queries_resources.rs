//! Bounded queries join only exact allocator-owned tuples within one sealed run.
use super::{QueryFailure, mapping};
use crate::{
    SimulatorBackendV1, hex_bytes, initialization_bits, protocol_address_space,
    protocol_scope_for_invocation,
};
use fe2o3_debug_protocol::*;
use fe2o3_kir_debugger::{
    DebugObservedSessionV1, RuntimeAllocationMissingV1, RuntimeAllocationObservationV1,
    RuntimeReplayWorkV1,
};
use fe2o3_kir_sim::{
    SimulationAllocationDescriptorV1, SimulationDebugAllocationV1, SimulationDebugCollectionV1,
    SimulationDebugRecordKindV1, SimulationDebugRecordV1, SimulationDebugUnavailableReasonV1,
};

pub(super) struct ProjectedResource {
    pub through: u64,
    pub page: Option<(u64, u64, u16)>,
    pub next: Option<usize>,
    pub result: ResourceQueryResultV2,
}
impl SimulatorBackendV1 {
    pub(super) fn project_observed_resource(
        &self,
        request: &ResourceRequestV2,
        start: usize,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<ProjectedResource, QueryFailure> {
        let observed = self.session.observed().ok_or(QueryFailure::Unavailable(
            RuntimeObservationUnavailableV1::NotRequested,
        ))?;
        let ledger = observed
            .current_allocations()
            .map_err(|error| QueryFailure::Unavailable(mapping::allocation_missing(error)))?;
        let through = ledger.through_sequence();
        work.charge(1).map_err(|_| QueryFailure::work())?;
        match request {
            ResourceRequestV2::QueryAllocationLifecycle { page, .. } => {
                let source = ledger.transitions();
                require_start(start, source.len())?;
                let mut rows = reserve_rows(page.max_items)?;
                let mut index = start;
                while index < source.len()
                    && index - start < usize::from(page.max_scanned)
                    && rows.len() < usize::from(page.max_items)
                {
                    work.charge(1).map_err(|_| QueryFailure::work())?;
                    let row = source[index];
                    if row.sequence() != index as u64 + 1 {
                        return Err(QueryFailure::Unavailable(
                            RuntimeObservationUnavailableV1::InvalidJoin,
                        ));
                    }
                    rows.push(mapping::transition(row));
                    index += 1;
                }
                Ok(ProjectedResource {
                    through,
                    page: Some((source.len() as u64, start as u64, (index - start) as u16)),
                    next: (index < source.len()).then_some(index),
                    result: ResourceQueryResultV2::AllocationLifecycle { transitions: rows },
                })
            }
            ResourceRequestV2::QueryAllocations {
                address_space,
                page,
                ..
            } => {
                let source = captured_memory(ledger.record())?;
                require_start(start, source.len())?;
                let mut rows = reserve_rows(page.max_items)?;
                let mut index = start;
                while index < source.len()
                    && index - start < usize::from(page.max_scanned)
                    && rows.len() < usize::from(page.max_items)
                {
                    work.charge(1).map_err(|_| QueryFailure::work())?;
                    let snapshot = &source[index];
                    index += 1;
                    if address_space.is_some_and(|space| {
                        space != protocol_address_space(snapshot.address_space)
                    }) {
                        continue;
                    }
                    let descriptor =
                        ledger
                            .descriptor(snapshot.allocation, work)
                            .map_err(|error| {
                                QueryFailure::Unavailable(mapping::allocation_missing(error))
                            })?;
                    validate_snapshot(snapshot, descriptor)?;
                    rows.push(ResourceAllocationV2 {
                        descriptor: mapping::descriptor(descriptor),
                        snapshot_bytes_available: true,
                        initialization_available: true,
                    });
                }
                Ok(ProjectedResource {
                    through,
                    page: Some((source.len() as u64, start as u64, (index - start) as u16)),
                    next: (index < source.len()).then_some(index),
                    result: ResourceQueryResultV2::Allocations { allocations: rows },
                })
            }
            ResourceRequestV2::QueryMemoryAccesses {
                allocation, page, ..
            } => {
                // First resolve all three fields at this exact current watermark.
                // A released generation cannot silently follow a replacement.
                let selected = select_live(ledger, *allocation, work)?;
                self.project_accesses(observed, selected, start, page, through, work)
            }
            ResourceRequestV2::ReadAllocationMemory {
                allocation, range, ..
            } => {
                let selected = select_live(ledger, *allocation, work)?;
                let source = captured_memory(ledger.record())?;
                let mut captured = None;
                for row in source {
                    work.charge(1).map_err(|_| QueryFailure::work())?;
                    if row.allocation == allocation.allocation.get() {
                        if captured.is_some() {
                            return Err(QueryFailure::Unavailable(
                                RuntimeObservationUnavailableV1::InvalidJoin,
                            ));
                        }
                        captured = Some(row);
                    }
                }
                let captured = captured.ok_or(QueryFailure::Unavailable(
                    RuntimeObservationUnavailableV1::NotCaptured,
                ))?;
                validate_snapshot(captured, selected)?;
                let end = range.end().map_err(|_| {
                    QueryFailure::Refused(
                        DebugErrorCodeV1::InvalidRequest,
                        "observed memory range overflow",
                    )
                })?;
                if end > selected.byte_len() {
                    return Err(QueryFailure::Refused(
                        DebugErrorCodeV1::InvalidRequest,
                        "observed memory range exceeds the selected allocation",
                    ));
                }
                let offset = usize::try_from(range.byte_offset.get()).map_err(|_| {
                    QueryFailure::invalid("observed byte offset is unrepresentable")
                })?;
                let end = usize::try_from(end)
                    .map_err(|_| QueryFailure::invalid("observed byte end is unrepresentable"))?;
                work.charge(end - offset)
                    .map_err(|_| QueryFailure::work())?;
                let bytes = captured
                    .bytes
                    .get(offset..end)
                    .ok_or(QueryFailure::invalid("observed memory byte join failed"))?;
                let initialized =
                    captured
                        .initialized
                        .get(offset..end)
                        .ok_or(QueryFailure::invalid(
                            "observed memory initialization join failed",
                        ))?;
                Ok(ProjectedResource {
                    through,
                    page: None,
                    next: None,
                    result: ResourceQueryResultV2::AllocationMemory {
                        memory: ResourceMemoryReadV2 {
                            allocation: mapping::identity(selected.identity()),
                            range: *range,
                            address_space: protocol_address_space(selected.address_space()),
                            bytes: hex_bytes(bytes),
                            initialized: initialization_bits(initialized),
                        },
                    },
                })
            }
        }
    }
    fn project_accesses(
        &self,
        observed: &DebugObservedSessionV1,
        selected: SimulationAllocationDescriptorV1,
        start: usize,
        page: &ResourceQueryPageV1,
        through: u64,
        work: &mut RuntimeReplayWorkV1,
    ) -> Result<ProjectedResource, QueryFailure> {
        let index = observed
            .legacy()
            .cursor_record_index()
            .ok_or(QueryFailure::Unavailable(
                RuntimeObservationUnavailableV1::NoSelectedRecord,
            ))?;
        let count = index
            .checked_add(1)
            .ok_or(QueryFailure::invalid("observed record prefix overflow"))?;
        let source = observed
            .legacy()
            .transcript()
            .records()
            .get(..count)
            .ok_or(QueryFailure::Unavailable(
                RuntimeObservationUnavailableV1::NoSelectedRecord,
            ))?;
        require_start(start, source.len())?;
        let mut rows = reserve_rows(page.max_items)?;
        let mut index = start;
        while index < source.len()
            && index - start < usize::from(page.max_scanned)
            && rows.len() < usize::from(page.max_items)
        {
            work.charge(1).map_err(|_| QueryFailure::work())?;
            let record = &source[index];
            let record_index = index;
            index += 1;
            if record.ordinal != record_index as u64 {
                return Err(QueryFailure::Unavailable(
                    RuntimeObservationUnavailableV1::InvalidJoin,
                ));
            }
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
            if *allocation != selected.identity().allocation() {
                continue;
            }
            // Validate against the actual ledger watermark of the access too.
            // This cannot obtain a future birth from the selected later record.
            let at_access = observed
                .allocations_at(record_index)
                .map_err(|error| QueryFailure::Unavailable(mapping::allocation_missing(error)))?;
            let then = select_live(at_access, mapping::identity(selected.identity()), work)?;
            if then != selected || *address_space != selected.address_space() {
                return Err(QueryFailure::Unavailable(
                    RuntimeObservationUnavailableV1::InvalidJoin,
                ));
            }
            let range = ResourceMemoryRangeV1 {
                byte_offset: mapping::number(*byte_offset as u64),
                byte_len: mapping::number(*byte_len as u64),
            };
            if !range.end().is_ok_and(|end| end <= selected.byte_len()) {
                return Err(QueryFailure::Unavailable(
                    RuntimeObservationUnavailableV1::InvalidJoin,
                ));
            }
            let function = self
                .module
                .module()
                .functions
                .get(record.site.function_ordinal)
                .ok_or(QueryFailure::invalid("observed access function is absent"))?;
            let body = function.body.as_ref().ok_or(QueryFailure::invalid(
                "observed access function body is absent",
            ))?;
            work.charge(body.blocks.len())
                .map_err(|_| QueryFailure::work())?;
            let block = body
                .blocks
                .iter()
                .position(|block| block.id == record.site.block)
                .ok_or(QueryFailure::invalid("observed access block is absent"))?;
            let scope = protocol_scope_for_invocation(record.invocation, self.wave_width).ok_or(
                QueryFailure::invalid("observed access scope is unrepresentable"),
            )?;
            rows.push(ResourceMemoryAccessV2 {
                occurrence: ResourceAccessOccurrenceV1 {
                    record_ordinal: record.ordinal,
                    event_sequence: record
                        .ordinal
                        .checked_add(1)
                        .ok_or(QueryFailure::invalid("observed access ordinal overflow"))?,
                    scope,
                    site: KirSiteV1 {
                        function_ordinal: record.site.function_ordinal as u64,
                        block_ordinal: block as u64,
                        point: KirSitePointV1::Operation {
                            operation_ordinal: u64::from(record.site.operation),
                        },
                    },
                    schedule: mapping::schedule(record.schedule),
                },
                invocation: mapping::invocation(record.invocation),
                allocation: mapping::identity(selected.identity()),
                range,
                address_space: protocol_address_space(*address_space),
                access: mapping::access(*access),
                origin: mapping::origin(observed.origin_at(record_index)),
            });
        }
        Ok(ProjectedResource {
            through,
            page: Some((source.len() as u64, start as u64, (index - start) as u16)),
            next: (index < source.len()).then_some(index),
            result: ResourceQueryResultV2::MemoryAccesses { accesses: rows },
        })
    }
}
fn reserve_rows<T>(maximum: u16) -> Result<Vec<T>, QueryFailure> {
    let mut rows = Vec::new();
    rows.try_reserve_exact(usize::from(maximum)).map_err(|_| {
        QueryFailure::Unavailable(RuntimeObservationUnavailableV1::AllocationFailure)
    })?;
    Ok(rows)
}
fn require_start(start: usize, count: usize) -> Result<(), QueryFailure> {
    if start > count {
        return Err(QueryFailure::invalid(
            "runtime cursor exceeds its exact captured source",
        ));
    }
    Ok(())
}
fn captured_memory(
    record: &SimulationDebugRecordV1,
) -> Result<&[SimulationDebugAllocationV1], QueryFailure> {
    match &record.kind {
        SimulationDebugRecordKindV1::Checkpoint {
            memory: SimulationDebugCollectionV1::Captured(rows),
            ..
        } => Ok(rows),
        SimulationDebugRecordKindV1::Checkpoint {
            memory: SimulationDebugCollectionV1::Unavailable { reason, .. },
            ..
        } => Err(QueryFailure::Unavailable(
            if *reason == SimulationDebugUnavailableReasonV1::AllocationFailure {
                RuntimeObservationUnavailableV1::AllocationFailure
            } else {
                RuntimeObservationUnavailableV1::NotCaptured
            },
        )),
        _ => Err(QueryFailure::Unavailable(
            RuntimeObservationUnavailableV1::NotCheckpoint,
        )),
    }
}
fn select_live(
    ledger: RuntimeAllocationObservationV1<'_>,
    expected: ResourceStorageIdentityV2,
    work: &mut RuntimeReplayWorkV1,
) -> Result<SimulationAllocationDescriptorV1, QueryFailure> {
    let descriptor = match ledger.descriptor(expected.allocation.get(), work) {
        Ok(value) => value,
        Err(RuntimeAllocationMissingV1::NotLive) => {
            return Err(QueryFailure::invalid(
                "allocation is not live at the selected record",
            ));
        }
        Err(error) => {
            return Err(QueryFailure::Unavailable(mapping::allocation_missing(
                error,
            )));
        }
    };
    if mapping::identity(descriptor.identity()) != expected {
        return Err(QueryFailure::invalid(
            "allocation storage slot or generation does not match the selected record",
        ));
    }
    Ok(descriptor)
}
fn validate_snapshot(
    snapshot: &SimulationDebugAllocationV1,
    descriptor: SimulationAllocationDescriptorV1,
) -> Result<(), QueryFailure> {
    if snapshot.allocation != descriptor.identity().allocation()
        || snapshot.address_space != descriptor.address_space()
        || snapshot.access != descriptor.access()
        || snapshot.alignment != descriptor.alignment()
        || snapshot.bytes.len() as u64 != descriptor.byte_len()
        || snapshot.initialized.len() != snapshot.bytes.len()
    {
        return Err(QueryFailure::Unavailable(
            RuntimeObservationUnavailableV1::InvalidJoin,
        ));
    }
    Ok(())
}
