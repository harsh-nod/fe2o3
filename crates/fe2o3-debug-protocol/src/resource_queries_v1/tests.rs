use super::*;
use crate::OpaqueIdentityV1;
use crate::{
    DebugCursorV1, SemanticSiteViewV1, SessionStateV1, SourceSiteAvailabilityV1,
    SourceSiteUnavailableReasonV1,
};

// These are codec conformance values, not claimed simulator/hardware captures.
fn anchor() -> DebugSnapshotAnchorV1 {
    DebugSnapshotAnchorV1 {
        cursor: DebugCursorV1 {
            configuration_identity: OpaqueIdentityV1::new([1; 32]).unwrap(),
            event_sequence: 9,
            state_revision: 6,
        },
        scope: ExecutionScopeV1::Lane {
            workgroup: [0; 3],
            wave: 0,
            lane: 0,
            logical_workitem: [0; 3],
            active_mask: 1,
            wave_width: 32,
            interpretation: WaveInterpretationV1::LogicalVisualization,
        },
        site: Some(SemanticSiteViewV1 {
            kir: KirSiteV1 {
                function_ordinal: 0,
                block_ordinal: 0,
                point: KirSitePointV1::Operation {
                    operation_ordinal: 3,
                },
            },
            source: SourceSiteAvailabilityV1::Unavailable {
                reason: SourceSiteUnavailableReasonV1::NotRepresented,
            },
        }),
        frame: None,
        occurrence: None,
    }
}

fn session() -> SessionViewV1 {
    SessionViewV1 {
        backend: DebugBackendV1::CpuKirSimulator,
        execution_kind: ExecutionKindV1::CpuKirSimulation,
        state: SessionStateV1::Stopped,
        revision: 6,
        configuration_identity: anchor().cursor.configuration_identity,
        cursor: anchor().cursor,
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
    }
}

fn request() -> ResourceRequestV1 {
    ResourceRequestV1::QueryAllocations {
        schema: ResourceRequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 6,
        expected_snapshot: Box::new(anchor()),
        address_space: None,
        page: ResourceQueryPageV1 {
            max_items: 8,
            max_scanned: 16,
            token: None,
        },
    }
}

fn access_request() -> ResourceRequestV1 {
    ResourceRequestV1::QueryMemoryAccesses {
        schema: ResourceRequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 6,
        expected_snapshot: Box::new(anchor()),
        filter: ResourceMemoryAccessFilterV1 {
            scope: ExecutionScopeSelectorV1::Dispatch,
            allocation: None,
            address_space: None,
            access: None,
            range: None,
        },
        page: request().page().clone(),
    }
}

fn response() -> ResourceResponseV1 {
    ResourceResponseV1::Ok {
        schema: ResourceResponseSchemaV1::V1,
        request_id: 1,
        operation: ResourceOperationV1::QueryAllocations,
        session: session(),
        snapshot: Box::new(anchor()),
        page: ResourcePageInfoV1 {
            source_count: 1,
            scanned: 1,
            completeness: CaptureCompletenessV1::Complete,
            next_token: None,
        },
        physical_registers: ResourceFactUnavailableV1::NotRepresented,
        result: ResourceQueryResultV1::Allocations {
            allocations: vec![ResourceAllocationV1 {
                allocation: AllocationIdentityV1 {
                    ordinal: 1,
                    generation: 0,
                },
                address_space: AddressSpaceV1::Global,
                access: ResourceAllocationAccessV1::ReadWrite,
                alignment: 4,
                capacity_bytes: ResourceDecimalU64V1::new(u64::MAX),
                snapshot_bytes_available: true,
                initialization_available: true,
                owning_scope: ResourceFactUnavailableV1::NotRepresented,
                lifetime: ResourceFactUnavailableV1::NotRepresented,
                physical_base: ResourceFactUnavailableV1::NotRepresented,
            }],
        },
    }
}

