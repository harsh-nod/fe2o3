mod common;

use common::*;
use fe2o3_semantic_trace::*;

type BodyEvent = (ExecutionScopeV1, Option<KirSiteClaimV1>, TraceEventKindV1);

fn operation_site() -> KirSiteClaimV1 {
    KirSiteClaimV1::new(0, 0, KirSitePointV1::Operation(0))
}

fn trace_with_lane_body(body: Vec<BodyEvent>) -> Result<TraceV1, TraceValidationErrorV1> {
    let mut events = vec![
        observed_event(
            0,
            dispatch_scope(),
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
            vec![],
        ),
        observed_event(
            1,
            lane_scope(),
            None,
            TraceEventKindV1::Invocation(InvocationEventV1::Begin),
            vec![],
        ),
    ];
    for (scope, site, kind) in body {
        events.push(observed_event(
            events.len() as u64,
            scope,
            site,
            kind,
            vec![],
        ));
    }
    events.push(observed_event(
        events.len() as u64,
        lane_scope(),
        None,
        TraceEventKindV1::Invocation(InvocationEventV1::End),
        vec![],
    ));
    events.push(observed_event(
        events.len() as u64,
        dispatch_scope(),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
        vec![],
    ));
    TraceV1::new(header(TraceCompletenessV1::Complete), events)
}

fn custom_header(
    producer: ProducerIdentityV1,
    execution: ExecutionKindV1,
    completeness: TraceCompletenessV1,
    boundaries: CaptureBoundariesV1,
) -> Result<TraceHeaderV1, TraceValidationErrorV1> {
    let baseline = header(TraceCompletenessV1::Complete);
    TraceHeaderV1::new(
        producer,
        execution,
        baseline.kernel_ir_claim(),
        baseline.semantic_mir(),
        baseline.lineage(),
        baseline.artifact(),
        baseline.dispatch(),
        baseline.launch(),
        baseline.bounds(),
        completeness,
        boundaries,
    )
}

