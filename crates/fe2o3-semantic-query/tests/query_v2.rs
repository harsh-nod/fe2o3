mod common;

#[test]
fn v2_preserves_every_provenance_class_without_authenticating_claims() {
    let mut cases = vec![
        (
            FactProvenanceV1::Declared,
            ProvenanceFilterV1::Declared,
            "declared",
            None,
            Some(EvidenceKindV1::Declaration),
        ),
        (
            FactProvenanceV1::Proved,
            ProvenanceFilterV1::Proved,
            "proved",
            None,
            Some(EvidenceKindV1::Proof),
        ),
        (
            FactProvenanceV1::Observed,
            ProvenanceFilterV1::Observed,
            "observed",
            None,
            None,
        ),
        (
            FactProvenanceV1::Inferred,
            ProvenanceFilterV1::Inferred,
            "inferred",
            None,
            Some(EvidenceKindV1::InferenceRule),
        ),
    ];
    for (reason, label) in [
        (UnavailableReasonV1::Unsupported, "unsupported"),
        (UnavailableReasonV1::NotCaptured, "not_captured"),
        (UnavailableReasonV1::OptimizedOut, "optimized_out"),
        (
            UnavailableReasonV1::OutsideCaptureScope,
            "outside_capture_scope",
        ),
        (UnavailableReasonV1::Truncated, "truncated"),
    ] {
        cases.push((
            FactProvenanceV1::Unavailable { reason },
            ProvenanceFilterV1::Unavailable,
            "unavailable",
            Some(label),
            None,
        ));
    }
    for (provenance, filter, label, reason, evidence_kind) in cases {
        let original = sample_trace(8);
        let mut events = original.events().to_vec();
        let event = &events[2];
        let evidence = evidence_kind
            .into_iter()
            .map(|kind| EvidenceRefV1::new(kind, identity(77)))
            .collect();
        events[2] = TraceEventV1::new(
            event.sequence(),
            event.timestamp(),
            provenance,
            event.scope(),
            event.site(),
            event.kind(),
            evidence,
        )
        .unwrap();
        let trace = TraceV1::new(original.header().clone(), events).unwrap();
        let request = QueryRequestV1::Page {
            kind: PageKindV1::ProvenanceAndEvidence,
            page: PageRequestV1::new(None, 1),
            filter: QueryFilterV1 {
                sequence_start: Some(2),
                sequence_end: Some(3),
                provenance: Some(filter),
                ..Default::default()
            },
        };
        let QueryResponseV1::Page { page: expected } =
            TraceQuerySessionV1::from_trace(trace.clone(), QueryLimitsV1::default())
                .unwrap()
                .query(request)
                .unwrap()
        else {
            panic!("page")
        };
        for version in VERSIONS {
            let session = TraceQuerySessionV2::from_trace(
                versioned_trace(&trace, version),
                QueryLimitsV1::default(),
            )
            .unwrap();
            let QueryResponseV1::Page { page } = session.query(request).unwrap() else {
                panic!("page")
            };
            assert_eq!(page.returned, 1);
            assert_eq!(page.items, expected.items);
            assert!(!page.context.kernel_ir.authenticated);
            let QueryItemV1::ProvenanceAndEvidence { event, .. } = &page.items[0] else {
                panic!("evidence")
            };
            assert_eq!(event.provenance.kind, label);
            assert_eq!(event.provenance.unavailable_reason, reason);
            assert_eq!(event.evidence.len(), usize::from(evidence_kind.is_some()));
            if !event.evidence.is_empty() {
                assert_eq!(event.evidence[0].identity.bytes, [77; 32]);
            }
        }
    }
}