fn access_response() -> ResourceResponseV1 {
    let mut response = response();
    if let ResourceResponseV1::Ok {
        operation,
        page,
        result,
        ..
    } = &mut response
    {
        *operation = ResourceOperationV1::QueryMemoryAccesses;
        page.source_count = 9;
        page.scanned = 9;
        *result = ResourceQueryResultV1::MemoryAccesses {
            accesses: vec![ResourceMemoryAccessV1 {
                occurrence: ResourceAccessOccurrenceV1 {
                    record_ordinal: 7,
                    event_sequence: 8,
                    scope: anchor().scope,
                    site: anchor().site.unwrap().kir,
                    schedule: ResourceAccessScheduleV1 {
                        identity: ResourceScheduleIdentityV1::WorkgroupMajorLocalZyxCooperativeV1,
                        decision_ordinal: 0,
                    },
                },
                allocation: AllocationIdentityV1 {
                    ordinal: 1,
                    generation: 0,
                },
                range: ResourceMemoryRangeV1 {
                    byte_offset: ResourceDecimalU64V1::new(9_007_199_254_740_993),
                    byte_len: ResourceDecimalU64V1::new(4),
                },
                address_space: AddressSpaceV1::Global,
                access: ResourceMemoryAccessKindV1::WriteCommitted,
                call_frame: ResourceFactUnavailableV1::NotRepresented,
                operation_occurrence: ResourceFactUnavailableV1::NotRepresented,
                source_association: ResourceFactUnavailableV1::NotRepresented,
            }],
        };
    }
    response
}

fn line<T: Serialize>(value: &T) -> Vec<u8> {
    let mut line = serde_json::to_vec(value).unwrap();
    line.push(b'\n');
    line
}

#[test]
fn separately_versioned_requests_and_responses_roundtrip_losslessly() {
    for request in [request(), access_request()] {
        assert_eq!(
            decode_resource_request_line_v1(&line(&request), ProtocolLimitsV1::default()).unwrap(),
            request
        );
        assert!(
            crate::decode_request_line_v1(&line(&request), ProtocolLimitsV1::default()).is_err()
        );
    }
    for (request, response) in [
        (request(), response()),
        (access_request(), access_response()),
    ] {
        response
            .validate_for_request(&request, ProtocolLimitsV1::default())
            .unwrap();
        let encoded =
            encode_resource_response_line_v1(&response, ProtocolLimitsV1::default()).unwrap();
        assert_eq!(
            decode_resource_response_line_v1(&encoded, ProtocolLimitsV1::default()).unwrap(),
            response
        );
    }
    assert!(
        String::from_utf8(line(&response()))
            .unwrap()
            .contains("\"18446744073709551615\"")
    );
    assert!(
        String::from_utf8(line(&access_response()))
            .unwrap()
            .contains("\"9007199254740993\"")
    );
}

#[test]
fn registered_mixed_reader_routes_resources_without_changing_legacy_requests() {
    let legacy = b"{\"operation\":\"get_state\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":7,\"expected_revision\":0}\n";
    let input = [
        legacy.as_slice(),
        &line(&request()),
        &line(&access_request()),
    ]
    .concat();
    let mut reader = std::io::BufReader::new(input.as_slice());
    assert!(matches!(
        crate::read_request_line_any_v2(&mut reader, ProtocolLimitsV1::default()).unwrap(),
        Some(crate::DebugRequestAnyV2::V1(_))
    ));
    for expected in [request(), access_request()] {
        assert!(
            matches!(crate::read_request_line_any_v2(&mut reader, ProtocolLimitsV1::default()).unwrap(), Some(crate::DebugRequestAnyV2::ResourceV1(actual)) if actual == expected)
        );
    }
    assert_eq!(
        crate::read_request_line_any_v2(&mut reader, ProtocolLimitsV1::default()).unwrap(),
        None
    );
}

#[test]
fn decimal_strings_reject_lossy_noncanonical_and_overflow_forms() {
    for bad in [
        "0",
        "1.0",
        "-1",
        "null",
        "\"\"",
        "\"01\"",
        "\"+1\"",
        "\"-1\"",
        "\"1e2\"",
        "\" 1\"",
        "\"18446744073709551616\"",
    ] {
        assert!(
            serde_json::from_str::<ResourceDecimalU64V1>(bad).is_err(),
            "{bad}"
        );
    }
    for good in [0, 1, u64::MAX] {
        assert_eq!(
            serde_json::from_str::<ResourceDecimalU64V1>(&format!("\"{good}\""))
                .unwrap()
                .get(),
            good
        );
    }
}

