use fe2o3_semantic_import::*;
use fe2o3_semantic_query::*;
use fe2o3_semantic_trace::*;
use std::io::Write;
use std::process::{Command, Stdio};

const SOURCE: &[u8] = include_bytes!(
    "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-1.1-stochastic-pc-sampling.json"
);

fn capture_bytes(source: &[u8]) -> Vec<u8> {
    let id = OpaqueIdentityV1::new([1; 32]).unwrap();
    let capture = import_rocprofv3_pc_sample_capture_v3(
        source,
        RocprofPcSampleCaptureBindingV3 {
            capture: RocprofCaptureBindingV1 {
                kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(id, 97).unwrap(),
                artifact: None,
                source_map: None,
                wave_width: WaveWidthV1::Wave64,
            },
            sampling_interval_cycles: 1_048_576,
        },
        ImportLimitsV1::default(),
    )
    .unwrap();
    encode_pc_sample_capture_v3(&capture).unwrap()
}

#[test]
fn raw_samples_are_observed_and_hotspots_are_bounded_inferences() {
    let bytes = capture_bytes(SOURCE);
    let session = PcSampleQuerySessionV3::open(&bytes, PcSampleQueryLimitsV3::default()).unwrap();
    let request = PcSampleQueryRequestV3::List {
        kind: PcSampleListKindV3::PcHotspots,
        page: PcSamplePageRequestV3 {
            limit: 8,
            ..Default::default()
        },
    };
    assert_eq!(
        session.query_json(request).unwrap(),
        session.query_json(request).unwrap()
    );
    let PcSampleQueryResponseV3::Page { page } = session.query(request).unwrap() else {
        panic!("expected page")
    };
    assert_eq!(page.items.len(), 4);
    assert!(page.items.iter().all(|item| matches!(
        item,
        PcSampleQueryItemV3::PcHotspot { hotspot }
            if hotspot.origin == TruthOriginV1::Inferred
                && hotspot.raw_sample_count == 1
                && hotspot.limitation.contains("not_instruction_count")
    )));

    let PcSampleQueryResponseV3::Page { page } = session
        .query(PcSampleQueryRequestV3::List {
            kind: PcSampleListKindV3::Samples,
            page: PcSamplePageRequestV3 {
                limit: 8,
                ..Default::default()
            },
        })
        .unwrap()
    else {
        panic!("expected page")
    };
    assert_eq!(page.items.len(), 5);
    assert!(page.items.iter().all(|item| matches!(
        item,
        PcSampleQueryItemV3::Sample { sample }
            if sample.origin == TruthOriginV1::Observed
                && sample.timestamp.domain == PcTimestampDomainV3::RocprofilerOpaqueCollectorClock
    )));
}

#[test]
fn cursors_bind_capture_operation_and_filters() {
    let bytes = capture_bytes(SOURCE);
    let session = PcSampleQuerySessionV3::open(&bytes, PcSampleQueryLimitsV3::default()).unwrap();
    let PcSampleQueryResponseV3::Page { page } = session
        .query(PcSampleQueryRequestV3::List {
            kind: PcSampleListKindV3::Samples,
            page: PcSamplePageRequestV3 {
                limit: 1,
                ..Default::default()
            },
        })
        .unwrap()
    else {
        panic!("expected page")
    };
    let cursor = page.next_cursor.unwrap();
    assert!(matches!(
        session.query(PcSampleQueryRequestV3::List {
            kind: PcSampleListKindV3::PcHotspots,
            page: PcSamplePageRequestV3 {
                limit: 1,
                cursor: Some(cursor),
                ..Default::default()
            }
        }),
        Err(PcSampleQueryErrorV3::CursorQueryMismatch)
    ));
    let capture = decode_pc_sample_capture_v3(&bytes).unwrap();
    assert!(matches!(
        session.query(PcSampleQueryRequestV3::List {
            kind: PcSampleListKindV3::Samples,
            page: PcSamplePageRequestV3 {
                limit: 1,
                cursor: Some(cursor),
                dispatch_filter: Some(capture.dispatches[0].identity),
                code_object_filter: None,
            }
        }),
        Err(PcSampleQueryErrorV3::CursorQueryMismatch)
    ));
}

