//! Read-only adapter from admitted CPU debugger records to resource-query DTOs.

use std::collections::BTreeMap;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};

use fe2o3_debug_protocol::*;
use fe2o3_kernel_ir::{AccessMode, AddressSpace};
use fe2o3_kir_debugger::DebugTranscriptCompletenessV1;
use fe2o3_kir_debugger::resource_projection_v1 as projection;
use fe2o3_kir_sim::{
    SimulationDebugMemoryAccessV1, SimulationDebugUnavailableReasonV1, SimulationScheduleIdentityV1,
};

use crate::{
    MAX_SESSION_COMMANDS_V1, SimulatorBackendV1, bounded_message, convert_scope_selector,
    protocol_address_space, protocol_scope_for_invocation, transcript_truncation_reason,
};

const MAX_RESOURCE_CURSOR_ENTRIES_V1: usize = 256;
static NEXT_TOKEN_NAMESPACE: AtomicU64 = AtomicU64::new(0);

/// Exactly one state per backend incarnation; neither this nor typed cursors is
/// deserialized. Token namespaces are process-local freshness, not authority.
pub(crate) struct ResourceQueryStateV1 {
    owner: projection::ResourceProjectionOwnerV1,
    namespace: u64,
    next_token: u64,
    revision: Option<u64>,
    cursors: BTreeMap<String, ResourceCursorEntryV1>,
}

struct ResourceCursorEntryV1 {
    request: ResourceRequestV1,
    cursor: projection::ResourcePageCursorV1,
}

impl ResourceQueryStateV1 {
    pub(crate) fn new() -> Result<Self, String> {
        let namespace = NEXT_TOKEN_NAMESPACE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| "resource token namespace exhausted".to_owned())?
            + 1;
        Ok(Self {
            owner: projection::ResourceProjectionOwnerV1::new()
                .map_err(|error| error.to_string())?,
            namespace,
            next_token: 0,
            revision: None,
            cursors: BTreeMap::new(),
        })
    }

    fn refresh(&mut self, revision: u64) {
        if self.revision != Some(revision) {
            self.cursors.clear();
            self.revision = Some(revision);
        }
    }

    fn take_cursor(
        &mut self,
        request: &ResourceRequestV1,
    ) -> Result<Option<projection::ResourcePageCursorV1>, &'static str> {
        let Some(token) = &request.page().token else {
            return Ok(None);
        };
        let entry = self
            .cursors
            .remove(token.as_str())
            .ok_or("resource cursor is unknown, consumed, or stale")?;
        if !same_query(&entry.request, request) {
            return Err("resource cursor query binding changed");
        }
        Ok(Some(entry.cursor))
    }

    fn retain_cursor(
        &mut self,
        request: &ResourceRequestV1,
        cursor: Option<projection::ResourcePageCursorV1>,
    ) -> Result<Option<ResourcePageTokenV1>, &'static str> {
        let Some(cursor) = cursor else {
            return Ok(None);
        };
        if self.cursors.len() >= MAX_RESOURCE_CURSOR_ENTRIES_V1 {
            return Err("resource cursor budget exhausted");
        }
        self.next_token = self
            .next_token
            .checked_add(1)
            .ok_or("resource token counter exhausted")?;
        let token = ResourcePageTokenV1::new(format!(
            "resource.{:x}.{:x}",
            self.namespace, self.next_token
        ))
        .map_err(|_| "resource token encoding failed")?;
        self.cursors.insert(
            token.as_str().to_owned(),
            ResourceCursorEntryV1 {
                request: request.clone(),
                cursor,
            },
        );
        Ok(Some(token))
    }
}

fn same_query(previous: &ResourceRequestV1, current: &ResourceRequestV1) -> bool {
    if previous.expected_snapshot() != current.expected_snapshot()
        || previous.expected_revision() != current.expected_revision()
        || previous.page().max_items != current.page().max_items
        || previous.page().max_scanned != current.page().max_scanned
    {
        return false;
    }
    match (previous, current) {
        (
            ResourceRequestV1::QueryAllocations {
                address_space: left,
                ..
            },
            ResourceRequestV1::QueryAllocations {
                address_space: right,
                ..
            },
        ) => left == right,
        (
            ResourceRequestV1::QueryMemoryAccesses { filter: left, .. },
            ResourceRequestV1::QueryMemoryAccesses { filter: right, .. },
        ) => left == right,
        _ => false,
    }
}