#[test]
fn unknown_duplicate_missing_null_and_mutating_operations_reject() {
    let original = String::from_utf8(line(&request())).unwrap();
    for bad in [
        original.replacen("{", "{\"extra\":1,", 1),
        original.replacen("\"request_id\":1", "\"request_id\":1,\"request_id\":2", 1),
        original.replace("query_allocations", "continue"),
        original.replace("\"max_items\":8", "\"max_items\":8,\"token\":null"),
    ] {
        assert!(
            decode_resource_request_line_v1(bad.as_bytes(), ProtocolLimitsV1::default()).is_err()
        );
    }
    let mut value = serde_json::to_value(request()).unwrap();
    value.as_object_mut().unwrap().remove("expected_snapshot");
    assert!(decode_resource_request_line_v1(&line(&value), ProtocolLimitsV1::default()).is_err());
    let mut value = serde_json::to_value(response()).unwrap();
    value["result"]["allocations"][0]["physical_base"] = serde_json::json!(0);
    assert!(decode_resource_response_line_v1(&line(&value), ProtocolLimitsV1::default()).is_err());
}

#[test]
fn requests_validate_page_revision_generation_lane_and_range_before_backend_use() {
    for bad in [0, 257, u16::MAX] {
        let mut request = request();
        if let ResourceRequestV1::QueryAllocations { page, .. } = &mut request {
            page.max_items = bad;
        }
        assert!(request.validate(ProtocolLimitsV1::default()).is_err());
    }
    for (offset, length) in [(0, 0), (u64::MAX, 1)] {
        let mut request = access_request();
        if let ResourceRequestV1::QueryMemoryAccesses { filter, .. } = &mut request {
            filter.range = Some(ResourceMemoryRangeV1 {
                byte_offset: ResourceDecimalU64V1::new(offset),
                byte_len: ResourceDecimalU64V1::new(length),
            });
        }
        assert!(request.validate(ProtocolLimitsV1::default()).is_err());
    }
    for variant in 0..5 {
        let mut request = access_request();
        if let ResourceRequestV1::QueryMemoryAccesses {
            request_id,
            expected_revision,
            filter,
            page,
            ..
        } = &mut request
        {
            match variant {
                0 => *request_id = 0,
                1 => *expected_revision = 5,
                2 => {
                    filter.allocation = Some(AllocationIdentityV1 {
                        ordinal: 1,
                        generation: 1,
                    })
                }
                3 => {
                    filter.scope = ExecutionScopeSelectorV1::Lane {
                        workgroup: [0; 3],
                        wave: 0,
                        lane: 32,
                    }
                }
                _ => page.max_scanned = 257,
            }
        }
        assert!(request.validate(ProtocolLimitsV1::default()).is_err());
    }
}

#[test]
fn tokens_are_bounded_opaque_non_null_lookup_keys() {
    for bad in [
        String::new(),
        "a".repeat(129),
        "token with space".into(),
        "../escape/".into(),
        "a\n".into(),
        "雪".into(),
    ] {
        assert!(ResourcePageTokenV1::new(bad).is_err());
    }
    assert_eq!(
        ResourcePageTokenV1::new("opaque_1-2.A".into())
            .unwrap()
            .as_str(),
        "opaque_1-2.A"
    );
}

#[test]
fn exact_snapshot_and_session_match_rejects_stale_and_cross_scope_responses() {
    for variant in 0..5 {
        let mut response = response();
        if let ResourceResponseV1::Ok {
            request_id,
            session,
            snapshot,
            ..
        } = &mut response
        {
            match variant {
                0 => *request_id = 2,
                1 => snapshot.cursor.state_revision = 7,
                2 => session.configuration_identity = OpaqueIdentityV1::new([3; 32]).unwrap(),
                3 => snapshot.site = None,
                _ => {
                    if let ExecutionScopeV1::Lane {
                        lane, active_mask, ..
                    } = &mut snapshot.scope
                    {
                        *lane = 1;
                        *active_mask = 3;
                    }
                }
            }
        }
        assert!(
            response
                .validate_for_request(&request(), ProtocolLimitsV1::default())
                .is_err()
        );
    }
}

#[test]
fn impossible_page_counts_truth_and_duplicate_accesses_reject() {
    for variant in 0..7 {
        let mut response = access_response();
        if let ResourceResponseV1::Ok {
            session,
            page,
            result,
            ..
        } = &mut response
        {
            match variant {
                0 => page.scanned = 257,
                1 => page.scanned = 0,
                2 => page.source_count = 7,
                3 => session.hardware_observed = true,
                4 => page.next_token = Some(ResourcePageTokenV1::new("next".into()).unwrap()),
                5 => {
                    if let ResourceQueryResultV1::MemoryAccesses { accesses } = result {
                        accesses.push(accesses[0].clone());
                    }
                }
                _ => {
                    if let ResourceQueryResultV1::MemoryAccesses { accesses } = result {
                        accesses[0].occurrence.record_ordinal = u64::MAX;
                    }
                }
            }
        }
        assert!(
            response.validate(ProtocolLimitsV1::default()).is_err(),
            "variant{variant}"
        );
    }
}

