use std::io::BufReader;

use fe2o3_debug_protocol::*;

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn simulation_session(revision: u64, state: SessionStateV1) -> SessionViewV1 {
    SessionViewV1 {
        backend: DebugBackendV1::CpuKirSimulator,
        execution_kind: ExecutionKindV1::CpuKirSimulation,
        state,
        revision,
        configuration_identity: identity(1),
        cursor: DebugCursorV1 {
            configuration_identity: identity(1),
            event_sequence: 7,
            state_revision: revision,
        },
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
    }
}

fn acknowledged_response() -> DebugResponseV1 {
    DebugResponseV1::Ok {
        schema: ResponseSchemaV1::V1,
        request_id: 1,
        operation: DebugOperationNameV1::SetBreakpoints,
        session: simulation_session(2, SessionStateV1::Stopped),
        result: Box::new(DebugResultV1::Acknowledged { accepted: 1 }),
    }
}

fn encode_request(request: &DebugRequestV1) -> Vec<u8> {
    let mut encoded = serde_json::to_vec(request).unwrap();
    encoded.push(b'\n');
    encoded
}

#[test]
fn strict_minimal_request_decodes_and_exposes_revision() {
    let request = decode_request_line_v1(
        br#"{"operation":"discover_capabilities","schema":"fe2o3-debug-request-v1","request_id":9,"expected_revision":3}
"#,
        ProtocolLimitsV1::default(),
    )
    .unwrap();
    assert_eq!(request.request_id(), 9);
    assert_eq!(request.expected_revision(), 3);
    assert_eq!(
        request.operation(),
        DebugOperationNameV1::DiscoverCapabilities
    );
}

#[test]
fn framing_requires_one_lf_and_rejects_crlf_or_embedded_lines() {
    let limits = ProtocolLimitsV1::default();
    assert_eq!(
        decode_request_line_v1(b"{}", limits).unwrap_err(),
        ProtocolCodecErrorV1::MissingLineTerminator
    );
    assert_eq!(
        decode_request_line_v1(b"\n", limits).unwrap_err(),
        ProtocolCodecErrorV1::EmptyLine
    );
    assert_eq!(
        decode_request_line_v1(b"{}\r\n", limits).unwrap_err(),
        ProtocolCodecErrorV1::EmbeddedLineBreak
    );
    assert_eq!(
        decode_request_line_v1(b"{}\n{}\n", limits).unwrap_err(),
        ProtocolCodecErrorV1::EmbeddedLineBreak
    );
}

#[test]
fn request_rejects_unknown_duplicate_null_and_unknown_operation() {
    let limits = ProtocolLimitsV1::default();
    for input in [
        br#"{"operation":"get_state","schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0,"extra":1}
"#
            .as_slice(),
        br#"{"operation":"get_state","schema":"fe2o3-debug-request-v1","request_id":1,"request_id":2,"expected_revision":0}
"#
            .as_slice(),
        br#"{"operation":"step","schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0,"direction":"forward","granularity":"operation","count":1,"focus":null}
"#
            .as_slice(),
        br#"{"operation":"evaluate_expression","schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0}
"#
            .as_slice(),
    ] {
        assert_eq!(
            decode_request_line_v1(input, limits).unwrap_err(),
            ProtocolCodecErrorV1::InvalidJson
        );
    }
}

#[test]
fn zero_request_and_invalid_step_count_are_validation_errors() {
    let limits = ProtocolLimitsV1::default();
    let zero_id = br#"{"operation":"get_state","schema":"fe2o3-debug-request-v1","request_id":0,"expected_revision":0}
"#;
    assert!(matches!(
        decode_request_line_v1(zero_id, limits),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::ZeroRequestId
        ))
    ));
    let zero_step = br#"{"operation":"step","schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0,"direction":"forward","granularity":"operation","count":0}
"#;
    assert!(matches!(
        decode_request_line_v1(zero_step, limits),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::CountOutOfRange("step count")
        ))
    ));
}

