#![allow(dead_code)]

use fe2o3_semantic_trace::*;

pub fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).expect("fixture identity is nonzero")
}

pub fn evidence(kind: EvidenceKindV1, byte: u8) -> EvidenceRefV1 {
    EvidenceRefV1::new(kind, identity(byte))
}

pub fn dispatch() -> DispatchIdentityV1 {
    DispatchIdentityV1::new(DispatchIdentityDomainV1::TraceLocal, identity(8))
}

pub fn occurrence(value: u64) -> OperationOccurrenceIdV1 {
    OperationOccurrenceIdV1::new(1, value).unwrap()
}

pub fn bounds() -> TraceBoundsV1 {
    TraceBoundsV1::new(256, 64 * 1024, 8).expect("fixture bounds are valid")
}

pub fn header(completeness: TraceCompletenessV1) -> TraceHeaderV1 {
    TraceHeaderV1::new(
        ProducerIdentityV1::new(
            ProducerKindV1::CpuKirSimulator,
            ProducerTextV1::new("fe2o3-kir-sim").unwrap(),
            ProducerTextV1::new("0.1.0-test").unwrap(),
            Some(identity(6)),
        ),
        ExecutionKindV1::CpuKirSimulation,
        KernelIrIdentityClaimV1::canonical_v6_claim(identity(9), 4_096).unwrap(),
        Some(
            ContentIdentityV1::new(
                ContentIdentitySchemeV1::RawCanonicalSha256,
                3,
                identity(10),
                2_048,
            )
            .unwrap(),
        ),
        Some(
            ContentIdentityV1::new(
                ContentIdentitySchemeV1::DomainSeparatedSha256,
                4,
                identity(11),
                1_024,
            )
            .unwrap(),
        ),
        None,
        dispatch(),
        LaunchGeometryV1::new([2, 1, 1], [96, 1, 1], WaveWidthV1::Wave64).unwrap(),
        bounds(),
        completeness,
        CaptureBoundariesV1::FULL_DISPATCH,
    )
    .unwrap()
}

pub fn dispatch_scope() -> ExecutionScopeV1 {
    ExecutionScopeV1::dispatch(dispatch())
}

pub fn lane_scope() -> ExecutionScopeV1 {
    ExecutionScopeV1::lane(
        dispatch(),
        [1, 0, 0],
        1,
        5,
        [165, 0, 0],
        ActiveMaskV1::new(WaveWidthV1::Wave64, u64::from(u32::MAX)).unwrap(),
    )
}

pub fn observed_event(
    sequence: u64,
    scope: ExecutionScopeV1,
    site: Option<KirSiteClaimV1>,
    kind: TraceEventKindV1,
    evidence_refs: Vec<EvidenceRefV1>,
) -> TraceEventV1 {
    TraceEventV1::new(
        sequence,
        TimestampV1::LogicalStep(sequence),
        FactProvenanceV1::Observed,
        scope,
        site,
        kind,
        evidence_refs,
    )
    .unwrap()
}

pub fn sample_events() -> Vec<TraceEventV1> {
    let dispatch_scope = dispatch_scope();
    let lane_scope = lane_scope();
    let operation_site = KirSiteClaimV1::new(0, 3, KirSitePointV1::Operation(4));
    vec![
        observed_event(
            0,
            dispatch_scope,
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
            vec![],
        ),
        observed_event(
            1,
            lane_scope,
            None,
            TraceEventKindV1::Invocation(InvocationEventV1::Begin),
            vec![],
        ),
        observed_event(
            2,
            lane_scope,
            Some(KirSiteClaimV1::new(0, 3, KirSitePointV1::BlockEntry)),
            TraceEventKindV1::BlockEnter,
            vec![],
        ),
        observed_event(
            3,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Operation(OperationEventV1::Begin(occurrence(1))),
            vec![
                evidence(EvidenceKindV1::Artifact, 22),
                evidence(EvidenceKindV1::Artifact, 21),
            ],
        ),
        observed_event(
            4,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Allocation(AllocationEventV1::Create {
                allocation: TraceAllocationIdV1::new(2, 0).unwrap(),
                byte_len: 512,
                address_space: AddressSpaceV1::Workgroup,
            }),
            vec![],
        ),
        observed_event(
            5,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Memory(
                MemoryEventV1::new(
                    MemoryAccessKindV1::Write,
                    TraceAllocationIdV1::new(2, 0).unwrap(),
                    20,
                    4,
                    AddressSpaceV1::Workgroup,
                    MemoryOutcomeV1::Completed,
                )
                .unwrap(),
            ),
            vec![],
        ),
        observed_event(
            6,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Barrier(BarrierEventV1::new(
                1,
                2,
                BarrierScopeV1::Workgroup,
                BarrierActionV1::Arrive,
            )),
            vec![],
        ),
        observed_event(
            7,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Operation(OperationEventV1::End(occurrence(1))),
            vec![],
        ),
        observed_event(
            8,
            lane_scope,
            Some(KirSiteClaimV1::new(0, 3, KirSitePointV1::Terminator)),
            TraceEventKindV1::Branch {
                target_block_ordinal: 4,
            },
            vec![],
        ),
        observed_event(
            9,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Diagnostic(DiagnosticEventV1::new(DiagnosticKindV1::Assert, 17)),
            vec![],
        ),
        observed_event(
            10,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Allocation(AllocationEventV1::Release {
                allocation: TraceAllocationIdV1::new(2, 0).unwrap(),
            }),
            vec![],
        ),
        observed_event(
            11,
            lane_scope,
            None,
            TraceEventKindV1::Invocation(InvocationEventV1::End),
            vec![],
        ),
        observed_event(
            12,
            dispatch_scope,
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
            vec![],
        ),
    ]
}

pub fn sample_trace() -> TraceV1 {
    TraceV1::new(header(TraceCompletenessV1::Complete), sample_events()).unwrap()
}

pub fn find_subsequence(haystack: &[u8], needle: &[u8]) -> usize {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
        .expect("fixture byte pattern must be present")
}
