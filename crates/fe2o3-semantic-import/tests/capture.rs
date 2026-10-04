use fe2o3_semantic_import::*;
use fe2o3_semantic_trace::*;
use std::io::Write;
use std::process::{Command, Stdio};

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn binding() -> RocprofCaptureBindingV1 {
    RocprofCaptureBindingV1 {
        kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(identity(1), 97).unwrap(),
        artifact: Some(ArtifactClaimV1 {
            identity: identity(2),
            canonical_len: 4_096,
            format_version: 1,
        }),
        source_map: Some(
            ContentIdentityV1::new(
                ContentIdentitySchemeV1::RawCanonicalSha256,
                1,
                identity(3),
                777,
            )
            .unwrap(),
        ),
        wave_width: WaveWidthV1::Wave64,
    }
}

fn record(agent: u64, start: u64, end: u64) -> String {
    format!(
        r#"{{"start_timestamp":{start},"end_timestamp":{end},"dispatch_info":{{"agent_id":{{"handle":{agent}}},"queue_id":{{"handle":99}},"kernel_id":7,"dispatch_id":8,"workgroup_size":{{"x":64,"y":1,"z":1}},"grid_size":{{"x":256,"y":1,"z":1}}}}}}"#,
    )
}

fn source() -> Vec<u8> {
    format!(
        r#"{{"rocprofiler-sdk-tool":[{{"buffer_records":{{"kernel_dispatch":[{},{}]}}}},{{"buffer_records":{{"kernel_dispatch":[{}]}}}}]}}"#,
        record(17, 100, 300),
        record(18, 400, 450),
        record(17, 500, 900),
    )
    .into_bytes()
}

#[test]
fn multi_dispatch_capture_is_canonical_content_addressed_and_truthful() {
    let first =
        import_rocprofv3_capture_v1(&source(), binding(), ImportLimitsV1::default()).unwrap();
    let second =
        import_rocprofv3_capture_v1(&source(), binding(), ImportLimitsV1::default()).unwrap();
    let first_bytes = encode_capture_v1(&first).unwrap();
    let second_bytes = encode_capture_v1(&second).unwrap();

    assert_eq!(first_bytes, second_bytes);
    assert_eq!(decode_capture_v1(&first_bytes).unwrap(), first);
    assert_eq!(
        capture_content_identity_v1(&first_bytes).unwrap(),
        capture_content_identity_v1(&second_bytes).unwrap()
    );
    assert_eq!(first.runs.len(), 1);
    assert_eq!(first.devices.len(), 2);
    assert_eq!(first.dispatches.len(), 3);
    assert_eq!(first.dispatches[0].process_index, 0);
    assert_eq!(first.dispatches[1].dispatch_index, 1);
    assert_eq!(first.dispatches[2].source_record_ordinal, 2);
    assert_eq!(first.dispatches[2].duration_ticks, 400);
    assert_eq!(first.dispatches[0].timing_origin, TruthOriginV1::Observed);
    assert_eq!(first.dispatches[0].artifact.origin, TruthOriginV1::Declared);
    assert_eq!(
        first.dispatches[0].source_map.origin,
        TruthOriginV1::Declared
    );
    assert_eq!(first.coverage.source_dispatch_records, 3);
    assert_eq!(first.coverage.captured_dispatch_records, 3);
    assert_eq!(first.coverage.sampling.mode, SamplingModeV1::NotSampled);
    assert_eq!(first.coverage.loss.state, LossStateV1::Unknown);
    assert_eq!(first.coverage.loss.origin, TruthOriginV1::Unavailable);
    assert_eq!(
        first.coverage.scope,
        CompletenessScopeV1::PartialSemanticExecutionHistory
    );
}

