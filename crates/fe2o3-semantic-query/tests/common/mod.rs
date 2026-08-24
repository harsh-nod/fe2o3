#![allow(dead_code)]

use fe2o3_semantic_trace::*;

pub fn identity(byte: u8) -> OpaqueIdentityV1 {
    OpaqueIdentityV1::new([byte; 32]).unwrap()
}

pub fn sample_trace(seed: u8) -> TraceV1 {
    let dispatch =
        DispatchIdentityV1::new(DispatchIdentityDomainV1::TraceLocal, identity(seed.max(1)));
    let bounds = TraceBoundsV1::new(256, 64 * 1024, 8).unwrap();
    let header = TraceHeaderV1::new(
        ProducerIdentityV1::new(
            ProducerKindV1::CpuKirSimulator,
            ProducerTextV1::new("fe2o3-query-test").unwrap(),
            ProducerTextV1::new("0.1.0").unwrap(),
            Some(identity(seed.wrapping_add(1).max(1))),
        ),
        ExecutionKindV1::CpuKirSimulation,
        KernelIrIdentityClaimV1::canonical_v6_claim(identity(seed.wrapping_add(2).max(1)), 4096)
            .unwrap(),
        None,
        None,
        None,
        dispatch,
        LaunchGeometryV1::new([2, 1, 1], [96, 1, 1], WaveWidthV1::Wave64).unwrap(),
        bounds,
        TraceCompletenessV1::Complete,
        CaptureBoundariesV1::FULL_DISPATCH,
    )
    .unwrap();
    let dispatch_scope = ExecutionScopeV1::dispatch(dispatch);
    let lane_scope = ExecutionScopeV1::lane(
        dispatch,
        [1, 0, 0],
        1,
        5,
        [165, 0, 0],
        ActiveMaskV1::new(WaveWidthV1::Wave64, u64::from(u32::MAX)).unwrap(),
    );
    let operation_site = KirSiteClaimV1::new(0, 3, KirSitePointV1::Operation(4));
    let occurrence = OperationOccurrenceIdV1::new(1, 1).unwrap();
    let allocation = TraceAllocationIdV1::new(2, 0).unwrap();
    let observed = |sequence, scope, site, kind, evidence| {
        TraceEventV1::new(
            sequence,
            TimestampV1::LogicalStep(sequence),
            FactProvenanceV1::Observed,
            scope,
            site,
            kind,
            evidence,
        )
        .unwrap()
    };
    let events = vec![
        observed(
            0,
            dispatch_scope,
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
            vec![],
        ),
        observed(
            1,
            lane_scope,
            None,
            TraceEventKindV1::Invocation(InvocationEventV1::Begin),
            vec![],
        ),
        observed(
            2,
            lane_scope,
            Some(KirSiteClaimV1::new(0, 3, KirSitePointV1::BlockEntry)),
            TraceEventKindV1::BlockEnter,
            vec![],
        ),
        observed(
            3,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Operation(OperationEventV1::Begin(occurrence)),
            vec![EvidenceRefV1::new(
                EvidenceKindV1::Artifact,
                identity(seed.wrapping_add(3).max(1)),
            )],
        ),
        observed(
            4,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Allocation(AllocationEventV1::Create {
                allocation,
                byte_len: 512,
                address_space: AddressSpaceV1::Workgroup,
            }),
            vec![],
        ),
        observed(
            5,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Memory(
                MemoryEventV1::new(
                    MemoryAccessKindV1::Write,
                    allocation,
                    20,
                    4,
                    AddressSpaceV1::Workgroup,
                    MemoryOutcomeV1::Unavailable(UnavailableReasonV1::NotCaptured),
                )
                .unwrap(),
            ),
            vec![],
        ),
        observed(
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
        observed(
            7,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Operation(OperationEventV1::End(occurrence)),
            vec![],
        ),
        observed(
            8,
            lane_scope,
            Some(KirSiteClaimV1::new(0, 3, KirSitePointV1::Terminator)),
            TraceEventKindV1::Branch {
                target_block_ordinal: 4,
            },
            vec![],
        ),
        observed(
            9,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Diagnostic(DiagnosticEventV1::new(DiagnosticKindV1::Assert, 17)),
            vec![],
        ),
        observed(
            10,
            lane_scope,
            Some(operation_site),
            TraceEventKindV1::Allocation(AllocationEventV1::Release { allocation }),
            vec![],
        ),
        observed(
            11,
            lane_scope,
            None,
            TraceEventKindV1::Invocation(InvocationEventV1::End),
            vec![],
        ),
        observed(
            12,
            dispatch_scope,
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
            vec![],
        ),
    ];
    TraceV1::new(header, events).unwrap()
}

pub fn encoded_trace(seed: u8) -> Vec<u8> {
    encode_trace_v1(&sample_trace(seed)).unwrap()
}