#[test]
fn absent_optional_round_trips_but_explicit_null_does_not() {
    let request = DebugRequestV1::Step {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 0,
        direction: StepDirectionV1::Forward,
        granularity: StepGranularityV1::Operation,
        count: 1,
        focus: None,
    };
    let encoded = encode_request(&request);
    assert!(
        !encoded
            .windows(b"null".len())
            .any(|window| window == b"null")
    );
    assert_eq!(
        decode_request_line_v1(&encoded, ProtocolLimitsV1::default()).unwrap(),
        request
    );
}

#[test]
fn frame_aware_step_granularities_have_closed_wire_tags() {
    for (granularity, tag) in [
        (StepGranularityV1::Over, "over"),
        (StepGranularityV1::Out, "out"),
    ] {
        let request = DebugRequestV1::Step {
            schema: RequestSchemaV1::V1,
            request_id: 1,
            expected_revision: 0,
            direction: StepDirectionV1::Forward,
            granularity,
            count: 1,
            focus: None,
        };
        let encoded = encode_request(&request);
        assert!(
            std::str::from_utf8(&encoded)
                .unwrap()
                .contains(&format!("\"granularity\":\"{tag}\""))
        );
        assert_eq!(
            decode_request_line_v1(&encoded, ProtocolLimitsV1::default()).unwrap(),
            request
        );
    }
}

#[test]
fn source_resolution_and_stack_queries_are_closed_and_versioned() {
    let resolve = br#"{"operation":"resolve_source","schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":4,"site":{"function_ordinal":0,"block_ordinal":0,"point":{"kind":"operation","operation_ordinal":2}}}
"#;
    let request = decode_request_line_v1(resolve, ProtocolLimitsV1::default()).unwrap();
    assert_eq!(request.operation(), DebugOperationNameV1::ResolveSource);

    let stack = br#"{"operation":"inspect_stack","schema":"fe2o3-debug-request-v1","request_id":2,"expected_revision":4,"scope":{"level":"lane","workgroup":[0,0,0],"wave":0,"lane":0},"page":{"limit":8}}
"#;
    let request = decode_request_line_v1(stack, ProtocolLimitsV1::default()).unwrap();
    assert_eq!(request.operation(), DebugOperationNameV1::InspectStack);

    for hostile in [
        br#"{"operation":"inspect_stack","schema":"fe2o3-debug-request-v1","request_id":2,"expected_revision":4,"scope":{"level":"dispatch"},"page":{"limit":8},"extra":1}
"#.as_slice(),
        br#"{"operation":"inspect_stack","schema":"fe2o3-debug-request-v1","request_id":2,"request_id":3,"expected_revision":4,"scope":{"level":"dispatch"},"page":{"limit":8}}
"#.as_slice(),
        br#"{"operation":"inspect_stack","schema":"fe2o3-debug-request-v1","request_id":2,"expected_revision":4,"scope":{"level":"dispatch"},"page":null}
"#.as_slice(),
    ] {
        assert_eq!(
            decode_request_line_v1(hostile, ProtocolLimitsV1::default()).unwrap_err(),
            ProtocolCodecErrorV1::InvalidJson
        );
    }
}

