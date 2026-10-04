use crate::*;
use std::io::Cursor;

fn n(value: u64) -> ResourceDecimalU64V1 {
    ResourceDecimalU64V1::new(value)
}
fn owner() -> RuntimeObservationOwnerV1 {
    RuntimeObservationOwnerV1 {
        backend_session: n(5),
        capture_instance: n(7),
    }
}
fn session() -> SessionViewV1 {
    let identity = OpaqueIdentityV1::new([3; 32]).unwrap();
    SessionViewV1 {
        backend: DebugBackendV1::CpuKirSimulator,
        execution_kind: ExecutionKindV1::CpuKirSimulation,
        state: SessionStateV1::Stopped,
        revision: 4,
        configuration_identity: identity,
        cursor: DebugCursorV1 {
            configuration_identity: identity,
            event_sequence: 9,
            state_revision: 4,
        },
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
    }
}
fn binding() -> RuntimeObservationBindingV1 {
    RuntimeObservationBindingV1 {
        owner: owner(),
        cursor: session().cursor,
    }
}
fn invocation() -> RuntimeInvocationV1 {
    RuntimeInvocationV1 {
        global: [n(0); 3],
        workgroup: [n(0); 3],
        local: [0; 3],
        workgroup_size: [1; 3],
        workgroup_count: [n(1); 3],
        launch_extent: [n(1); 3],
    }
}
fn site() -> RuntimeOperationSiteV1 {
    RuntimeOperationSiteV1 {
        function_ordinal: n(0),
        block: 3,
        operation: 1,
    }
}
fn runtime_request() -> RuntimeObservationRequestV1 {
    RuntimeObservationRequestV1::InspectCurrentRecord {
        schema: RuntimeObservationRequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 4,
        expected_cursor: session().cursor,
        expected_owner: Some(owner()),
    }
}
fn runtime_response() -> RuntimeObservationResponseV1 {
    RuntimeObservationResponseV1::Ok {
        schema: RuntimeObservationResponseSchemaV1::V1,
        request_id: 1,
        session: session(),
        binding: binding(),
        invocation: Box::new(invocation()),
        origin: RuntimeOperationObservationV1::Available {
            identity: RuntimeOperationIdentityV1 {
                activation: n(17),
                attempt: n(22),
                site: site(),
            },
        },
        frames: RuntimeFramesV1::Captured {
            frames: vec![RuntimeFrameV1 {
                legacy_depth: 0,
                function_ordinal: n(0),
                block: 3,
                next_operation: Some(2),
                activation: n(17),
                operation: RuntimeFrameOperationV1::ActiveOperation {
                    attempt: n(22),
                    site: site(),
                },
                parent: RuntimeFrameParentV1::Root,
            }],
        },
        allocation_watermark: RuntimeAllocationWatermarkV1::Available {
            through_sequence: n(3),
        },
        completeness: CaptureCompletenessV1::Complete,
        origin_coverage: RuntimeMetadataCoverageV1::Complete,
        frame_coverage: RuntimeMetadataCoverageV1::Complete,
        lifecycle_coverage: RuntimeMetadataCoverageV1::Complete,
    }
}
fn identity() -> ResourceStorageIdentityV2 {
    ResourceStorageIdentityV2 {
        allocation: n(11),
        storage_slot: n(3),
        generation: n(2),
    }
}
fn descriptor() -> ResourceAllocationDescriptorV2 {
    ResourceAllocationDescriptorV2 {
        identity: identity(),
        address_space: AddressSpaceV1::Private,
        access: ResourceAllocationAccessV1::ReadWrite,
        alignment: 4,
        byte_len: n(4),
        owning_scope: ResourceAllocationScopeV2::Invocation {
            invocation: invocation(),
        },
        creation_site: Some(ResourceCreationSiteV2 {
            function_ordinal: n(0),
            block: 3,
            operation: Some(0),
        }),
    }
}
fn page() -> ResourceQueryPageV1 {
    ResourceQueryPageV1 {
        max_items: 8,
        max_scanned: 8,
        token: None,
    }
}
fn read_request() -> ResourceRequestV2 {
    ResourceRequestV2::ReadAllocationMemory {
        schema: ResourceRequestSchemaV2::V2,
        request_id: 1,
        expected_revision: 4,
        expected_binding: binding(),
        allocation: identity(),
        range: ResourceMemoryRangeV1 {
            byte_offset: n(0),
            byte_len: n(4),
        },
    }
}
fn read_response() -> ResourceResponseV2 {
    ResourceResponseV2::Ok {
        schema: ResourceResponseSchemaV2::V2,
        request_id: 1,
        operation: ResourceOperationV2::ReadAllocationMemory,
        session: session(),
        binding: binding(),
        through_sequence: n(3),
        completeness: CaptureCompletenessV1::Complete,
        page: None,
        result: ResourceQueryResultV2::AllocationMemory {
            memory: ResourceMemoryReadV2 {
                allocation: identity(),
                range: ResourceMemoryRangeV1 {
                    byte_offset: n(0),
                    byte_len: n(4),
                },
                address_space: AddressSpaceV1::Private,
                bytes: "0x01020304".into(),
                initialized: "0x0f".into(),
            },
        },
    }
}
fn line<T: serde::Serialize>(value: &T) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap();
    bytes.push(b'\n');
    bytes
}