fn header_with_bounds(bounds: TraceBoundsV1) -> TraceHeaderV1 {
    let baseline = header(TraceCompletenessV1::Complete);
    TraceHeaderV1::new(
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
    .unwrap()
}

#[test]
fn kir_digest_and_sites_remain_unresolved_claims() {
    let trace = sample_trace();
    let claim = trace.header().kernel_ir_claim();
    assert_eq!(claim.wire_version(), KERNEL_IR_WIRE_VERSION_V6);
    assert_eq!(claim.identity_policy(), KERNEL_IR_IDENTITY_POLICY_V1);

    // Large ordinals are syntactically representable claims. This crate has no
    // catalog and deliberately cannot assert that this site exists.
    let site = KirSiteClaimV1::new(u64::MAX, u64::MAX, KirSitePointV1::Operation(u64::MAX));
    assert_eq!(site.function_ordinal(), u64::MAX);
    assert_eq!(site.block_ordinal(), u64::MAX);
}

#[test]
fn producer_and_execution_kind_must_match_exhaustively() {
    let cpu = header(TraceCompletenessV1::Complete).producer().clone();
    assert_eq!(
        custom_header(
            cpu,
            ExecutionKindV1::KfdHardware,
            TraceCompletenessV1::Complete,
            CaptureBoundariesV1::FULL_DISPATCH,
        ),
        Err(TraceValidationErrorV1::ProducerExecutionMismatch {
            producer: ProducerKindV1::CpuKirSimulator,
            execution: ExecutionKindV1::KfdHardware,
        })
    );
}

#[test]
fn declared_resident_budget_must_cover_bounded_trace_storage() {
    assert!(matches!(
        TraceBoundsV1::new_with_resident(1_000, 64 * 1_024, 64 * 1_024, 1),
        Err(TraceValidationErrorV1::ResidentLimitExceeded {
            actual,
            max: 65_536,
        }) if actual > 65_536
    ));
}

#[test]
fn public_constructors_bound_copies_and_account_retained_capacity() {
    let oversized = "x".repeat(MAX_PRODUCER_TEXT_BYTES_V1 + 1);
    assert_eq!(
        ProducerTextV1::new(&oversized),
        Err(TraceValidationErrorV1::InvalidProducerText {
            len: MAX_PRODUCER_TEXT_BYTES_V1 + 1,
        })
    );
    assert_eq!(
        MemoryEventV1::new(
            MemoryAccessKindV1::Read,
            TraceAllocationIdV1::new(1, 0).unwrap(),
            0,
            0,
            AddressSpaceV1::Global,
            MemoryOutcomeV1::Completed,
        ),
        Err(TraceValidationErrorV1::ZeroMemoryAccessLength)
    );

    let bounds = TraceBoundsV1::new_with_resident(2, 4 * 1_024, 64 * 1_024, 1).unwrap();
    let mut events = Vec::with_capacity(2_048);
    events.push(observed_event(
        0,
        dispatch_scope(),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
        vec![],
    ));
    events.push(observed_event(
        1,
        dispatch_scope(),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
        vec![],
    ));
    assert!(matches!(
        TraceV1::new(header_with_bounds(bounds), events),
        Err(TraceValidationErrorV1::ResidentLimitExceeded {
            actual,
            max: 65_536,
        }) if actual > 65_536
    ));
}

#[test]
fn large_reverse_allocation_lifecycle_uses_indexed_validation() {
    const ALLOCATIONS: u64 = 10_000;
    let event_count = 2 * ALLOCATIONS + 2;
    let bounds =
        TraceBoundsV1::new_with_resident(event_count, 8 * 1_024 * 1_024, 64 * 1_024 * 1_024, 1)
            .unwrap();
    let mut events = Vec::new();
    events.try_reserve_exact(event_count as usize).unwrap();
    events.push(observed_event(
        0,
        dispatch_scope(),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
        vec![],
    ));
    for ordinal in (1..=ALLOCATIONS).rev() {
        events.push(observed_event(
            events.len() as u64,
            dispatch_scope(),
            None,
            TraceEventKindV1::Allocation(AllocationEventV1::Create {
                allocation: TraceAllocationIdV1::new(ordinal, 0).unwrap(),
                byte_len: 0,
                address_space: AddressSpaceV1::Private,
            }),
            vec![],
        ));
    }
    for ordinal in 1..=ALLOCATIONS {
        events.push(observed_event(
            events.len() as u64,
            dispatch_scope(),
            None,
            TraceEventKindV1::Allocation(AllocationEventV1::Release {
                allocation: TraceAllocationIdV1::new(ordinal, 0).unwrap(),
            }),
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
    TraceV1::new(header_with_bounds(bounds), events).unwrap();
}

#[test]
fn d1_d3_linearization_and_tail_masks_are_canonical() {
    let geometry = LaunchGeometryV1::new([2, 3, 4], [4, 2, 1], WaveWidthV1::Wave32).unwrap();
    assert_eq!(geometry.linear_workgroup([1, 2, 3]), Some(23));
    assert_eq!(geometry.linear_local_workitem([3, 1, 0]), Some(7));
    assert_eq!(geometry.valid_lane_mask([0, 0, 0], 0), Some(0xff));
    assert_eq!(geometry.valid_lane_mask([0, 0, 0], 1), None);

    let invalid_tail = ExecutionScopeV1::wave(
        dispatch(),
        [1, 0, 0],
        1,
        ActiveMaskV1::new(WaveWidthV1::Wave64, u64::MAX).unwrap(),
    );
    let event = observed_event(
        0,
        invalid_tail,
        Some(operation_site()),
        TraceEventKindV1::Operation(OperationEventV1::Begin(occurrence(1))),
        vec![],
    );
    assert_eq!(
        TraceV1::new(
            truncated_interior_header(1, DroppedEventCountV1::Unknown),
            vec![event],
        ),
        Err(TraceValidationErrorV1::ActiveMaskMismatch {
            expected: u64::from(u32::MAX),
            actual: u64::MAX,
        })
    );
}

#[test]
fn exact_logical_grid_controls_d2_d3_multiwave_tail_masks() {
    let d2 =
        LaunchGeometryV1::new_exact([10, 9, 1], [2, 2, 1], [8, 8, 1], WaveWidthV1::Wave32).unwrap();
    assert_eq!(d2.valid_lane_mask([0, 0, 0], 0), Some(u64::from(u32::MAX)));
    assert_eq!(d2.valid_lane_mask([0, 0, 0], 1), Some(u64::from(u32::MAX)));
    assert_eq!(d2.valid_lane_mask([1, 1, 0], 0), Some(0b11));
    assert_eq!(d2.valid_lane_mask([1, 1, 0], 1), Some(0));

    let d3 =
        LaunchGeometryV1::new_exact([5, 3, 3], [2, 2, 2], [4, 2, 2], WaveWidthV1::Wave64).unwrap();
    assert_eq!(d3.logical_grid(), [5, 3, 3]);
    assert_eq!(d3.valid_lane_mask([1, 1, 1], 0), Some(1));
    assert_eq!(
        LaunchGeometryV1::new_exact([9, 9, 1], [1, 1, 1], [8, 8, 1], WaveWidthV1::Wave64,),
        Err(TraceValidationErrorV1::LogicalGridWorkgroupMismatch { axis: 0 })
    );
}

#[test]
fn lane_scope_correlates_workgroup_wave_lane_logical_coordinate_and_active_mask() {
    let mask = ActiveMaskV1::new(WaveWidthV1::Wave64, u64::from(u32::MAX)).unwrap();
    let wrong_coordinate = ExecutionScopeV1::lane(dispatch(), [1, 0, 0], 1, 5, [164, 0, 0], mask);
    let event = observed_event(
        0,
        wrong_coordinate,
        None,
        TraceEventKindV1::Invocation(InvocationEventV1::Begin),
        vec![],
    );
    assert_eq!(
        TraceV1::new(
            truncated_interior_header(1, DroppedEventCountV1::Unknown),
            vec![event],
        ),
        Err(TraceValidationErrorV1::LogicalWorkitemWaveLaneMismatch)
    );

    let inactive = ExecutionScopeV1::lane(
        dispatch(),
        [1, 0, 0],
        1,
        5,
        [165, 0, 0],
        ActiveMaskV1::new(WaveWidthV1::Wave64, 1).unwrap(),
    );
    let event = observed_event(
        0,
        inactive,
        None,
        TraceEventKindV1::Invocation(InvocationEventV1::Begin),
        vec![],
    );
    assert_eq!(
        TraceV1::new(
            truncated_interior_header(1, DroppedEventCountV1::Unknown),
            vec![event],
        ),
        Err(TraceValidationErrorV1::ActiveMaskMismatch {
            expected: u64::from(u32::MAX),
            actual: 1,
        })
    );
}

fn allocation(ordinal: u64) -> TraceAllocationIdV1 {
    TraceAllocationIdV1::new(ordinal, 0).unwrap()
}

fn create(allocation: TraceAllocationIdV1, byte_len: u64) -> BodyEvent {
    (
        dispatch_scope(),
        None,
        TraceEventKindV1::Allocation(AllocationEventV1::Create {
            allocation,
            byte_len,
            address_space: AddressSpaceV1::Global,
        }),
    )
}

fn memory(
    allocation: TraceAllocationIdV1,
    byte_offset: u64,
    byte_len: u64,
    address_space: AddressSpaceV1,
    outcome: MemoryOutcomeV1,
) -> BodyEvent {
    (
        lane_scope(),
        Some(operation_site()),
        TraceEventKindV1::Memory(
            MemoryEventV1::new(
                MemoryAccessKindV1::Read,
                allocation,
                byte_offset,
                byte_len,
                address_space,
                outcome,
            )
            .unwrap(),
        ),
    )
}

#[test]
fn allocation_ids_are_nonzero_and_memory_requires_an_introduced_live_region() {
    assert_eq!(
        TraceAllocationIdV1::new(0, 0),
        Err(TraceValidationErrorV1::ZeroAllocationIdentity)
    );
    let id = allocation(1);
    assert_eq!(
        trace_with_lane_body(vec![memory(
            id,
            0,
            4,
            AddressSpaceV1::Global,
            MemoryOutcomeV1::Completed,
        )]),
        Err(TraceValidationErrorV1::UseOfUnknownAllocation { allocation: id })
    );
    assert_eq!(
        trace_with_lane_body(vec![
            create(id, 16),
            memory(
                id,
                14,
                4,
                AddressSpaceV1::Global,
                MemoryOutcomeV1::Completed,
            ),
        ]),
        Err(TraceValidationErrorV1::MemoryOutcomeInconsistent)
    );
    assert_eq!(
        trace_with_lane_body(vec![
            create(id, 16),
            memory(
                id,
                0,
                4,
                AddressSpaceV1::Workgroup,
                MemoryOutcomeV1::Completed,
            ),
        ]),
        Err(TraceValidationErrorV1::MemoryOutcomeInconsistent)
    );
}

#[test]
fn allocation_release_and_explicit_unknown_lifecycle_are_checked() {
    let id = allocation(2);
    let release = || {
        (
            dispatch_scope(),
            None,
            TraceEventKindV1::Allocation(AllocationEventV1::Release { allocation: id }),
        )
    };
    assert_eq!(
        trace_with_lane_body(vec![create(id, 16), release(), release()]),
        Err(TraceValidationErrorV1::DuplicateAllocationRelease { allocation: id })
    );
    assert_eq!(
        trace_with_lane_body(vec![
            create(id, 16),
            release(),
            memory(id, 0, 4, AddressSpaceV1::Global, MemoryOutcomeV1::Completed,),
        ]),
        Err(TraceValidationErrorV1::MemoryOutcomeInconsistent)
    );
    trace_with_lane_body(vec![
        (
            dispatch_scope(),
            None,
            TraceEventKindV1::Allocation(AllocationEventV1::UnknownLifecycle { allocation: id }),
        ),
        memory(
            id,
            u64::MAX,
            1,
            AddressSpaceV1::Generic,
            MemoryOutcomeV1::Fault(MemoryFaultKindV1::OutOfBounds),
        ),
    ])
    .unwrap();
}

#[test]
fn allocation_generations_are_exact_per_ordinal_transitions() {
    let id = |generation| TraceAllocationIdV1::new(7, generation).unwrap();
    let unknown = |generation| {
        (
            dispatch_scope(),
            None,
            TraceEventKindV1::Allocation(AllocationEventV1::UnknownLifecycle {
                allocation: id(generation),
            }),
        )
    };
    let release = |generation| {
        (
            dispatch_scope(),
            None,
            TraceEventKindV1::Allocation(AllocationEventV1::Release {
                allocation: id(generation),
            }),
        )
    };

    assert_eq!(
        trace_with_lane_body(vec![create(id(3), 0)]),
        Err(TraceValidationErrorV1::AllocationGenerationMustStartAtZero { allocation: id(3) })
    );
    assert_eq!(
        trace_with_lane_body(vec![unknown(3)]),
        Err(TraceValidationErrorV1::AllocationGenerationMustStartAtZero { allocation: id(3) })
    );
    assert_eq!(
        trace_with_lane_body(vec![unknown(0), unknown(1)]),
        Err(TraceValidationErrorV1::AllocationOrdinalAlreadyLive {
            ordinal: 7,
            current_generation: 0,
            attempted_generation: 1,
        })
    );
    assert_eq!(
        trace_with_lane_body(vec![create(id(0), 0), create(id(1), 0)]),
        Err(TraceValidationErrorV1::AllocationOrdinalAlreadyLive {
            ordinal: 7,
            current_generation: 0,
            attempted_generation: 1,
        })
    );
    assert_eq!(
        trace_with_lane_body(vec![create(id(0), 0), release(0), create(id(2), 0)]),
        Err(TraceValidationErrorV1::AllocationGenerationOutOfSequence {
            ordinal: 7,
            expected: 1,
            actual: 2,
        })
    );
    assert_eq!(
        trace_with_lane_body(vec![
            create(id(0), 0),
            release(0),
            create(id(1), 0),
            release(1),
            create(id(0), 0),
        ]),
        Err(TraceValidationErrorV1::AllocationGenerationOutOfSequence {
            ordinal: 7,
            expected: 2,
            actual: 0,
        })
    );
}

#[test]
fn complete_sequences_are_exact_and_truncated_gaps_match_declared_loss() {
    let begin = observed_event(
        0,
        dispatch_scope(),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
        vec![],
    );
    let end = observed_event(
        2,
        dispatch_scope(),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
        vec![],
    );
    assert_eq!(
        TraceV1::new(
            header(TraceCompletenessV1::Complete),
            vec![begin.clone(), end.clone()],
        ),
        Err(TraceValidationErrorV1::CompleteSequenceMismatch {
            expected: 1,
            actual: 2,
        })
    );
    assert_eq!(
        TraceV1::new(
            truncated_full_header(2, DroppedEventCountV1::Known(1)),
            vec![
                observed_event(
                    2,
                    dispatch_scope(),
                    None,
                    TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
                    vec![],
                ),
                observed_event(
                    4,
                    dispatch_scope(),
                    None,
                    TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed,)),
                    vec![],
                ),
            ],
        ),
        Err(TraceValidationErrorV1::DroppedEventCountTooSmall {
            declared: 1,
            observed_gaps: 3,
        })
    );
}

#[test]
fn zero_known_loss_and_implicit_incomplete_boundaries_are_rejected() {
    let baseline = header(TraceCompletenessV1::Complete);
    assert_eq!(
        TraceHeaderV1::new(
            baseline.producer().clone(),
            baseline.execution_kind(),
            baseline.kernel_ir_claim(),
            baseline.semantic_mir(),
            baseline.lineage(),
            baseline.artifact(),
            baseline.dispatch(),
            baseline.launch(),
            baseline.bounds(),
            TraceCompletenessV1::Truncated {
                reason: TruncationReasonV1::CollectorLoss,
                emitted_events: 0,
                dropped_events: DroppedEventCountV1::Known(0),
            },
            CaptureBoundariesV1::FULL_DISPATCH,
        ),
        Err(TraceValidationErrorV1::ZeroKnownDroppedEvents)
    );
    assert_eq!(
        custom_header(
            baseline.producer().clone(),
            baseline.execution_kind(),
            TraceCompletenessV1::Complete,
            CaptureBoundariesV1::new(
                CaptureStartBoundaryV1::DispatchAlreadyActive,
                CaptureEndBoundaryV1::DispatchContinuesAfterCapture,
            ),
        ),
        Err(TraceValidationErrorV1::CompleteTraceRequiresFullBoundaries)
    );
}

#[test]
fn provenance_has_one_canonical_evidence_membership_and_counts_toward_limits() {
    let proof = evidence(EvidenceKindV1::Proof, 31);
    assert_eq!(
        TraceEventV1::new(
            0,
            TimestampV1::LogicalStep(0),
            FactProvenanceV1::Proved,
            dispatch_scope(),
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
            vec![],
        ),
        Err(TraceValidationErrorV1::ProvenanceEvidenceCardinality {
            kind: EvidenceKindV1::Proof,
            actual: 0,
        })
    );
    let event = TraceEventV1::new(
        0,
        TimestampV1::LogicalStep(0),
        FactProvenanceV1::Proved,
        dispatch_scope(),
        None,
        TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
        vec![proof, evidence(EvidenceKindV1::Artifact, 32)],
    )
    .unwrap();
    let baseline = header(TraceCompletenessV1::Complete);
    let tight = TraceHeaderV1::new(
        baseline.producer().clone(),
        baseline.execution_kind(),
        baseline.kernel_ir_claim(),
        baseline.semantic_mir(),
        baseline.lineage(),
        baseline.artifact(),
        baseline.dispatch(),
        baseline.launch(),
        TraceBoundsV1::new(4, 64 * 1024, 1).unwrap(),
        TraceCompletenessV1::Truncated {
            reason: TruncationReasonV1::UserStopped,
            emitted_events: 1,
            dropped_events: DroppedEventCountV1::Unknown,
        },
        CaptureBoundariesV1::new(
            CaptureStartBoundaryV1::DispatchBeginIncluded,
            CaptureEndBoundaryV1::DispatchContinuesAfterCapture,
        ),
    )
    .unwrap();
    assert_eq!(
        TraceV1::new(tight, vec![event]),
        Err(TraceValidationErrorV1::TooManyEvidenceReferences { actual: 2, max: 1 })
    );
}

#[test]
fn dispatch_invocation_operation_lifecycles_and_scopes_are_enforced() {
    assert_eq!(
        TraceEventV1::new(
            0,
            TimestampV1::LogicalStep(0),
            FactProvenanceV1::Observed,
            lane_scope(),
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
            vec![],
        ),
        Err(TraceValidationErrorV1::EventScopeMismatch)
    );

    let operation_end = (
        lane_scope(),
        Some(operation_site()),
        TraceEventKindV1::Operation(OperationEventV1::End(occurrence(1))),
    );
    assert_eq!(
        trace_with_lane_body(vec![operation_end]),
        Err(TraceValidationErrorV1::OperationEndWithoutBegin)
    );

    let events = vec![
        observed_event(
            0,
            dispatch_scope(),
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
            vec![],
        ),
        observed_event(
            1,
            lane_scope(),
            None,
            TraceEventKindV1::Invocation(InvocationEventV1::End),
            vec![],
        ),
        observed_event(
            2,
            dispatch_scope(),
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
            vec![],
        ),
    ];
    assert_eq!(
        TraceV1::new(header(TraceCompletenessV1::Complete), events),
        Err(TraceValidationErrorV1::InvocationEndWithoutBegin)
    );
}

#[test]
fn serial_loop_may_revisit_a_closed_operation_site() {
    let visit = |dynamic| {
        vec![
            (
                lane_scope(),
                Some(operation_site()),
                TraceEventKindV1::Operation(OperationEventV1::Begin(occurrence(dynamic))),
            ),
            (
                lane_scope(),
                Some(operation_site()),
                TraceEventKindV1::Operation(OperationEventV1::End(occurrence(dynamic))),
            ),
        ]
    };
    let trace = trace_with_lane_body(visit(1).into_iter().chain(visit(2)).collect()).unwrap();
    let encoded = encode_trace_v1(&trace).unwrap();
    assert_eq!(decode_trace_v1(&encoded).unwrap(), trace);
}

#[test]
fn recursive_same_site_occurrences_are_distinct_and_nested() {
    let outer = OperationOccurrenceIdV1::new(1, 1).unwrap();
    let inner = OperationOccurrenceIdV1::new(2, 2).unwrap();
    let body = vec![
        (
            lane_scope(),
            Some(operation_site()),
            TraceEventKindV1::Operation(OperationEventV1::Begin(outer)),
        ),
        (
            lane_scope(),
            Some(operation_site()),
            TraceEventKindV1::Operation(OperationEventV1::Begin(inner)),
        ),
        (
            lane_scope(),
            Some(operation_site()),
            TraceEventKindV1::Operation(OperationEventV1::End(inner)),
        ),
        (
            lane_scope(),
            Some(operation_site()),
            TraceEventKindV1::Operation(OperationEventV1::End(outer)),
        ),
    ];
    let trace = trace_with_lane_body(body).unwrap();
    assert_eq!(
        decode_trace_v1(&encode_trace_v1(&trace).unwrap()).unwrap(),
        trace
    );
    assert_eq!(
        OperationOccurrenceIdV1::new(0, 1),
        Err(TraceValidationErrorV1::ZeroOperationOccurrenceIdentity)
    );
}

#[test]
fn dispatch_identity_site_roles_and_post_invocation_events_cannot_disagree() {
    let wrong_dispatch =
        DispatchIdentityV1::new(DispatchIdentityDomainV1::TraceLocal, identity(77));
    let event = observed_event(
        0,
        ExecutionScopeV1::dispatch(wrong_dispatch),
        None,
        TraceEventKindV1::Allocation(AllocationEventV1::Preexisting {
            allocation: allocation(70),
            byte_len: 4,
            address_space: AddressSpaceV1::Global,
        }),
        vec![],
    );
    assert_eq!(
        TraceV1::new(
            truncated_interior_header(1, DroppedEventCountV1::Unknown),
            vec![event],
        ),
        Err(TraceValidationErrorV1::DispatchIdentityMismatch)
    );

    assert_eq!(
        TraceEventV1::new(
            0,
            TimestampV1::LogicalStep(0),
            FactProvenanceV1::Observed,
            lane_scope(),
            Some(KirSiteClaimV1::new(0, 0, KirSitePointV1::Terminator,)),
            TraceEventKindV1::Operation(OperationEventV1::Begin(occurrence(1))),
            vec![],
        ),
        Err(TraceValidationErrorV1::EventSiteMismatch)
    );

    let events = vec![
        observed_event(
            0,
            dispatch_scope(),
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::Begin),
            vec![],
        ),
        observed_event(
            1,
            lane_scope(),
            None,
            TraceEventKindV1::Invocation(InvocationEventV1::Begin),
            vec![],
        ),
        observed_event(
            2,
            lane_scope(),
            None,
            TraceEventKindV1::Invocation(InvocationEventV1::End),
            vec![],
        ),
        observed_event(
            3,
            lane_scope(),
            Some(operation_site()),
            TraceEventKindV1::Diagnostic(DiagnosticEventV1::new(DiagnosticKindV1::Fault, 1)),
            vec![],
        ),
        observed_event(
            4,
            dispatch_scope(),
            None,
            TraceEventKindV1::Dispatch(DispatchEventV1::End(DispatchOutcomeV1::Completed)),
            vec![],
        ),
    ];
    assert_eq!(
        TraceV1::new(header(TraceCompletenessV1::Complete), events),
        Err(TraceValidationErrorV1::EventAfterInvocationEnd)
    );
}

#[test]
fn explicitly_interior_capture_can_omit_dispatch_boundaries() {
    let id = allocation(9);
    let event = observed_event(
        7,
        dispatch_scope(),
        None,
        TraceEventKindV1::Allocation(AllocationEventV1::Preexisting {
            allocation: id,
            byte_len: 64,
            address_space: AddressSpaceV1::Global,
        }),
        vec![],
    );
    TraceV1::new(
        truncated_interior_header(1, DroppedEventCountV1::Unknown),
        vec![event],
    )
    .unwrap();
}

fn truncated_interior_header(
    emitted_events: u64,
    dropped_events: DroppedEventCountV1,
) -> TraceHeaderV1 {
    let baseline = header(TraceCompletenessV1::Complete);
    custom_header(
        baseline.producer().clone(),
        baseline.execution_kind(),
        TraceCompletenessV1::Truncated {
            reason: TruncationReasonV1::CollectorLoss,
            emitted_events,
            dropped_events,
        },
        CaptureBoundariesV1::new(
            CaptureStartBoundaryV1::DispatchAlreadyActive,
            CaptureEndBoundaryV1::DispatchContinuesAfterCapture,
        ),
    )
    .unwrap()
}

fn truncated_full_header(
    emitted_events: u64,
    dropped_events: DroppedEventCountV1,
) -> TraceHeaderV1 {
    let baseline = header(TraceCompletenessV1::Complete);
    custom_header(
        baseline.producer().clone(),
        baseline.execution_kind(),
        TraceCompletenessV1::Truncated {
            reason: TruncationReasonV1::CollectorLoss,
            emitted_events,
            dropped_events,
        },
        CaptureBoundariesV1::FULL_DISPATCH,
    )
    .unwrap()
}
