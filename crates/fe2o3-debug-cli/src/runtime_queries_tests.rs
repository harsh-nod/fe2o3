//! Pure custody/projection controls. Actual ordinary-source CLI coverage is a
//! separate end-to-end gate; these tests do not fabricate retained owner facts.
use super::*;
use fe2o3_kir_debugger::{
    RuntimeAllocationMissingV1, RuntimeFrameMissingV1, RuntimeObservationCoverageV1,
    RuntimeObservationCutoffV1, RuntimeOriginMissingV1, RuntimeReplayWorkV1,
};
use fe2o3_kir_sim::{
    SimulationAllocationObservationUnavailableV1, SimulationAllocationWatermarkV1,
    SimulationDebugOriginUnavailableV1, SimulationInvocationV1,
};

fn cursor() -> DebugCursorV1 {
    DebugCursorV1 {
        configuration_identity: OpaqueIdentityV1::new([4; 32]).unwrap(),
        event_sequence: 8,
        state_revision: 2,
    }
}
fn request(state: &RuntimeQueryStateV1) -> ResourceRequestV2 {
    ResourceRequestV2::QueryAllocations {
        schema: ResourceRequestSchemaV2::V2,
        request_id: 1,
        expected_revision: 2,
        expected_binding: RuntimeObservationBindingV1 {
            owner: state.owner(17),
            cursor: cursor(),
        },
        address_space: None,
        page: ResourceQueryPageV1 {
            max_items: 4,
            max_scanned: 8,
            token: None,
        },
    }
}
fn refresh(state: &mut RuntimeQueryStateV1, request: &ResourceRequestV2) {
    state.refresh(Some(request.expected_binding()));
}
fn with_token(mut request: ResourceRequestV2, token: ResourcePageTokenV1) -> ResourceRequestV2 {
    let ResourceRequestV2::QueryAllocations { page, .. } = &mut request else {
        unreachable!()
    };
    page.token = Some(token);
    request
}
#[test]
fn runtime_query_backend_lifetime_tokens_are_distinct_even_for_same_capture_number() {
    let first = RuntimeQueryStateV1::new().unwrap();
    let second = RuntimeQueryStateV1::new().unwrap();
    assert_ne!(first.owner(17), second.owner(17));
}
#[test]
fn runtime_query_tokens_are_owned_consumed_and_never_decoded_as_position() {
    let mut state = RuntimeQueryStateV1::new().unwrap();
    let original = request(&state);
    refresh(&mut state, &original);
    let token = state.retain_page(&original, Some(5)).unwrap().unwrap();
    let next = with_token(original.clone(), token);
    let mut work = state.work().unwrap();
    assert_eq!(state.take_page(&next, &mut work).unwrap(), 5);
    assert!(state.take_page(&next, &mut work).is_err());
    let invented = with_token(
        original,
        ResourcePageTokenV1::new("runtime.1.5".into()).unwrap(),
    );
    assert!(state.take_page(&invented, &mut work).is_err());
}
#[test]
fn runtime_query_changed_filter_limits_or_binding_consumes_and_refuses_token() {
    for control in 0..5 {
        let mut state = RuntimeQueryStateV1::new().unwrap();
        let original = request(&state);
        refresh(&mut state, &original);
        let token = state.retain_page(&original, Some(4)).unwrap().unwrap();
        let correct = with_token(original, token);
        let mut changed = correct.clone();
        let ResourceRequestV2::QueryAllocations {
            address_space,
            page,
            expected_binding,
            ..
        } = &mut changed
        else {
            unreachable!()
        };
        match control {
            0 => *address_space = Some(AddressSpaceV1::Private),
            1 => page.max_items += 1,
            2 => page.max_scanned += 1,
            3 => expected_binding.owner.capture_instance = number(18),
            _ => expected_binding.cursor.event_sequence += 1,
        }
        let mut work = state.work().unwrap();
        assert!(state.take_page(&changed, &mut work).is_err());
        assert!(state.take_page(&correct, &mut work).is_err());
    }
}
#[test]
fn runtime_query_cursor_fences_cover_revision_record_configuration_and_capture() {
    for control in 0..5 {
        let mut state = RuntimeQueryStateV1::new().unwrap();
        let original = request(&state);
        refresh(&mut state, &original);
        let token = state.retain_page(&original, Some(2)).unwrap().unwrap();
        let next = with_token(original.clone(), token);
        let mut binding = original.expected_binding();
        match control {
            0 => binding.cursor.state_revision += 1,
            1 => binding.cursor.event_sequence += 1,
            2 => binding.cursor.configuration_identity = OpaqueIdentityV1::new([5; 32]).unwrap(),
            3 => binding.owner.capture_instance = number(18),
            _ => binding.owner.backend_session = number(binding.owner.backend_session.get() + 1),
        }
        state.refresh(Some(binding));
        let mut work = state.work().unwrap();
        assert!(state.take_page(&next, &mut work).is_err());
        state.refresh(Some(original.expected_binding()));
        assert!(state.take_page(&next, &mut work).is_err());
    }
}
#[test]
fn runtime_query_continuation_limits_are_hard_and_clear_does_not_reuse_token() {
    let mut state = RuntimeQueryStateV1::new().unwrap();
    let original = request(&state);
    refresh(&mut state, &original);
    let first = state.retain_page(&original, Some(1)).unwrap().unwrap();
    for index in 1..256 {
        state.retain_page(&original, Some(index + 1)).unwrap();
    }
    assert!(state.retain_page(&original, Some(257)).is_err());
    state.refresh(None);
    refresh(&mut state, &original);
    let fresh = state.retain_page(&original, Some(1)).unwrap().unwrap();
    assert_ne!(first, fresh);
}
#[test]
fn runtime_query_cursor_work_is_prepaid_and_cumulative_budget_is_not_refunded() {
    let mut state = RuntimeQueryStateV1::new().unwrap();
    let original = request(&state);
    refresh(&mut state, &original);
    let token = state.retain_page(&original, Some(1)).unwrap().unwrap();
    let request = with_token(original, token);
    let mut none = RuntimeReplayWorkV1::new(0).unwrap();
    assert!(state.take_page(&request, &mut none).is_err());
    let mut sufficient = state.work().unwrap();
    assert_eq!(state.take_page(&request, &mut sufficient).unwrap(), 1);
    // Actual work objects spend 1m per iteration; no fabricated refund/negative
    // accounting or infinite query-reset budget is accepted.
    for _ in 0..64 {
        let mut work = state.work().unwrap();
        let before = work.remaining();
        work.charge(before).unwrap();
        state.settle_work(before, work.remaining());
    }
    assert!(state.work().is_err());
    state.refresh(None);
    assert!(state.work().is_err());
}
#[test]
fn runtime_query_memory_read_cannot_mint_a_page_cursor() {
    let mut state = RuntimeQueryStateV1::new().unwrap();
    let original = request(&state);
    refresh(&mut state, &original);
    let read = ResourceRequestV2::ReadAllocationMemory {
        schema: ResourceRequestSchemaV2::V2,
        request_id: 1,
        expected_revision: 2,
        expected_binding: original.expected_binding(),
        allocation: ResourceStorageIdentityV2 {
            allocation: number(1),
            storage_slot: number(1),
            generation: number(1),
        },
        range: ResourceMemoryRangeV1 {
            byte_offset: number(0),
            byte_len: number(4),
        },
    };
    assert!(state.retain_page(&read, Some(1)).is_err());
}
#[test]
fn runtime_query_mapping_preserves_full_invocation_without_lane_reconstruction() {
    let input = SimulationInvocationV1 {
        global: [9, 5, 2],
        workgroup: [2, 1, 1],
        local: [1, 2, 0],
        workgroup_size: [4, 3, 2],
        workgroup_count: [3, 2, 2],
        launch_extent: [10, 6, 4],
    };
    let mapped = mapping::invocation(input);
    mapped.validate().unwrap();
    assert_eq!(mapped.global.map(|n| n.get()), input.global);
    assert_eq!(mapped.workgroup.map(|n| n.get()), input.workgroup);
    assert_eq!(mapped.local, input.local);
    assert_eq!(mapped.workgroup_size, input.workgroup_size);
    assert_eq!(
        mapped.workgroup_count.map(|n| n.get()),
        input.workgroup_count
    );
    assert_eq!(mapped.launch_extent.map(|n| n.get()), input.launch_extent);
}
#[test]
fn runtime_query_unavailable_origin_and_frames_never_inherit_a_prior_checkpoint() {
    for error in [
        RuntimeOriginMissingV1::NoCurrentRecord,
        RuntimeOriginMissingV1::Disabled,
        RuntimeOriginMissingV1::InvalidJoin,
        RuntimeOriginMissingV1::RuntimeUnavailable(
            SimulationDebugOriginUnavailableV1::NoMatchingOperation,
        ),
    ] {
        assert!(matches!(
            mapping::origin(Err(error)),
            RuntimeOperationObservationV1::Unavailable { .. }
        ));
    }
    for error in [
        RuntimeFrameMissingV1::NoCurrentRecord,
        RuntimeFrameMissingV1::Disabled,
        RuntimeFrameMissingV1::InvalidJoin,
    ] {
        assert!(matches!(
            mapping::frames(Err(error), 64),
            RuntimeFramesV1::Unavailable { .. }
        ));
    }
}
#[test]
fn runtime_query_allocation_stoppage_is_not_a_complete_empty_lifecycle() {
    let reason = mapping::allocation_missing(RuntimeAllocationMissingV1::RuntimeUnavailable(
        SimulationAllocationWatermarkV1::Unavailable {
            reason: SimulationAllocationObservationUnavailableV1::ObservationStopped,
        },
    ));
    assert_eq!(reason, RuntimeObservationUnavailableV1::ObservationStopped);
    assert_eq!(
        mapping::allocation_missing(RuntimeAllocationMissingV1::WorkLimit),
        RuntimeObservationUnavailableV1::WorkLimit
    );
}
#[test]
fn runtime_query_coverage_reports_the_actual_independent_validation_work_cutoff() {
    assert_eq!(
        mapping::coverage(
            RuntimeObservationCoverageV1::PrefixTruncated(
                RuntimeObservationCutoffV1::ValidationWorkLimit
            ),
            12
        ),
        RuntimeMetadataCoverageV1::PrefixTruncated {
            retained_records: number(12),
            reason: RuntimeMetadataCutoffV1::ValidationWorkLimit
        }
    );
}