struct MappedPageV1 {
    source_count: u64,
    scanned: u16,
    completeness: CaptureCompletenessV1,
    result: Result<
        ResourceQueryResultV1,
        (
            ResourceQueryUnavailableReasonV1,
            Option<ResourceDecimalU64V1>,
        ),
    >,
    next_cursor: Option<projection::ResourcePageCursorV1>,
}

impl SimulatorBackendV1 {
    pub(crate) fn handle_resource_queries_v1(
        &mut self,
        request: ResourceRequestV1,
    ) -> ResourceResponseV1 {
        self.resource_queries.refresh(self.revision);
        if let Err(error) = request.validate(self.protocol_limits) {
            return self.resource_error(
                &request,
                DebugErrorCodeV1::InvalidRequest,
                &error.to_string(),
            );
        }
        if request.expected_revision() != self.revision {
            return self.resource_error(
                &request,
                DebugErrorCodeV1::StaleRevision,
                "expected_revision does not match the current session revision",
            );
        }
        if self.terminated {
            return self.resource_error(
                &request,
                DebugErrorCodeV1::InvalidState,
                "debug session is terminated",
            );
        }
        if self.command_count >= MAX_SESSION_COMMANDS_V1 {
            return self.resource_error(
                &request,
                DebugErrorCodeV1::ResourceLimit,
                "debug session command budget is exhausted",
            );
        }
        let Some(snapshot) = self.current_anchor(None) else {
            return self.resource_error(
                &request,
                DebugErrorCodeV1::InvalidState,
                "the selected record has no independently captured checkpoint anchor",
            );
        };
        if request.expected_snapshot() != &snapshot {
            return self.resource_error(&request, DebugErrorCodeV1::InvalidCursor, "expected_snapshot does not exactly match the independently produced checkpoint anchor");
        }
        self.command_count += 1;
        let cursor = match self.resource_queries.take_cursor(&request) {
            Ok(cursor) => cursor,
            Err(message) => {
                return self.resource_error(&request, DebugErrorCodeV1::InvalidCursor, message);
            }
        };
        // Bind only after the full protocol anchor check, using backend-owned
        // configuration and revision. The incoming anchor is not an authority.
        let view = match self.resource_queries.owner.bind(
            &self.session,
            self.configuration_identity.as_bytes(),
            self.revision,
        ) {
            Ok(view) => view,
            Err(error) => {
                return self.resource_error(
                    &request,
                    DebugErrorCodeV1::InvalidState,
                    &error.to_string(),
                );
            }
        };
        let page_request = projection::ResourcePageRequestV1 {
            limits: projection::ResourcePageLimitsV1 {
                max_items: request.page().max_items,
                max_scanned: request.page().max_scanned,
            },
            cursor,
        };
        let completeness = self.resource_completeness();
        let mapped = match &request {
            ResourceRequestV1::QueryAllocations { address_space, .. } => view
                .allocations(
                    &view.selection(),
                    address_space.map(debugger_address_space),
                    page_request,
                )
                .map_err(|error| error.to_string())
                .and_then(|page| {
                    map_page(page, completeness, |rows| {
                        Ok(ResourceQueryResultV1::Allocations {
                            allocations: rows.into_iter().map(map_allocation).collect(),
                        })
                    })
                }),
            ResourceRequestV1::QueryMemoryAccesses { filter, .. } => view
                .accesses(
                    &view.selection(),
                    projection::ResourceAccessFilterV1 {
                        scope: convert_scope_selector(filter.scope),
                        allocation: filter.allocation.map(|allocation| {
                            projection::ResourceAllocationIdentityV1 {
                                ordinal: allocation.ordinal,
                                generation: allocation.generation,
                            }
                        }),
                        address_space: filter.address_space.map(debugger_address_space),
                        access: filter.access.map(debugger_access),
                        range: filter.range.map(|range| projection::ResourceByteRangeV1 {
                            byte_offset: range.byte_offset.get(),
                            byte_len: range.byte_len.get(),
                        }),
                    },
                    page_request,
                )
                .map_err(|error| error.to_string())
                .and_then(|page| {
                    map_page(page, completeness, |rows| {
                        rows.into_iter()
                            .map(|row| self.map_resource_access(row))
                            .collect::<Result<Vec<_>, _>>()
                            .map(|accesses| ResourceQueryResultV1::MemoryAccesses { accesses })
                    })
                }),
        };
        let mapped = match mapped {
            Ok(mapped) => mapped,
            Err(message) => {
                return self.resource_error(&request, DebugErrorCodeV1::InvalidCursor, &message);
            }
        };
        match mapped.result {
            Err((reason, required)) => ResourceResponseV1::Unavailable {
                schema: ResourceResponseSchemaV1::V1,
                request_id: request.request_id(),
                operation: request.operation(),
                session: self.session_view(),
                reason,
                required,
                completeness: mapped.completeness,
            },
            Ok(result) => {
                let next_token = match self
                    .resource_queries
                    .retain_cursor(&request, mapped.next_cursor)
                {
                    Ok(token) => token,
                    Err(message) => {
                        return self.resource_error(
                            &request,
                            DebugErrorCodeV1::ResourceLimit,
                            message,
                        );
                    }
                };
                ResourceResponseV1::Ok {
                    schema: ResourceResponseSchemaV1::V1,
                    request_id: request.request_id(),
                    operation: request.operation(),
                    session: self.session_view(),
                    snapshot: Box::new(snapshot),
                    page: ResourcePageInfoV1 {
                        source_count: mapped.source_count,
                        scanned: mapped.scanned,
                        completeness: mapped.completeness,
                        next_token,
                    },
                    result,
                    physical_registers: ResourceFactUnavailableV1::NotRepresented,
                }
            }
        }
    }