#[test]
fn source_provenance_and_stack_availability_round_trip_without_addresses() {
    let response = DebugResponseV1::Ok {
        schema: ResponseSchemaV1::V1,
        request_id: 3,
        operation: DebugOperationNameV1::InspectStack,
        session: simulation_session(4, SessionStateV1::Stopped),
        result: Box::new(DebugResultV1::Stack {
            snapshot: DebugSnapshotAnchorV1 {
                cursor: simulation_session(4, SessionStateV1::Stopped).cursor,
                scope: ExecutionScopeV1::Dispatch,
                site: Some(SemanticSiteViewV1 {
                    kir: KirSiteV1 {
                        function_ordinal: 0,
                        block_ordinal: 0,
                        point: KirSitePointV1::Operation {
                            operation_ordinal: 2,
                        },
                    },
                    source: SourceSiteAvailabilityV1::Resolved {
                        location: SourceLocationV1 {
                            map_identity: identity(4),
                            provenance: SourceMapProvenanceV1::CompilerBundleAuthenticated,
                            file_identity: identity(5),
                            byte_start: 7,
                            byte_end: 12,
                        },
                    },
                }),
                frame: None,
                occurrence: None,
            },
            frames: vec![StackFrameV1 {
                frame: 1,
                function_ordinal: 0,
                block_ordinal: 0,
                next_operation: Some(3),
                values: StackValuesAvailabilityV1::Unavailable {
                    reason: ValueUnavailableReasonV1::Truncated,
                },
            }],
            next_cursor: None,
        }),
    };
    let encoded = encode_response_line_v1(&response, ProtocolLimitsV1::default()).unwrap();
    let text = std::str::from_utf8(&encoded).unwrap();
    assert!(text.contains("compiler_bundle_authenticated"));
    assert!(!text.contains("address"));
    assert_eq!(
        decode_response_line_v1(&encoded, ProtocolLimitsV1::default()).unwrap(),
        response
    );

    let mut duplicate = response;
    let DebugResponseV1::Ok { result, .. } = &mut duplicate else {
        unreachable!()
    };
    let DebugResultV1::Stack { frames, .. } = result.as_mut() else {
        unreachable!()
    };
    frames.push(frames[0]);
    assert!(matches!(
        encode_response_line_v1(&duplicate, ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::DuplicateIdentity("stack frames")
        ))
    ));
}

#[test]
fn reader_consumes_exactly_one_frame_and_detects_partial_eof() {
    let first = br#"{"operation":"get_state","schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0}
"#;
    let second = br#"{"operation":"pause","schema":"fe2o3-debug-request-v1","request_id":2,"expected_revision":0}
"#;
    let mut stream = Vec::new();
    stream.extend_from_slice(first);
    stream.extend_from_slice(second);
    let mut reader = BufReader::with_capacity(7, stream.as_slice());
    assert_eq!(
        read_request_line_v1(&mut reader, ProtocolLimitsV1::default())
            .unwrap()
            .unwrap()
            .request_id(),
        1
    );
    assert_eq!(
        read_request_line_v1(&mut reader, ProtocolLimitsV1::default())
            .unwrap()
            .unwrap()
            .request_id(),
        2
    );
    assert!(
        read_request_line_v1(&mut reader, ProtocolLimitsV1::default())
            .unwrap()
            .is_none()
    );

    let mut partial = BufReader::new(&first[..first.len() - 1]);
    assert_eq!(
        read_request_line_v1(&mut partial, ProtocolLimitsV1::default()).unwrap_err(),
        ProtocolCodecErrorV1::MissingLineTerminator
    );
}

#[test]
fn reader_and_direct_decoder_enforce_line_bound() {
    let limits = ProtocolLimitsV1::new(8, 128, 1, 1, 1).unwrap();
    assert_eq!(
        decode_request_line_v1(b"12345678\n", limits).unwrap_err(),
        ProtocolCodecErrorV1::LineTooLarge
    );
    let mut reader = BufReader::new(b"12345678\n".as_slice());
    assert_eq!(
        read_request_line_v1(&mut reader, limits).unwrap_err(),
        ProtocolCodecErrorV1::LineTooLarge
    );
}

#[test]
fn response_round_trip_is_compact_bounded_and_newline_terminated() {
    let response = acknowledged_response();
    let encoded = encode_response_line_v1(&response, ProtocolLimitsV1::default()).unwrap();
    assert_eq!(encoded.last(), Some(&b'\n'));
    assert_eq!(encoded.iter().filter(|byte| **byte == b'\n').count(), 1);
    assert_eq!(
        decode_response_line_v1(&encoded, ProtocolLimitsV1::default()).unwrap(),
        response
    );

    let tight = ProtocolLimitsV1::new(128, 32, 1, 1, 1).unwrap();
    assert_eq!(
        encode_response_line_v1(&response, tight).unwrap_err(),
        ProtocolCodecErrorV1::ResponseTooLarge
    );
}

