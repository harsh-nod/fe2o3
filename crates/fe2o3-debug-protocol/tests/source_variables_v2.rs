use std::io::BufReader;

use fe2o3_debug_protocol::*;

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

#[test]
fn exact_identity_name_and_all_selectors_are_separately_versioned() {
    for selector in [
        r#"{"selector":"identity","variable_identity":"0101010101010101010101010101010101010101010101010101010101010101"}"#,
        r#"{"selector":"name","name":"item"}"#,
        r#"{"selector":"all"}"#,
    ] {
        let line = format!(
            "{{\"operation\":\"inspect_source_variables\",\"schema\":\"{SOURCE_VARIABLE_REQUEST_SCHEMA_V2}\",\"request_id\":7,\"expected_revision\":2,\"scope\":{{\"level\":\"lane\",\"workgroup\":[0,0,0],\"wave\":0,\"lane\":0}},\"frame\":1,\"selector\":{selector},\"page\":{{\"limit\":8}}}}\n"
        );
        let request =
            decode_source_variable_request_line_v2(line.as_bytes(), ProtocolLimitsV1::default())
                .unwrap();
        assert_eq!(request.request_id(), 7);
    }
}

#[test]
fn mixed_reader_preserves_exact_v1_and_v2_decoding() {
    let input = concat!(
        "{\"operation\":\"get_state\",\"schema\":\"fe2o3-debug-request-v1\",\"request_id\":1,\"expected_revision\":0}\n",
        "{\"operation\":\"inspect_source_variables\",\"schema\":\"fe2o3-debug-source-variable-request-v2\",\"request_id\":2,\"expected_revision\":0,\"scope\":{\"level\":\"dispatch\"},\"selector\":{\"selector\":\"all\"},\"page\":{\"limit\":1}}\n"
    );
    let mut reader = BufReader::new(input.as_bytes());
    assert!(matches!(
        read_request_line_any_v2(&mut reader, ProtocolLimitsV1::default()).unwrap(),
        Some(DebugRequestAnyV2::V1(DebugRequestV1::GetState { .. }))
    ));
    assert!(matches!(
        read_request_line_any_v2(&mut reader, ProtocolLimitsV1::default()).unwrap(),
        Some(DebugRequestAnyV2::SourceVariablesV2(_))
    ));
    assert_eq!(
        read_request_line_any_v2(&mut reader, ProtocolLimitsV1::default()).unwrap(),
        None
    );
}

#[test]
fn v1_rejects_v2_and_hostile_v2_is_rejected() {
    let valid = br#"{"operation":"inspect_source_variables","schema":"fe2o3-debug-source-variable-request-v2","request_id":2,"expected_revision":0,"scope":{"level":"dispatch"},"selector":{"selector":"all"},"page":{"limit":1}}
"#;
    assert_eq!(
        decode_request_line_v1(valid, ProtocolLimitsV1::default()).unwrap_err(),
        ProtocolCodecErrorV1::InvalidJson
    );
    for hostile in [
        br#"{"operation":"inspect_source_variables","schema":"fe2o3-debug-source-variable-request-v2","request_id":2,"request_id":3,"expected_revision":0,"scope":{"level":"dispatch"},"selector":{"selector":"all"},"page":{"limit":1}}
"#.as_slice(),
        br#"{"operation":"inspect_source_variables","schema":"fe2o3-debug-source-variable-request-v2","request_id":2,"expected_revision":0,"scope":{"level":"dispatch"},"selector":{"selector":"identity","variable_identity":"0000000000000000000000000000000000000000000000000000000000000000"},"page":{"limit":1}}
"#.as_slice(),
        br#"{"operation":"inspect_source_variables","schema":"fe2o3-debug-source-variable-request-v2","request_id":2,"expected_revision":0,"scope":{"level":"dispatch"},"selector":{"selector":"all"},"page":{"limit":0}}
"#.as_slice(),
    ] {
        assert!(decode_source_variable_request_line_v2(
            hostile,
            ProtocolLimitsV1::default()
        )
        .is_err());
    }
}