    fn resource_completeness(&self) -> CaptureCompletenessV1 {
        match self.session.transcript().completeness() {
            DebugTranscriptCompletenessV1::Complete => CaptureCompletenessV1::Complete,
            DebugTranscriptCompletenessV1::Truncated(reason) => CaptureCompletenessV1::Truncated {
                reason: transcript_truncation_reason(reason),
                emitted_events: self.session.transcript().records().len() as u64,
                dropped_events: None,
            },
        }
    }

    fn resource_error(
        &self,
        request: &ResourceRequestV1,
        code: DebugErrorCodeV1,
        message: &str,
    ) -> ResourceResponseV1 {
        ResourceResponseV1::Error {
            schema: ResourceResponseSchemaV1::V1,
            request_id: (request.request_id() != 0).then_some(request.request_id()),
            operation: request.operation(),
            session: self.session_view(),
            error: DebugErrorV1 {
                stage: DebugErrorStageV1::Session,
                code,
                message: bounded_message(message),
                state_changed: false,
            },
        }
    }

    fn map_resource_access(
        &self,
        row: projection::ResourceAccessObservationV1,
    ) -> Result<ResourceMemoryAccessV1, String> {
        let site = row.occurrence.site;
        let function = self
            .module
            .module()
            .functions
            .get(site.function_ordinal)
            .ok_or("resource function is absent")?;
        let body = function
            .body
            .as_ref()
            .ok_or("resource function body is absent")?;
        let block = body
            .blocks
            .iter()
            .position(|block| block.id == site.block)
            .ok_or("resource block is absent")?;
        let scope = protocol_scope_for_invocation(row.occurrence.invocation, self.wave_width)
            .ok_or("resource scope is not protocol-representable")?;
        Ok(ResourceMemoryAccessV1 {
            occurrence: ResourceAccessOccurrenceV1 {
                record_ordinal: row.occurrence.record_ordinal,
                event_sequence: row.occurrence.event_sequence,
                scope,
                site: KirSiteV1 {
                    function_ordinal: u64::try_from(site.function_ordinal)
                        .map_err(|_| "resource function ordinal overflow")?,
                    block_ordinal: u64::try_from(block)
                        .map_err(|_| "resource block ordinal overflow")?,
                    point: KirSitePointV1::Operation {
                        operation_ordinal: u64::from(site.operation),
                    },
                },
                schedule: ResourceAccessScheduleV1 {
                    identity: match row.occurrence.schedule.identity {
                        SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxSerialV1 => {
                            ResourceScheduleIdentityV1::WorkgroupMajorLocalZyxSerialV1
                        }
                        SimulationScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1 => {
                            ResourceScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1
                        }
                        SimulationScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1 => {
                            ResourceScheduleIdentityV1::WorkgroupMajorSeededRunnableCooperativeV1
                        }
                    },
                    decision_ordinal: row.occurrence.schedule.decision_ordinal,
                },
            },
            allocation: AllocationIdentityV1 {
                ordinal: row.allocation.ordinal,
                generation: row.allocation.generation,
            },
            range: ResourceMemoryRangeV1 {
                byte_offset: ResourceDecimalU64V1::new(row.range.byte_offset),
                byte_len: ResourceDecimalU64V1::new(row.range.byte_len),
            },
            address_space: protocol_address_space(row.address_space),
            access: protocol_access(row.access),
            call_frame: ResourceFactUnavailableV1::NotRepresented,
            operation_occurrence: ResourceFactUnavailableV1::NotRepresented,
            source_association: ResourceFactUnavailableV1::NotRepresented,
        })
    }
}