#[test]
fn capabilities_expose_pc_evidence_without_att_or_correlation_overclaims() {
    let bytes = capture_bytes(SOURCE);
    let session = PcSampleQuerySessionV3::open(&bytes, PcSampleQueryLimitsV3::default()).unwrap();
    let PcSampleQueryResponseV3::Capabilities { capabilities, .. } =
        session.query(PcSampleQueryRequestV3::Capabilities).unwrap()
    else {
        panic!("expected capabilities")
    };
    for name in [
        PcSampleCapabilityNameV3::RawPcSamples,
        PcSampleCapabilityNameV3::SampledWaveLocations,
        PcSampleCapabilityNameV3::PcHotspots,
    ] {
        assert!(capabilities.iter().any(|item| item.name == name
            && item.availability == PcSampleCapabilityAvailabilityV3::Available));
    }
    for name in [
        PcSampleCapabilityNameV3::SourceCorrelation,
        PcSampleCapabilityNameV3::IsaCorrelation,
        PcSampleCapabilityNameV3::ClockConversion,
        PcSampleCapabilityNameV3::AttWaveTimeline,
        PcSampleCapabilityNameV3::CompleteInstructionTimeline,
        PcSampleCapabilityNameV3::CrossCaptureComparison,
        PcSampleCapabilityNameV3::ExecutionControl,
    ] {
        assert!(capabilities.iter().any(|item| item.name == name
            && item.availability == PcSampleCapabilityAvailabilityV3::Unavailable));
    }
    let PcSampleQueryResponseV3::Open { coverage, .. } =
        session.query(PcSampleQueryRequestV3::Open).unwrap()
    else {
        panic!("expected open")
    };
    assert_eq!(coverage.loss.state, LossStateV1::Unknown);
    assert_eq!(
        coverage.pc_sample_scope,
        PcSampleScopeV3::StochasticSamplesOnly
    );
    assert_eq!(
        coverage.exec_mask_semantics,
        PcExecMaskSemanticsV3 {
            origin: TruthOriginV1::Declared,
            meaning:
                PcExecMaskMeaningV3::RocprofilerActiveLaneMaskNoPerLaneInstructionExecutionProof,
        }
    );
}

#[test]
fn stdin_only_cli_emits_agent_native_json() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-pc-sample-query"))
        .args(["list-samples", "--limit", "2"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&capture_bytes(SOURCE))
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout.last(), Some(&b'\n'));
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["response"], "page");
    assert_eq!(response["page"]["returned"], 2);
    assert!(response["page"]["next_cursor"].is_object());
}

#[test]
fn late_pages_are_streamed_and_responses_remain_hard_bounded() {
    let mut document: serde_json::Value = serde_json::from_slice(SOURCE).unwrap();
    let samples = document["rocprofiler-sdk-tool"][0]["buffer_records"]["pc_sample_stochastic"]
        .as_array_mut()
        .unwrap();
    let template = samples[0].clone();
    samples.clear();
    for ordinal in 0..5_000_u64 {
        let mut sample = template.clone();
        sample["record"]["timestamp"] = (5_380_230_786_000_000_u64 + ordinal).into();
        sample["record"]["pc"]["code_object_offset"] = (7_960_u64 + ordinal * 4).into();
        sample["record"]["wrkgrp_id"]["x"] = (ordinal % 4).into();
        samples.push(sample);
    }
    let source = serde_json::to_vec(&document).unwrap();
    let bytes = capture_bytes(&source);
    let session = PcSampleQuerySessionV3::open(&bytes, PcSampleQueryLimitsV3::default()).unwrap();
    let mut cursor = None;
    let mut seen = 0_usize;
    loop {
        let PcSampleQueryResponseV3::Page { page } = session
            .query(PcSampleQueryRequestV3::List {
                kind: PcSampleListKindV3::Samples,
                page: PcSamplePageRequestV3 {
                    limit: 127,
                    cursor,
                    dispatch_filter: None,
                    code_object_filter: None,
                },
            })
            .unwrap()
        else {
            panic!("expected page")
        };
        assert!(page.items.len() <= 127);
        seen += page.items.len();
        cursor = page.next_cursor;
        if cursor.is_none() {
            break;
        }
    }
    assert_eq!(seen, 5_000);

    let bounded = PcSampleQuerySessionV3::open(
        &bytes,
        PcSampleQueryLimitsV3::new(MAX_PC_SAMPLE_CAPTURE_BYTES_V3, 4096, 1).unwrap(),
    )
    .unwrap();
    assert!(
        bounded
            .query_json(PcSampleQueryRequestV3::Capabilities)
            .unwrap()
            .len()
            <= 4096
    );
    assert!(matches!(
        bounded.query(PcSampleQueryRequestV3::List {
            kind: PcSampleListKindV3::Samples,
            page: PcSamplePageRequestV3 {
                limit: 2,
                ..Default::default()
            }
        }),
        Err(PcSampleQueryErrorV3::LimitOutOfRange)
    ));
}
