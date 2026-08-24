mod common;

use common::*;
use fe2o3_semantic_trace::*;

#[test]
fn every_v1_event_family_round_trips_canonically() {
    let trace = sample_trace();
    let first = encode_trace_v1(&trace).unwrap();
    let decoded = decode_trace_v1(&first).unwrap();
    let second = encode_trace_v1(&decoded).unwrap();

    assert_eq!(decoded, trace);
    assert_eq!(first, second);
    assert_eq!(&first[..10], b"FE2O3TR1\x01\x00");
}

#[test]
fn wrong_magic_version_unknown_tag_and_trailing_bytes_are_rejected() {
    let bytes = encode_trace_v1(&sample_trace()).unwrap();

    let mut wrong_magic = bytes.clone();
    wrong_magic[0] ^= 1;
    assert_eq!(
        decode_trace_v1(&wrong_magic),
        Err(TraceDecodeErrorV1::InvalidMagic)
    );

    let mut wrong_version = bytes.clone();
    wrong_version[8..10].copy_from_slice(&2_u16.to_le_bytes());
    assert_eq!(
        decode_trace_v1(&wrong_version),
        Err(TraceDecodeErrorV1::UnsupportedVersion(2))
    );

    let mut unknown_producer = bytes.clone();
    unknown_producer[10] = u8::MAX;
    assert_eq!(
        decode_trace_v1(&unknown_producer),
        Err(TraceDecodeErrorV1::UnknownTag {
            context: "producer kind",
            tag: u8::MAX,
        })
    );

    let mut trailing = bytes;
    trailing.push(0);
    assert_eq!(
        decode_trace_v1(&trailing),
        Err(TraceDecodeErrorV1::TrailingBytes(1))
    );
}

#[test]
fn truncation_is_explicit_and_emitted_count_is_exact() {
    let events = sample_events();
    let completeness = TraceCompletenessV1::Truncated {
        reason: TruncationReasonV1::CollectorLoss,
        emitted_events: events.len() as u64,
        dropped_events: DroppedEventCountV1::Unknown,
    };
    let trace = TraceV1::new(header(completeness), events).unwrap();
    assert_eq!(
        decode_trace_v1(&encode_trace_v1(&trace).unwrap()).unwrap(),
        trace
    );

    let error = TraceV1::new(
        header(TraceCompletenessV1::Truncated {
            reason: TruncationReasonV1::EventLimit,
            emitted_events: 99,
            dropped_events: DroppedEventCountV1::Known(1),
        }),
        vec![],
    )
    .unwrap_err();
    assert_eq!(
        error,
        TraceValidationErrorV1::TruncatedEventCountMismatch {
            declared: 99,
            actual: 0,
        }
    );
}

#[test]
fn encoder_enforces_declared_byte_limit_before_unbounded_growth() {
    let tiny_bounds = TraceBoundsV1::new(1, 16, 1).unwrap();
    let baseline = header(TraceCompletenessV1::Complete);
    let header = TraceHeaderV1::new(
        baseline.producer().clone(),
        baseline.execution_kind(),
        baseline.kernel_ir_claim(),
        baseline.semantic_mir(),
        baseline.lineage(),
        baseline.artifact(),
        baseline.dispatch(),
        baseline.launch(),
        tiny_bounds,
        TraceCompletenessV1::Truncated {
            reason: TruncationReasonV1::ByteLimit,
            emitted_events: 1,
            dropped_events: DroppedEventCountV1::Unknown,
        },
        CaptureBoundariesV1::new(
            CaptureStartBoundaryV1::DispatchAlreadyActive,
            CaptureEndBoundaryV1::DispatchContinuesAfterCapture,
        ),
    )
    .unwrap();
    let trace = TraceV1::new(
        header,
        vec![observed_event(
            0,
            dispatch_scope(),
            None,
            TraceEventKindV1::Allocation(AllocationEventV1::Preexisting {
                allocation: TraceAllocationIdV1::new(99, 0).unwrap(),
                byte_len: 4,
                address_space: AddressSpaceV1::Global,
            }),
            vec![],
        )],
    )
    .unwrap();
    assert!(matches!(
        encode_trace_v1(&trace),
        Err(TraceEncodeErrorV1::EncodedLengthExceedsLimit { max: 16, .. })
    ));
}

