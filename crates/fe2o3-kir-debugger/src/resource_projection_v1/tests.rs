use super::*;
use crate::{DebuggerLimitsV1, capture_debugger_run_v1};
use fe2o3_kernel_ir::VerifiedCanonicalKernelIrV7;
use fe2o3_kir_sim::{
    AdmittedSimulationModuleV1, BufferArgumentV1, ScalarBitsV1, SimulationArgumentV1,
    SimulationDebugCaptureLimitsV1, SimulationLimitsV1, SimulationRequestV1, SimulationTargetV1,
};

// Positive observations come from executing the existing retained canonical KIR
// fixture. It is deliberately not described as ordinary-Rust source evidence.
fn captured(memory_budget: usize, record_budget: usize) -> DebugSessionV1 {
    captured_grid(memory_budget, record_budget, 4)
}

fn captured_grid(
    memory_budget: usize,
    record_budget: usize,
    invocation_count: usize,
) -> DebugSessionV1 {
    let canonical = include_bytes!("../../../fe2o3-kir-sim-cli/tutorial/fill-v1/kernel.kir");
    let verified = VerifiedCanonicalKernelIrV7::from_canonical_bytes(canonical.to_vec()).unwrap();
    let admitted =
        AdmittedSimulationModuleV1::admit(verified, SimulationLimitsV1::default()).unwrap();
    let buffer = BufferArgumentV1::from_scalars(
        AccessMode::ReadWrite,
        4,
        &vec![ScalarBitsV1::u32(0); invocation_count],
        SimulationTargetV1::amdgpu_64(),
    )
    .unwrap();
    let request = SimulationRequestV1::new(
        "fill",
        [invocation_count as u64, 1, 1],
        [64, 1, 1],
        vec![SimulationArgumentV1::Buffer(buffer)],
    );
    let run = capture_debugger_run_v1(
        &admitted,
        &request,
        SimulationTargetV1::amdgpu_64(),
        SimulationLimitsV1::default(),
        SimulationDebugCaptureLimitsV1::new(16, 256, 16, memory_budget).unwrap(),
        DebuggerLimitsV1::new(record_budget, 32_768, 1 << 20).unwrap(),
        DebugWaveWidthV1::Wave32,
    );
    if record_budget == 4096 {
        assert!(run.execution.is_ok(), "{:?}", run.execution);
    }
    DebugSessionV1::new(run.transcript)
}

fn session() -> DebugSessionV1 {
    captured(4096, 4096)
}

fn select_last(session: &mut DebugSessionV1) {
    let last = session.transcript().records().len() - 1;
    session.seek_record_index(last);
}

fn items<T>(page: ResourcePageV1<T>) -> Vec<T> {
    match page.data {
        ResourcePageDataV1::Available(items) => items,
        ResourcePageDataV1::Unavailable { reason, .. } => {
            panic!("unexpected unavailable: {reason:?}")
        }
    }
}

fn small_request() -> ResourcePageRequestV1 {
    ResourcePageRequestV1 {
        limits: ResourcePageLimitsV1 {
            max_items: 1,
            max_scanned: 1,
        },
        cursor: None,
    }
}

#[test]
fn actual_checkpoint_inventory_preserves_capacity_permissions_and_availability() {
    let mut session = session();
    session.seek_record_index(0);
    let owner = ResourceProjectionOwnerV1::new().unwrap();
    let view = owner.bind(&session, [1; 32], 7).unwrap();
    let selected = view.selection();
    let page = view
        .allocations(&selected, None, ResourcePageRequestV1::default())
        .unwrap();
    assert_eq!(page.selection, selected);
    assert_eq!(page.completeness, DebugTranscriptCompletenessV1::Complete);
    assert_eq!(page.scanned, 1);
    assert_eq!(page.source_count, 1);
    assert!(page.next_cursor.is_none());
    assert_eq!(
        page.physical_registers,
        ResourceUnavailableFactV1::NotRepresented
    );
    assert_eq!(
        items(page),
        vec![ResourceAllocationObservationV1 {
            allocation: ResourceAllocationIdentityV1 {
                ordinal: 1,
                generation: 0
            },
            address_space: AddressSpace::Global,
            access: AccessMode::ReadWrite,
            alignment: 4,
            capacity_bytes: 16,
            snapshot_bytes_available: true,
            initialization_available: true,
            owning_scope: ResourceUnavailableFactV1::NotRepresented,
            lifetime: ResourceUnavailableFactV1::NotRepresented,
            physical_base: ResourceUnavailableFactV1::NotRepresented,
        }]
    );
    let filtered = view
        .allocations(&selected, Some(AddressSpace::Workgroup), small_request())
        .unwrap();
    assert_eq!(filtered.scanned, 1);
    assert!(items(filtered).is_empty());
}

