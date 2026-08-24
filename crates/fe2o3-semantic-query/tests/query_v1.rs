mod common;

use common::*;
use fe2o3_semantic_query::*;
use fe2o3_semantic_trace::{MAX_TRACE_BYTES_V1, TraceDecodeErrorV1};

fn session(seed: u8) -> TraceQuerySessionV1 {
    TraceQuerySessionV1::open(&encoded_trace(seed), QueryLimitsV1::default()).unwrap()
}

fn page_request(kind: PageKindV1, limit: u16, filter: QueryFilterV1) -> QueryRequestV1 {
    QueryRequestV1::Page {
        kind,
        page: PageRequestV1::new(None, limit),
        filter,
    }
}

#[test]
fn capability_discovery_is_explicit_about_absent_and_forbidden_state() {
    let QueryResponseV1::Capabilities {
        context,
        capabilities,
    } = session(8).query(QueryRequestV1::Capabilities).unwrap()
    else {
        panic!("expected capabilities")
    };
    assert_eq!(context.capture.completeness, "complete");
    assert!(!context.kernel_ir.authenticated);
    assert!(capabilities.iter().any(|capability| {
        capability.name == CapabilityNameV1::RegisterValues
            && capability.reason == Some(CapabilityUnavailableReasonV1::NotRepresentedByTraceV1)
    }));
    assert!(capabilities.iter().any(|capability| {
        capability.name == CapabilityNameV1::RawNativeAddresses
            && capability.reason == Some(CapabilityUnavailableReasonV1::ForbiddenAuthority)
    }));
    assert!(capabilities.iter().any(|capability| {
        capability.name == CapabilityNameV1::PerformancePrediction
            && capability.reason == Some(CapabilityUnavailableReasonV1::OutsideCurrentScope)
    }));
}

#[test]
fn dispatch_summary_counts_semantic_observations_without_inventing_entities() {
    let QueryResponseV1::DispatchSummary { context, summary } =
        session(8).query(QueryRequestV1::DispatchSummary).unwrap()
    else {
        panic!("expected summary")
    };
    assert_eq!(context.event_count, 13);
    assert_eq!(summary.dispatch_begin_sequence, Some(0));
    assert_eq!(summary.dispatch_end_sequence, Some(12));
    assert_eq!(summary.dispatch_outcome, Some("completed"));
    assert_eq!(summary.lane_scoped_events, 11);
    assert_eq!(summary.operation_occurrences, 1);
    assert_eq!(summary.memory_accesses, 1);
    assert_eq!(summary.unavailable_memory_events, 1);
    assert_eq!(summary.diagnostic_events, 1);
}

#[test]
fn every_page_kind_is_available_and_preserves_capture_context() {
    for kind in [
        PageKindV1::Workgroups,
        PageKindV1::Waves,
        PageKindV1::Lanes,
        PageKindV1::Sites,
        PageKindV1::OperationOccurrences,
        PageKindV1::MemoryAccesses,
        PageKindV1::MemoryRegions,
        PageKindV1::Faults,
        PageKindV1::ProvenanceAndEvidence,
    ] {
        let QueryResponseV1::Page { page } = session(8)
            .query(page_request(kind, 16, QueryFilterV1::default()))
            .unwrap()
        else {
            panic!("expected page")
        };
        assert!(page.returned > 0, "{kind:?}");
        assert_eq!(page.context.capture.completeness, "complete");
        assert!(page.items.iter().all(|item| match item {
            QueryItemV1::ScopeObservation { event, .. }
            | QueryItemV1::SemanticSite { event, .. }
            | QueryItemV1::OperationOccurrence { event, .. }
            | QueryItemV1::MemoryAccess { event, .. }
            | QueryItemV1::MemoryRegion { event, .. }
            | QueryItemV1::Fault { event, .. }
            | QueryItemV1::ProvenanceAndEvidence { event, .. } =>
                event.provenance.kind == "observed",
        }));
    }
}

#[test]
fn pagination_is_deterministic_gap_free_and_trace_bound() {
    let first_session = session(8);
    let request = page_request(PageKindV1::Sites, 2, QueryFilterV1::default());
    let first = first_session.query(request).unwrap();
    assert_eq!(first, first_session.query(request).unwrap());
    let QueryResponseV1::Page { page: first_page } = first else {
        panic!("expected page")
    };
    let cursor = first_page.next_cursor.expect("more site events");
    let QueryResponseV1::Page { page: second_page } = first_session
        .query(QueryRequestV1::Page {
            kind: PageKindV1::Sites,
            page: PageRequestV1::new(Some(cursor), 2),
            filter: QueryFilterV1::default(),
        })
        .unwrap()
    else {
        panic!("expected page")
    };
    let first_sequences: Vec<_> = first_page.items.iter().map(item_sequence).collect();
    let second_sequences: Vec<_> = second_page.items.iter().map(item_sequence).collect();
    assert_eq!(first_sequences, vec![2, 3]);
    assert_eq!(second_sequences, vec![4, 5]);

    let error = session(9)
        .query(QueryRequestV1::Page {
            kind: PageKindV1::Sites,
            page: PageRequestV1::new(Some(cursor), 2),
            filter: QueryFilterV1::default(),
        })
        .unwrap_err();
    assert!(matches!(error, QueryErrorV1::CursorQueryMismatch));

    let error = first_session
        .query(QueryRequestV1::Page {
            kind: PageKindV1::Lanes,
            page: PageRequestV1::new(Some(cursor), 2),
            filter: QueryFilterV1::default(),
        })
        .unwrap_err();
    assert!(matches!(error, QueryErrorV1::CursorQueryMismatch));

    let error = first_session
        .query(QueryRequestV1::Page {
            kind: PageKindV1::Sites,
            page: PageRequestV1::new(Some(cursor), 2),
            filter: QueryFilterV1 {
                sequence_start: Some(4),
                ..QueryFilterV1::default()
            },
        })
        .unwrap_err();
    assert!(matches!(error, QueryErrorV1::CursorQueryMismatch));
}