#[test]
fn unavailable_is_distinct_and_can_never_change_state() {
    let response = DebugResponseV1::Unavailable {
        schema: ResponseSchemaV1::V1,
        request_id: 7,
        operation: DebugOperationNameV1::Step,
        session: simulation_session(0, SessionStateV1::Stopped),
        unavailable: CapabilityUnavailableV1 {
            capability: DebugCapabilityNameV1::HardwareWaveState,
            reason: CapabilityUnavailableReasonV1::LogicalVisualizationOnly,
            state_changed: false,
            detail: "simulator waves are logical visualization groups".into(),
        },
    };
    let encoded = encode_response_line_v1(&response, ProtocolLimitsV1::default()).unwrap();
    assert!(
        std::str::from_utf8(&encoded)
            .unwrap()
            .contains("\"status\":\"unavailable\"")
    );

    let invalid = DebugResponseV1::Unavailable {
        schema: ResponseSchemaV1::V1,
        request_id: 7,
        operation: DebugOperationNameV1::Step,
        session: simulation_session(0, SessionStateV1::Stopped),
        unavailable: CapabilityUnavailableV1 {
            capability: DebugCapabilityNameV1::HardwareWaveState,
            reason: CapabilityUnavailableReasonV1::LogicalVisualizationOnly,
            state_changed: true,
            detail: "invalid mutation".into(),
        },
    };
    assert!(matches!(
        encode_response_line_v1(&invalid, ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::UnavailableChangedState
        ))
    ));
}

#[test]
fn simulator_and_hardware_truth_classifications_cannot_be_conflated() {
    let mut invalid = acknowledged_response();
    let DebugResponseV1::Ok { session, .. } = &mut invalid else {
        unreachable!()
    };
    session.hardware_observed = true;
    assert!(matches!(
        encode_response_line_v1(&invalid, ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::InvalidTruthClassification
        ))
    ));
}

fn value_path() -> ValuePathV1 {
    ValuePathV1 {
        root: ValueRootV1::Argument { ordinal: 0 },
        components: Vec::new(),
    }
}

fn snapshot_anchor() -> DebugSnapshotAnchorV1 {
    DebugSnapshotAnchorV1 {
        cursor: DebugCursorV1 {
            configuration_identity: identity(1),
            event_sequence: 7,
            state_revision: 2,
        },
        scope: ExecutionScopeV1::Lane {
            workgroup: [0, 0, 0],
            wave: 0,
            lane: 0,
            logical_workitem: [0, 0, 0],
            active_mask: 1,
            wave_width: 64,
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
                reason: SourceSiteUnavailableReasonV1::RequiresAuthenticatedMap,
            },
        }),
        frame: Some(1),
        occurrence: Some(1),
    }
}

#[test]
fn pointers_are_only_allocation_relative_and_native_fields_are_rejected() {
    let response = DebugResponseV1::Ok {
        schema: ResponseSchemaV1::V1,
        request_id: 1,
        operation: DebugOperationNameV1::InspectValues,
        session: simulation_session(2, SessionStateV1::Stopped),
        result: Box::new(DebugResultV1::Values {
            snapshot: snapshot_anchor(),
            values: vec![DebugValueV1 {
                path: value_path(),
                availability: ValueAvailabilityV1::Captured {
                    value_type: DebugValueTypeV1::Pointer {
                        address_space: AddressSpaceV1::Global,
                    },
                    value: CapturedValueV1::AllocationRelativePointer {
                        allocation: AllocationIdentityV1 {
                            ordinal: 1,
                            generation: 0,
                        },
                        byte_offset: 12,
                    },
                    provenance: ValueProvenanceV1::SimulatedObservation,
                },
            }],
            next_cursor: None,
        }),
    };
    let encoded = encode_response_line_v1(&response, ProtocolLimitsV1::default()).unwrap();
    let text = std::str::from_utf8(&encoded).unwrap();
    assert!(text.contains("allocation_relative_pointer"));
    for forbidden in [
        "native_address",
        "gpu_address",
        "pointer_value",
        "handle",
        "fd",
    ] {
        assert!(!text.contains(forbidden));
    }

    let injected = text.replace(
        "\"byte_offset\":12",
        "\"byte_offset\":12,\"native_address\":4096",
    );
    assert_eq!(
        decode_response_line_v1(injected.as_bytes(), ProtocolLimitsV1::default()).unwrap_err(),
        ProtocolCodecErrorV1::InvalidJson
    );
}