#[test]
fn cursor_binding_includes_each_exact_kir_claim_field() {
    for version in VERSIONS {
        let bytes = encoded_trace_v2(8, version);
        let session = TraceQuerySessionV2::open(&bytes, QueryLimitsV1::default()).unwrap();
        let QueryResponseV1::Page { page } = session
            .query(page_request(PageKindV1::Sites, None, 1))
            .unwrap()
        else {
            panic!("page")
        };
        let request = page_request(PageKindV1::Sites, page.next_cursor, 2);
        for offset in [74, 106] {
            let mut changed = bytes.clone();
            changed[offset] ^= 1;
            let other = TraceQuerySessionV2::open(&changed, QueryLimitsV1::default()).unwrap();
            let response = other.query(QueryRequestV1::DispatchSummary).unwrap();
            assert_ne!(context(&response).trace_binding, page.context.trace_binding);
            assert!(matches!(
                other.query(request),
                Err(QueryErrorV1::CursorQueryMismatch)
            ));
        }
    }
}

#[test]
fn frozen_v1_responses_match_pristine_dc8_bytes() {
    let session = TraceQuerySessionV1::open(&encoded_trace(8), QueryLimitsV1::default()).unwrap();
    assert_eq!(
        session.query_json(QueryRequestV1::DispatchSummary).unwrap(),
        include_bytes!("golden/v1-summary.json")
    );
    assert_eq!(
        session
            .query_json(page_request(PageKindV1::Sites, None, 1))
            .unwrap(),
        include_bytes!("golden/v1-sites.json")
    );
}

#[test]
fn optional_claims_and_maximum_inert_length_are_preserved_without_authentication() {
    let trace = sample_trace(8);
    let h = trace.header();
    let mir = ContentIdentityV1::new(
        ContentIdentitySchemeV1::RawCanonicalSha256,
        3,
        identity(51),
        12,
    )
    .unwrap();
    let lineage = ContentIdentityV1::new(
        ContentIdentitySchemeV1::DomainSeparatedSha256,
        2,
        identity(52),
        13,
    )
    .unwrap();
    let artifact = ContentIdentityV1::new(
        ContentIdentitySchemeV1::RawCanonicalSha256,
        1,
        identity(53),
        14,
    )
    .unwrap();
    for version in VERSIONS {
        let header = TraceHeaderV2::new(
            h.producer().clone(),
            h.execution_kind(),
            KernelIrIdentityClaimV2::exact_canonical_claim(version, identity(50), u64::MAX)
                .unwrap(),
            Some(mir),
            Some(lineage),
            Some(artifact),
            h.dispatch(),
            h.launch(),
            h.bounds(),
            h.completeness(),
            h.boundaries(),
        )
        .unwrap();
        let trace = TraceEnvelopeV2::new(header, trace.events().to_vec()).unwrap();
        let bytes = encode_trace_v2(&trace).unwrap();
        let session = TraceQuerySessionV2::open(&bytes, QueryLimitsV1::default()).unwrap();
        let response = session.query(QueryRequestV1::DispatchSummary).unwrap();
        let context = context(&response);
        assert_eq!(context.input_bytes, bytes.len() as u64);
        assert_eq!(context.kernel_ir.wire_version, version.as_u16());
        assert_eq!(
            context.kernel_ir.identity_policy,
            KERNEL_IR_IDENTITY_POLICY_V1
        );
        assert_eq!(context.kernel_ir.digest.bytes, [50; 32]);
        assert_eq!(context.kernel_ir.canonical_len, u64::MAX);
        assert!(!context.kernel_ir.authenticated);
        for (actual, expected, scheme) in [
            (context.semantic_mir.unwrap(), mir, "raw_canonical_sha256"),
            (context.lineage.unwrap(), lineage, "domain_separated_sha256"),
            (context.artifact.unwrap(), artifact, "raw_canonical_sha256"),
        ] {
            assert_eq!(actual.scheme, scheme);
            assert_eq!(actual.format_version, expected.format_version());
            assert_eq!(&actual.digest.bytes, expected.digest().as_bytes());
            assert_eq!(actual.canonical_len, expected.canonical_len());
            assert!(!actual.authenticated);
        }
    }
}