#[test]
fn observed_runtime_roundtrip_keeps_completed_site_and_next_operation_distinct() {
    let request = runtime_request();
    let response = runtime_response();
    let limits = ProtocolLimitsV1::default();
    response.validate_for_request(&request, limits).unwrap();
    let encoded = encode_runtime_observation_response_line_v1(&response, limits).unwrap();
    assert_eq!(
        decode_runtime_observation_response_line_v1(&encoded, limits).unwrap(),
        response
    );
    assert_eq!(
        decode_runtime_observation_request_line_v1(&line(&request), limits).unwrap(),
        request
    );
}
#[test]
fn observed_reader_wraps_legacy_and_never_consumes_next_frame() {
    let limits = ProtocolLimitsV1::default();
    let legacy = DebugRequestV1::GetState {
        schema: RequestSchemaV1::V1,
        request_id: 2,
        expected_revision: 0,
    };
    let mut bytes = line(&legacy);
    bytes.extend(line(&runtime_request()));
    bytes.extend(line(&read_request()));
    let mut reader = Cursor::new(bytes);
    assert_eq!(
        read_request_line_any_v3(&mut reader, limits).unwrap(),
        Some(DebugRequestAnyV3::Legacy(DebugRequestAnyV2::V1(legacy)))
    );
    assert_eq!(
        read_request_line_any_v3(&mut reader, limits).unwrap(),
        Some(DebugRequestAnyV3::RuntimeObservationV1(runtime_request()))
    );
    assert_eq!(
        read_request_line_any_v3(&mut reader, limits).unwrap(),
        Some(DebugRequestAnyV3::ResourceV2(read_request()))
    );
    assert!(
        read_request_line_any_v3(&mut reader, limits)
            .unwrap()
            .is_none()
    );
}
#[test]
fn observed_request_rejects_unknown_fields_null_and_bad_decimal() {
    let limits = ProtocolLimitsV1::default();
    let base = serde_json::to_value(runtime_request()).unwrap();
    for (key, value) in [
        ("extra", serde_json::json!(1)),
        ("expected_owner", serde_json::Value::Null),
    ] {
        let mut candidate = base.clone();
        candidate[key] = value;
        assert!(decode_runtime_observation_request_line_v1(&line(&candidate), limits).is_err());
    }
    for bad in [
        serde_json::json!(7),
        serde_json::json!("07"),
        serde_json::json!("+7"),
        serde_json::json!("18446744073709551616"),
        serde_json::json!(""),
    ] {
        let mut candidate = base.clone();
        candidate["expected_owner"]["capture_instance"] = bad;
        assert!(decode_runtime_observation_request_line_v1(&line(&candidate), limits).is_err());
    }
}
#[test]
fn observed_reader_rejects_duplicate_schema_unknown_schema_and_partial_lines() {
    let limits = ProtocolLimitsV1::default();
    let bytes = line(&runtime_request());
    for candidate in [
        Vec::new(),
        b"\n".to_vec(),
        b"{}\r\n".to_vec(),
        b"{}\n\n".to_vec(),
        bytes[..bytes.len() - 1].to_vec(),
    ] {
        assert!(decode_runtime_observation_request_line_v1(&candidate, limits).is_err());
    }
    let mut value = serde_json::to_value(runtime_request()).unwrap();
    value["schema"] = serde_json::json!("fe2o3-debug-runtime-observation-request-v99");
    assert!(read_request_line_any_v3(&mut Cursor::new(line(&value)), limits).is_err());
    let text = String::from_utf8(bytes).unwrap();
    let duplicate = text.replacen(
        "{",
        "{\"schema\":\"fe2o3-debug-runtime-observation-request-v1\",",
        1,
    );
    assert!(read_request_line_any_v3(&mut Cursor::new(duplicate), limits).is_err());
}
#[test]
fn observed_limits_apply_before_body_retention_and_during_response_encoding() {
    let request = line(&runtime_request());
    let mut limits = ProtocolLimitsV1 {
        max_request_line_bytes: request.len() - 1,
        ..ProtocolLimitsV1::default()
    };
    assert!(matches!(
        read_request_line_any_v3(&mut Cursor::new(request), limits),
        Err(ProtocolCodecErrorV1::LineTooLarge)
    ));
    limits.max_response_line_bytes = 1;
    assert!(matches!(
        encode_runtime_observation_response_line_v1(&runtime_response(), limits),
        Err(ProtocolCodecErrorV1::ResponseTooLarge)
    ));
}
#[test]
fn observed_bootstrap_can_omit_owner_but_known_owner_cannot_cross_rebind() {
    let limits = ProtocolLimitsV1::default();
    let mut request = runtime_request();
    let RuntimeObservationRequestV1::InspectCurrentRecord { expected_owner, .. } = &mut request;
    *expected_owner = None;
    runtime_response()
        .validate_for_request(&request, limits)
        .unwrap();
    let RuntimeObservationRequestV1::InspectCurrentRecord { expected_owner, .. } = &mut request;
    *expected_owner = Some(RuntimeObservationOwnerV1 {
        backend_session: n(6),
        ..owner()
    });
    assert!(
        runtime_response()
            .validate_for_request(&request, limits)
            .is_err()
    );
    let RuntimeObservationRequestV1::InspectCurrentRecord { expected_owner, .. } = &mut request;
    *expected_owner = Some(RuntimeObservationOwnerV1 {
        capture_instance: n(8),
        ..owner()
    });
    assert!(
        runtime_response()
            .validate_for_request(&request, limits)
            .is_err()
    );
}
#[test]
fn observed_full_cursor_comparison_rejects_same_revision_other_record_or_configuration() {
    let limits = ProtocolLimitsV1::default();
    for index in 0..3 {
        let mut request = runtime_request();
        let RuntimeObservationRequestV1::InspectCurrentRecord {
            expected_cursor, ..
        } = &mut request;
        match index {
            0 => expected_cursor.event_sequence += 1,
            1 => expected_cursor.configuration_identity = OpaqueIdentityV1::new([4; 32]).unwrap(),
            _ => expected_cursor.state_revision += 1,
        }
        assert!(
            runtime_response()
                .validate_for_request(&request, limits)
                .is_err()
        );
    }
}
#[test]
fn observed_invocation_validates_all_axes_and_checked_coordinate_arithmetic() {
    invocation().validate().unwrap();
    for axis in 0..3 {
        for component in 0..6 {
            let mut row = invocation();
            match component {
                0 => row.global[axis] = n(1),
                1 => row.workgroup[axis] = n(1),
                2 => row.local[axis] = 1,
                3 => row.workgroup_size[axis] = 0,
                4 => row.workgroup_count[axis] = n(2),
                _ => row.launch_extent[axis] = n(0),
            }
            assert!(
                row.validate().is_err(),
                "axis {axis}, component {component}"
            );
        }
    }
}
fn two_frames() -> RuntimeFramesV1 {
    RuntimeFramesV1::Captured {
        frames: vec![
            RuntimeFrameV1 {
                legacy_depth: 0,
                function_ordinal: n(0),
                block: 3,
                next_operation: Some(1),
                activation: n(17),
                operation: RuntimeFrameOperationV1::Suspended {
                    attempt: n(22),
                    site: site(),
                },
                parent: RuntimeFrameParentV1::Root,
            },
            RuntimeFrameV1 {
                legacy_depth: 1,
                function_ordinal: n(1),
                block: 8,
                next_operation: Some(1),
                activation: n(19),
                operation: RuntimeFrameOperationV1::ActiveOperation {
                    attempt: n(1),
                    site: RuntimeOperationSiteV1 {
                        function_ordinal: n(1),
                        block: 8,
                        operation: 1,
                    },
                },
                parent: RuntimeFrameParentV1::Caller {
                    activation: n(17),
                    attempt: n(22),
                    call_site: site(),
                },
            },
        ],
    }
}
#[test]
fn observed_frame_roster_checks_real_parent_and_unique_activations() {
    let limits = ProtocolLimitsV1::default();
    two_frames().validate(limits).unwrap();
    for control in 0..6 {
        let RuntimeFramesV1::Captured { mut frames } = two_frames() else {
            unreachable!()
        };
        match control {
            0 => frames[1].activation = frames[0].activation,
            1 => frames[1].parent = RuntimeFrameParentV1::Root,
            2 => frames[1].legacy_depth = 0,
            3 => frames[0].operation = RuntimeFrameOperationV1::Ready,
            4 => {
                frames[1].parent = RuntimeFrameParentV1::Caller {
                    activation: n(18),
                    attempt: n(22),
                    call_site: site(),
                }
            }
            _ => {
                frames[1].parent = RuntimeFrameParentV1::Caller {
                    activation: n(17),
                    attempt: n(23),
                    call_site: site(),
                }
            }
        }
        assert!(
            RuntimeFramesV1::Captured { frames }
                .validate(limits)
                .is_err()
        );
    }
}
#[test]
fn observed_origin_cannot_attach_to_unrelated_frame_at_same_depth() {
    let mut response = runtime_response();
    let RuntimeObservationResponseV1::Ok { origin, .. } = &mut response else {
        unreachable!()
    };
    *origin = RuntimeOperationObservationV1::Available {
        identity: RuntimeOperationIdentityV1 {
            activation: n(18),
            attempt: n(22),
            site: site(),
        },
    };
    assert!(response.validate(ProtocolLimitsV1::default()).is_err());
}
#[test]
fn observed_resource_read_roundtrips_exact_triple_and_mask() {
    let limits = ProtocolLimitsV1::default();
    read_response()
        .validate_for_request(&read_request(), limits)
        .unwrap();
    assert_eq!(
        decode_resource_request_line_v2(&line(&read_request()), limits).unwrap(),
        read_request()
    );
    let wire = encode_resource_response_line_v2(&read_response(), limits).unwrap();
    assert_eq!(
        decode_resource_response_line_v2(&wire, limits).unwrap(),
        read_response()
    );
}
#[test]
fn observed_old_generation_or_slot_never_matches_replacement_bytes() {
    let limits = ProtocolLimitsV1::default();
    for control in 0..3 {
        let mut request = read_request();
        let ResourceRequestV2::ReadAllocationMemory { allocation, .. } = &mut request else {
            unreachable!()
        };
        match control {
            0 => allocation.allocation = n(10),
            1 => allocation.storage_slot = n(4),
            _ => allocation.generation = n(1),
        }
        assert!(
            read_response()
                .validate_for_request(&request, limits)
                .is_err()
        );
    }
}
#[test]
fn observed_resource_requires_nonzero_triple_and_owner() {
    let limits = ProtocolLimitsV1::default();
    for control in 0..5 {
        let mut request = read_request();
        let ResourceRequestV2::ReadAllocationMemory {
            allocation,
            expected_binding,
            ..
        } = &mut request
        else {
            unreachable!()
        };
        match control {
            0 => allocation.allocation = n(0),
            1 => allocation.storage_slot = n(0),
            2 => allocation.generation = n(0),
            3 => expected_binding.owner.backend_session = n(0),
            _ => expected_binding.owner.capture_instance = n(0),
        }
        assert!(request.validate(limits).is_err());
    }
}
#[test]
fn observed_read_range_is_bounded_nonempty_checked_and_exact() {
    let limits = ProtocolLimitsV1::default();
    for (offset, len) in [
        (0, 0),
        (0, MAX_RESOURCE_MEMORY_READ_BYTES_V2 + 1),
        (u64::MAX, 1),
    ] {
        let mut request = read_request();
        let ResourceRequestV2::ReadAllocationMemory { range, .. } = &mut request else {
            unreachable!()
        };
        *range = ResourceMemoryRangeV1 {
            byte_offset: n(offset),
            byte_len: n(len),
        };
        assert!(request.validate(limits).is_err());
    }
    let mut request = read_request();
    let ResourceRequestV2::ReadAllocationMemory { range, .. } = &mut request else {
        unreachable!()
    };
    range.byte_len = n(3);
    assert!(
        read_response()
            .validate_for_request(&request, limits)
            .is_err()
    );
}
#[test]
fn observed_memory_rejects_padding_uppercase_partial_data_and_extra_page() {
    let limits = ProtocolLimitsV1::default();
    for control in 0..4 {
        let mut response = read_response();
        let ResourceResponseV2::Ok {
            result: ResourceQueryResultV2::AllocationMemory { memory },
            page,
            ..
        } = &mut response
        else {
            unreachable!()
        };
        match control {
            0 => memory.initialized = "0xff".into(),
            1 => memory.bytes = "0x0102030F".into(),
            2 => memory.bytes = "0x010203".into(),
            _ => {
                *page = Some(ResourcePageInfoV2 {
                    source_count: n(0),
                    source_start: n(0),
                    scanned: 0,
                    next_token: None,
                })
            }
        }
        assert!(response.validate(limits).is_err());
    }
}
fn lifecycle_response() -> ResourceResponseV2 {
    let mut first = descriptor();
    first.identity = ResourceStorageIdentityV2 {
        allocation: n(10),
        storage_slot: n(3),
        generation: n(1),
    };
    ResourceResponseV2::Ok {
        schema: ResourceResponseSchemaV2::V2,
        request_id: 1,
        operation: ResourceOperationV2::QueryAllocationLifecycle,
        session: session(),
        binding: binding(),
        through_sequence: n(3),
        completeness: CaptureCompletenessV1::Complete,
        page: Some(ResourcePageInfoV2 {
            source_count: n(3),
            source_start: n(0),
            scanned: 3,
            next_token: None,
        }),
        result: ResourceQueryResultV2::AllocationLifecycle {
            transitions: vec![
                ResourceAllocationTransitionV2 {
                    sequence: n(1),
                    descriptor: first,
                    kind: ResourceAllocationTransitionKindV2::Create {
                        previous_allocation: None,
                    },
                },
                ResourceAllocationTransitionV2 {
                    sequence: n(2),
                    descriptor: first,
                    kind: ResourceAllocationTransitionKindV2::Release,
                },
                ResourceAllocationTransitionV2 {
                    sequence: n(3),
                    descriptor: descriptor(),
                    kind: ResourceAllocationTransitionKindV2::Create {
                        previous_allocation: Some(n(10)),
                    },
                },
            ],
        },
    }
}
#[test]
fn observed_literal_lifecycle_carries_real_release_before_recreate() {
    let request = ResourceRequestV2::QueryAllocationLifecycle {
        schema: ResourceRequestSchemaV2::V2,
        request_id: 1,
        expected_revision: 4,
        expected_binding: binding(),
        page: page(),
    };
    lifecycle_response()
        .validate_for_request(&request, ProtocolLimitsV1::default())
        .unwrap();
}
#[test]
fn observed_lifecycle_refuses_future_rows_missing_rows_and_false_generation() {
    let limits = ProtocolLimitsV1::default();
    for control in 0..5 {
        let mut response = lifecycle_response();
        let ResourceResponseV2::Ok {
            result: ResourceQueryResultV2::AllocationLifecycle { transitions },
            through_sequence,
            ..
        } = &mut response
        else {
            unreachable!()
        };
        match control {
            0 => transitions[1].sequence = n(4),
            1 => {
                transitions.remove(1);
            }
            2 => transitions[2].descriptor.identity.generation = n(1),
            3 => *through_sequence = n(2),
            _ => {
                transitions[2].kind = ResourceAllocationTransitionKindV2::Create {
                    previous_allocation: None,
                }
            }
        };
        assert!(response.validate(limits).is_err());
    }
}
#[test]
fn observed_resource_queries_keep_legacy_generation_zero_unchanged() {
    let snapshot = DebugSnapshotAnchorV1 {
        cursor: session().cursor,
        scope: ExecutionScopeV1::Lane {
            workgroup: [0; 3],
            wave: 0,
            lane: 0,
            logical_workitem: [0; 3],
            active_mask: 1,
            wave_width: 32,
            interpretation: WaveInterpretationV1::LogicalVisualization,
        },
        site: None,
        frame: None,
        occurrence: None,
    };
    let mut request = ResourceRequestV1::QueryMemoryAccesses {
        schema: ResourceRequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 4,
        expected_snapshot: Box::new(snapshot),
        filter: ResourceMemoryAccessFilterV1 {
            scope: ExecutionScopeSelectorV1::Dispatch,
            allocation: Some(AllocationIdentityV1 {
                ordinal: 11,
                generation: 0,
            }),
            address_space: None,
            access: None,
            range: None,
        },
        page: page(),
    };
    request.validate(ProtocolLimitsV1::default()).unwrap();
    let ResourceRequestV1::QueryMemoryAccesses { filter, .. } = &mut request else {
        unreachable!()
    };
    filter.allocation.as_mut().unwrap().generation = 2;
    assert!(request.validate(ProtocolLimitsV1::default()).is_err());
}

