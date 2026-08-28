use std::io::Write;
use std::process::{Command, Stdio};

use fe2o3_semantic_import::*;
use fe2o3_semantic_trace::*;

const SOURCE: &[u8] = include_bytes!("fixtures/rocprofv3-1.1-counter-collection.json");

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}
fn binding() -> RocprofCounterCaptureBindingV2 {
    RocprofCaptureBindingV1 {
        kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(identity(1), 97).unwrap(),
        artifact: Some(ArtifactClaimV1 {
            identity: identity(2),
            canonical_len: 4096,
            format_version: 1,
        }),
        source_map: None,
        wave_width: WaveWidthV1::Wave64,
    }
}
fn imported_capture() -> SemanticCounterCaptureV2 {
    import_rocprofv3_counter_capture_v2(SOURCE, binding(), ImportLimitsV1::default()).unwrap()
}

#[test]
fn real_rocprofv3_1_1_shape_is_canonical_and_truth_preserving() {
    let capture = imported_capture();
    let bytes = encode_counter_capture_v2(&capture).unwrap();
    assert_eq!(decode_counter_capture_v2(&bytes).unwrap(), capture);
    assert_eq!(
        bytes,
        encode_counter_capture_v2(&imported_capture()).unwrap()
    );
    assert_eq!(capture.counter_definitions.len(), 2);
    assert_eq!(capture.dispatches.len(), 2);
    assert_eq!(capture.dispatches[0].values.len(), 3);
    assert_eq!(capture.dispatches[0].values[0].value(), 1.5);
    assert_eq!(
        capture.dispatches[0].values[0].origin,
        TruthOriginV1::Observed
    );
    assert_eq!(capture.dispatches[1].source_collection_ordinal, 1);
    assert_eq!(capture.dispatches[1].values[0].source_record_ordinal, 3);
    assert_eq!(capture.coverage.loss.origin, TruthOriginV1::Unavailable);
    assert_eq!(capture.coverage.loss.state, LossStateV1::Unknown);
    assert_eq!(
        capture.coverage.dimension_correlation,
        CounterDimensionCorrelationV2::UnavailableRecordHasNoInstanceIdentity
    );
    assert_eq!(
        capture.dispatches[0].source_and_isa_correlation,
        CounterCorrelationStatusV2::UnavailableNoAuthenticatedSourceOrIsaMap
    );
    assert_eq!(
        capture.dispatches[0].artifact.origin,
        TruthOriginV1::Declared
    );
    assert_eq!(
        capture.dispatches[0].source_map.origin,
        TruthOriginV1::Unavailable
    );
}

#[test]
fn malformed_catalogs_and_collections_are_rejected() {
    let missing = String::from_utf8(SOURCE.to_vec()).unwrap().replace(
        "\"handle\":101},\"value\":1.5",
        "\"handle\":999},\"value\":1.5",
    );
    assert!(matches!(
        import_rocprofv3_counter_capture_v2(
            missing.as_bytes(),
            binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::CounterDefinitionNotFound)
    ));
    let duplicate = String::from_utf8(SOURCE.to_vec()).unwrap().replace("\"callback_records\"", "\"counters\":[{\"agent_id\":{\"handle\":17},\"id\":{\"handle\":101},\"is_constant\":0,\"is_derived\":0,\"name\":\"duplicate\"}],\"callback_records\"");
    assert!(matches!(
        import_rocprofv3_counter_capture_v2(
            duplicate.as_bytes(),
            binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::InvalidRocprofJson)
    ));
    let duplicate_key = String::from_utf8(SOURCE.to_vec())
        .unwrap()
        .replace("\"handle\":102", "\"handle\":101");
    assert!(matches!(
        import_rocprofv3_counter_capture_v2(
            duplicate_key.as_bytes(),
            binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::InvalidCounterCatalog)
    ));
    let reversed = String::from_utf8(SOURCE.to_vec()).unwrap().replace(
        "\"start_timestamp\":100,\"end_timestamp\":140",
        "\"start_timestamp\":141,\"end_timestamp\":140",
    );
    assert!(matches!(
        import_rocprofv3_counter_capture_v2(
            reversed.as_bytes(),
            binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::InvalidCounterCollection)
    ));
}