#[test]
fn v2_filters_preserve_tail_coordinates_and_never_invent_observations() {
    let original = sample_trace(8);
    let h = original.header();
    let launch =
        LaunchGeometryV1::new_exact([166, 1, 1], [2, 1, 1], [96, 1, 1], WaveWidthV1::Wave64)
            .unwrap();
    for version in VERSIONS {
        let header = TraceHeaderV2::new(
            h.producer().clone(),
            h.execution_kind(),
            KernelIrIdentityClaimV2::exact_canonical_claim(
                version,
                h.kernel_ir_claim().digest(),
                4096,
            )
            .unwrap(),
            None,
            None,
            None,
            h.dispatch(),
            launch,
            h.bounds(),
            h.completeness(),
            h.boundaries(),
        )
        .unwrap();
        let events = original
            .events()
            .iter()
            .map(|event| {
                let scope = if matches!(event.scope().level(), ExecutionLevelV1::Lane { .. }) {
                    ExecutionScopeV1::lane(
                        h.dispatch(),
                        [1, 0, 0],
                        1,
                        5,
                        [165, 0, 0],
                        ActiveMaskV1::new(WaveWidthV1::Wave64, 0x3f).unwrap(),
                    )
                } else {
                    event.scope()
                };
                TraceEventV1::new(
                    event.sequence(),
                    event.timestamp(),
                    event.provenance(),
                    scope,
                    event.site(),
                    event.kind(),
                    event.evidence_refs().to_vec(),
                )
                .unwrap()
            })
            .collect();
        let trace = TraceEnvelopeV2::new(header, events).unwrap();
        let session = TraceQuerySessionV2::from_trace(trace, QueryLimitsV1::default()).unwrap();
        for (group, lane, expected) in [(1, 5, 11), (0, 5, 0), (1, 6, 0)] {
            let QueryResponseV1::Page { page } = session
                .query(QueryRequestV1::Page {
                    kind: PageKindV1::Lanes,
                    page: PageRequestV1::new(None, 64),
                    filter: QueryFilterV1 {
                        workgroup: Some([group, 0, 0]),
                        wave: Some(1),
                        lane: Some(lane),
                        ..Default::default()
                    },
                })
                .unwrap()
            else {
                panic!("page")
            };
            assert_eq!(page.returned, expected);
            for item in page.items {
                let QueryItemV1::ScopeObservation { event, .. } = item else {
                    panic!("scope")
                };
                assert_eq!(event.scope.logical_workitem, Some([165, 0, 0]));
                assert_eq!(event.scope.active_mask, Some(0x3f));
                assert_eq!(event.scope.wave_width, Some(64));
            }
        }
        for (kind, filter, count) in [
            (
                PageKindV1::Sites,
                QueryFilterV1 {
                    function_ordinal: Some(0),
                    block_ordinal: Some(3),
                    operation_ordinal: Some(4),
                    ..Default::default()
                },
                7,
            ),
            (
                PageKindV1::MemoryAccesses,
                QueryFilterV1 {
                    allocation: Some((2, 0)),
                    memory_access: Some(MemoryAccessFilterV1::Write),
                    ..Default::default()
                },
                1,
            ),
            (
                PageKindV1::ProvenanceAndEvidence,
                QueryFilterV1 {
                    sequence_start: Some(3),
                    sequence_end: Some(4),
                    provenance: Some(ProvenanceFilterV1::Observed),
                    evidence_kind: Some(EvidenceKindFilterV1::Artifact),
                    ..Default::default()
                },
                1,
            ),
        ] {
            let QueryResponseV1::Page { page } = session
                .query(QueryRequestV1::Page {
                    kind,
                    page: PageRequestV1::new(None, 64),
                    filter,
                })
                .unwrap()
            else {
                panic!("page")
            };
            assert_eq!(page.returned, count);
        }
    }
}

use common::*;
use fe2o3_semantic_query::*;
use fe2o3_semantic_trace::*;
use serde_json::Value;

const VERSIONS: [KernelIrWireVersionV2; 2] =
    [KernelIrWireVersionV2::V9, KernelIrWireVersionV2::V10];