#[test]
fn observed_origin_must_be_current_active_frame_not_suspended_ancestor() {
    let mut response = runtime_response();
    let RuntimeObservationResponseV1::Ok { frames, .. } = &mut response else {
        unreachable!()
    };
    *frames = two_frames();
    assert!(response.validate(ProtocolLimitsV1::default()).is_err());
}
#[test]
fn observed_child_activation_must_follow_its_parent() {
    let RuntimeFramesV1::Captured { mut frames } = two_frames() else {
        unreachable!()
    };
    frames[1].activation = n(16);
    assert!(
        RuntimeFramesV1::Captured { frames }
            .validate(ProtocolLimitsV1::default())
            .is_err()
    );
}
#[test]
fn observed_available_metadata_cannot_claim_disabled_or_invalid_coverage() {
    let limits = ProtocolLimitsV1::default();
    for coverage in [
        RuntimeMetadataCoverageV1::Disabled,
        RuntimeMetadataCoverageV1::InvalidJoin,
        RuntimeMetadataCoverageV1::PrefixTruncated {
            retained_records: n(8),
            reason: RuntimeMetadataCutoffV1::RowLimit,
        },
    ] {
        for component in 0..3 {
            let mut response = runtime_response();
            let RuntimeObservationResponseV1::Ok {
                origin_coverage,
                frame_coverage,
                lifecycle_coverage,
                ..
            } = &mut response
            else {
                unreachable!()
            };
            match component {
                0 => *origin_coverage = coverage,
                1 => *frame_coverage = coverage,
                _ => *lifecycle_coverage = coverage,
            }
            assert!(response.validate(limits).is_err());
        }
    }
    let mut response = runtime_response();
    let RuntimeObservationResponseV1::Ok {
        origin_coverage,
        frame_coverage,
        lifecycle_coverage,
        ..
    } = &mut response
    else {
        unreachable!()
    };
    *origin_coverage = RuntimeMetadataCoverageV1::PrefixTruncated {
        retained_records: n(9),
        reason: RuntimeMetadataCutoffV1::RowLimit,
    };
    *frame_coverage = *origin_coverage;
    *lifecycle_coverage = *origin_coverage;
    response.validate(limits).unwrap();
}
#[test]
fn observed_create_requires_real_site_and_nondispatch_owner() {
    let mut row = ResourceAllocationTransitionV2 {
        sequence: n(3),
        descriptor: descriptor(),
        kind: ResourceAllocationTransitionKindV2::Create {
            previous_allocation: Some(n(10)),
        },
    };
    row.validate().unwrap();
    row.descriptor.creation_site = None;
    assert!(row.validate().is_err());
    row.descriptor = descriptor();
    row.descriptor.owning_scope = ResourceAllocationScopeV2::Dispatch;
    row.descriptor.address_space = AddressSpaceV1::Global;
    assert!(row.validate().is_err());
}
#[test]
fn observed_legacy_reader_error_parity_for_bounds_terminators_and_cr() {
    let limits = ProtocolLimitsV1::default();
    let valid = line(&DebugRequestV1::GetState {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 0,
    });
    for bytes in [
        valid[..valid.len() - 1].to_vec(),
        b"{}\r\n".to_vec(),
        b"\n".to_vec(),
    ] {
        let old = read_request_line_any_v2(&mut Cursor::new(&bytes), limits);
        let new = read_request_line_any_v3(&mut Cursor::new(&bytes), limits);
        assert_eq!(
            format!("{:?}", old.unwrap_err()),
            format!("{:?}", new.unwrap_err())
        );
    }
    let mut limits = limits;
    limits.max_request_line_bytes = valid.len() - 1;
    assert_eq!(
        format!(
            "{:?}",
            read_request_line_any_v2(&mut Cursor::new(&valid), limits).unwrap_err()
        ),
        format!(
            "{:?}",
            read_request_line_any_v3(&mut Cursor::new(&valid), limits).unwrap_err()
        )
    );
}
#[test]
fn observed_legacy_completeness_never_claims_a_cursor_beyond_retained_prefix() {
    let bad = CaptureCompletenessV1::Truncated {
        reason: CaptureTruncationReasonV1::EventLimit,
        emitted_events: 8,
        dropped_events: None,
    };
    let mut runtime = runtime_response();
    let RuntimeObservationResponseV1::Ok { completeness, .. } = &mut runtime else {
        unreachable!()
    };
    *completeness = bad;
    assert!(runtime.validate(ProtocolLimitsV1::default()).is_err());
    let mut resource = read_response();
    let ResourceResponseV2::Ok { completeness, .. } = &mut resource else {
        unreachable!()
    };
    *completeness = bad;
    assert!(resource.validate(ProtocolLimitsV1::default()).is_err());
}

