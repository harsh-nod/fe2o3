use fe2o3_semantic_import::*;
use fe2o3_semantic_query::*;
use fe2o3_semantic_trace::*;
use std::io::Write;
use std::process::{Command, Stdio};

fn id(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}
fn binding() -> RocprofCaptureBindingV1 {
    RocprofCaptureBindingV1 {
        kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(id(1), 97).unwrap(),
        artifact: None,
        source_map: None,
        wave_width: WaveWidthV1::Wave64,
    }
}
fn dispatch_bytes(start: u64) -> Vec<u8> {
    let source = format!(
        r#"{{"rocprofiler-sdk-tool":[{{"buffer_records":{{"kernel_dispatch":[{{"start_timestamp":{start},"end_timestamp":{},"dispatch_info":{{"agent_id":{{"handle":17}},"workgroup_size":{{"x":64,"y":1,"z":1}},"grid_size":{{"x":64,"y":1,"z":1}}}}}}]}}}}]}}"#,
        start + 10
    );
    encode_capture_v1(
        &import_rocprofv3_capture_v1(source.as_bytes(), binding(), ImportLimitsV1::default())
            .unwrap(),
    )
    .unwrap()
}
fn counter_bytes(value: u64) -> Vec<u8> {
    let source = format!(
        r#"{{"rocprofiler-sdk-tool":[{{"buffer_records":{{}},"counters":[{{"agent_id":{{"handle":17}},"id":{{"handle":101}},"is_constant":0,"is_derived":0,"name":"SQ_WAVES"}}],"callback_records":{{"counter_collection":[{{"dispatch_data":{{"start_timestamp":1,"end_timestamp":2,"dispatch_info":{{"agent_id":{{"handle":17}},"workgroup_size":{{"x":64,"y":1,"z":1}},"grid_size":{{"x":64,"y":1,"z":1}}}}}},"records":[{{"counter_id":{{"handle":101}},"value":{value}.0}}]}}]}}}}]}}"#
    );
    encode_counter_capture_v2(
        &import_rocprofv3_counter_capture_v2(
            source.as_bytes(),
            binding(),
            ImportLimitsV1::default(),
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn identical_evidence_only_proves_byte_equality_not_a_cross_run_regression() {
    let bytes = dispatch_bytes(1);
    let result = compare_dispatch_captures_v1(&bytes, &bytes).unwrap();
    assert_eq!(
        result.disposition,
        ComparisonDispositionV1::IdenticalCanonicalEvidenceOnly
    );
    assert!(result.deltas.is_empty());
    assert_eq!(
        result.confidence,
        ComparisonConfidenceV1::ExactCanonicalEqualityOnly
    );
    assert!(result.compatibility.iter().any(|fact| fact.requirement
        == CompatibilityRequirementV1::StableEnvironmentIdentity
        && fact.status == CompatibilityStatusV1::Unavailable));
    assert!(result.next_capture.is_some());
    assert!(
        result
            .compatibility
            .iter()
            .filter(|fact| matches!(
                fact.requirement,
                CompatibilityRequirementV1::KernelIrIdentity
                    | CompatibilityRequirementV1::ArtifactIdentity
                    | CompatibilityRequirementV1::SourceMapIdentity
            ))
            .all(|fact| fact.origin == TruthOriginV1::Declared)
    );
    assert_eq!(
        encode_capture_comparison_v1(&result).unwrap(),
        encode_capture_comparison_v1(&result).unwrap()
    );
}

#[test]
fn distinct_v1_and_v2_captures_fail_closed_on_source_bound_identities() {
    for result in [
        compare_dispatch_captures_v1(&dispatch_bytes(1), &dispatch_bytes(2)).unwrap(),
        compare_counter_captures_v2(&counter_bytes(1), &counter_bytes(2)).unwrap(),
    ] {
        assert_eq!(
            result.disposition,
            ComparisonDispositionV1::UnavailableSourceBoundIdentity
        );
        assert!(result.deltas.is_empty());
        assert_eq!(result.confidence, ComparisonConfidenceV1::Unavailable);
        assert!(result.evidence.contradicting.is_empty());
        assert!(!result.evidence.blocking.is_empty());
        assert!(result.next_capture.is_some());
        assert!(result.compatibility.iter().any(|fact| fact.requirement
            == CompatibilityRequirementV1::DeviceIdentity
            && fact.status == CompatibilityStatusV1::Mismatch));
    }
}

#[test]
fn substitution_malformed_loss_nan_and_oversize_never_become_deltas() {
    let baseline = counter_bytes(1);
    let mut malformed = baseline.clone();
    malformed[0] = b'[';
    assert!(matches!(
        compare_counter_captures_v2(&baseline, &malformed),
        Err(CaptureCompareErrorV1::InvalidCandidate)
    ));
    let mut capture = decode_counter_capture_v2(&baseline).unwrap();
    capture.coverage.loss.state = LossStateV1::NoneReported;
    assert!(matches!(
        compare_counter_captures_v2(&baseline, &serde_json::to_vec(&capture).unwrap()),
        Err(CaptureCompareErrorV1::InvalidCandidate)
    ));
    let mut capture = decode_counter_capture_v2(&baseline).unwrap();
    capture.dispatches[0].values[0].value_f64_bits = f64::NAN.to_bits();
    assert!(matches!(
        compare_counter_captures_v2(&baseline, &serde_json::to_vec(&capture).unwrap()),
        Err(CaptureCompareErrorV1::InvalidCandidate)
    ));
    let oversized = vec![b'x'; usize::try_from(MAX_COMPARISON_INPUT_BYTES_V1).unwrap()];
    assert!(matches!(
        compare_dispatch_captures_v1(&oversized, b"x"),
        Err(CaptureCompareErrorV1::InputTooLarge)
    ));
}

#[test]
fn stdin_only_comparison_cli_uses_exact_bounded_framing() {
    let baseline = counter_bytes(1);
    let candidate = counter_bytes(2);
    let mut frame = Vec::new();
    frame.extend_from_slice(&u64::try_from(baseline.len()).unwrap().to_le_bytes());
    frame.extend_from_slice(&baseline);
    frame.extend_from_slice(&candidate);
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-capture-compare"))
        .arg("counter-v2")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&frame).unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    let json: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(json["disposition"], "unavailable_source_bound_identity");
    assert_eq!(json["deltas"], serde_json::json!([]));
}