#[test]
fn scalar_bits_are_exact_width_lowercase_and_type_matched() {
    let valid = DebugValueV1 {
        path: value_path(),
        availability: ValueAvailabilityV1::Captured {
            value_type: DebugValueTypeV1::Integer {
                signed: false,
                bits: 32,
            },
            value: CapturedValueV1::Bits {
                bits: "0x0000002a".into(),
            },
            provenance: ValueProvenanceV1::SimulatedObservation,
        },
    };
    let response = DebugResponseV1::Ok {
        schema: ResponseSchemaV1::V1,
        request_id: 1,
        operation: DebugOperationNameV1::InspectValues,
        session: simulation_session(2, SessionStateV1::Stopped),
        result: Box::new(DebugResultV1::Values {
            snapshot: snapshot_anchor(),
            values: vec![valid],
            next_cursor: None,
        }),
    };
    encode_response_line_v1(&response, ProtocolLimitsV1::default()).unwrap();

    let mut wrong_width = response.clone();
    let DebugResponseV1::Ok { result, .. } = &mut wrong_width else {
        unreachable!()
    };
    let DebugResultV1::Values { values, .. } = result.as_mut() else {
        unreachable!()
    };
    let ValueAvailabilityV1::Captured {
        value: CapturedValueV1::Bits { bits },
        ..
    } = &mut values[0].availability
    else {
        unreachable!()
    };
    *bits = "0x2a".into();
    assert!(matches!(
        encode_response_line_v1(&wrong_width, ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::InvalidBitVector
        ))
    ));
}

#[test]
fn memory_requires_exact_bytes_and_canonical_initialization_tail() {
    let response = DebugResponseV1::Ok {
        schema: ResponseSchemaV1::V1,
        request_id: 1,
        operation: DebugOperationNameV1::ReadMemory,
        session: simulation_session(2, SessionStateV1::Stopped),
        result: Box::new(DebugResultV1::Memory {
            snapshot: snapshot_anchor(),
            memory: MemoryReadV1 {
                allocation: AllocationIdentityV1 {
                    ordinal: 1,
                    generation: 0,
                },
                byte_offset: 12,
                requested_bytes: 1,
                returned_bytes: 1,
                availability: MemoryAvailabilityV1::Captured {
                    address_space: AddressSpaceV1::Global,
                    bytes: "0xff".into(),
                    initialized: "0x01".into(),
                    truncated: false,
                },
            },
        }),
    };
    encode_response_line_v1(&response, ProtocolLimitsV1::default()).unwrap();

    let mut invalid = response;
    let DebugResponseV1::Ok { result, .. } = &mut invalid else {
        unreachable!()
    };
    let DebugResultV1::Memory {
        memory:
            MemoryReadV1 {
                availability: MemoryAvailabilityV1::Captured { initialized, .. },
                ..
            },
        ..
    } = result.as_mut()
    else {
        unreachable!()
    };
    *initialized = "0x03".into();
    assert!(matches!(
        encode_response_line_v1(&invalid, ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::InvalidInitializationBits
        ))
    ));
}

fn nested_not(depth: usize) -> PredicateV1 {
    let leaf = PredicateV1::Compare {
        left: PredicateOperandV1::Bool { value: true },
        comparison: IntegerComparisonV1::Equal,
        right: PredicateOperandV1::Bool { value: true },
    };
    (0..depth).fold(leaf, |predicate, _| PredicateV1::Not {
        predicate_value: Box::new(predicate),
    })
}