#[test]
fn terminal_unavailable_has_no_selected_row_to_compare_with_truncated_prefix() {
    let completeness = CaptureCompletenessV1::Truncated {
        reason: CaptureTruncationReasonV1::EventLimit,
        emitted_events: 8,
        dropped_events: None,
    };
    let request = runtime_request();
    let response = RuntimeObservationResponseV1::Unavailable {
        schema: RuntimeObservationResponseSchemaV1::V1,
        request_id: 1,
        session: session(),
        binding: Some(binding()),
        completeness,
        reason: RuntimeObservationUnavailableV1::NoSelectedRecord,
    };
    response
        .validate_for_request(&request, ProtocolLimitsV1::default())
        .unwrap();
    let resource = ResourceResponseV2::Unavailable {
        schema: ResourceResponseSchemaV2::V2,
        request_id: 1,
        operation: ResourceOperationV2::ReadAllocationMemory,
        session: session(),
        binding: binding(),
        completeness,
        reason: RuntimeObservationUnavailableV1::NoSelectedRecord,
    };
    resource
        .validate_for_request(&read_request(), ProtocolLimitsV1::default())
        .unwrap();
    let mut invalid = runtime_response();
    if let RuntimeObservationResponseV1::Ok {
        completeness: actual,
        ..
    } = &mut invalid
    {
        *actual = completeness;
    }
    assert!(invalid.validate(ProtocolLimitsV1::default()).is_err());
    let mut invalid = read_response();
    if let ResourceResponseV2::Ok {
        completeness: actual,
        ..
    } = &mut invalid
    {
        *actual = completeness;
    }
    assert!(invalid.validate(ProtocolLimitsV1::default()).is_err());
}
