use std::io::Write;
use std::process::{Command, Stdio};

use fe2o3_semantic_import::*;
use fe2o3_semantic_query::*;
use fe2o3_semantic_trace::*;

fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

fn source(offset: u64) -> Vec<u8> {
    format!(
        r#"{{"rocprofiler-sdk-tool":[{{"buffer_records":{{"kernel_dispatch":[
        {{"start_timestamp":{},"end_timestamp":{},"dispatch_info":{{"agent_id":{{"handle":17}},"workgroup_size":{{"x":64,"y":1,"z":1}},"grid_size":{{"x":256,"y":1,"z":1}}}}}},
        {{"start_timestamp":{},"end_timestamp":{},"dispatch_info":{{"agent_id":{{"handle":18}},"workgroup_size":{{"x":64,"y":1,"z":1}},"grid_size":{{"x":128,"y":1,"z":1}}}}}},
        {{"start_timestamp":{},"end_timestamp":{},"dispatch_info":{{"agent_id":{{"handle":17}},"workgroup_size":{{"x":32,"y":1,"z":1}},"grid_size":{{"x":32,"y":1,"z":1}}}}}}
        ]}}}}]}}"#,
        offset + 100,
        offset + 300,
        offset + 400,
        offset + 450,
        offset + 500,
        offset + 900,
    )
    .into_bytes()
}

fn capture_bytes(offset: u64) -> Vec<u8> {
    let capture = import_rocprofv3_capture_v1(
        &source(offset),
        RocprofCaptureBindingV1 {
            kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(identity(1), 97).unwrap(),
            artifact: None,
            source_map: None,
            wave_width: WaveWidthV1::Wave64,
        },
        ImportLimitsV1::default(),
    )
    .unwrap();
    encode_capture_v1(&capture).unwrap()
}

#[test]
fn capture_queries_are_deterministic_bounded_and_truth_preserving() {
    let session =
        CaptureQuerySessionV1::open(&capture_bytes(0), CaptureQueryLimitsV1::default()).unwrap();
    let request = CaptureQueryRequestV1::List {
        kind: CaptureListKindV1::Hotspots,
        page: CapturePageRequestV1 {
            limit: 3,
            cursor: None,
        },
    };
    assert_eq!(
        session.query_json(request).unwrap(),
        session.query_json(request).unwrap()
    );
    let CaptureQueryResponseV1::Page { page } = session.query(request).unwrap() else {
        panic!("expected hotspot page")
    };
    let durations: Vec<_> = page
        .items
        .iter()
        .map(|item| match item {
            CaptureQueryItemV1::Hotspot { hotspot } => {
                assert_eq!(hotspot.origin, TruthOriginV1::Observed);
                assert_eq!(hotspot.comparison_scope, "captured_dispatch_envelopes_only");
                hotspot.duration_ticks
            }
            _ => panic!("unexpected item"),
        })
        .collect();
    assert_eq!(durations, [400, 200, 50]);

    let bounded = CaptureQueryLimitsV1::new(MAX_CAPTURE_BYTES_V1, 4_096, 2).unwrap();
    let bounded_session = CaptureQuerySessionV1::open(&capture_bytes(0), bounded).unwrap();
    let output = bounded_session
        .query_json(CaptureQueryRequestV1::Capabilities)
        .unwrap();
    assert!(output.len() <= 4_096);
    assert!(matches!(
        bounded_session.query(CaptureQueryRequestV1::List {
            kind: CaptureListKindV1::Dispatches,
            page: CapturePageRequestV1 {
                limit: 3,
                cursor: None
            }
        }),
        Err(CaptureQueryErrorV1::PageLimitOutOfRange)
    ));
}

#[test]
fn pagination_cursors_bind_capture_and_operation() {
    let first_bytes = capture_bytes(0);
    let first = CaptureQuerySessionV1::open(&first_bytes, CaptureQueryLimitsV1::default()).unwrap();
    let CaptureQueryResponseV1::Page { page } = first
        .query(CaptureQueryRequestV1::List {
            kind: CaptureListKindV1::Dispatches,
            page: CapturePageRequestV1 {
                limit: 1,
                cursor: None,
            },
        })
        .unwrap()
    else {
        panic!("expected dispatch page")
    };
    let cursor = page.next_cursor.unwrap();
    assert!(matches!(
        first.query(CaptureQueryRequestV1::List {
            kind: CaptureListKindV1::Hotspots,
            page: CapturePageRequestV1 {
                limit: 1,
                cursor: Some(cursor)
            }
        }),
        Err(CaptureQueryErrorV1::CursorQueryMismatch)
    ));

    let second =
        CaptureQuerySessionV1::open(&capture_bytes(1), CaptureQueryLimitsV1::default()).unwrap();
    assert!(matches!(
        second.query(CaptureQueryRequestV1::List {
            kind: CaptureListKindV1::Dispatches,
            page: CapturePageRequestV1 {
                limit: 1,
                cursor: Some(cursor)
            }
        }),
        Err(CaptureQueryErrorV1::CursorQueryMismatch)
    ));
}

#[test]
fn inspect_and_capabilities_do_not_claim_missing_profiler_facts() {
    let bytes = capture_bytes(0);
    let capture = decode_capture_v1(&bytes).unwrap();
    let session = CaptureQuerySessionV1::open(&bytes, CaptureQueryLimitsV1::default()).unwrap();
    let CaptureQueryResponseV1::InspectDispatch { dispatch, .. } = session
        .query(CaptureQueryRequestV1::InspectDispatch {
            identity: capture.dispatches[1].identity,
        })
        .unwrap()
    else {
        panic!("expected inspected dispatch")
    };
    assert_eq!(dispatch.duration_ticks, 50);
    assert_eq!(dispatch.source_map.origin, TruthOriginV1::Unavailable);

    let CaptureQueryResponseV1::Capabilities { capabilities, .. } =
        session.query(CaptureQueryRequestV1::Capabilities).unwrap()
    else {
        panic!("expected capabilities")
    };
    for name in [
        CaptureCapabilityNameV1::CounterRecords,
        CaptureCapabilityNameV1::PcSamples,
        CaptureCapabilityNameV1::AttWaveEvents,
        CaptureCapabilityNameV1::ExecutionControl,
    ] {
        assert_eq!(
            capabilities
                .iter()
                .find(|capability| capability.name == name)
                .unwrap()
                .availability,
            CaptureCapabilityAvailabilityV1::Unavailable
        );
    }
}

#[test]
fn every_catalog_operation_is_available_and_capture_bound() {
    let bytes = capture_bytes(0);
    let session = CaptureQuerySessionV1::open(&bytes, CaptureQueryLimitsV1::default()).unwrap();
    for (kind, expected) in [
        (CaptureListKindV1::Runs, 1),
        (CaptureListKindV1::Devices, 2),
        (CaptureListKindV1::Dispatches, 3),
        (CaptureListKindV1::Hotspots, 3),
    ] {
        let CaptureQueryResponseV1::Page { page } = session
            .query(CaptureQueryRequestV1::List {
                kind,
                page: CapturePageRequestV1::default(),
            })
            .unwrap()
        else {
            panic!("expected catalog page")
        };
        assert_eq!(page.returned, expected);
        assert_eq!(
            page.context.capture_identity.canonical_len,
            bytes.len() as u64
        );
    }
}

#[test]
fn capture_query_cli_opens_canonical_stdin_only_documents() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fe2o3-capture-query"))
        .arg("open")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(&capture_bytes(0))
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["response"], "open");
    assert_eq!(response["context"]["dispatch_count"], 3);
    assert_eq!(response["coverage"]["loss"]["state"], "unknown");
}