const KINDS: [PageKindV1; 9] = [
    PageKindV1::Workgroups,
    PageKindV1::Waves,
    PageKindV1::Lanes,
    PageKindV1::Sites,
    PageKindV1::OperationOccurrences,
    PageKindV1::MemoryAccesses,
    PageKindV1::MemoryRegions,
    PageKindV1::Faults,
    PageKindV1::ProvenanceAndEvidence,
];

fn context(response: &QueryResponseV1) -> &TraceContextViewV1 {
    match response {
        QueryResponseV1::Capabilities { context, .. }
        | QueryResponseV1::DispatchSummary { context, .. }
        | QueryResponseV1::PlanNextCapture { context, .. }
        | QueryResponseV1::DiagnosisStatus { context, .. } => context,
        QueryResponseV1::Page { page } => &page.context,
    }
}

fn page_request(kind: PageKindV1, cursor: Option<QueryCursorV1>, limit: u16) -> QueryRequestV1 {
    QueryRequestV1::Page {
        kind,
        page: PageRequestV1::new(cursor, limit),
        filter: QueryFilterV1::default(),
    }
}

fn requests() -> Vec<QueryRequestV1> {
    let mut requests = vec![
        QueryRequestV1::Capabilities,
        QueryRequestV1::DispatchSummary,
    ];
    for goal in [
        CaptureGoalV1::MemoryFault,
        CaptureGoalV1::BarrierDivergence,
        CaptureGoalV1::PerformanceHotspot,
        CaptureGoalV1::CorrectnessMismatch,
    ] {
        requests.push(QueryRequestV1::PlanNextCapture { goal });
        requests.push(QueryRequestV1::DiagnosisStatus { goal });
    }
    for kind in KINDS {
        requests.push(page_request(kind, None, 64));
    }
    requests
}

// Only the independently checked envelope binding changes in shared plan references.
fn replace_binding(value: &mut Value, before: &Value, after: &Value) {
    if value == before {
        *value = after.clone();
    } else {
        match value {
            Value::Object(fields) => {
                for value in fields.values_mut() {
                    replace_binding(value, before, after);
                }
            }
            Value::Array(values) => {
                for value in values {
                    replace_binding(value, before, after);
                }
            }
            _ => {}
        }
    }
}

#[test]
fn both_versions_preserve_every_query_and_capture_contract() {
    for trace in [
        sample_trace(8),
        memory_fault_trace(8),
        inferred_memory_fault_trace(8),
        truncated_trace(8),
        sparse_att_trace(8),
        rocprof_dispatch_trace(8),
        fully_paired_barrier_trace(8),
        mixed_provenance_barrier_trace(8),
    ] {
        let v1 = TraceQuerySessionV1::from_trace(trace.clone(), QueryLimitsV1::default()).unwrap();
        for version in VERSIONS {
            let v2 = TraceQuerySessionV2::from_trace(versioned_trace(&trace, version), v1.limits())
                .unwrap();
            assert_eq!(v1.limits(), v2.limits());
            for request in requests() {
                let original = v1.query(request).unwrap();
                let actual = v2.query(request).unwrap();
                let mut expected_context = context(&original).clone();
                expected_context.schema = QUERY_SCHEMA_V2;
                expected_context.kernel_ir.wire_version = version.as_u16();
                expected_context.trace_binding = context(&actual).trace_binding;
                expected_context.input_bytes = context(&actual).input_bytes;
                assert_eq!(&expected_context, context(&actual));
                assert_ne!(
                    context(&original).trace_binding,
                    context(&actual).trace_binding
                );
                assert!(!context(&actual).kernel_ir.authenticated);
                let mut expected = serde_json::to_value(&original).unwrap();
                replace_binding(
                    &mut expected,
                    &serde_json::to_value(context(&original).trace_binding).unwrap(),
                    &serde_json::to_value(context(&actual).trace_binding).unwrap(),
                );
                let context_slot = if expected.get("page").is_some() {
                    &mut expected["page"]["context"]
                } else {
                    &mut expected["context"]
                };
                *context_slot = serde_json::to_value(expected_context).unwrap();
                assert_eq!(
                    expected,
                    serde_json::to_value(&actual).unwrap(),
                    "{version:?} {request:?}"
                );
                assert_eq!(
                    v2.query_json(request).unwrap(),
                    v2.query_json(request).unwrap()
                );
            }
        }
    }
}

