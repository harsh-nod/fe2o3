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
        KernelIrIdentityClaimV1::canonical_v7_claim(identity(seed.wrapping_add(2).max(1)), 4096)
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

pub fn memory_fault_trace(seed: u8) -> TraceV1 {
    replace_memory_outcome(
        sample_trace(seed),
        FactProvenanceV1::Observed,
        MemoryOutcomeV1::Fault(MemoryFaultKindV1::OutOfBounds),
        vec![],
    )
}

pub fn inferred_memory_fault_trace(seed: u8) -> TraceV1 {
    replace_memory_outcome(
        sample_trace(seed),
        FactProvenanceV1::Inferred,
        MemoryOutcomeV1::Fault(MemoryFaultKindV1::OutOfBounds),
        vec![EvidenceRefV1::new(
            EvidenceKindV1::InferenceRule,
            identity(seed.wrapping_add(9).max(1)),
        )],
    )
}

pub fn truncated_trace(seed: u8) -> TraceV1 {
    let trace = sample_trace(seed);
    let emitted_events = trace.events().len() as u64;
    replace_header(
        &trace,
        trace.header().producer().clone(),
        ExecutionKindV1::CpuKirSimulation,
        TraceCompletenessV1::Truncated {
            reason: TruncationReasonV1::CollectorLoss,
            emitted_events,
            dropped_events: DroppedEventCountV1::Unknown,
        },
        CaptureBoundariesV1::FULL_DISPATCH,
        trace.events().to_vec(),
    )
}

pub fn sparse_att_trace(seed: u8) -> TraceV1 {
    let trace = sample_trace(seed);
    replace_header(
        &trace,
        ProducerIdentityV1::new(
            ProducerKindV1::RocprofImporter,
            ProducerTextV1::new("rocprofv3-att-manifest-import").unwrap(),
            ProducerTextV1::new("v1").unwrap(),
            None,
        ),
        ExecutionKindV1::RocprofImport,
        TraceCompletenessV1::Truncated {
            reason: TruncationReasonV1::CollectorLoss,
            emitted_events: 0,
            dropped_events: DroppedEventCountV1::Unknown,
        },
        CaptureBoundariesV1::new(
            CaptureStartBoundaryV1::DispatchAlreadyActive,
            CaptureEndBoundaryV1::DispatchContinuesAfterCapture,
        ),
        vec![],
    )
}

pub fn rocprof_dispatch_trace(seed: u8) -> TraceV1 {
    let trace = sample_trace(seed);
    let dispatch = trace.header().dispatch();
    let scope = ExecutionScopeV1::dispatch(dispatch);
    let clock = identity(seed.wrapping_add(11).max(1));
    let evidence = vec![EvidenceRefV1::new(
        EvidenceKindV1::RuntimeObservation,
        identity(seed.wrapping_add(12).max(1)),
    )];
    let events = vec![
        TraceEventV1::new(
            0,
            TimestampV1::Clock {
                domain: clock,
                ticks: 100,
            },
            FactProvenanceV1::Observed,
            scope,
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
            evidence.clone(),
        )
        .unwrap(),
        TraceEventV1::new(
            1,
            TimestampV1::Clock {
                domain: clock,
                ticks: 350,
            },
            FactProvenanceV1::Observed,
            scope,
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
            evidence,
        )
        .unwrap(),
    ];
    replace_header(
        &trace,
        ProducerIdentityV1::new(
            ProducerKindV1::RocprofImporter,
            ProducerTextV1::new("rocprofv3-json-import").unwrap(),
            ProducerTextV1::new("v1").unwrap(),
            None,
        ),
        ExecutionKindV1::RocprofImport,
        TraceCompletenessV1::Truncated {
            reason: TruncationReasonV1::CollectorLoss,
            emitted_events: 2,
            dropped_events: DroppedEventCountV1::Unknown,
        },
        CaptureBoundariesV1::FULL_DISPATCH,
        events,
    )
}

