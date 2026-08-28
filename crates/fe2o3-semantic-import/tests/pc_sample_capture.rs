use std::io::Write;
use std::process::{Command, Stdio};

use fe2o3_semantic_import::*;
use fe2o3_semantic_trace::*;

const SOURCE: &[u8] = include_bytes!("fixtures/rocprofv3-1.1-stochastic-pc-sampling.json");

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn binding() -> RocprofPcSampleCaptureBindingV3 {
    RocprofPcSampleCaptureBindingV3 {
        capture: RocprofCaptureBindingV1 {
            kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(identity(1), 97).unwrap(),
            artifact: Some(ArtifactClaimV1 {
                identity: identity(2),
                canonical_len: 4096,
                format_version: 1,
            }),
            source_map: None,
            wave_width: WaveWidthV1::Wave64,
        },
        sampling_interval_cycles: 1_048_576,
    }
}

fn imported_capture() -> SemanticPcSampleCaptureV3 {
    import_rocprofv3_pc_sample_capture_v3(SOURCE, binding(), ImportLimitsV1::default()).unwrap()
}

#[test]
fn real_rocprofv3_1_1_stochastic_shape_is_canonical_and_bounded() {
    let capture = imported_capture();
    let bytes = encode_pc_sample_capture_v3(&capture).unwrap();
    assert_eq!(decode_pc_sample_capture_v3(&bytes).unwrap(), capture);
    assert_eq!(
        bytes,
        encode_pc_sample_capture_v3(&imported_capture()).unwrap()
    );
    assert_eq!(capture.dispatches.len(), 2);
    assert_eq!(capture.dispatches[0].launch.workgroup_size, [64, 1, 1]);
    assert_eq!(capture.dispatches[1].launch.workgroup_size, [256, 1, 1]);
    assert_eq!(capture.dispatches[0].sample_count, 2);
    assert_eq!(capture.dispatches[1].sample_count, 3);
    assert_eq!(capture.samples.len(), 5);
    assert_eq!(capture.code_objects.len(), 2);
    assert_eq!(capture.samples[0].origin, TruthOriginV1::Observed);
    assert_eq!(capture.samples[0].pc.code_object_offset, Some(7960));
    assert_eq!(capture.samples[0].exec_mask, u64::MAX);
    assert_eq!(
        capture.coverage.exec_mask_semantics,
        PcExecMaskSemanticsV3 {
            origin: TruthOriginV1::Declared,
            meaning:
                PcExecMaskMeaningV3::RocprofilerActiveLaneMaskNoPerLaneInstructionExecutionProof,
        }
    );
    assert_eq!(capture.samples[0].wave.wave_in_group, 0);
    assert_eq!(
        capture.samples[0].timestamp.domain,
        PcTimestampDomainV3::RocprofilerOpaqueCollectorClock
    );
    assert_eq!(
        capture.samples[4].pc.unavailable_reason,
        Some(PcPositionUnavailableReasonV3::NativeVirtualAddressRedacted)
    );
    assert_eq!(capture.samples[4].pc.code_object_offset, None);
    assert!(capture.samples[3].memory_counters_present_but_not_imported);
    assert_eq!(capture.coverage.loss.origin, TruthOriginV1::Unavailable);
    assert_eq!(capture.coverage.loss.state, LossStateV1::Unknown);
    assert_eq!(
        capture.coverage.sampling.interval_origin,
        TruthOriginV1::Declared
    );
    assert_eq!(
        capture.dispatches[0].source_and_isa_correlation,
        PcSourceAndIsaCorrelationV3::UnavailableNoAuthenticatedSourceOrIsaMap
    );
}