fn map_page<T>(
    page: projection::ResourcePageV1<T>,
    completeness: CaptureCompletenessV1,
    convert: impl FnOnce(Vec<T>) -> Result<ResourceQueryResultV1, String>,
) -> Result<MappedPageV1, String> {
    let result = match page.data {
        projection::ResourcePageDataV1::Available(rows) => Ok(convert(rows)?),
        projection::ResourcePageDataV1::Unavailable { reason, required } => Err((
            match reason {
                projection::ResourceProjectionUnavailableV1::NoSelectedRecord => {
                    ResourceQueryUnavailableReasonV1::NoSelectedRecord
                }
                projection::ResourceProjectionUnavailableV1::NotCheckpoint => {
                    ResourceQueryUnavailableReasonV1::NotCheckpoint
                }
                projection::ResourceProjectionUnavailableV1::MemoryCapture(reason) => {
                    match reason {
                        SimulationDebugUnavailableReasonV1::FrameLimit => {
                            ResourceQueryUnavailableReasonV1::FrameLimit
                        }
                        SimulationDebugUnavailableReasonV1::ValueLimit => {
                            ResourceQueryUnavailableReasonV1::ValueLimit
                        }
                        SimulationDebugUnavailableReasonV1::AllocationLimit => {
                            ResourceQueryUnavailableReasonV1::AllocationLimit
                        }
                        SimulationDebugUnavailableReasonV1::MemoryByteLimit => {
                            ResourceQueryUnavailableReasonV1::MemoryByteLimit
                        }
                        SimulationDebugUnavailableReasonV1::AllocationFailure => {
                            ResourceQueryUnavailableReasonV1::AllocationFailure
                        }
                        SimulationDebugUnavailableReasonV1::NotCaptured => {
                            ResourceQueryUnavailableReasonV1::NotCaptured
                        }
                    }
                }
            },
            required.map(ResourceDecimalU64V1::new),
        )),
    };
    Ok(MappedPageV1 {
        source_count: u64::try_from(page.source_count)
            .map_err(|_| "resource source count overflow")?,
        scanned: page.scanned,
        completeness,
        result,
        next_cursor: page.next_cursor,
    })
}

fn map_allocation(row: projection::ResourceAllocationObservationV1) -> ResourceAllocationV1 {
    ResourceAllocationV1 {
        allocation: AllocationIdentityV1 {
            ordinal: row.allocation.ordinal,
            generation: row.allocation.generation,
        },
        address_space: protocol_address_space(row.address_space),
        access: match row.access {
            AccessMode::ReadOnly => ResourceAllocationAccessV1::ReadOnly,
            AccessMode::WriteOnly => ResourceAllocationAccessV1::WriteOnly,
            AccessMode::ReadWrite => ResourceAllocationAccessV1::ReadWrite,
        },
        alignment: row.alignment,
        capacity_bytes: ResourceDecimalU64V1::new(row.capacity_bytes),
        snapshot_bytes_available: row.snapshot_bytes_available,
        initialization_available: row.initialization_available,
        owning_scope: ResourceFactUnavailableV1::NotRepresented,
        lifetime: ResourceFactUnavailableV1::NotRepresented,
        physical_base: ResourceFactUnavailableV1::NotRepresented,
    }
}