#[test]
fn stale_identities_origins_ordinals_and_noncanonical_bytes_are_rejected() {
    let capture = imported_capture();
    let mut hostile = capture.clone();
    hostile.devices[0].identity = CaptureIdentityV1::new([0xdd; 32]).unwrap();
    for definition in &mut hostile.counter_definitions {
        if definition.device_identity == capture.devices[0].identity {
            definition.device_identity = hostile.devices[0].identity;
        }
    }
    for dispatch in &mut hostile.dispatches {
        if dispatch.device_identity == capture.devices[0].identity {
            dispatch.device_identity = hostile.devices[0].identity;
        }
    }
    assert!(matches!(
        decode_counter_capture_v2(&serde_json::to_vec(&hostile).unwrap()),
        Err(CounterCaptureErrorV2::InvalidDeviceCatalog)
    ));
    let mut hostile = capture.clone();
    hostile.counter_definitions[0].name.push_str("_forged");
    assert!(matches!(
        decode_counter_capture_v2(&serde_json::to_vec(&hostile).unwrap()),
        Err(CounterCaptureErrorV2::InvalidCounterCatalog)
    ));
    let mut hostile = capture.clone();
    hostile.dispatches[0].values[0].identity = CaptureIdentityV1::new([0xee; 32]).unwrap();
    assert!(matches!(
        decode_counter_capture_v2(&serde_json::to_vec(&hostile).unwrap()),
        Err(CounterCaptureErrorV2::InvalidCounterValue)
    ));
    let mut hostile = capture.clone();
    hostile.counter_definitions[0].source_definition_ordinal = 8;
    assert!(matches!(
        decode_counter_capture_v2(&serde_json::to_vec(&hostile).unwrap()),
        Err(CounterCaptureErrorV2::InvalidCounterCatalog)
    ));
    let mut hostile = capture.clone();
    hostile.counter_definitions[1].source_definition_ordinal = 3;
    assert!(matches!(
        decode_counter_capture_v2(&serde_json::to_vec(&hostile).unwrap()),
        Err(CounterCaptureErrorV2::InvalidCounterCatalog)
    ));
    let mut hostile = capture.clone();
    hostile.dispatches[0].artifact.origin = TruthOriginV1::Proved;
    assert!(matches!(
        decode_counter_capture_v2(&serde_json::to_vec(&hostile).unwrap()),
        Err(CounterCaptureErrorV2::InvalidContentClaim)
    ));
    let mut hostile = capture.clone();
    hostile.dispatches[1].collection_index = 7;
    assert!(matches!(
        decode_counter_capture_v2(&serde_json::to_vec(&hostile).unwrap()),
        Err(CounterCaptureErrorV2::NonCanonicalSourceSelector)
    ));
    let mut bytes = encode_counter_capture_v2(&capture).unwrap();
    bytes.push(b'\n');
    assert!(matches!(
        decode_counter_capture_v2(&bytes),
        Err(CounterCaptureErrorV2::NonCanonicalEncoding)
    ));
}

#[test]
fn counter_cli_reads_structured_stdin_and_emits_v2() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-trace-import"))
        .args([
            "rocprofv3-counter-capture",
            "--kir-sha256",
            &"01".repeat(32),
            "--kir-len",
            "97",
            "--wave-width",
            "64",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(SOURCE).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let capture = decode_counter_capture_v2(&output.stdout).unwrap();
    assert_eq!(capture.schema_version, 2);
}

#[test]
fn v1_capture_bytes_remain_v1_and_do_not_decode_as_v2() {
    let v1_source = br#"{"rocprofiler-sdk-tool":[{"buffer_records":{"kernel_dispatch":[{"start_timestamp":1,"end_timestamp":2,"dispatch_info":{"agent_id":{"handle":17},"workgroup_size":{"x":64,"y":1,"z":1},"grid_size":{"x":64,"y":1,"z":1}}}]}}]}"#;
    let v1 = import_rocprofv3_capture_v1(v1_source, binding(), ImportLimitsV1::default()).unwrap();
    let bytes = encode_capture_v1(&v1).unwrap();
    assert!(decode_capture_v1(&bytes).is_ok());
    assert!(decode_counter_capture_v2(&bytes).is_err());
}