#[test]
fn actual_accesses_preserve_each_record_occurrence_scope_site_and_range() {
    let mut session = session();
    select_last(&mut session);
    let owner = ResourceProjectionOwnerV1::new().unwrap();
    let view = owner.bind(&session, [1; 32], 1).unwrap();
    let selected = view.selection();
    let projected = items(
        view.accesses(
            &selected,
            ResourceAccessFilterV1::default(),
            ResourcePageRequestV1::default(),
        )
        .unwrap(),
    );
    assert_eq!(projected.len(), 4);
    let actual: Vec<_> = session
        .transcript()
        .records()
        .iter()
        .filter(|record| matches!(record.kind, SimulationDebugRecordKindV1::Memory { .. }))
        .collect();
    for (index, (row, record)) in projected.iter().zip(actual).enumerate() {
        assert_eq!(row.occurrence.record_ordinal, record.ordinal);
        assert_eq!(row.occurrence.event_sequence, record.ordinal + 1);
        assert_eq!(row.occurrence.invocation, record.invocation);
        assert_eq!(row.occurrence.site, record.site);
        assert_eq!(row.occurrence.schedule, record.schedule);
        assert_eq!(row.occurrence.checkpoint_phase, None);
        assert_eq!(row.occurrence.hierarchy.workgroup, [0, 0, 0]);
        assert_eq!(row.occurrence.hierarchy.lane, index as u16);
        assert_eq!(
            row.range,
            ResourceByteRangeV1 {
                byte_offset: index as u64 * 4,
                byte_len: 4
            }
        );
        assert_eq!(
            row.allocation,
            ResourceAllocationIdentityV1 {
                ordinal: 1,
                generation: 0
            }
        );
        assert_eq!(row.address_space, AddressSpace::Global);
        assert_eq!(row.access, SimulationDebugMemoryAccessV1::WriteCommitted);
        assert_eq!(
            row.operation_occurrence,
            ResourceUnavailableFactV1::NotRepresented
        );
    }
    assert!(
        projected
            .windows(2)
            .all(|rows| rows[0].occurrence.record_ordinal < rows[1].occurrence.record_ordinal)
    );
}

#[test]
fn history_never_includes_records_after_the_selected_cursor() {
    let mut session = session();
    let first_access = session
        .transcript()
        .records()
        .iter()
        .position(|record| matches!(record.kind, SimulationDebugRecordKindV1::Memory { .. }))
        .unwrap();
    session.seek_record_index(first_access - 1);
    let owner = ResourceProjectionOwnerV1::new().unwrap();
    let view = owner.bind(&session, [1; 32], 1).unwrap();
    let page = view
        .accesses(
            &view.selection(),
            ResourceAccessFilterV1::default(),
            ResourcePageRequestV1::default(),
        )
        .unwrap();
    assert_eq!(page.source_count, first_access);
    assert!(items(page).is_empty());
    session.seek_record_index(first_access);
    let view = owner.bind(&session, [1; 32], 2).unwrap();
    assert_eq!(
        items(
            view.accesses(
                &view.selection(),
                ResourceAccessFilterV1::default(),
                ResourcePageRequestV1::default()
            )
            .unwrap()
        )
        .len(),
        1
    );
    assert!(matches!(
        view.allocations(&view.selection(), None, ResourcePageRequestV1::default())
            .unwrap()
            .data,
        ResourcePageDataV1::Unavailable {
            reason: ResourceProjectionUnavailableV1::NotCheckpoint,
            ..
        }
    ));
}

