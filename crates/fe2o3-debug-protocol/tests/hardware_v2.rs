use fe2o3_debug_protocol::*;

fn request(line: &str) -> Result<HardwareDebugRequestV2, HardwareProtocolCodecErrorV2> {
    decode_hardware_request_line_v2(line.as_bytes(), HardwareProtocolLimitsV2::default())
}

#[test]
fn strict_hardware_request_v2_round_trip() {
    let decoded = request(
        r#"{"schema":"fe2o3-hardware-debug-request-v2","operation":"inspect_hardware_queues","request_id":7,"expected_control_revision":3,"page":{"expected_generation":4,"start":0,"limit":16}}
"#,
    )
    .unwrap();
    assert_eq!(decoded.request_id(), 7);
    assert_eq!(decoded.expected_control_revision(), 3);
    assert_eq!(
        decoded.operation(),
        HardwareDebugOperationV2::InspectHardwareQueues
    );
}

#[test]
fn hostile_json_and_unbounded_requests_are_rejected() {
    for line in [
        r#"{"schema":"fe2o3-hardware-debug-request-v2","operation":"get_state","request_id":1,"expected_control_revision":0,"pid":12}
"#,
        r#"{"schema":"fe2o3-hardware-debug-request-v2","operation":"get_state","request_id":1,"request_id":2,"expected_control_revision":0}
"#,
        r#"{"schema":null,"operation":"get_state","request_id":1,"expected_control_revision":0}
"#,
        r#"{"schema":"fe2o3-hardware-debug-request-v2","operation":"suspend_queues","request_id":1,"expected_control_revision":0,"queues":[],"grace_period":0}
"#,
        r#"{"schema":"fe2o3-hardware-debug-request-v2","operation":"inspect_hardware_devices","request_id":1,"expected_control_revision":0,"page":{"expected_generation":0,"start":0,"limit":257}}
"#,
    ] {
        assert!(request(line).is_err(), "hostile request admitted: {line}");
    }

    let duplicate = format!(
        "{{\"schema\":\"{}\",\"operation\":\"resume_queues\",\"request_id\":1,\"expected_control_revision\":0,\"queues\":[{{\"generation\":1,\"ordinal\":1}},{{\"generation\":1,\"ordinal\":1}}]}}\n",
        HARDWARE_REQUEST_SCHEMA_V2
    );
    assert!(matches!(
        request(&duplicate),
        Err(HardwareProtocolCodecErrorV2::Validation(
            HardwareProtocolValidationErrorV2::DuplicateLogicalId("queue")
        ))
    ));
}

#[test]
fn framing_is_bounded_and_requires_one_json_line() {
    let limits = HardwareProtocolLimitsV2 {
        max_request_line_bytes: 64,
        ..HardwareProtocolLimitsV2::default()
    };
    assert!(matches!(
        decode_hardware_request_line_v2(&[b'x'; 65], limits),
        Err(HardwareProtocolCodecErrorV2::LineTooLarge)
    ));
    assert!(matches!(
        decode_hardware_request_line_v2(b"{}", limits),
        Err(HardwareProtocolCodecErrorV2::MissingLineTerminator)
    ));
    assert!(matches!(
        decode_hardware_request_line_v2(b"{}\r\n", limits),
        Err(HardwareProtocolCodecErrorV2::EmbeddedLineBreak)
    ));
}

#[test]
fn response_contains_only_logical_hardware_identity() {
    let response = HardwareDebugResponseV2::Ok {
        schema: HardwareResponseSchemaV2::V2,
        request_id: 1,
        operation: HardwareDebugOperationV2::InspectHardwareQueues,
        session: HardwareSessionViewV2 {
            state: HardwareSessionStateV2::Running,
            commands_processed: 1,
            control_revision: 0,
            observation_sequence: 0,
            identity_generation: 9,
            runtime_enabled: true,
            hardware_observed: true,
            simulated: false,
            performance_prediction: false,
        },
        result: HardwareDebugResultV2::Queues {
            generation: 9,
            items: vec![HardwareQueueViewV2 {
                id: HardwareQueueIdV2 {
                    generation: 9,
                    ordinal: 1,
                },
                device: HardwareDeviceIdV2 {
                    generation: 9,
                    ordinal: 1,
                },
                ring_bytes: 4096,
                queue_type: 0,
                context_save_area_bytes: 0,
                suspended_by_session: false,
            }],
            next_start: 0,
        },
    };
    let line =
        encode_hardware_response_line_v2(&response, HardwareProtocolLimitsV2::default()).unwrap();
    let text = std::str::from_utf8(&line).unwrap();
    for forbidden in [
        "pid", "fd", "native", "address", "argv", "queue_id", "gpu_id",
    ] {
        assert!(!text.contains(forbidden), "leaked field name: {forbidden}");
    }
    assert_eq!(
        decode_hardware_response_line_v2(&line, HardwareProtocolLimitsV2::default()).unwrap(),
        response
    );
}

#[test]
fn public_hardware_schema_has_no_native_authority_fields() {
    let source = include_str!("../src/hardware_v2.rs");
    for forbidden in [
        "pub pid",
        "pub fd",
        "pub gpu_id",
        "pub queue_id",
        "pub address",
        "pub argv",
        "pub pointer",
    ] {
        assert!(
            !source.contains(forbidden),
            "hardware V2 exposed forbidden field fragment: {forbidden}"
        );
    }
}
