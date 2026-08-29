use fe2o3_semantic_import::*;
use fe2o3_semantic_query::*;
use fe2o3_semantic_trace::*;
use std::io::Write;
use std::process::{Command, Stdio};

const CSV: &[u8] =
    include_bytes!("../../fe2o3-semantic-import/tests/fixtures/rocprofv3-1.1-kernel-dispatch.csv");
const ATT: &[u8] =
    include_bytes!("../../fe2o3-semantic-import/tests/fixtures/rocprofv3-1.1-att-manifest.json");
const COUNTERS: &[u8] = include_bytes!(
    "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-1.1-counter-collection.json"
);
const PC_SAMPLES: &[u8] = include_bytes!(
    "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-1.1-stochastic-pc-sampling.json"
);

fn opaque(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn content(byte: u8, len: u64) -> ContentIdentityRecordV1 {
    ContentIdentityRecordV1 {
        scheme: ContentSchemeV1::DomainSeparatedSha256,
        format_version: 1,
        digest: CaptureIdentityV1::new([byte; 32]).unwrap(),
        canonical_len: len,
    }
}

fn environment(environment: u8) -> ProfilerEnvironmentBindingV4 {
    ProfilerEnvironmentBindingV4 {
        environment: content(environment, 200),
        collector_tool: content(11, 50),
        collector_configuration: content(12, 80),
        stable_device_bindings: vec![
            ProfilerDeviceBindingV4 {
                source_agent_id: 17,
                stable_identity: content(20, 64),
            },
            ProfilerDeviceBindingV4 {
                source_agent_id: 19,
                stable_identity: content(21, 64),
            },
        ],
    }
}

fn capture_binding(environment_byte: u8) -> ProfilerDispatchBindingV4 {
    ProfilerDispatchBindingV4 {
        environment: environment(environment_byte),
        kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(opaque(1), 97).unwrap(),
        artifact: Some(ArtifactClaimV1 {
            identity: opaque(2),
            canonical_len: 4_096,
            format_version: 1,
        }),
        source_map: None,
        wave_width: WaveWidthV1::Wave64,
    }
}

fn bundle(environment: u8) -> Vec<u8> {
    encode_profiler_bundle_v4(
        &import_rocprofv3_csv_profiler_bundle_v4(CSV, capture_binding(environment)).unwrap(),
    )
    .unwrap()
}

#[test]
fn open_pages_and_hotspots_are_deterministic_and_evidence_linked() {
    let bytes = bundle(10);
    let session = ProfilerQuerySessionV4::open(&bytes, ProfilerQueryLimitsV4::default()).unwrap();
    let first = session
        .query(ProfilerQueryRequestV4::List {
            kind: ProfilerListKindV4::Dispatches,
            page: ProfilerPageRequestV4 {
                limit: 1,
                cursor: None,
            },
        })
        .unwrap();
    let ProfilerQueryResponseV4::Page { page } = first else {
        panic!("expected page")
    };
    assert_eq!(page.returned, 1);
    let cursor = page.next_cursor.unwrap();
    let second = session
        .query(ProfilerQueryRequestV4::List {
            kind: ProfilerListKindV4::Dispatches,
            page: ProfilerPageRequestV4 {
                limit: 1,
                cursor: Some(cursor),
            },
        })
        .unwrap();
    let ProfilerQueryResponseV4::Page { page } = second else {
        panic!("expected page")
    };
    assert_eq!(page.returned, 1);
    assert!(page.next_cursor.is_none());
    assert_eq!(
        session
            .encode_response(&ProfilerQueryResponseV4::Page { page: page.clone() })
            .unwrap(),
        session
            .encode_response(&ProfilerQueryResponseV4::Page { page })
            .unwrap()
    );

    assert!(matches!(
        session.query(ProfilerQueryRequestV4::List {
            kind: ProfilerListKindV4::Devices,
            page: ProfilerPageRequestV4 {
                limit: 1,
                cursor: Some(cursor),
            },
        }),
        Err(ProfilerQueryErrorV4::CursorMismatch)
    ));

    let response = session
        .query(ProfilerQueryRequestV4::List {
            kind: ProfilerListKindV4::DurationHotspots,
            page: ProfilerPageRequestV4::default(),
        })
        .unwrap();
    let ProfilerQueryResponseV4::Page { page } = response else {
        panic!("expected page")
    };
    let ProfilerQueryItemV4::DurationHotspot { hotspot } = &page.items[0] else {
        panic!("expected hotspot")
    };
    assert_eq!(hotspot.duration_ticks, 80);
    assert_eq!(hotspot.origin, TruthOriginV1::Inferred);
    assert_eq!(hotspot.evidence.bundle, page.context.bundle_identity.digest);
    assert!(hotspot.evidence.record.is_some());
}

#[test]
fn waits_are_typed_unavailable_and_plan_is_bounded() {
    let att = import_rocprofv3_att_profiler_bundle_v4(
        ATT,
        ProfilerAttBindingV4 {
            environment: ProfilerEnvironmentBindingV4 {
                stable_device_bindings: vec![ProfilerDeviceBindingV4 {
                    source_agent_id: 17,
                    stable_identity: content(20, 64),
                }],
                ..environment(10)
            },
            source_agent_id: 17,
            referenced_artifacts: Vec::new(),
        },
    )
    .unwrap();
    let bytes = encode_profiler_bundle_v4(&att).unwrap();
    let session = ProfilerQuerySessionV4::open(&bytes, ProfilerQueryLimitsV4::default()).unwrap();
    let response = session
        .query(ProfilerQueryRequestV4::List {
            kind: ProfilerListKindV4::Waits,
            page: ProfilerPageRequestV4::default(),
        })
        .unwrap();
    let ProfilerQueryResponseV4::Page { page } = response else {
        panic!("expected page")
    };
    assert!(matches!(
        &page.items[0],
        ProfilerQueryItemV4::Unavailable {
            evidence: ProfilerEvidenceV4 {
                origin: TruthOriginV1::Unavailable,
                ..
            },
            ..
        }
    ));

    let response = session
        .query(ProfilerQueryRequestV4::PlanNextCapture {
            goal: ProfilerCaptureGoalV4::ExplainWaits,
        })
        .unwrap();
    let ProfilerQueryResponseV4::PlanNextCapture { plan, .. } = response else {
        panic!("expected plan")
    };
    assert!(plan.steps.len() <= MAX_PROFILER_CAPTURE_PLAN_STEPS_V4);
    assert!(
        plan.steps
            .contains(&ProfilerCaptureStepV4::DecodeAttWithSupportedRocprofComputeViewer)
    );
    assert!(
        plan.limitations
            .iter()
            .any(|value| value.contains("full-grid"))
    );
}

#[test]
fn comparison_requires_exact_environment_and_emits_numeric_duration_delta() {
    let baseline = bundle(10);
    let mut candidate_source = String::from_utf8(CSV.to_vec()).unwrap();
    candidate_source = candidate_source.replace(",200,260", ",200,300");
    let candidate = encode_profiler_bundle_v4(
        &import_rocprofv3_csv_profiler_bundle_v4(candidate_source.as_bytes(), capture_binding(10))
            .unwrap(),
    )
    .unwrap();
    let comparison = compare_profiler_bundles_v4(&baseline, &candidate).unwrap();
    assert!(comparison.comparable);
    assert_eq!(comparison.deltas.len(), 1);
    assert_eq!(
        f64::from_bits(comparison.deltas[0].baseline_f64_bits),
        140.0
    );
    assert_eq!(
        f64::from_bits(comparison.deltas[0].candidate_f64_bits),
        180.0
    );
    assert_eq!(f64::from_bits(comparison.deltas[0].delta_f64_bits), 40.0);
    assert!(!comparison.deltas[0].baseline_evidence.is_empty());
    assert!(encode_profiler_bundle_comparison_v4(&comparison).is_ok());
    let mut contradictory = comparison.clone();
    contradictory.comparable = false;
    assert!(matches!(
        encode_profiler_bundle_comparison_v4(&contradictory),
        Err(ProfilerQueryErrorV4::InvalidComparison)
    ));

    let launch_mismatch_source = String::from_utf8(CSV.to_vec())
        .unwrap()
        .replace(",32,2,1,128,2,1,", ",64,1,1,128,1,1,");
    let launch_mismatch = encode_profiler_bundle_v4(
        &import_rocprofv3_csv_profiler_bundle_v4(
            launch_mismatch_source.as_bytes(),
            capture_binding(10),
        )
        .unwrap(),
    )
    .unwrap();
    let launch_mismatch = compare_profiler_bundles_v4(&baseline, &launch_mismatch).unwrap();
    assert!(!launch_mismatch.comparable);
    assert!(launch_mismatch.deltas.is_empty());
    assert!(launch_mismatch.compatibility.iter().any(|fact| {
        fact.requirement == ProfilerCompatibilityRequirementV4::DispatchWorkload
            && fact.status == ProfilerCompatibilityStatusV4::Mismatch
    }));

    let mismatch = compare_profiler_bundles_v4(&baseline, &bundle(30)).unwrap();
    assert!(!mismatch.comparable);
    assert!(mismatch.deltas.is_empty());
    assert_eq!(
        mismatch.compatibility[0].status,
        ProfilerCompatibilityStatusV4::Mismatch
    );
}

#[test]
fn comparison_treats_absent_artifact_identity_as_unavailable() {
    let mut binding = capture_binding(10);
    binding.artifact = None;
    let bundle =
        encode_profiler_bundle_v4(&import_rocprofv3_csv_profiler_bundle_v4(CSV, binding).unwrap())
            .unwrap();
    let comparison = compare_profiler_bundles_v4(&bundle, &bundle).unwrap();
    assert!(!comparison.comparable);
    assert!(comparison.deltas.is_empty());
    assert!(comparison.compatibility.iter().any(|fact| {
        fact.requirement == ProfilerCompatibilityRequirementV4::Artifact
            && fact.status == ProfilerCompatibilityStatusV4::Unavailable
            && fact.origin == TruthOriginV1::Unavailable
    }));
}

#[test]
fn counters_emit_deltas_but_capture_local_pc_dimensions_fail_closed() {
    let binding = RocprofCaptureBindingV1 {
        kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(opaque(1), 97).unwrap(),
        artifact: Some(ArtifactClaimV1 {
            identity: opaque(2),
            canonical_len: 4_096,
            format_version: 1,
        }),
        source_map: None,
        wave_width: WaveWidthV1::Wave64,
    };
    let counters = encode_counter_capture_v2(
        &import_rocprofv3_counter_capture_v2(COUNTERS, binding, ImportLimitsV1::default()).unwrap(),
    )
    .unwrap();
    let counter_comparison = compare_counter_values_v2(&counters, &counters).unwrap();
    assert_eq!(counter_comparison.deltas.len(), 2);
    assert_eq!(
        counter_comparison.stable_environment,
        ProfilerCompatibilityStatusV4::Unavailable
    );
    assert!(
        counter_comparison
            .deltas
            .iter()
            .all(|delta| f64::from_bits(delta.delta_f64_bits) == 0.0)
    );
    assert!(
        counter_comparison
            .deltas
            .iter()
            .all(|delta| !delta.baseline_evidence.is_empty())
    );

    let pc = encode_pc_sample_capture_v3(
        &import_rocprofv3_pc_sample_capture_v3(
            PC_SAMPLES,
            RocprofPcSampleCaptureBindingV3 {
                capture: binding,
                sampling_interval_cycles: 1_048_576,
            },
            ImportLimitsV1::default(),
        )
        .unwrap(),
    )
    .unwrap();
    let pc_comparison = compare_pc_sample_counts_v3(&pc, &pc).unwrap();
    assert!(!pc_comparison.numeric_dimensions_comparable);
    assert!(pc_comparison.deltas.is_empty());
    assert!(
        pc_comparison
            .unavailable
            .iter()
            .any(|reason| reason.contains("capture-local"))
    );
}

#[test]
fn response_and_input_bounds_fail_closed() {
    let bytes = bundle(10);
    let limits = ProfilerQueryLimitsV4::new(MAX_PROFILER_BUNDLE_BYTES_V4, 4_096, 4_096).unwrap();
    let session = ProfilerQuerySessionV4::open(&bytes, limits).unwrap();
    let response = session.query(ProfilerQueryRequestV4::Capabilities).unwrap();
    assert!(session.encode_response(&response).is_ok());
    let mut stale = response;
    let ProfilerQueryResponseV4::Capabilities { context, .. } = &mut stale else {
        unreachable!()
    };
    context.device_count += 1;
    assert!(matches!(
        session.encode_response(&stale),
        Err(ProfilerQueryErrorV4::InvalidResponse)
    ));
    assert!(matches!(
        ProfilerQuerySessionV4::open(
            &vec![b' '; MAX_PROFILER_BUNDLE_BYTES_V4 as usize + 1],
            ProfilerQueryLimitsV4::default()
        ),
        Err(ProfilerQueryErrorV4::InputTooLarge)
    ));
    assert!(matches!(
        session.query(ProfilerQueryRequestV4::List {
            kind: ProfilerListKindV4::Dispatches,
            page: ProfilerPageRequestV4 {
                limit: 0,
                cursor: None,
            },
        }),
        Err(ProfilerQueryErrorV4::PageLimitOutOfRange)
    ));
}

#[test]
fn profiler_query_and_compare_clis_expose_v4_operations() {
    let bytes = bundle(10);
    let mut query = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-query"))
        .arg("hotspots")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    query.stdin.take().unwrap().write_all(&bytes).unwrap();
    let output = query.wait_with_output().unwrap();
    assert!(output.status.success());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["response"], "page");
    assert_eq!(response["page"]["kind"], "duration_hotspots");

    let mut framed = Vec::new();
    framed.extend_from_slice(&(bytes.len() as u64).to_le_bytes());
    framed.extend_from_slice(&bytes);
    framed.extend_from_slice(&bytes);
    let mut compare = Command::new(env!("CARGO_BIN_EXE_fe2o3-profiler-compare"))
        .arg("bundle-v4")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    compare.stdin.take().unwrap().write_all(&framed).unwrap();
    let output = compare.wait_with_output().unwrap();
    assert!(output.status.success());
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["comparable"], true);
    assert_eq!(response["deltas"][0]["delta_f64_bits"], 0);
}