#[test]
fn empty_filtered_pages_continue_and_each_query_has_an_independent_raw_scan_budget() {
    let mut session = session();
    select_last(&mut session);
    let owner = ResourceProjectionOwnerV1::new().unwrap();
    let view = owner.bind(&session, [1; 32], 1).unwrap();
    let mut request = small_request();
    let mut total_scanned = 0;
    let mut observed = Vec::new();
    loop {
        let page = view
            .accesses(
                &view.selection(),
                ResourceAccessFilterV1::default(),
                request.clone(),
            )
            .unwrap();
        assert_eq!(
            page,
            view.accesses(
                &view.selection(),
                ResourceAccessFilterV1::default(),
                request.clone()
            )
            .unwrap()
        );
        assert!(page.scanned <= 1);
        total_scanned += usize::from(page.scanned);
        let next = page.next_cursor.clone();
        observed.extend(items(page));
        match next {
            Some(cursor) => request.cursor = Some(cursor),
            None => break,
        }
    }
    assert_eq!(total_scanned, session.transcript().records().len());
    assert_eq!(observed.len(), 4);
    let filtered = ResourceAccessFilterV1 {
        address_space: Some(AddressSpace::Workgroup),
        ..Default::default()
    };
    let empty = view
        .accesses(&view.selection(), filtered, small_request())
        .unwrap();
    assert_eq!(empty.scanned, 1);
    assert!(empty.next_cursor.is_some());
    assert!(items(empty).is_empty());
    let output_limited = view
        .accesses(
            &view.selection(),
            ResourceAccessFilterV1::default(),
            ResourcePageRequestV1 {
                limits: ResourcePageLimitsV1 {
                    max_items: 1,
                    max_scanned: 256,
                },
                cursor: None,
            },
        )
        .unwrap();
    assert!(output_limited.scanned > 1);
    assert!(output_limited.next_cursor.is_some());
    assert_eq!(items(output_limited).len(), 1);
}

#[test]
fn cursors_reject_other_owners_filters_query_kinds_and_page_limits() {
    let mut session = session();
    select_last(&mut session);
    let owner = ResourceProjectionOwnerV1::new().unwrap();
    let other_owner = ResourceProjectionOwnerV1::new().unwrap();
    let view = owner.bind(&session, [1; 32], 1).unwrap();
    let cursor = view
        .accesses(
            &view.selection(),
            ResourceAccessFilterV1::default(),
            small_request(),
        )
        .unwrap()
        .next_cursor
        .unwrap();
    let request = ResourcePageRequestV1 {
        cursor: Some(cursor),
        ..small_request()
    };
    let other = other_owner.bind(&session, [1; 32], 1).unwrap();
    assert_eq!(
        other
            .accesses(
                &view.selection(),
                ResourceAccessFilterV1::default(),
                request.clone()
            )
            .unwrap_err(),
        ResourceProjectionErrorV1::SelectionMismatch
    );
    assert_eq!(
        other
            .accesses(
                &other.selection(),
                ResourceAccessFilterV1::default(),
                request.clone()
            )
            .unwrap_err(),
        ResourceProjectionErrorV1::CursorMismatch
    );
    let filter = ResourceAccessFilterV1 {
        allocation: Some(ResourceAllocationIdentityV1 {
            ordinal: 1,
            generation: 0,
        }),
        ..ResourceAccessFilterV1::default()
    };
    assert_eq!(
        view.accesses(&view.selection(), filter, request.clone())
            .unwrap_err(),
        ResourceProjectionErrorV1::CursorMismatch
    );
    assert_eq!(
        view.allocations(&view.selection(), None, request.clone())
            .unwrap_err(),
        ResourceProjectionErrorV1::CursorMismatch
    );
    let changed_limits = ResourcePageRequestV1 {
        limits: ResourcePageLimitsV1 {
            max_items: 2,
            max_scanned: 1,
        },
        ..request
    };
    assert_eq!(
        view.accesses(
            &view.selection(),
            ResourceAccessFilterV1::default(),
            changed_limits
        )
        .unwrap_err(),
        ResourceProjectionErrorV1::CursorMismatch
    );
}