#[test]
fn noncanonical_evidence_order_is_rejected_instead_of_silently_normalized() {
    let mut bytes = encode_trace_v1(&sample_trace()).unwrap();
    let mut first = vec![evidence_kind_tag_for_test(EvidenceKindV1::Artifact)];
    first.extend_from_slice(identity(21).as_bytes());
    let mut second = vec![evidence_kind_tag_for_test(EvidenceKindV1::Artifact)];
    second.extend_from_slice(identity(22).as_bytes());
    let first_at = find_subsequence(&bytes, &first);
    let second_at = find_subsequence(&bytes, &second);
    assert_eq!(second_at, first_at + first.len());
    bytes[first_at..first_at + first.len()].copy_from_slice(&second);
    bytes[second_at..second_at + second.len()].copy_from_slice(&first);

    assert_eq!(
        decode_trace_v1(&bytes),
        Err(TraceDecodeErrorV1::NonCanonicalEncoding)
    );
}

fn evidence_kind_tag_for_test(kind: EvidenceKindV1) -> u8 {
    match kind {
        EvidenceKindV1::Declaration => 0,
        EvidenceKindV1::Proof => 1,
        EvidenceKindV1::InferenceRule => 2,
        EvidenceKindV1::RuntimeObservation => 3,
        EvidenceKindV1::Artifact => 4,
    }
}

#[test]
fn impossible_event_count_is_rejected_before_allocation() {
    let mut bytes = encode_trace_v1(&sample_trace()).unwrap();
    let declared = (sample_trace().events().len() as u64).to_le_bytes();
    let event_count_at = find_subsequence(&bytes, &declared);
    bytes[event_count_at..event_count_at + 8].copy_from_slice(&200_u64.to_le_bytes());

    assert!(matches!(
        decode_trace_v1(&bytes),
        Err(TraceDecodeErrorV1::ImpossibleEventCount { declared: 200, .. })
    ));
}

#[test]
fn zero_kir_identity_and_short_input_are_rejected() {
    let mut bytes = encode_trace_v1(&sample_trace()).unwrap();
    let kir_digest_at = find_subsequence(&bytes, &[9; 32]);
    bytes[kir_digest_at..kir_digest_at + 32].fill(0);
    assert_eq!(
        decode_trace_v1(&bytes),
        Err(TraceDecodeErrorV1::Validation(
            TraceValidationErrorV1::ZeroIdentity
        ))
    );

    assert_eq!(
        decode_trace_v1(b"FE2O3"),
        Err(TraceDecodeErrorV1::UnexpectedEof)
    );
}

#[test]
fn large_trace_materializes_from_one_exact_count() {
    const DIAGNOSTICS: usize = 20_000;
    let bounds = TraceBoundsV1::new_with_resident(
        (DIAGNOSTICS + 2) as u64,
        4 * 1_024 * 1_024,
        32 * 1_024 * 1_024,
        1,
    )
    .unwrap();
    let baseline = header(TraceCompletenessV1::Complete);
    let header = TraceHeaderV1::new(
        baseline.producer().clone(),
        baseline.execution_kind(),
        baseline.kernel_ir_claim(),
        baseline.semantic_mir(),
        baseline.lineage(),
        baseline.artifact(),
        baseline.dispatch(),
        baseline.launch(),
        bounds,
        TraceCompletenessV1::Complete,
        CaptureBoundariesV1::FULL_DISPATCH,
    )
    .unwrap();
    let mut events = Vec::new();
    events.try_reserve_exact(DIAGNOSTICS + 2).unwrap();
    events.push(observed_event(
        0,
        dispatch_scope(),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
        vec![],
    ));
    for code in 0..DIAGNOSTICS {
        events.push(observed_event(
            events.len() as u64,
            dispatch_scope(),
            None,
            TraceEventKindV1::Diagnostic(DiagnosticEventV1::new(
                DiagnosticKindV1::Fault,
                code as u32,
            )),
            vec![],
        ));
    }
    events.push(observed_event(
        events.len() as u64,
        dispatch_scope(),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
        vec![],
    ));
    let trace = TraceV1::new(header, events).unwrap();
    let encoded = encode_trace_v1(&trace).unwrap();
    assert_eq!(decode_trace_v1(&encoded).unwrap(), trace);
}