#[test]
fn capture_rejects_noncanonical_malformed_and_stale_documents() {
    let capture =
        import_rocprofv3_capture_v1(&source(), binding(), ImportLimitsV1::default()).unwrap();
    let canonical = encode_capture_v1(&capture).unwrap();
    let mut whitespace = canonical.clone();
    whitespace.push(b'\n');
    assert!(matches!(
        decode_capture_v1(&whitespace),
        Err(CaptureErrorV1::NonCanonicalEncoding)
    ));

    let mut stale = capture.clone();
    stale.dispatches[0].identity = CaptureIdentityV1::new([0xee; 32]).unwrap();
    let stale_bytes = serde_json::to_vec(&stale).unwrap();
    assert!(matches!(
        decode_capture_v1(&stale_bytes),
        Err(CaptureErrorV1::StaleDispatchIdentity)
    ));

    let mut unsupported = capture.clone();
    unsupported.schema_version = 2;
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&unsupported).unwrap()),
        Err(CaptureErrorV1::UnsupportedVersion(2))
    ));

    let mut false_loss_claim = capture.clone();
    false_loss_claim.coverage.loss = LossStatusV1 {
        origin: TruthOriginV1::Observed,
        state: LossStateV1::NoneReported,
        lost_records: None,
        unavailable_reason: None,
    };
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&false_loss_claim).unwrap()),
        Err(CaptureErrorV1::InvalidCoverage)
    ));

    let with_unknown = String::from_utf8(canonical)
        .unwrap()
        .replacen("{", "{\"unknown\":1,", 1);
    assert!(matches!(
        decode_capture_v1(with_unknown.as_bytes()),
        Err(CaptureErrorV1::JsonDecode)
    ));
}

#[test]
fn capture_rejects_inexact_and_overflowing_launch_geometry() {
    let capture =
        import_rocprofv3_capture_v1(&source(), binding(), ImportLimitsV1::default()).unwrap();

    let mut inconsistent = capture.clone();
    inconsistent.dispatches[0].launch.logical_grid[0] = 129;
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&inconsistent).unwrap()),
        Err(CaptureErrorV1::InvalidObservedEnvelope)
    ));

    let mut overflowing = capture;
    overflowing.dispatches[0].launch.logical_grid = [u64::from(u32::MAX), u64::from(u32::MAX), 2];
    overflowing.dispatches[0].launch.grid_workgroups = [u32::MAX, u32::MAX, 2];
    overflowing.dispatches[0].launch.workgroup_size = [1, 1, 1];
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&overflowing).unwrap()),
        Err(CaptureErrorV1::InvalidObservedEnvelope)
    ));

    let mut workgroup_product_overflow =
        import_rocprofv3_capture_v1(&source(), binding(), ImportLimitsV1::default()).unwrap();
    workgroup_product_overflow.dispatches[0].launch.logical_grid = [1, 1, 1];
    workgroup_product_overflow.dispatches[0]
        .launch
        .grid_workgroups = [1, 1, 1];
    workgroup_product_overflow.dispatches[0]
        .launch
        .workgroup_size = [u32::MAX, u32::MAX, 2];
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&workgroup_product_overflow).unwrap()),
        Err(CaptureErrorV1::InvalidObservedEnvelope)
    ));
}

#[test]
fn capture_rejects_provenance_upgrades_and_noncanonical_source_selectors() {
    let capture =
        import_rocprofv3_capture_v1(&source(), binding(), ImportLimitsV1::default()).unwrap();
    let mut rewritten_device = capture.clone();
    rewritten_device.devices[0].identity = CaptureIdentityV1::new([0xdd; 32]).unwrap();
    for dispatch in &mut rewritten_device.dispatches {
        if dispatch.device_identity == capture.devices[0].identity {
            dispatch.device_identity = rewritten_device.devices[0].identity;
        }
    }
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&rewritten_device).unwrap()),
        Err(CaptureErrorV1::InvalidDeviceIdentity)
    ));
    for origin in [
        TruthOriginV1::Proved,
        TruthOriginV1::Observed,
        TruthOriginV1::Inferred,
    ] {
        let mut hostile = capture.clone();
        hostile.dispatches[0].artifact.origin = origin;
        assert!(matches!(
            decode_capture_v1(&serde_json::to_vec(&hostile).unwrap()),
            Err(CaptureErrorV1::InvalidAvailableFact)
        ));
    }
    let mut unavailable_with_value = capture.clone();
    unavailable_with_value.dispatches[0].artifact.origin = TruthOriginV1::Unavailable;
    unavailable_with_value.dispatches[0]
        .artifact
        .unavailable_reason = Some(CaptureUnavailableReasonV1::NotProvided);
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&unavailable_with_value).unwrap()),
        Err(CaptureErrorV1::InvalidUnavailableFact)
    ));
    let mut declared_without_value = capture.clone();
    declared_without_value.dispatches[0].artifact.value = None;
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&declared_without_value).unwrap()),
        Err(CaptureErrorV1::InvalidAvailableFact)
    ));

    let mut skipped_ordinal = capture.clone();
    skipped_ordinal.dispatches[1].source_record_ordinal = 9;
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&skipped_ordinal).unwrap()),
        Err(CaptureErrorV1::StaleDispatchIdentity) | Err(CaptureErrorV1::NonCanonicalDispatchOrder)
    ));
    let mut reordered = capture.clone();
    reordered.dispatches.swap(0, 1);
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&reordered).unwrap()),
        Err(CaptureErrorV1::NonCanonicalDispatchOrder)
    ));
    let mut rewritten_selector = capture.clone();
    rewritten_selector.dispatches[1].dispatch_index = 7;
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&rewritten_selector).unwrap()),
        Err(CaptureErrorV1::NonCanonicalSourceSelector)
    ));
    let mut regressed_process = capture.clone();
    regressed_process.dispatches[2].process_index = 0;
    regressed_process.dispatches[2].dispatch_index = 0;
    assert!(matches!(
        decode_capture_v1(&serde_json::to_vec(&regressed_process).unwrap()),
        Err(CaptureErrorV1::NonCanonicalSourceSelector)
    ));
}