#[test]
fn stale_configuration_revision_scope_and_reverse_cursor_selections_reject() {
    let mut session = session();
    session.seek_record_index(0);
    let owner = ResourceProjectionOwnerV1::new().unwrap();
    let selected = owner.bind(&session, [1; 32], 1).unwrap().selection();
    for (configuration, revision) in [([2; 32], 1), ([1; 32], 2)] {
        let view = owner.bind(&session, configuration, revision).unwrap();
        assert_eq!(
            view.allocations(&selected, None, ResourcePageRequestV1::default())
                .unwrap_err(),
            ResourceProjectionErrorV1::SelectionMismatch
        );
    }
    session.seek_record_index(1);
    session.seek_record_index(0);
    let view = owner.bind(&session, [1; 32], 3).unwrap();
    assert_eq!(
        view.allocations(&selected, None, ResourcePageRequestV1::default())
            .unwrap_err(),
        ResourceProjectionErrorV1::SelectionMismatch
    );
    let mut wrong_scope = view.selection();
    wrong_scope.record.as_mut().unwrap().hierarchy.lane = 1;
    assert_eq!(
        view.allocations(&wrong_scope, None, ResourcePageRequestV1::default())
            .unwrap_err(),
        ResourceProjectionErrorV1::SelectionMismatch
    );
}

#[test]
fn half_open_range_and_logical_scope_filters_do_not_merge_workgroups() {
    // The unchanged retained kernel declares workgroup size64. Executing65
    // invocations genuinely visits a second workgroup; no scope is fabricated.
    let mut session = captured_grid(4096, 4096, 65);
    select_last(&mut session);
    let owner = ResourceProjectionOwnerV1::new().unwrap();
    let view = owner.bind(&session, [1; 32], 1).unwrap();
    let filter = ResourceAccessFilterV1 {
        scope: DebugScopeSelectorV1::Workgroup([1, 0, 0]),
        range: Some(ResourceByteRangeV1 {
            byte_offset: 256,
            byte_len: 4,
        }),
        ..ResourceAccessFilterV1::default()
    };
    let mut request = ResourcePageRequestV1::default();
    let mut projected = Vec::new();
    loop {
        let page = view
            .accesses(&view.selection(), filter.clone(), request.clone())
            .unwrap();
        assert_eq!(page.completeness, DebugTranscriptCompletenessV1::Complete);
        let next = page.next_cursor.clone();
        projected.extend(items(page));
        match next {
            Some(cursor) => request.cursor = Some(cursor),
            None => break,
        }
    }
    assert_eq!(projected.len(), 1);
    assert_eq!(projected[0].range.byte_offset, 256);
    assert_eq!(projected[0].occurrence.hierarchy.workgroup, [1, 0, 0]);
    assert_eq!(projected[0].occurrence.hierarchy.lane, 0);
}

#[test]
fn missing_memory_capture_and_partial_transcript_remain_explicit() {
    let mut no_memory = captured(1, 4096);
    let owner = ResourceProjectionOwnerV1::new().unwrap();
    let view = owner.bind(&no_memory, [1; 32], 0).unwrap();
    assert!(matches!(
        view.allocations(&view.selection(), None, ResourcePageRequestV1::default())
            .unwrap()
            .data,
        ResourcePageDataV1::Unavailable {
            reason: ResourceProjectionUnavailableV1::NoSelectedRecord,
            ..
        }
    ));
    no_memory.seek_record_index(0);
    let view = owner.bind(&no_memory, [1; 32], 1).unwrap();
    assert!(matches!(
        view.allocations(&view.selection(), None, ResourcePageRequestV1::default())
            .unwrap()
            .data,
        ResourcePageDataV1::Unavailable {
            reason: ResourceProjectionUnavailableV1::MemoryCapture(
                SimulationDebugUnavailableReasonV1::MemoryByteLimit
            ),
            required: Some(32)
        }
    ));
    let mut partial = captured(4096, 3);
    select_last(&mut partial);
    let partial_owner = ResourceProjectionOwnerV1::new().unwrap();
    let view = partial_owner.bind(&partial, [2; 32], 1).unwrap();
    let page = view
        .accesses(
            &view.selection(),
            ResourceAccessFilterV1::default(),
            ResourcePageRequestV1::default(),
        )
        .unwrap();
    assert!(matches!(
        page.completeness,
        DebugTranscriptCompletenessV1::Truncated(_)
    ));
    assert_eq!(page.source_count, 3);
}