pub fn fully_paired_barrier_trace(seed: u8) -> TraceV1 {
    let dispatch =
        DispatchIdentityV1::new(DispatchIdentityDomainV1::TraceLocal, identity(seed.max(1)));
    let header = TraceHeaderV1::new(
        ProducerIdentityV1::new(
            ProducerKindV1::CpuKirSimulator,
            ProducerTextV1::new("fe2o3-query-test").unwrap(),
            ProducerTextV1::new("0.1.0").unwrap(),
            None,
        ),
        ExecutionKindV1::CpuKirSimulation,
        KernelIrIdentityClaimV1::canonical_v7_claim(identity(seed.wrapping_add(2).max(1)), 64)
            .unwrap(),
        None,
        None,
        None,
        dispatch,
        LaunchGeometryV1::new([1, 1, 1], [1, 1, 1], WaveWidthV1::Wave64).unwrap(),
        TraceBoundsV1::new(16, 16 * 1024, 2).unwrap(),
        TraceCompletenessV1::Complete,
        CaptureBoundariesV1::FULL_DISPATCH,
    )
    .unwrap();
    let dispatch_scope = ExecutionScopeV1::dispatch(dispatch);
    let lane_scope = ExecutionScopeV1::lane(
        dispatch,
        [0, 0, 0],
        0,
        0,
        [0, 0, 0],
        ActiveMaskV1::new(WaveWidthV1::Wave64, 1).unwrap(),
    );
    let site = Some(KirSiteClaimV1::new(0, 0, KirSitePointV1::Operation(0)));
    let observed = |sequence, scope, site, kind| {
        TraceEventV1::new(
            sequence,
            TimestampV1::LogicalStep(sequence),
            FactProvenanceV1::Observed,
            scope,
            site,
            kind,
            vec![],
        )
        .unwrap()
    };
    TraceV1::new(
        header,
        vec![
            observed(
                0,
                dispatch_scope,
                None,
                TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
            ),
            observed(
                1,
                lane_scope,
                None,
                TraceEventKindV1::Invocation(InvocationEventV1::Begin),
            ),
            observed(
                2,
                lane_scope,
                site,
                TraceEventKindV1::Barrier(BarrierEventV1::new(
                    0,
                    0,
                    BarrierScopeV1::Workgroup,
                    BarrierActionV1::Arrive,
                )),
            ),
            observed(
                3,
                lane_scope,
                site,
                TraceEventKindV1::Barrier(BarrierEventV1::new(
                    0,
                    0,
                    BarrierScopeV1::Workgroup,
                    BarrierActionV1::Release,
                )),
            ),
            observed(
                4,
                lane_scope,
                None,
                TraceEventKindV1::Invocation(InvocationEventV1::End),
            ),
            observed(
                5,
                dispatch_scope,
                None,
                TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
            ),
        ],
    )
    .unwrap()
}

pub fn mixed_provenance_barrier_trace(seed: u8) -> TraceV1 {
    let trace = fully_paired_barrier_trace(seed);
    let mut events = trace.events().to_vec();
    let end = &events[4];
    events[4] = TraceEventV1::new(
        end.sequence(),
        end.timestamp(),
        FactProvenanceV1::Inferred,
        end.scope(),
        None,
        TraceEventKindV1::Invocation(InvocationEventV1::End),
        vec![EvidenceRefV1::new(
            EvidenceKindV1::InferenceRule,
            identity(seed.wrapping_add(13).max(1)),
        )],
    )
    .unwrap();
    rebuild_same_header(&trace, events).unwrap()
}

pub fn duplicate_invocation_scope_error(seed: u8) -> TraceValidationErrorV1 {
    let trace = fully_paired_barrier_trace(seed);
    let begin = &trace.events()[1];
    let end = &trace.events()[4];
    let dispatch_end = &trace.events()[5];
    let duplicate = TraceEventV1::new(
        2,
        TimestampV1::LogicalStep(2),
        FactProvenanceV1::Observed,
        begin.scope(),
        None,
        TraceEventKindV1::Invocation(InvocationEventV1::Begin),
        vec![],
    )
    .unwrap();
    let resequenced_end = TraceEventV1::new(
        3,
        TimestampV1::LogicalStep(3),
        FactProvenanceV1::Observed,
        end.scope(),
        None,
        TraceEventKindV1::Invocation(InvocationEventV1::End),
        vec![],
    )
    .unwrap();
    let resequenced_dispatch_end = TraceEventV1::new(
        4,
        TimestampV1::LogicalStep(4),
        FactProvenanceV1::Observed,
        dispatch_end.scope(),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
        vec![],
    )
    .unwrap();
    rebuild_same_header(
        &trace,
        vec![
            trace.events()[0].clone(),
            begin.clone(),
            duplicate,
            resequenced_end,
            resequenced_dispatch_end,
        ],
    )
    .unwrap_err()
}