#[test]
fn v2_pages_are_gap_free_and_cursors_bind_version_capture_kind_and_filters() {
    for version in VERSIONS {
        let session =
            TraceQuerySessionV2::open(&encoded_trace_v2(8, version), QueryLimitsV1::default())
                .unwrap();
        for kind in KINDS {
            let QueryResponseV1::Page { page: expected } =
                session.query(page_request(kind, None, 64)).unwrap()
            else {
                panic!("expected page")
            };
            let mut cursor = None;
            let mut items = Vec::new();
            for index in 0..64 {
                let QueryResponseV1::Page { page } = session
                    .query(page_request(kind, cursor, 1 + index % 3))
                    .unwrap()
                else {
                    panic!("expected page")
                };
                items.extend(page.items);
                cursor = page.next_cursor;
                if cursor.is_none() {
                    break;
                }
            }
            assert!(cursor.is_none());
            assert_eq!(items, expected.items, "{kind:?}");
        }

        let request = page_request(PageKindV1::Sites, None, 1);
        let QueryResponseV1::Page { page } = session.query(request).unwrap() else {
            panic!("page")
        };
        let cursor = page.next_cursor.expect("more site events");
        let request = page_request(PageKindV1::Sites, Some(cursor), 2);
        let other = if version == KernelIrWireVersionV2::V9 {
            KernelIrWireVersionV2::V10
        } else {
            KernelIrWireVersionV2::V9
        };
        for bytes in [encoded_trace_v2(8, other), encoded_trace_v2(9, version)] {
            let other = TraceQuerySessionV2::open(&bytes, session.limits()).unwrap();
            assert!(matches!(
                other.query(request),
                Err(QueryErrorV1::CursorQueryMismatch)
            ));
        }
        let v1 = TraceQuerySessionV1::open(&encoded_trace(8), session.limits()).unwrap();
        assert!(matches!(
            v1.query(request),
            Err(QueryErrorV1::CursorQueryMismatch)
        ));
        let QueryResponseV1::Page { page } =
            v1.query(page_request(PageKindV1::Sites, None, 1)).unwrap()
        else {
            panic!("page")
        };
        assert!(matches!(
            session.query(page_request(PageKindV1::Sites, page.next_cursor, 2)),
            Err(QueryErrorV1::CursorQueryMismatch)
        ));
        assert!(matches!(
            session.query(page_request(PageKindV1::Lanes, Some(cursor), 2)),
            Err(QueryErrorV1::CursorQueryMismatch)
        ));
        assert!(matches!(
            session.query(QueryRequestV1::Page {
                kind: PageKindV1::Sites,
                page: PageRequestV1::new(Some(cursor), 2),
                filter: QueryFilterV1 {
                    workgroup: Some([1, 0, 0]),
                    ..Default::default()
                },
            }),
            Err(QueryErrorV1::CursorQueryMismatch)
        ));
    }
}