fn debugger_address_space(space: AddressSpaceV1) -> AddressSpace {
    match space {
        AddressSpaceV1::Private => AddressSpace::Private,
        AddressSpaceV1::Workgroup => AddressSpace::Workgroup,
        AddressSpaceV1::Global => AddressSpace::Global,
        AddressSpaceV1::Constant => AddressSpace::Constant,
        AddressSpaceV1::Generic => AddressSpace::Generic,
    }
}

fn debugger_access(access: ResourceMemoryAccessKindV1) -> SimulationDebugMemoryAccessV1 {
    match access {
        ResourceMemoryAccessKindV1::Read => SimulationDebugMemoryAccessV1::Read,
        ResourceMemoryAccessKindV1::WriteCommitted => SimulationDebugMemoryAccessV1::WriteCommitted,
        ResourceMemoryAccessKindV1::AtomicRead => SimulationDebugMemoryAccessV1::AtomicRead,
        ResourceMemoryAccessKindV1::AtomicWriteCommitted => {
            SimulationDebugMemoryAccessV1::AtomicWriteCommitted
        }
        ResourceMemoryAccessKindV1::AtomicReadWriteCommitted => {
            SimulationDebugMemoryAccessV1::AtomicReadWriteCommitted
        }
    }
}

fn protocol_access(access: SimulationDebugMemoryAccessV1) -> ResourceMemoryAccessKindV1 {
    match access {
        SimulationDebugMemoryAccessV1::Read => ResourceMemoryAccessKindV1::Read,
        SimulationDebugMemoryAccessV1::WriteCommitted => ResourceMemoryAccessKindV1::WriteCommitted,
        SimulationDebugMemoryAccessV1::AtomicRead => ResourceMemoryAccessKindV1::AtomicRead,
        SimulationDebugMemoryAccessV1::AtomicWriteCommitted => {
            ResourceMemoryAccessKindV1::AtomicWriteCommitted
        }
        SimulationDebugMemoryAccessV1::AtomicReadWriteCommitted => {
            ResourceMemoryAccessKindV1::AtomicReadWriteCommitted
        }
    }
}

/// A size fallback can make an already retained next token inaccessible to the
/// client. It stays within the same 256-entry budget and is cleared on revision
/// change/session destruction; this writer never guesses or reconstructs it.
pub(crate) fn write_resource_response_v1<W: Write>(
    writer: &mut W,
    response: &ResourceResponseV1,
    limits: ProtocolLimitsV1,
) -> Result<(), String> {
    let bytes = match encode_resource_response_line_v1(response, limits) {
        Ok(bytes) => bytes,
        Err(ProtocolCodecErrorV1::ResponseTooLarge) => {
            let (request_id, operation, session) = match response {
                ResourceResponseV1::Ok {
                    request_id,
                    operation,
                    session,
                    ..
                }
                | ResourceResponseV1::Unavailable {
                    request_id,
                    operation,
                    session,
                    ..
                } => (Some(*request_id), *operation, *session),
                ResourceResponseV1::Error {
                    request_id,
                    operation,
                    session,
                    ..
                } => (*request_id, *operation, *session),
            };
            encode_resource_response_line_v1(
                &ResourceResponseV1::Error {
                    schema: ResourceResponseSchemaV1::V1,
                    request_id,
                    operation,
                    session,
                    error: DebugErrorV1 {
                        stage: DebugErrorStageV1::Output,
                        code: DebugErrorCodeV1::ResponseTooLarge,
                        message: "resource response exceeds the configured JSONL bound".to_owned(),
                        state_changed: false,
                    },
                },
                limits,
            )
            .map_err(|error| format!("failed to encode resource fallback: {error}"))?
        }
        Err(error) => return Err(format!("failed to encode resource response: {error}")),
    };
    writer
        .write_all(&bytes)
        .and_then(|()| writer.flush())
        .map_err(|_| "failed to write resource response".to_owned())
}

#[cfg(test)]
mod tests;