#[test]
fn conjunctive_filters_select_exact_lane_site_allocation_and_evidence() {
    let filter = QueryFilterV1 {
        sequence_start: Some(3),
        sequence_end: Some(6),
        workgroup: Some([1, 0, 0]),
        wave: Some(1),
        lane: Some(5),
        function_ordinal: Some(0),
        block_ordinal: Some(3),
        operation_ordinal: Some(4),
        allocation: Some((2, 0)),
        memory_access: Some(MemoryAccessFilterV1::Write),
        provenance: Some(ProvenanceFilterV1::Observed),
        evidence_kind: None,
    };
    let QueryResponseV1::Page { page } = session(8)
        .query(page_request(PageKindV1::MemoryAccesses, 4, filter))
        .unwrap()
    else {
        panic!("expected page")
    };
    assert_eq!(page.returned, 1);
    assert_eq!(item_sequence(&page.items[0]), 5);
    assert!(matches!(
        &page.items[0],
        QueryItemV1::MemoryAccess {
            outcome: "unavailable",
            unavailable_reason: Some("not_captured"),
            ..
        }
    ));

    let QueryResponseV1::Page { page } = session(8)
        .query(page_request(
            PageKindV1::ProvenanceAndEvidence,
            4,
            QueryFilterV1 {
                evidence_kind: Some(EvidenceKindFilterV1::Artifact),
                ..QueryFilterV1::default()
            },
        ))
        .unwrap()
    else {
        panic!("expected page")
    };
    assert_eq!(page.returned, 1);
    assert_eq!(item_sequence(&page.items[0]), 3);
}

#[test]
fn hostile_limits_and_invalid_filters_fail_before_query_allocation() {
    let limits = QueryLimitsV1::new(8, 16, 64 * 1024).unwrap();
    let error = TraceQuerySessionV1::open(&[0_u8; 9], limits).unwrap_err();
    assert!(matches!(
        error,
        QueryErrorV1::InputTooLarge { actual: 9, max: 8 }
    ));

    let error = session(8)
        .query(QueryRequestV1::Page {
            kind: PageKindV1::Lanes,
            page: PageRequestV1::new(None, 129),
            filter: QueryFilterV1::default(),
        })
        .unwrap_err();
    assert!(matches!(error, QueryErrorV1::PageLimitOutOfRange { .. }));

    let error = session(8)
        .query(page_request(
            PageKindV1::Lanes,
            1,
            QueryFilterV1 {
                workgroup: Some([2, 0, 0]),
                ..QueryFilterV1::default()
            },
        ))
        .unwrap_err();
    assert!(matches!(error, QueryErrorV1::WorkgroupOutsideLaunch { .. }));
}

#[test]
fn response_budget_is_enforced_conservatively_and_exact_json_stays_bounded() {
    let trace = encoded_trace(8);
    let limits = QueryLimitsV1::new(MAX_TRACE_BYTES_V1, 8, MIN_QUERY_RESPONSE_BYTES_V1).unwrap();
    let session = TraceQuerySessionV1::open(&trace, limits).unwrap();
    let bytes = session.query_json(QueryRequestV1::Capabilities).unwrap();
    assert!(u64::try_from(bytes.len()).unwrap() <= MIN_QUERY_RESPONSE_BYTES_V1);
    assert_eq!(bytes.last(), Some(&b'\n'));

    let error = session
        .query(page_request(PageKindV1::Lanes, 2, QueryFilterV1::default()))
        .unwrap_err();
    assert!(matches!(
        error,
        QueryErrorV1::PageExceedsResponseBudget { .. }
    ));
}

#[test]
fn malformed_canonical_input_is_rejected_by_trace_validation() {
    let error = TraceQuerySessionV1::open(b"not-a-trace", QueryLimitsV1::default()).unwrap_err();
    assert!(matches!(
        error,
        QueryErrorV1::TraceDecode(TraceDecodeErrorV1::InvalidMagic)
    ));
}

fn item_sequence(item: &QueryItemV1) -> u64 {
    match item {
        QueryItemV1::ScopeObservation { event, .. }
        | QueryItemV1::SemanticSite { event, .. }
        | QueryItemV1::OperationOccurrence { event, .. }
        | QueryItemV1::MemoryAccess { event, .. }
        | QueryItemV1::MemoryRegion { event, .. }
        | QueryItemV1::Fault { event, .. }
        | QueryItemV1::ProvenanceAndEvidence { event, .. } => event.sequence,
    }
}
