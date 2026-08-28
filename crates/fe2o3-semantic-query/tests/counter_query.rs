use fe2o3_semantic_import::*;
use fe2o3_semantic_query::*;
use fe2o3_semantic_trace::*;

const SOURCE: &[u8] = include_bytes!(
    "../../fe2o3-semantic-import/tests/fixtures/rocprofv3-1.1-counter-collection.json"
);

fn capture_bytes() -> Vec<u8> {
    let id = OpaqueIdentityV1::new([1; 32]).unwrap();
    let capture = import_rocprofv3_counter_capture_v2(
        SOURCE,
        RocprofCaptureBindingV1 {
            kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(id, 97).unwrap(),
            artifact: None,
            source_map: None,
            wave_width: WaveWidthV1::Wave64,
        },
        ImportLimitsV1::default(),
    )
    .unwrap();
    encode_counter_capture_v2(&capture).unwrap()
}

#[test]
fn raw_values_are_observed_and_aggregates_are_inferred_deterministically() {
    let session =
        CounterQuerySessionV2::open(&capture_bytes(), CounterQueryLimitsV2::default()).unwrap();
    let request = CounterQueryRequestV2::List {
        kind: CounterListKindV2::Hotspots,
        page: CounterPageRequestV2 {
            limit: 8,
            ..Default::default()
        },
    };
    assert_eq!(
        session.query_json(request).unwrap(),
        session.query_json(request).unwrap()
    );
    let CounterQueryResponseV2::Page { page } = session.query(request).unwrap() else {
        panic!("expected page")
    };
    let CounterQueryItemV2::Hotspot { hotspot } = page.items[0] else {
        panic!("expected hotspot")
    };
    assert_eq!(f64::from_bits(hotspot.aggregate_f64_bits), 9.0);
    assert_eq!(hotspot.origin, TruthOriginV1::Inferred);
    assert_eq!(hotspot.raw_record_count, 1);
    let CounterQueryItemV2::Hotspot { hotspot } = page.items[1] else {
        panic!("expected hotspot")
    };
    assert_eq!(f64::from_bits(hotspot.aggregate_f64_bits), 7.0);
    let CounterQueryItemV2::Hotspot { hotspot } = page.items[2] else {
        panic!("expected hotspot")
    };
    assert_eq!(f64::from_bits(hotspot.aggregate_f64_bits), 4.0);
    assert_eq!(hotspot.raw_record_count, 2);

    let values = session
        .query(CounterQueryRequestV2::List {
            kind: CounterListKindV2::Values,
            page: CounterPageRequestV2 {
                limit: 8,
                ..Default::default()
            },
        })
        .unwrap();
    let CounterQueryResponseV2::Page { page } = values else {
        panic!("expected page")
    };
    assert!(page.items.iter().all(|item| matches!(item, CounterQueryItemV2::Value { value, .. } if value.origin == TruthOriginV1::Observed)));
}

