use crate::*;
use std::io::Cursor;

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
        owner: RuntimeObservationOwnerV1 {
            backend_session: RuntimeDecimalU64V1::new(5),
            capture_instance: RuntimeDecimalU64V1::new(7),
        },
        cursor: session().cursor,
    }
}
fn request() -> DeclaredTargetRequestV1 {
    DeclaredTargetRequestV1 {
        schema: DeclaredTargetRequestSchemaV1::V1,
        operation: DeclaredTargetOperationV1::InspectDeclaredTarget,
        request_id: 1,
        expected_revision: 4,
        expected_binding: binding(),
    }
}
fn response() -> DeclaredTargetResponseV1 {
    DeclaredTargetResponseV1::Ok {
        schema: DeclaredTargetResponseSchemaV1::V1,
        operation: DeclaredTargetOperationV1::InspectDeclaredTarget,
        request_id: 1,
        session: session(),
        binding: binding(),
        logical_wave_width: 32,
        target: DeclaredTargetObservationV1::Declared {
            target: DeclaredGpuTargetV1::Gfx942XnackOff,
            provenance: DeclaredTargetProvenanceV1::VerifiedSimulationBundle,
            envelope_version: 1,
            envelope_identity: OpaqueIdentityV1::new([11; 32]).unwrap(),
            subject_identity: OpaqueIdentityV1::new([12; 32]).unwrap(),
            admitted_module: DeclaredTargetModuleV1 {
                wire_version: 7,
                sha256: OpaqueIdentityV1::new([13; 32]).unwrap(),
                canonical_bytes: RuntimeDecimalU64V1::new(245),
            },
        },
    }
}
fn line(value: &impl serde::Serialize) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(value).unwrap();
    bytes.push(b'\n');
    bytes
}
#[test]
fn exact_target_families_round_trip_and_every_supported_envelope_has_exact_kir_version() {
    let limits = ProtocolLimitsV1::default();
    let request = request();
    assert_eq!(
        decode_declared_target_request_line_v1(&line(&request), limits).unwrap(),
        request
    );
    for target in [
        DeclaredGpuTargetV1::Gfx942XnackOff,
        DeclaredGpuTargetV1::Gfx950XnackOff,
    ] {
        for version in 1..=6 {
            let mut response = response();
            if let DeclaredTargetResponseV1::Ok {
                target:
                    DeclaredTargetObservationV1::Declared {
                        target: gpu,
                        envelope_version,
                        admitted_module,
                        ..
                    },
                ..
            } = &mut response
            {
                *gpu = target;
                *envelope_version = version;
                admitted_module.wire_version = match version {
                    1..=4 => 7,
                    5 => 10,
                    _ => 11,
                };
            }
            response.validate_for_request(&request, limits).unwrap();
            let bytes = encode_declared_target_response_line_v1(&response, limits).unwrap();
            assert!(bytes.len() <= MAX_DECLARED_TARGET_LINE_BYTES_V1);
            assert_eq!(
                decode_declared_target_response_line_v1(&bytes, limits).unwrap(),
                response
            );
        }
    }
}
#[test]
fn raw_input_unavailable_remains_exact_owner_bound() {
    let mut response = response();
    if let DeclaredTargetResponseV1::Ok { target, .. } = &mut response {
        *target = DeclaredTargetObservationV1::Unavailable {
            reason: DeclaredTargetUnavailableV1::RawInputHasNoDeclaredGpuTarget,
        };
    }
    response
        .validate_for_request(&request(), ProtocolLimitsV1::default())
        .unwrap();
}
#[test]
fn owner_capture_configuration_record_and_revision_must_all_match() {
    let limits = ProtocolLimitsV1::default();
    for field in 0..5 {
        let mut expected = request();
        match field {
            0 => expected.expected_binding.owner.backend_session = RuntimeDecimalU64V1::new(6),
            1 => expected.expected_binding.owner.capture_instance = RuntimeDecimalU64V1::new(8),
            2 => {
                expected.expected_binding.cursor.configuration_identity =
                    OpaqueIdentityV1::new([14; 32]).unwrap()
            }
            3 => expected.expected_binding.cursor.event_sequence += 1,
            4 => {
                expected.expected_binding.cursor.state_revision += 1;
                expected.expected_revision += 1;
            }
            _ => unreachable!(),
        }
        assert!(
            response().validate_for_request(&expected, limits).is_err(),
            "field {field}"
        );
    }
}
#[test]
fn declared_target_unknown_duplicate_null_and_override_fields_refuse() {
    let limits = ProtocolLimitsV1::default();
    let good = String::from_utf8(line(&request())).unwrap();
    for bad in [
        good.replacen("{", "{\"request_id\":1,", 1),
        good.replace(
            "\"backend_session\":\"5\"",
            "\"backend_session\":\"5\",\"backend_session\":\"5\"",
        ),
        good.replace(
            "\"expected_binding\":{",
            "\"expected_binding\":null,\"extra_binding\":{",
        ),
        good.replacen("{", "{\"target\":\"gfx950:xnack-\",", 1),
        good.replace("inspect_declared_target", "set_declared_target"),
        good.replace("\"backend_session\":\"5\"", "\"backend_session\":\"0\""),
    ] {
        assert_ne!(bad, good);
        assert!(decode_declared_target_request_line_v1(bad.as_bytes(), limits).is_err());
    }
    let value = serde_json::to_value(response()).unwrap();
    for (path, replacement) in [
        (vec!["target", "target"], serde_json::json!("gfx942")),
        (
            vec!["target", "provenance"],
            serde_json::json!("hardware_observed"),
        ),
        (vec!["target", "envelope_version"], serde_json::json!(7)),
        (
            vec!["target", "admitted_module", "canonical_bytes"],
            serde_json::json!("0"),
        ),
        (
            vec!["target", "admitted_module", "wire_version"],
            serde_json::json!(11),
        ),
        (vec!["logical_wave_width"], serde_json::json!(48)),
        (
            vec!["session", "hardware_observed"],
            serde_json::json!(true),
        ),
    ] {
        let mut changed = value.clone();
        let mut cursor = &mut changed;
        for key in path {
            cursor = &mut cursor[key];
        }
        *cursor = replacement;
        assert!(decode_declared_target_response_line_v1(&line(&changed), limits).is_err());
    }
}
#[test]
fn target_family_caps_include_newline_and_do_not_raise_caller_caps() {
    let limits = ProtocolLimitsV1::default();
    let mut request = line(&request());
    request.pop();
    request.resize(MAX_DECLARED_TARGET_LINE_BYTES_V1 - 1, b' ');
    request.push(b'\n');
    decode_declared_target_request_line_v1(&request, limits).unwrap();
    request.insert(request.len() - 1, b' ');
    assert_eq!(
        decode_declared_target_request_line_v1(&request, limits).unwrap_err(),
        ProtocolCodecErrorV1::LineTooLarge
    );
    assert_eq!(
        read_request_line_any_v4(&mut Cursor::new(request), limits).unwrap_err(),
        ProtocolCodecErrorV1::LineTooLarge
    );
    let small = ProtocolLimitsV1 {
        max_response_line_bytes: 64,
        ..limits
    };
    assert_eq!(
        encode_declared_target_response_line_v1(&response(), small).unwrap_err(),
        ProtocolCodecErrorV1::ResponseTooLarge
    );
    let mut overlarge = line(&response());
    overlarge.pop();
    overlarge.resize(MAX_DECLARED_TARGET_LINE_BYTES_V1, b' ');
    overlarge.push(b'\n');
    assert_eq!(
        decode_declared_target_response_line_v1(&overlarge, limits).unwrap_err(),
        ProtocolCodecErrorV1::LineTooLarge
    );
}
#[test]
fn framing_rejects_bom_invalid_utf8_missing_newline_and_embedded_lines() {
    let limits = ProtocolLimitsV1::default();
    let good = line(&request());
    let mut bom = vec![0xef, 0xbb, 0xbf];
    bom.extend_from_slice(&good);
    let mut utf8 = good.clone();
    utf8[2] = 0xff;
    let mut embedded = good.clone();
    embedded.insert(1, b'\n');
    for bad in [bom, utf8, embedded, good[..good.len() - 1].to_vec()] {
        assert!(decode_declared_target_request_line_v1(&bad, limits).is_err());
    }
}
#[test]
fn old_v3_enum_and_old_stream_results_are_unchanged() {
    let limits = ProtocolLimitsV1::default();
    let legacy = DebugRequestV1::GetState {
        schema: RequestSchemaV1::V1,
        request_id: 3,
        expected_revision: 0,
    };
    let bytes = line(&legacy);
    let old = read_request_line_any_v3(&mut Cursor::new(&bytes), limits)
        .unwrap()
        .unwrap();
    assert_eq!(
        read_request_line_any_v4(&mut Cursor::new(&bytes), limits).unwrap(),
        Some(DebugRequestAnyV4::Legacy(old))
    );
    let bytes = line(&request());
    assert_eq!(
        read_request_line_any_v4(&mut Cursor::new(&bytes), limits).unwrap(),
        Some(DebugRequestAnyV4::DeclaredTargetV1(request()))
    );
    assert!(read_request_line_any_v3(&mut Cursor::new(&bytes), limits).is_err());
    for bytes in [
        b"{}\n".as_slice(),
        b"\n",
        b"{\"schema\":\"unknown\"}\n",
        b"{}",
    ] {
        assert_eq!(
            read_request_line_any_v4(&mut Cursor::new(bytes), limits).unwrap_err(),
            read_request_line_any_v3(&mut Cursor::new(bytes), limits).unwrap_err()
        );
    }
}
#[test]
fn errors_preserve_no_state_changed_contract() {
    let mut response = DeclaredTargetResponseV1::Error {
        schema: DeclaredTargetResponseSchemaV1::V1,
        operation: DeclaredTargetOperationV1::InspectDeclaredTarget,
        request_id: 1,
        session: session(),
        error: DebugErrorV1 {
            stage: DebugErrorStageV1::Session,
            code: DebugErrorCodeV1::InvalidCursor,
            message: "stale target binding".to_owned(),
            state_changed: false,
        },
    };
    response
        .validate_for_request(&request(), ProtocolLimitsV1::default())
        .unwrap();
    if let DeclaredTargetResponseV1::Error { error, .. } = &mut response {
        error.state_changed = true;
    }
    assert!(response.validate(ProtocolLimitsV1::default()).is_err());
}