#[test]
fn generation_zero_and_page_range_bounds_are_checked_before_scanning() {
    let mut session = session();
    select_last(&mut session);
    let owner = ResourceProjectionOwnerV1::new().unwrap();
    let view = owner.bind(&session, [1; 32], 1).unwrap();
    let filter = ResourceAccessFilterV1 {
        allocation: Some(ResourceAllocationIdentityV1 {
            ordinal: 1,
            generation: 1,
        }),
        ..ResourceAccessFilterV1::default()
    };
    assert_eq!(
        view.accesses(&view.selection(), filter, ResourcePageRequestV1::default())
            .unwrap_err(),
        ResourceProjectionErrorV1::UnsupportedAllocationGeneration
    );
    for (range, expected) in [
        (
            ResourceByteRangeV1 {
                byte_offset: 0,
                byte_len: 0,
            },
            ResourceProjectionErrorV1::ZeroLengthRange,
        ),
        (
            ResourceByteRangeV1 {
                byte_offset: u64::MAX,
                byte_len: 1,
            },
            ResourceProjectionErrorV1::RangeOverflow,
        ),
    ] {
        let filter = ResourceAccessFilterV1 {
            range: Some(range),
            ..ResourceAccessFilterV1::default()
        };
        assert_eq!(
            view.accesses(&view.selection(), filter, ResourcePageRequestV1::default())
                .unwrap_err(),
            expected
        );
    }
    for limits in [
        ResourcePageLimitsV1 {
            max_items: 0,
            max_scanned: 1,
        },
        ResourcePageLimitsV1 {
            max_items: 257,
            max_scanned: 1,
        },
        ResourcePageLimitsV1 {
            max_items: 1,
            max_scanned: 257,
        },
    ] {
        assert_eq!(
            view.allocations(
                &view.selection(),
                None,
                ResourcePageRequestV1 {
                    limits,
                    cursor: None
                }
            )
            .unwrap_err(),
            ResourceProjectionErrorV1::InvalidPageLimits
        );
    }
}

#[test]
fn mutated_zero_length_and_overflowing_access_records_fail_closed() {
    // Hostile mutations of an actual retained record exercise rejection only.
    for (offset, length, expected) in [
        (0, 0, ResourceProjectionErrorV1::ZeroLengthRange),
        (usize::MAX, 1, ResourceProjectionErrorV1::RangeOverflow),
    ] {
        let mut session = session();
        let record = session
            .transcript
            .records
            .iter_mut()
            .find(|record| matches!(record.kind, SimulationDebugRecordKindV1::Memory { .. }))
            .unwrap();
        if let SimulationDebugRecordKindV1::Memory {
            byte_offset,
            byte_len,
            ..
        } = &mut record.kind
        {
            *byte_offset = offset;
            *byte_len = length;
        }
        select_last(&mut session);
        let owner = ResourceProjectionOwnerV1::new().unwrap();
        let view = owner.bind(&session, [1; 32], 1).unwrap();
        assert_eq!(
            view.accesses(
                &view.selection(),
                ResourceAccessFilterV1::default(),
                ResourcePageRequestV1::default()
            )
            .unwrap_err(),
            expected
        );
    }
}

#[test]
fn process_local_owner_ids_are_nonzero_and_do_not_wrap() {
    let counter = AtomicU64::new(0);
    assert_eq!(allocate_owner(&counter), Ok(1));
    assert_eq!(allocate_owner(&counter), Ok(2));
    let exhausted = AtomicU64::new(u64::MAX);
    assert_eq!(
        allocate_owner(&exhausted),
        Err(ResourceProjectionErrorV1::OwnerExhausted)
    );
    let session = session();
    let owner = ResourceProjectionOwnerV1::new().unwrap();
    assert!(matches!(
        owner.bind(&session, [0; 32], 0),
        Err(ResourceProjectionErrorV1::ZeroConfigurationIdentity)
    ));
}