#[test]
fn malformed_truncated_nan_and_uncorrelated_sources_are_rejected() {
    assert!(matches!(
        import_rocprofv3_pc_sample_capture_v3(
            &SOURCE[..SOURCE.len() - 2],
            binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::InvalidRocprofJson)
    ));
    let non_json_number = String::from_utf8(SOURCE.to_vec())
        .unwrap()
        .replace("\"timestamp\":5380230786023534", "\"timestamp\":NaN");
    assert!(matches!(
        import_rocprofv3_pc_sample_capture_v3(
            non_json_number.as_bytes(),
            binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::InvalidRocprofJson)
    ));
    let missing_dispatch = String::from_utf8(SOURCE.to_vec()).unwrap().replace(
        "\"timestamp\":5380230786603534,\"dispatch_id\":42",
        "\"timestamp\":5380230786603534,\"dispatch_id\":99",
    );
    assert!(matches!(
        import_rocprofv3_pc_sample_capture_v3(
            missing_dispatch.as_bytes(),
            binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::PcSampleDispatchNotFound)
    ));
    let invalid_flag = String::from_utf8(SOURCE.to_vec()).unwrap().replacen(
        "\"wave_issued\":1",
        "\"wave_issued\":2",
        1,
    );
    assert!(matches!(
        import_rocprofv3_pc_sample_capture_v3(
            invalid_flag.as_bytes(),
            binding(),
            ImportLimitsV1::default()
        ),
        Err(ImportErrorV1::InvalidPcSampleRecord)
    ));
    let mut invalid_binding = binding();
    invalid_binding.sampling_interval_cycles = 0;
    assert!(matches!(
        import_rocprofv3_pc_sample_capture_v3(SOURCE, invalid_binding, ImportLimitsV1::default()),
        Err(ImportErrorV1::InvalidPcSamplingConfiguration)
    ));
}

#[test]
fn process_local_handle_collisions_remain_distinct() {
    let mut document: serde_json::Value = serde_json::from_slice(SOURCE).unwrap();
    let processes = document["rocprofiler-sdk-tool"].as_array_mut().unwrap();
    processes.push(processes[0].clone());
    let source = serde_json::to_vec(&document).unwrap();
    let capture =
        import_rocprofv3_pc_sample_capture_v3(&source, binding(), ImportLimitsV1::default())
            .unwrap();
    assert_eq!(capture.devices.len(), 2);
    assert_eq!(capture.code_objects.len(), 4);
    assert_ne!(
        capture.samples[0].pc.code_object_identity,
        capture.samples[5].pc.code_object_identity
    );
    assert_ne!(
        capture.dispatches[0].device_identity,
        capture.dispatches[2].device_identity
    );
}