#[test]
fn cursors_bind_capture_operation_and_filters() {
    let session =
        CounterQuerySessionV2::open(&capture_bytes(), CounterQueryLimitsV2::default()).unwrap();
    let CounterQueryResponseV2::Page { page } = session
        .query(CounterQueryRequestV2::List {
            kind: CounterListKindV2::Values,
            page: CounterPageRequestV2 {
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
        session.query(CounterQueryRequestV2::List {
            kind: CounterListKindV2::Hotspots,
            page: CounterPageRequestV2 {
                limit: 1,
                cursor: Some(cursor),
                ..Default::default()
            }
        }),
        Err(CounterQueryErrorV2::CursorQueryMismatch)
    ));
    let capture = decode_counter_capture_v2(&capture_bytes()).unwrap();
    assert!(matches!(
        session.query(CounterQueryRequestV2::List {
            kind: CounterListKindV2::Values,
            page: CounterPageRequestV2 {
                limit: 1,
                cursor: Some(cursor),
                counter_filter: Some(capture.counter_definitions[0].identity),
                dispatch_filter: None
            }
        }),
        Err(CounterQueryErrorV2::CursorQueryMismatch)
    ));
}

#[test]
fn capabilities_and_hard_response_bounds_do_not_overclaim() {
    let session =
        CounterQuerySessionV2::open(&capture_bytes(), CounterQueryLimitsV2::default()).unwrap();
    let CounterQueryResponseV2::Capabilities { capabilities, .. } =
        session.query(CounterQueryRequestV2::Capabilities).unwrap()
    else {
        panic!("expected capabilities")
    };
    assert!(capabilities.iter().any(|item| item.name
        == CounterCapabilityNameV2::DispatchCounterValues
        && item.availability == CounterCapabilityAvailabilityV2::Available));
    for name in [
        CounterCapabilityNameV2::HardwareInstanceCorrelation,
        CounterCapabilityNameV2::SourceCorrelation,
        CounterCapabilityNameV2::IsaCorrelation,
        CounterCapabilityNameV2::PcSamples,
        CounterCapabilityNameV2::AttWaveEvents,
        CounterCapabilityNameV2::ExecutionControl,
    ] {
        assert!(capabilities.iter().any(|item| item.name == name
            && item.availability == CounterCapabilityAvailabilityV2::Unavailable));
    }
    let bounded = CounterQuerySessionV2::open(
        &capture_bytes(),
        CounterQueryLimitsV2::new(MAX_COUNTER_CAPTURE_BYTES_V2, 4096, 1).unwrap(),
    )
    .unwrap();
    assert!(
        bounded
            .query_json(CounterQueryRequestV2::Capabilities)
            .unwrap()
            .len()
            <= 4096
    );
    assert!(matches!(
        bounded.query(CounterQueryRequestV2::List {
            kind: CounterListKindV2::Values,
            page: CounterPageRequestV2 {
                limit: 2,
                ..Default::default()
            }
        }),
        Err(CounterQueryErrorV2::LimitOutOfRange)
    ));
}

#[test]
fn late_value_pages_and_dispatch_pages_remain_page_bounded() {
    let mut records = String::new();
    for index in 0..5_000 {
        if index != 0 {
            records.push(',');
        }
        records.push_str(&format!(
            r#"{{"counter_id":{{"handle":101}},"value":{index}.0}}"#
        ));
    }
    let source = format!(
        r#"{{"rocprofiler-sdk-tool":[{{"buffer_records":{{}},"counters":[{{"agent_id":{{"handle":17}},"id":{{"handle":101}},"is_constant":0,"is_derived":0,"name":"SQ_WAVES"}}],"callback_records":{{"counter_collection":[{{"dispatch_data":{{"start_timestamp":1,"end_timestamp":2,"dispatch_info":{{"agent_id":{{"handle":17}},"workgroup_size":{{"x":64,"y":1,"z":1}},"grid_size":{{"x":64,"y":1,"z":1}}}}}},"records":[{records}]}}]}}}}]}}"#
    );
    let id = OpaqueIdentityV1::new([1; 32]).unwrap();
    let capture = import_rocprofv3_counter_capture_v2(
        source.as_bytes(),
        RocprofCaptureBindingV1 {
            kernel_ir_claim: KernelIrIdentityClaimV1::canonical_v7_claim(id, 97).unwrap(),
            artifact: None,
            source_map: None,
            wave_width: WaveWidthV1::Wave64,
        },
        ImportLimitsV1::default(),
    )
    .unwrap();
    let bytes = encode_counter_capture_v2(&capture).unwrap();
    let session = CounterQuerySessionV2::open(&bytes, CounterQueryLimitsV2::default()).unwrap();
    let mut cursor = None;
    let mut seen = 0;
    loop {
        let CounterQueryResponseV2::Page { page } = session
            .query(CounterQueryRequestV2::List {
                kind: CounterListKindV2::Values,
                page: CounterPageRequestV2 {
                    limit: 127,
                    cursor,
                    dispatch_filter: None,
                    counter_filter: None,
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
    let output = session
        .query_json(CounterQueryRequestV2::List {
            kind: CounterListKindV2::Dispatches,
            page: CounterPageRequestV2 {
                limit: 1,
                ..Default::default()
            },
        })
        .unwrap();
    assert!(output.len() < 4_096);
}
