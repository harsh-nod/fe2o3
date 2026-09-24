//! Borrowed typed CPU views projected into the unchanged JSONL grammar.
use super::*;
pub(super) fn capabilities() -> Vec<CapabilityViewV1> {
    use DebugCapabilityNameV1::*;
    [
        HierarchyInspection,
        KirSites,
        SourceSites,
        CallStack,
        Breakpoints,
        Watchpoints,
        ForwardStep,
        ReverseStep,
        Pause,
        DeterministicReplay,
        KirSsaValues,
        SourceVariableValues,
        RegisterValues,
        AllocationRelativeMemory,
        SemanticTrace,
        HardwareWaveState,
        KfdDispatchControl,
    ]
    .into_iter()
    .map(|name| {
        let available = matches!(
            name,
            KirSites | ForwardStep | ReverseStep | KirSsaValues | AllocationRelativeMemory
        );
        CapabilityViewV1 {
            name,
            availability: if available {
                CapabilityAvailabilityV1::Available
            } else {
                CapabilityAvailabilityV1::Unavailable
            },
            reason: if available {
                None
            } else {
                Some(if matches!(name, SourceSites | SourceVariableValues) {
                    CapabilityUnavailableReasonV1::RequiresAuthenticatedMap
                } else {
                    CapabilityUnavailableReasonV1::NotExposedByBackend
                })
            },
        }
    })
    .collect()
}
fn scope(record: Record<'_>) -> ExecutionScopeV1 {
    let invocation = record.invocation();
    ExecutionScopeV1::Lane {
        workgroup: invocation.workgroup.map(|n| n as u32),
        wave: 0,
        lane: invocation.local[0] as u16,
        logical_workitem: invocation.global,
        active_mask: u64::MAX,
        wave_width: 64,
        interpretation: WaveInterpretationV1::LogicalVisualization,
    }
}
pub(super) fn same_scope(record: Record<'_>, selector: ExecutionScopeSelectorV1) -> bool {
    matches!((scope(record),selector),(ExecutionScopeV1::Lane {workgroup:a,wave:b,lane:c,..},
 ExecutionScopeSelectorV1::Lane {workgroup:x,wave:y,lane:z}) if(a,b,c)==(x,y,z))
}
pub(super) fn anchor(record: Record<'_>, view: SessionViewV1) -> DebugSnapshotAnchorV1 {
    let site = record.site();
    DebugSnapshotAnchorV1 {
        cursor: view.cursor,
        scope: scope(record),
        site: Some(SemanticSiteViewV1 {
            kir: KirSiteV1 {
                function_ordinal: site.function_ordinal as u64,
                block_ordinal: u64::from(site.block.0),
                point: KirSitePointV1::Operation {
                    operation_ordinal: u64::from(site.operation),
                },
            },
            source: SourceSiteAvailabilityV1::Unavailable {
                reason: SourceSiteUnavailableReasonV1::RequiresAuthenticatedMap,
            },
        }),
        // No runtime activation/occurrence identity is invented by this observation adapter.
        frame: None,
        occurrence: None,
    }
}
pub(super) fn stop(sequence: u64, total: usize) -> StopViewV1 {
    let end = sequence == total as u64 + 1;
    StopViewV1 {
        reason: if end {
            StopReasonV1::Completed
        } else if sequence == 0 {
            StopReasonV1::Entry
        } else {
            StopReasonV1::Step
        },
        breakpoint_id: None,
        watchpoint_id: None,
        outcome: if end {
            ExecutionOutcomeV1::Completed
        } else {
            ExecutionOutcomeV1::Active
        },
        exact: true,
    }
}
pub(super) fn snapshot(
    backend: &Backend,
    sequence: u64,
    view: SessionViewV1,
) -> SnapshotAvailabilityV1 {
    let record = sequence
        .checked_sub(1)
        .and_then(|n| usize::try_from(n).ok())
        .and_then(|n| backend.session.record(n));
    let Some(record) = record.filter(|r| r.phase().is_some()) else {
        return SnapshotAvailabilityV1::Unavailable {
            reason: SnapshotUnavailableReasonV1::NotCaptured,
        };
    };
    SnapshotAvailabilityV1::Captured {
        snapshot: Box::new(DebugSnapshotV1 {
            anchor: anchor(record, view),
            stop: stop(sequence, backend.session.records_len()),
            values: Vec::new(),
        }),
    }
}
fn value(record: Record<'_>, index: usize) -> Option<DebugValueV1> {
    let binding = record.binding(0, index)?;
    let availability = if let Some(scalar) = binding.scalar() {
        let (value_type, width) = protocol_scalar_type(scalar);
        ValueAvailabilityV1::Captured {
            value_type,
            value: CapturedValueV1::Bits {
                bits: fixed_width_bits(scalar.bits(), width),
            },
            provenance: ValueProvenanceV1::SimulatedObservation,
        }
    } else if binding.symbolic_kind().is_some() {
        // Keep the actual binding, but never serialize invented pointer/carry bits.
        ValueAvailabilityV1::Unavailable {
            reason: ValueUnavailableReasonV1::NotRepresented,
        }
    } else if let (Some((allocation, offset)), Some(address_space)) = (
        binding.logical_pointer(),
        binding.logical_pointer_address_space(),
    ) {
        ValueAvailabilityV1::Captured {
            value_type: DebugValueTypeV1::Pointer {
                address_space: protocol_address_space(address_space),
            },
            value: CapturedValueV1::AllocationRelativePointer {
                allocation: AllocationIdentityV1 {
                    ordinal: allocation,
                    generation: 0,
                },
                byte_offset: offset as u64,
            },
            provenance: ValueProvenanceV1::SimulatedObservation,
        }
    } else {
        ValueAvailabilityV1::Unavailable {
            reason: ValueUnavailableReasonV1::NotRepresented,
        }
    };
    Some(DebugValueV1 {
        path: ValuePathV1 {
            root: ValueRootV1::Ssa {
                function_ordinal: record.site().function_ordinal as u64,
                // Closed no-call physical profile: logical stack depth 0 is displayed as frame 1.
                frame: 1,
                value_ordinal: u64::from(binding.value().0),
            },
            components: Vec::new(),
        },
        availability,
    })
}
pub(super) fn values(
    backend: &Backend,
    id: u64,
    scope: ExecutionScopeSelectorV1,
    frame: Option<u64>,
    selector: ValueSelectorV1,
    page: PageRequestV1,
) -> DebugResponseV1 {
    let op = DebugOperationNameV1::InspectValues;
    let unavailable = || {
        backend.unavailable(
            id,
            op,
            DebugCapabilityNameV1::KirSsaValues,
            CapabilityUnavailableReasonV1::OutsideCaptureScope,
        )
    };
    let Some(record) = backend
        .session
        .current()
        .filter(|r| r.phase().is_some() && same_scope(*r, scope))
    else {
        return unavailable();
    };
    if frame.is_some_and(|f| f != 1) || page.limit as usize > PAGE {
        return unavailable();
    }
    if !matches!(&selector, ValueSelectorV1::All)
        && !matches!(&selector,ValueSelectorV1::Roots {roots} if roots.as_slice()==[ValueRootClassV1::Ssa])
    {
        return backend.unavailable(
            id,
            op,
            DebugCapabilityNameV1::KirSsaValues,
            CapabilityUnavailableReasonV1::NotExposedByBackend,
        );
    }
    let mut hash = Sha256::new();
    hash.update(b"fe2o3-debug-physical-v20-ssa-page-v1\0");
    hash.update(backend.configuration.as_bytes());
    hash.update(backend.sequence().to_le_bytes());
    hash.update(backend.revision.to_le_bytes());
    let Ok(query) = OpaqueIdentityV1::new(hash.finalize().into()) else {
        return unavailable();
    };
    let start = match page.cursor {
        None => 0,
        Some(c) if c.query_identity == query => match usize::try_from(c.position) {
            Ok(n) => n,
            Err(_) => return unavailable(),
        },
        _ => return unavailable(),
    };
    let count = record.binding_count(0).unwrap_or(0);
    if start > count {
        return unavailable();
    }
    let end = start.saturating_add(usize::from(page.limit)).min(count);
    let values = (start..end).filter_map(|n| value(record, n)).collect();
    backend.ok_at(
        id,
        op,
        DebugResultV1::Values {
            snapshot: anchor(record, backend.view()),
            values,
            next_cursor: (end < count).then_some(PageCursorV1 {
                query_identity: query,
                position: end as u64,
            }),
        },
        backend.view(),
    )
}
pub(super) fn memory(
    backend: &Backend,
    id: u64,
    allocation: AllocationIdentityV1,
    offset: u64,
    length: u64,
) -> DebugResponseV1 {
    let op = DebugOperationNameV1::ReadMemory;
    let unavailable = || {
        backend.unavailable(
            id,
            op,
            DebugCapabilityNameV1::AllocationRelativeMemory,
            CapabilityUnavailableReasonV1::OutsideCaptureScope,
        )
    };
    let Some(record) = backend.session.current().filter(|r| r.phase().is_some()) else {
        return unavailable();
    };
    if allocation.generation != 0 || length == 0 || length > 256 {
        return unavailable();
    }
    let Some((index, (_, bytes))) = (0..8).find_map(|index| {
        record
            .memory_allocation(index)
            .filter(|(id, _)| *id == allocation.ordinal)
            .map(|row| (index, row))
    }) else {
        return unavailable();
    };
    let Ok(offset_usize) = usize::try_from(offset) else {
        return unavailable();
    };
    let Some(end) = offset_usize
        .checked_add(length as usize)
        .filter(|n| *n <= bytes)
    else {
        return unavailable();
    };
    let Some(space) = record.memory_address_space(index) else {
        return unavailable();
    };
    let mut bytes = Vec::with_capacity(length as usize);
    let mut initialized = Vec::with_capacity(length as usize);
    for position in offset_usize..end {
        let Some((byte, init)) = record.memory_byte_at(index, position) else {
            return unavailable();
        };
        bytes.push(byte);
        initialized.push(init);
    }
    backend.ok_at(
        id,
        op,
        DebugResultV1::Memory {
            snapshot: anchor(record, backend.view()),
            memory: MemoryReadV1 {
                allocation,
                byte_offset: offset,
                requested_bytes: length,
                returned_bytes: length,
                availability: MemoryAvailabilityV1::Captured {
                    address_space: protocol_address_space(space),
                    bytes: hex_bytes(&bytes),
                    initialized: initialization_bits(&initialized),
                    truncated: false,
                },
            },
        },
        backend.view(),
    )
}