#[test]
fn predicate_ast_is_closed_and_depth_bounded() {
    let request = DebugRequestV1::SetBreakpoints {
        schema: RequestSchemaV1::V1,
        request_id: 1,
        expected_revision: 0,
        breakpoints: vec![BreakpointSpecV1 {
            client_label: None,
            enabled: true,
            scope: None,
            hit_condition: None,
            kind: BreakpointKindV1::Value {
                predicate: nested_not(MAX_PREDICATE_DEPTH_V1),
            },
        }],
    };
    assert!(matches!(
        decode_request_line_v1(&encode_request(&request), ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::PredicateDepthExceeded
        ))
    ));

    let arbitrary_eval = br#"{"operation":"set_breakpoints","schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0,"breakpoints":[{"enabled":true,"kind":{"kind":"value","predicate":{"predicate":"eval","expression":"*ptr == 7"}}}]}
"#;
    assert_eq!(
        decode_request_line_v1(arbitrary_eval, ProtocolLimitsV1::default()).unwrap_err(),
        ProtocolCodecErrorV1::InvalidJson
    );
}

#[test]
fn watchpoint_ranges_are_checked_and_null_scope_is_rejected() {
    let overflow = br#"{"operation":"set_watchpoints","schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0,"watchpoints":[{"enabled":true,"allocation":{"ordinal":1,"generation":0},"byte_offset":18446744073709551615,"byte_len":2,"access":"write","timing":"before_commit"}]}
"#;
    assert!(matches!(
        decode_request_line_v1(overflow, ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::RangeOverflow("watchpoint")
        ))
    ));

    let null_scope = br#"{"operation":"set_watchpoints","schema":"fe2o3-debug-request-v1","request_id":1,"expected_revision":0,"watchpoints":[{"enabled":true,"scope":null,"allocation":{"ordinal":1,"generation":0},"byte_offset":0,"byte_len":1,"access":"write","timing":"before_commit"}]}
"#;
    assert_eq!(
        decode_request_line_v1(null_scope, ProtocolLimitsV1::default()).unwrap_err(),
        ProtocolCodecErrorV1::InvalidJson
    );
}

#[test]
fn opaque_identities_require_nonzero_lowercase_exact_hex() {
    for identity in [
        "0000000000000000000000000000000000000000000000000000000000000000",
        "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
        "01",
    ] {
        let line = format!(
            "{{\"operation\":\"seek\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":1,\"expected_revision\":0,\"cursor\":{{\"configuration_identity\":\"{identity}\",\"event_sequence\":0,\"state_revision\":0}}}}\n"
        );
        assert_eq!(
            decode_request_line_v1(line.as_bytes(), ProtocolLimitsV1::default()).unwrap_err(),
            ProtocolCodecErrorV1::InvalidJson
        );
    }
}

#[test]
fn response_error_omits_absent_correlation_and_rejects_explicit_null() {
    let response = DebugResponseV1::Error {
        schema: ResponseSchemaV1::V1,
        request_id: None,
        operation: None,
        session: None,
        error: DebugErrorV1 {
            stage: DebugErrorStageV1::Framing,
            code: DebugErrorCodeV1::InvalidJson,
            message: "request identity was not recoverable".into(),
            state_changed: false,
        },
    };
    let encoded = encode_response_line_v1(&response, ProtocolLimitsV1::default()).unwrap();
    let text = std::str::from_utf8(&encoded).unwrap();
    assert!(!text.contains("request_id"));
    assert!(!text.contains("null"));

    let with_null = text.replace("\"schema\":", "\"request_id\":null,\"schema\":");
    assert_eq!(
        decode_response_line_v1(with_null.as_bytes(), ProtocolLimitsV1::default()).unwrap_err(),
        ProtocolCodecErrorV1::InvalidJson
    );
}