#[test]
fn capture_enforces_source_and_container_bounds_before_use() {
    let source_without_agent = br#"{"rocprofiler-sdk-tool":[{"buffer_records":{"kernel_dispatch":[{"start_timestamp":1,"end_timestamp":2,"dispatch_info":{"workgroup_size":{"x":1,"y":1,"z":1},"grid_size":{"x":1,"y":1,"z":1}}}]}}]}"#;
    assert!(matches!(
        import_rocprofv3_capture_v1(source_without_agent, binding(), ImportLimitsV1::default()),
        Err(ImportErrorV1::MissingCaptureDeviceIdentity)
    ));

    assert!(matches!(
        decode_capture_v1(&vec![b' '; MAX_CAPTURE_BYTES_V1 as usize + 1]),
        Err(CaptureErrorV1::CaptureTooLarge { .. })
    ));

    let empty_dispatches =
        br#"{"rocprofiler-sdk-tool":[{"buffer_records":{"kernel_dispatch":[]}}]}"#;
    assert!(matches!(
        import_rocprofv3_capture_v1(empty_dispatches, binding(), ImportLimitsV1::default()),
        Err(ImportErrorV1::CaptureDispatchCountOutOfRange { actual: 0, .. })
    ));
}

#[test]
fn legacy_single_dispatch_trace_import_remains_available() {
    let source = source();
    let imported = import_rocprofv3_json_v1(
        &source,
        RocprofBindingV1 {
            kernel_ir_claim: binding().kernel_ir_claim,
            artifact: binding().artifact,
            wave_width: WaveWidthV1::Wave64,
            selection: RocprofDispatchSelectionV1 {
                process_index: 0,
                dispatch_index: 1,
            },
        },
        ImportLimitsV1::default(),
    )
    .unwrap();
    let trace = encode_trace_v1(imported.trace()).unwrap();
    assert_eq!(decode_trace_v1(&trace).unwrap(), *imported.trace());
    assert_eq!(imported.rocprof_dispatch().unwrap().dispatch_index, 1);
}

#[test]
fn capture_import_cli_uses_the_same_bounded_structured_adapter() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-trace-import"))
        .args([
            "rocprofv3-capture",
            "--kir-sha256",
            &"01".repeat(32),
            "--kir-len",
            "97",
            "--wave-width",
            "64",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&source()).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let capture = decode_capture_v1(&output.stdout).unwrap();
    assert_eq!(capture.dispatches.len(), 3);

    let output = Command::new(env!("CARGO_BIN_EXE_fe2o3-trace-import"))
        .args([
            "rocprofv3-json",
            "--kir-sha256",
            &"01".repeat(32),
            "--kir-len",
            "97",
            "--wave-width",
            "64",
            "--process-index",
            "0",
            "--dispatch-index",
            "0",
            "--source-map-sha256",
            &"03".repeat(32),
            "--source-map-len",
            "10",
            "--source-map-format",
            "1",
        ])
        .output()
        .unwrap();
    assert!(!output.status.success());
}