#[test]
fn response_supports_typed_ambiguity_without_v1_enum_changes() {
    let configuration = identity(2);
    let session = SessionViewV1 {
        backend: DebugBackendV1::CpuKirSimulator,
        execution_kind: ExecutionKindV1::CpuKirSimulation,
        state: SessionStateV1::Stopped,
        revision: 0,
        configuration_identity: configuration,
        cursor: DebugCursorV1 {
            configuration_identity: configuration,
            event_sequence: 0,
            state_revision: 0,
        },
        simulated: true,
        hardware_observed: false,
        performance_prediction: false,
    };
    let response = SourceVariableResponseV2::Ok {
        schema: SourceVariableResponseSchemaV2::V2,
        request_id: 1,
        operation: SourceVariableOperationV2::InspectSourceVariables,
        session,
        snapshot: Box::new(DebugSnapshotAnchorV1 {
            cursor: session.cursor,
            scope: ExecutionScopeV1::Dispatch,
            site: None,
            frame: None,
            occurrence: None,
        }),
        values: vec![SourceVariableValueV2 {
            variable_identity: identity(3),
            name: "item".into(),
            function_ordinal: 0,
            scope_identity: identity(4),
            scope_depth: 2,
            generation: 1,
            availability: SourceVariableValueAvailabilityV2::Ambiguous,
        }],
        next_cursor: None,
    };
    let encoded =
        encode_source_variable_response_line_v2(&response, ProtocolLimitsV1::default()).unwrap();
    assert!(
        std::str::from_utf8(&encoded)
            .unwrap()
            .contains("\"status\":\"ambiguous\"")
    );
    assert_eq!(
        decode_source_variable_response_line_v2(&encoded, ProtocolLimitsV1::default()).unwrap(),
        response
    );

    let mut hostile_response = response.clone();
    let SourceVariableResponseV2::Ok { next_cursor, .. } = &mut hostile_response else {
        unreachable!()
    };
    *next_cursor = Some(PageCursorV1 {
        query_identity: identity(5),
        position: 0,
    });
    let mut hostile = serde_json::to_vec(&hostile_response).unwrap();
    hostile.push(b'\n');
    assert!(
        decode_source_variable_response_line_v2(&hostile, ProtocolLimitsV1::default()).is_err()
    );

    let mut unrelated_snapshot = response.clone();
    let SourceVariableResponseV2::Ok { snapshot, .. } = &mut unrelated_snapshot else {
        unreachable!()
    };
    snapshot.cursor.event_sequence = 7;
    let mut hostile = serde_json::to_vec(&unrelated_snapshot).unwrap();
    hostile.push(b'\n');
    assert!(matches!(
        decode_source_variable_response_line_v2(&hostile, ProtocolLimitsV1::default()),
        Err(ProtocolCodecErrorV1::Validation(
            ProtocolValidationErrorV1::IdentityMismatch("source variable snapshot cursor")
        ))
    ));

    let mut zero_generation = response.clone();
    {
        let SourceVariableResponseV2::Ok { values, .. } = &mut zero_generation else {
            unreachable!()
        };
        values[0].generation = 0;
        values[0].availability = SourceVariableValueAvailabilityV2::Value {
            value: ValueAvailabilityV1::Captured {
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
    }
    let mut hostile = serde_json::to_vec(&zero_generation).unwrap();
    hostile.push(b'\n');
    assert!(
        decode_source_variable_response_line_v2(&hostile, ProtocolLimitsV1::default()).is_err()
    );

    let SourceVariableResponseV2::Ok { values, .. } = &mut zero_generation else {
        unreachable!()
    };
    values[0].availability = SourceVariableValueAvailabilityV2::Value {
        value: ValueAvailabilityV1::Unavailable {
            reason: ValueUnavailableReasonV1::OptimizedOut,
        },
    };
    let mut fallback = serde_json::to_vec(&zero_generation).unwrap();
    fallback.push(b'\n');
    assert!(
        decode_source_variable_response_line_v2(&fallback, ProtocolLimitsV1::default()).is_ok()
    );
}