#[test]
fn version_specific_admission_rejects_relabeling_and_malformed_bytes() {
    for version in VERSIONS {
        let bytes = encoded_trace_v2(8, version);
        assert!(TraceQuerySessionV1::open(&bytes, QueryLimitsV1::default()).is_err());
        assert!(TraceQuerySessionV2::open(&encoded_trace(8), QueryLimitsV1::default()).is_err());
        // This fixture's exact producer layout places the 44-byte KIR claim here.
        let claim = 70;
        assert_eq!(&bytes[claim..claim + 2], &version.as_u16().to_le_bytes());
        assert_eq!(&bytes[claim + 4..claim + 36], &[10; 32]);
        for wire in [7_u16, 11, 12, u16::MAX] {
            let mut changed = bytes.clone();
            changed[claim..claim + 2].copy_from_slice(&wire.to_le_bytes());
            assert!(matches!(
                TraceQuerySessionV2::open(&changed, QueryLimitsV1::default()),
                Err(QueryErrorV1::TraceDecode(
                    TraceDecodeErrorV1::UnsupportedKernelIrClaim { .. }
                ))
            ));
        }
        for policy in [0_u16, 2, u16::MAX] {
            let mut changed = bytes.clone();
            changed[claim + 2..claim + 4].copy_from_slice(&policy.to_le_bytes());
            assert!(TraceQuerySessionV2::open(&changed, QueryLimitsV1::default()).is_err());
        }
        for schema in [1_u16, 3] {
            let mut changed = bytes.clone();
            changed[8..10].copy_from_slice(&schema.to_le_bytes());
            assert!(TraceQuerySessionV2::open(&changed, QueryLimitsV1::default()).is_err());
        }
        for range in [claim + 4..claim + 36, claim + 36..claim + 44] {
            let mut changed = bytes.clone();
            changed[range].fill(0);
            assert!(TraceQuerySessionV2::open(&changed, QueryLimitsV1::default()).is_err());
        }
        for end in [
            0,
            7,
            8,
            9,
            claim + 1,
            claim + 3,
            claim + 35,
            claim + 43,
            bytes.len() - 1,
        ] {
            assert!(TraceQuerySessionV2::open(&bytes[..end], QueryLimitsV1::default()).is_err());
        }
        let mut trailing = bytes.clone();
        trailing.push(0);
        assert!(matches!(
            TraceQuerySessionV2::open(&trailing, QueryLimitsV1::default()),
            Err(QueryErrorV1::TraceDecode(
                TraceDecodeErrorV1::TrailingBytes(1)
            ))
        ));
        let mut relabeled = bytes;
        relabeled[..8].copy_from_slice(b"FE2O3TR1");
        relabeled[8..10].copy_from_slice(&1_u16.to_le_bytes());
        assert!(TraceQuerySessionV1::open(&relabeled, QueryLimitsV1::default()).is_err());
    }
}

#[test]
fn v2_input_page_and_output_limits_remain_enforced() {
    let bytes = encoded_trace_v2(8, KernelIrWireVersionV2::V10);
    let limits =
        QueryLimitsV1::new(bytes.len() as u64 - 1, 1, MIN_QUERY_RESPONSE_BYTES_V1).unwrap();
    assert!(matches!(
        TraceQuerySessionV2::open(&bytes, limits),
        Err(QueryErrorV1::InputTooLarge { .. })
    ));
    assert!(matches!(
        TraceQuerySessionV2::from_trace(
            versioned_trace(&sample_trace(8), KernelIrWireVersionV2::V10),
            limits
        ),
        Err(QueryErrorV1::InputTooLarge { .. })
    ));
    let limits = QueryLimitsV1::new(bytes.len() as u64, 2, MIN_QUERY_RESPONSE_BYTES_V1).unwrap();
    let session = TraceQuerySessionV2::open(&bytes, limits).unwrap();
    assert!(matches!(
        session.query(page_request(PageKindV1::Sites, None, 0)),
        Err(QueryErrorV1::PageLimitOutOfRange { .. })
    ));
    assert!(matches!(
        session.query(page_request(PageKindV1::Sites, None, 3)),
        Err(QueryErrorV1::PageLimitOutOfRange { .. })
    ));
    assert!(matches!(
        session.query(page_request(PageKindV1::Sites, None, 2)),
        Err(QueryErrorV1::PageExceedsResponseBudget { .. })
    ));
    let mut response = session
        .query(page_request(PageKindV1::Sites, None, 1))
        .unwrap();
    if let QueryResponseV1::Page { page } = &mut response {
        page.context.producer.name = "x".repeat(MIN_QUERY_RESPONSE_BYTES_V1 as usize);
    }
    assert!(matches!(
        session.encode_json(&response),
        Err(QueryErrorV1::ResponseTooLarge { .. })
    ));
}