pub fn mismatched_invocation_scope_error(seed: u8) -> TraceValidationErrorV1 {
    let trace = sample_trace(seed);
    let mut events = trace.events().to_vec();
    let end = &events[11];
    let mismatched_scope = ExecutionScopeV1::lane(
        trace.header().dispatch(),
        [1, 0, 0],
        1,
        6,
        [166, 0, 0],
        ActiveMaskV1::new(WaveWidthV1::Wave64, u64::from(u32::MAX)).unwrap(),
    );
    events[11] = TraceEventV1::new(
        end.sequence(),
        end.timestamp(),
        FactProvenanceV1::Observed,
        mismatched_scope,
        None,
        TraceEventKindV1::Invocation(InvocationEventV1::End),
        vec![],
    )
    .unwrap();
    rebuild_same_header(&trace, events).unwrap_err()
}

fn replace_memory_outcome(
    trace: TraceV1,
    provenance: FactProvenanceV1,
    outcome: MemoryOutcomeV1,
    evidence: Vec<EvidenceRefV1>,
) -> TraceV1 {
    let mut events = trace.events().to_vec();
    let original = &events[5];
    let TraceEventKindV1::Memory(memory) = original.kind() else {
        panic!("sample memory event")
    };
    events[5] = TraceEventV1::new(
        original.sequence(),
        original.timestamp(),
        provenance,
        original.scope(),
        original.site(),
        TraceEventKindV1::Memory(
            MemoryEventV1::new(
                memory.kind(),
                memory.allocation(),
                if outcome == MemoryOutcomeV1::Fault(MemoryFaultKindV1::OutOfBounds) {
                    512
                } else {
                    memory.byte_offset()
                },
                memory.byte_len(),
                memory.address_space(),
                outcome,
            )
            .unwrap(),
        ),
        evidence.clone(),
    )
    .unwrap();
    let dispatch_end = &events[12];
    events[12] = TraceEventV1::new(
        dispatch_end.sequence(),
        dispatch_end.timestamp(),
        provenance,
        dispatch_end.scope(),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Failed)),
        evidence,
    )
    .unwrap();
    replace_header(
        &trace,
        trace.header().producer().clone(),
        trace.header().execution_kind(),
        trace.header().completeness(),
        trace.header().boundaries(),
        events,
    )
}

fn replace_header(
    trace: &TraceV1,
    producer: ProducerIdentityV1,
    execution: ExecutionKindV1,
    completeness: TraceCompletenessV1,
    boundaries: CaptureBoundariesV1,
    events: Vec<TraceEventV1>,
) -> TraceV1 {
    let header = trace.header();
    TraceV1::new(
        TraceHeaderV1::new(
            producer,
            execution,
            header.kernel_ir_claim(),
            header.semantic_mir(),
            header.lineage(),
            header.artifact(),
            header.dispatch(),
            header.launch(),
            header.bounds(),
            completeness,
            boundaries,
        )
        .unwrap(),
        events,
    )
    .unwrap()
}

fn rebuild_same_header(
    trace: &TraceV1,
    events: Vec<TraceEventV1>,
) -> Result<TraceV1, TraceValidationErrorV1> {
    let header = trace.header();
    TraceV1::new(
        TraceHeaderV1::new(
            header.producer().clone(),
            header.execution_kind(),
            header.kernel_ir_claim(),
            header.semantic_mir(),
            header.lineage(),
            header.artifact(),
            header.dispatch(),
            header.launch(),
            header.bounds(),
            header.completeness(),
            header.boundaries(),
        )?,
        events,
    )
}