#[test]
fn empty_filtered_pages_keep_continuation_but_cannot_reuse_same_token() {
    let mut request = access_request();
    let token = ResourcePageTokenV1::new("page.1".into()).unwrap();
    if let ResourceRequestV1::QueryMemoryAccesses { page, .. } = &mut request {
        page.token = Some(token.clone());
    }
    let mut response = access_response();
    if let ResourceResponseV1::Ok { page, result, .. } = &mut response {
        page.scanned = 1;
        page.next_token = Some(token);
        *result = ResourceQueryResultV1::MemoryAccesses { accesses: vec![] };
    }
    response.validate(ProtocolLimitsV1::default()).unwrap();
    assert!(
        response
            .validate_for_request(&request, ProtocolLimitsV1::default())
            .is_err()
    );
    if let ResourceResponseV1::Ok { page, .. } = &mut response {
        page.next_token = Some(ResourcePageTokenV1::new("page.2".into()).unwrap());
    }
    response
        .validate_for_request(&request, ProtocolLimitsV1::default())
        .unwrap();
}

#[test]
fn response_filter_binding_and_requested_scan_budget_are_checked() {
    for variant in 0..4 {
        let mut request = access_request();
        if let ResourceRequestV1::QueryMemoryAccesses { filter, page, .. } = &mut request {
            match variant {
                0 => filter.address_space = Some(AddressSpaceV1::Workgroup),
                1 => {
                    filter.allocation = Some(AllocationIdentityV1 {
                        ordinal: 2,
                        generation: 0,
                    })
                }
                2 => {
                    filter.scope = ExecutionScopeSelectorV1::Workgroup {
                        workgroup: [1, 0, 0],
                    }
                }
                _ => page.max_scanned = 1,
            }
        }
        assert!(
            access_response()
                .validate_for_request(&request, ProtocolLimitsV1::default())
                .is_err()
        );
    }
}

#[test]
fn unavailable_capture_requirement_and_operation_are_consistent() {
    let mut response = ResourceResponseV1::Unavailable {
        schema: ResourceResponseSchemaV1::V1,
        request_id: 1,
        operation: ResourceOperationV1::QueryAllocations,
        session: session(),
        reason: ResourceQueryUnavailableReasonV1::MemoryByteLimit,
        required: Some(ResourceDecimalU64V1::new(32)),
        completeness: CaptureCompletenessV1::Complete,
    };
    response
        .validate_for_request(&request(), ProtocolLimitsV1::default())
        .unwrap();
    if let ResourceResponseV1::Unavailable { required, .. } = &mut response {
        *required = None;
    }
    assert!(response.validate(ProtocolLimitsV1::default()).is_err());
    if let ResourceResponseV1::Unavailable {
        reason, operation, ..
    } = &mut response
    {
        *reason = ResourceQueryUnavailableReasonV1::NotCheckpoint;
        *operation = ResourceOperationV1::QueryMemoryAccesses;
    }
    assert!(response.validate(ProtocolLimitsV1::default()).is_err());
}

#[test]
fn framing_and_bounded_encoding_use_existing_protocol_limits() {
    let limits = ProtocolLimitsV1 {
        max_request_line_bytes: 1,
        ..ProtocolLimitsV1::default()
    };
    assert_eq!(
        decode_resource_request_line_v1(&line(&request()), limits),
        Err(ProtocolCodecErrorV1::LineTooLarge)
    );
    let limits = ProtocolLimitsV1 {
        max_response_line_bytes: 1,
        ..ProtocolLimitsV1::default()
    };
    assert_eq!(
        encode_resource_response_line_v1(&response(), limits),
        Err(ProtocolCodecErrorV1::ResponseTooLarge)
    );
    let mut missing = line(&request());
    missing.pop();
    assert_eq!(
        decode_resource_request_line_v1(&missing, ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::MissingLineTerminator)
    );
    let embedded = [line(&request()), line(&request())].concat();
    assert_eq!(
        decode_resource_request_line_v1(&embedded, ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::EmbeddedLineBreak)
    );
}