#[test]
fn hostile_substitution_origins_counts_and_noncanonical_bytes_are_rejected() {
    let capture = imported_capture();
    let mut hostile = capture.clone();
    hostile.samples[0].exec_mask ^= 1;
    assert!(matches!(
        decode_pc_sample_capture_v3(&serde_json::to_vec(&hostile).unwrap()),
        Err(PcSampleCaptureErrorV3::InvalidSampleIdentity)
    ));
    let mut hostile = capture.clone();
    hostile.samples[0].origin = TruthOriginV1::Inferred;
    assert!(matches!(
        decode_pc_sample_capture_v3(&serde_json::to_vec(&hostile).unwrap()),
        Err(PcSampleCaptureErrorV3::InvalidSampleRecord)
    ));
    let mut hostile = capture.clone();
    hostile.samples[0].dispatch_identity = capture.dispatches[1].identity;
    assert!(matches!(
        decode_pc_sample_capture_v3(&serde_json::to_vec(&hostile).unwrap()),
        Err(PcSampleCaptureErrorV3::InvalidSampleIdentity)
    ));
    let mut hostile = capture.clone();
    hostile.dispatches[0].launch.logical_grid[0] -= 1;
    assert!(matches!(
        decode_pc_sample_capture_v3(&serde_json::to_vec(&hostile).unwrap()),
        Err(PcSampleCaptureErrorV3::InvalidDispatchIdentity)
    ));
    let mut hostile = capture.clone();
    hostile.dispatches[0].launch.grid_workgroups[0] += 1;
    assert!(matches!(
        decode_pc_sample_capture_v3(&serde_json::to_vec(&hostile).unwrap()),
        Err(PcSampleCaptureErrorV3::InvalidDispatchEnvelope)
    ));
    let mut hostile = capture.clone();
    hostile.dispatches[0].dispatch_index += 1;
    assert!(matches!(
        decode_pc_sample_capture_v3(&serde_json::to_vec(&hostile).unwrap()),
        Err(PcSampleCaptureErrorV3::InvalidDispatchIdentity)
    ));
    let mut hostile = capture.clone();
    hostile.samples[0].wave.wave_in_group = 15;
    assert!(matches!(
        decode_pc_sample_capture_v3(&serde_json::to_vec(&hostile).unwrap()),
        Err(PcSampleCaptureErrorV3::InvalidSampleRecord)
    ));
    let mut hostile = capture.clone();
    hostile.devices[0].identity = CaptureIdentityV1::new([0xdd; 32]).unwrap();
    assert!(matches!(
        decode_pc_sample_capture_v3(&serde_json::to_vec(&hostile).unwrap()),
        Err(PcSampleCaptureErrorV3::InvalidDeviceCatalog)
    ));
    let mut hostile = capture.clone();
    hostile.dispatches[0].sample_count += 1;
    assert!(matches!(
        decode_pc_sample_capture_v3(&serde_json::to_vec(&hostile).unwrap()),
        Err(PcSampleCaptureErrorV3::InvalidDispatchSampleCount)
    ));
    let mut hostile = capture.clone();
    hostile.coverage.exec_mask_semantics.origin = TruthOriginV1::Proved;
    assert!(matches!(
        decode_pc_sample_capture_v3(&serde_json::to_vec(&hostile).unwrap()),
        Err(PcSampleCaptureErrorV3::InvalidCoverage)
    ));
    let hostile = String::from_utf8(encode_pc_sample_capture_v3(&capture).unwrap())
        .unwrap()
        .replace(
            "rocprofiler_active_lane_mask_no_per_lane_instruction_execution_proof",
            "per_lane_instruction_execution_proof",
        );
    assert!(matches!(
        decode_pc_sample_capture_v3(hostile.as_bytes()),
        Err(PcSampleCaptureErrorV3::JsonDecode)
    ));
    let mut hostile = capture.clone();
    hostile
        .samples
        .resize(MAX_PC_SAMPLE_RECORDS_V3 + 1, capture.samples[0]);
    assert!(matches!(
        hostile.validate(),
        Err(PcSampleCaptureErrorV3::InvalidSampleCount)
    ));
    let mut bytes = encode_pc_sample_capture_v3(&capture).unwrap();
    bytes.push(b'\n');
    assert!(matches!(
        decode_pc_sample_capture_v3(&bytes),
        Err(PcSampleCaptureErrorV3::NonCanonicalEncoding)
    ));
}

#[test]
fn pc_sample_cli_reads_structured_stdin_and_emits_v3() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-trace-import"))
        .args([
            "rocprofv3-pc-sample-capture",
            "--kir-sha256",
            &"01".repeat(32),
            "--kir-len",
            "97",
            "--wave-width",
            "64",
            "--sampling-interval-cycles",
            "1048576",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(SOURCE).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let capture = decode_pc_sample_capture_v3(&output.stdout).unwrap();
    assert_eq!(capture.schema_version, PC_SAMPLE_CAPTURE_SCHEMA_VERSION_V3);
}

#[test]
fn earlier_capture_bytes_remain_distinct_from_v3() {
    let v1_source = br#"{"rocprofiler-sdk-tool":[{"buffer_records":{"kernel_dispatch":[{"start_timestamp":1,"end_timestamp":2,"dispatch_info":{"agent_id":{"handle":17},"workgroup_size":{"x":64,"y":1,"z":1},"grid_size":{"x":64,"y":1,"z":1}}}]}}]}"#;
    let v1 = import_rocprofv3_capture_v1(v1_source, binding().capture, ImportLimitsV1::default())
        .unwrap();
    let bytes = encode_capture_v1(&v1).unwrap();
    assert!(decode_capture_v1(&bytes).is_ok());
    assert!(decode_pc_sample_capture_v3(&bytes).is_err());
}
