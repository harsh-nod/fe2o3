// Executable constructor-origin reader traces. Storage observations remain explicit inputs.
verus! {

struct LifecycleReaderTraceV1 {
    origin: LifecycleOriginV1,
    actual: Seq<ContextProducerReadJournalV1>,
    model: Seq<logical::ProducerReadContentsV1>,
    events: Seq<LifecyclePairedEventV1>,
}

spec fn lifecycle_reader_expected_event_v1(phase: nat) -> LifecyclePairedEventV1
    recommends 1 <= phase <= 13,
{
    let entry = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 };
    let allocation = AllocationReferenceV1 { slot: 0, key: entry.key };
    let producer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 {
        context_generation: 7, local: 10, kind: WriterKindV1::Submission } };
    let replacement = WriterReferenceV1 { slot: 0, key: WriterKeyV1 {
        context_generation: 7, local: 11, kind: WriterKindV1::Submission } };
    let producer_reference = ContextProducerReadReferenceV1 { slot: 0, incarnation: 1,
        consumer: WriterKeyV1 { context_generation: 7, local: 40, kind: WriterKindV1::Submission } };
    let stable_reference = ContextReadLeaseReferenceV1 { slot: 0,
        incarnation: if phase == 10 || phase == 11 { 2u64 } else { 1u64 },
        consumer: WriterKeyV1 { context_generation: 7, local: 50, kind: WriterKindV1::Submission } };
    let producer_request = ContextProducerReadV1 { producer, read: ContextAllocationReadV1 {
        allocation, device: entry.device, byte_extent: 16, byte_offset: 0, byte_len: 8,
        attempt_epoch: 1, content_lineage: 0 } };
    let stable_request = ContextAllocationReadV1 {
        allocation, device: entry.device, byte_extent: 16, byte_offset: 0, byte_len: 8,
        attempt_epoch: 1, content_lineage: 1 };
    if phase == 1 {
        LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::EnrollScalar { entry, result: Ok(allocation) },
            model: logical::LifecycleStepV1::EnrollScalar { entry: enrollment_view(entry), result: Ok(allocation_reference_view(allocation)) },
        }
    } else if phase == 2 || phase == 6 {
        let writer = if phase == 2 { producer } else { replacement };
        LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::Register { key: writer.key, result: Ok(writer) },
            model: logical::LifecycleStepV1::Register { key: writer_key_view(writer.key), result: Ok(writer_reference_view(writer)) },
        }
    } else if phase == 3 {
        let roster = seq![AllocationWriteV1 { allocation, device: entry.device, byte_extent: 16 }];
        LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::Begin { writer: producer, roster, result: Ok(()) },
            model: logical::LifecycleStepV1::Begin { writer: writer_reference_view(producer), roster: begin_roster_view(roster), result: Ok(()) },
        }
    } else if phase == 4 {
        LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::AcquireProducer { consumer: producer_reference.consumer,
                requests: seq![producer_request], original: seq![None], output: seq![Some(producer_reference)], result: Ok(()) },
            model: logical::LifecycleStepV1::AcquireProducer { consumer: writer_key_view(producer_reference.consumer),
                requests: seq![producer_read_view(producer_request)], original: seq![None],
                output: seq![Some(producer_reference_view(producer_reference))], result: Ok(()) },
        }
    } else if phase == 5 {
        LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::SettleSuccess { writer: producer, evidence: producer,
                writer_storage: 1, member_storage: 3, result: Ok(()) },
            model: logical::LifecycleStepV1::SettleSuccess { writer: writer_reference_view(producer), evidence: writer_reference_view(producer),
                writer_storage: 1, member_storage: 3, result: Ok(()) },
        }
    } else if phase == 7 || phase == 10 {
        LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::AcquireStable { consumer: stable_reference.consumer,
                requests: seq![stable_request], original: seq![None], output: seq![Some(stable_reference)], result: Ok(()) },
            model: logical::LifecycleStepV1::AcquireStable { consumer: writer_key_view(stable_reference.consumer),
                requests: seq![allocation_read_view(stable_request)], original: seq![None],
                output: seq![Some(read_reference_view(stable_reference))], result: Ok(()) },
        }
    } else if phase == 8 {
        LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::ReleaseProducer { consumer: producer_reference.consumer,
                references: seq![producer_reference], evidence: producer_reference.consumer, capacity: 2, result: Ok(()) },
            model: logical::LifecycleStepV1::ReleaseProducer { consumer: writer_key_view(producer_reference.consumer),
                references: seq![producer_reference_view(producer_reference)], evidence: writer_key_view(producer_reference.consumer),
                capacity: 2, result: Ok(()) },
        }
    } else if phase == 9 || phase == 11 {
        LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::ReleaseStable { consumer: stable_reference.consumer,
                references: seq![stable_reference], evidence: stable_reference.consumer, capacity: 2, result: Ok(()) },
            model: logical::LifecycleStepV1::ReleaseStable { consumer: writer_key_view(stable_reference.consumer),
                references: seq![read_reference_view(stable_reference)], evidence: writer_key_view(stable_reference.consumer),
                capacity: 2, result: Ok(()) },
        }
    } else if phase == 12 {
        LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::Abort { reference: replacement, capacity: 1, result: Ok(()) },
            model: logical::LifecycleStepV1::Abort { reference: writer_reference_view(replacement), capacity: 1, result: Ok(()) },
        }
    } else {
        LifecyclePairedEventV1::Observe { query: LifecycleQueryV1::ProducerStatus(producer_reference),
            actual_answer: LifecycleAnswerV1::Status(Ok(ContextProducerReadStatusV1::Success)),
            model_answer: LifecycleAnswerV1::Status(Ok(ContextProducerReadStatusV1::Success)) }
    }
}

spec fn lifecycle_reader_extension_v1(before: LifecycleReaderTraceV1, after: LifecycleReaderTraceV1,
    actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1, phase: nat) -> bool
{
    &&& 1 <= phase <= 13
    &&& after.origin == before.origin
    &&& after.actual == before.actual.push(actual)
    &&& after.model == before.model.push(model)
    &&& after.events == before.events.push(lifecycle_reader_expected_event_v1(phase))
}

spec fn lifecycle_reader_trace_v1(trace: LifecycleReaderTraceV1,
    actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1) -> bool
{
    &&& lifecycle_paired_trace_v1(trace.origin, trace.actual, trace.model, trace.events)
    &&& lifecycle_origin_input_shape_v1(trace.origin)
    &&& trace.origin.context == 7 && trace.origin.allocations == 3
    &&& trace.origin.writers == 1 && trace.origin.reads == 2
    &&& trace.actual.len() == trace.events.len() + 1
    &&& trace.model.len() == trace.actual.len()
    &&& trace.actual.last() == actual && trace.model.last() == model
    &&& forall|i: int| 0 <= i < trace.events.len() ==> lifecycle_event_input_shape_v1(#[trigger] trace.events[i])
}

#[verifier::spinoff_prover]
proof fn lifecycle_reader_reached_v1(trace: LifecycleReaderTraceV1,
    actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1)
    requires lifecycle_reader_trace_v1(trace, actual, model),
    ensures producer_represents(actual, model),
        logical::producer_invariant_v1(model),
        logical::lifecycle_invariant_v1(model, logical::witness_storage_v1(3, 1),
            lifecycle_paired_history_v1(trace.events, trace.events.len())),
{
    lifecycle_paired_prefix_v1(trace.origin, trace.actual, trace.model, trace.events,
        logical::witness_storage_v1(3, 1), trace.events.len());
}

#[verifier::spinoff_prover]
proof fn lifecycle_reader_append_event_v1(trace: LifecycleReaderTraceV1,
    before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1,
    event: LifecyclePairedEventV1) -> (next: LifecycleReaderTraceV1)
    requires lifecycle_reader_trace_v1(trace, before, model_before),
        lifecycle_event_relation_v1(before, after, model_before, model_after, event),
        lifecycle_event_input_shape_v1(event),
    ensures lifecycle_reader_trace_v1(next, after, model_after),
        next.events == trace.events.push(event), next.origin == trace.origin,
        next.actual == trace.actual.push(after), next.model == trace.model.push(model_after),
{
    let next = LifecycleReaderTraceV1 { origin: trace.origin,
        actual: trace.actual.push(after), model: trace.model.push(model_after), events: trace.events };
    assert forall|i: int| 0 <= i < next.events.len() implies lifecycle_event_relation_v1(
        next.actual[i], next.actual[i + 1], next.model[i], next.model[i + 1], #[trigger] next.events[i]) by {
        if i < trace.events.len() {
            assert(lifecycle_event_relation_v1(trace.actual[i], trace.actual[i + 1],
                trace.model[i], trace.model[i + 1], trace.events[i]));
        } else { assert(i == trace.events.len()); }
    }
    assert forall|i: int| 0 <= i < next.events.len() implies lifecycle_event_input_shape_v1(#[trigger] next.events[i]) by {
        if i < trace.events.len() { assert(next.events[i] == trace.events[i]); }
    }
    next
}

#[verifier::spinoff_prover]
proof fn lifecycle_reader_append_v1(trace: LifecycleReaderTraceV1,
    before: ContextProducerReadJournalV1, after: ContextProducerReadJournalV1,
    model_before: logical::ProducerReadContentsV1, model_after: logical::ProducerReadContentsV1,
    event: LifecyclePairedEventV1, phase: nat) -> (next: LifecycleReaderTraceV1)
    requires lifecycle_reader_trace_v1(trace, before, model_before),
        lifecycle_event_relation_v1(before, after, model_before, model_after, event),
        lifecycle_event_input_shape_v1(event),
        1 <= phase <= 13, event == lifecycle_reader_expected_event_v1(phase),
    ensures lifecycle_reader_trace_v1(next, after, model_after),
        lifecycle_reader_extension_v1(trace, next, after, model_after, phase),
        next.events == trace.events.push(event), next.origin == trace.origin,
        next.actual == trace.actual.push(after), next.model == trace.model.push(model_after),
{
    lifecycle_reader_append_event_v1(trace, before, after, model_before, model_after, event)
}

// 0 empty, 1 enrolled, 2 reserved, 3 pending, 4 producer read, 5 settled, 6 writer reused,
// 7 both reads, 8 stable read, 9 released, 10 stable slot reused, 11 released, 12 aborted.
spec fn lifecycle_reader_contents_v1(owner: ContextProducerReadJournalV1, phase: nat) -> bool {
    let allocation = AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } };
    let device = ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 };
    let writer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 {
        context_generation: 7, local: 10, kind: WriterKindV1::Submission } };
    let replacement = WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission };
    let producer_consumer = WriterKeyV1 { context_generation: 7, local: 40, kind: WriterKindV1::Submission };
    let stable_consumer = WriterKeyV1 { context_generation: 7, local: 50, kind: WriterKindV1::Submission };
    let pending = phase == 3 || phase == 4;
    let reserved = phase == 2 || (6 <= phase && phase < 12);
    let producer_live = 4 <= phase && phase < 8;
    let stable_live = phase == 7 || phase == 8 || phase == 10;
    let producer_request = ContextProducerReadV1 { producer: writer, read: ContextAllocationReadV1 {
        allocation, device, byte_extent: 16, byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0 } };
    let stable_request = ContextAllocationReadV1 {
        allocation, device, byte_extent: 16, byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 1 };
    &&& phase <= 12
    &&& owner.stable.journal.context_generation == 7
    &&& owner.stable.journal.allocation_capacity == 3 && owner.stable.journal.writer_capacity == 1
    &&& owner.stable.journal.registration_watermark == if phase < 2 { 0u64 } else if phase < 6 { 10u64 } else { 11u64 }
    &&& owner.stable.journal.reserved_count == if reserved { 1usize } else { 0usize }
    &&& owner.stable.journal.writers@ == seq![if pending {
        Some(WriterEntryV1::Pending { key: writer.key, head: Some(0usize), count: 1usize })
    } else if reserved { Some(WriterEntryV1::Reserved(if phase == 2 { writer.key } else { replacement })) } else { None }]
    &&& owner.stable.journal.free@ == if pending || reserved { seq![] } else { seq![0usize] }
    &&& owner.stable.journal.allocations@ == if phase == 0 { seq![None, None, None] } else {
        seq![Some(AllocationEntryV1 { key: allocation.key, device, byte_extent: 16,
            attempt_epoch: if phase < 3 { 0u64 } else { 1u64 },
            content_lineage: if phase < 5 { 0u64 } else { 1u64 },
            pending_member: if pending { Some(0usize) } else { None } }), None, None]
    }
    &&& owner.stable.journal.allocation_free@ == if phase == 0 { seq![2usize, 1, 0] } else { seq![2usize, 1] }
    &&& owner.stable.journal.members@ == seq![if pending { Some(MemberEntryV1 {
        writer, allocation, prior_lineage: 0u64, attempt_epoch: 1u64, next: None }) } else { None }, None, None]
    &&& owner.stable.journal.member_free@ == if pending { seq![2usize, 1] } else { seq![2usize, 1, 0] }
    &&& owner.stable.journal.scratch@.len() == 3
    &&& owner.reservations@ == seq![if producer_live { Some(ReservationV1 {
        reference: ContextProducerReadReferenceV1 { slot: 0, incarnation: 1, consumer: producer_consumer },
        request: producer_request }) } else { None }, None]
    &&& owner.counts@ == seq![if producer_live { 1usize } else { 0usize }, 0, 0]
    &&& owner.free@ == if producer_live { seq![1usize] } else { seq![1usize, 0] }
    &&& owner.next_incarnation == if phase < 4 { 1u64 } else { 2u64 }
    &&& owner.stable.leases@ == seq![if stable_live { Some(ReadLeaseV1 {
        reference: ContextReadLeaseReferenceV1 { slot: 0, incarnation: if phase == 10 { 2u64 } else { 1u64 }, consumer: stable_consumer },
        request: stable_request }) } else { None }, None]
    &&& owner.stable.readers@ == seq![if stable_live { 1usize } else { 0usize }, 0, 0]
    &&& owner.stable.free_reads@ == if stable_live { seq![1usize] } else { seq![1usize, 0] }
    &&& owner.stable.next_incarnation == if phase < 7 { 1u64 } else if phase < 10 { 2u64 } else { 3u64 }
}

#[verifier::spinoff_prover]
fn lifecycle_reader_origin_v1()
    -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1, Ghost<LifecycleReaderTraceV1>))
    ensures lifecycle_reader_trace_v1(result.2@, result.0, result.1),
        lifecycle_reader_contents_v1(result.0, 0), result.2@.events.len() == 0,
{
    let (actual, model, origin) = lifecycle_constructor_origin_fixture_v1();
    assert(actual.stable.journal.writers@ =~= seq![None]);
    assert(actual.stable.journal.free@ =~= seq![0usize]);
    assert(actual.stable.journal.allocations@ =~= seq![None, None, None]);
    assert(actual.stable.journal.allocation_free@ =~= seq![2usize, 1, 0]);
    assert(actual.stable.journal.members@ =~= seq![None, None, None]);
    assert(actual.stable.journal.member_free@ =~= seq![2usize, 1, 0]);
    assert(actual.stable.leases@ =~= seq![None, None]);
    assert(actual.stable.free_reads@ =~= seq![1usize, 0]);
    assert(actual.stable.readers@ =~= seq![0usize, 0, 0]);
    assert(actual.reservations@ =~= seq![None, None]);
    assert(actual.free@ =~= seq![1usize, 0]);
    assert(actual.counts@ =~= seq![0usize, 0, 0]);
    let ghost trace = LifecycleReaderTraceV1 {
        origin: origin@, actual: seq![actual], model: seq![model], events: Seq::empty(),
    };
    (actual, model, Ghost(trace))
}

#[verifier::spinoff_prover]
fn lifecycle_reader_enroll_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    Ghost(trace): Ghost<LifecycleReaderTraceV1>) -> (next: Ghost<LifecycleReaderTraceV1>)
    requires lifecycle_reader_trace_v1(trace, *old(actual), *old(model)), lifecycle_reader_contents_v1(*old(actual), 0),
    ensures lifecycle_reader_trace_v1(next@, *final(actual), *final(model)), lifecycle_reader_contents_v1(*final(actual), 1),
        lifecycle_reader_extension_v1(trace, next@, *final(actual), *final(model), 1),
        next@.events.len() == trace.events.len() + 1,
{
    hide(lifecycle_reader_trace_v1);
    proof { lifecycle_reader_reached_v1(trace, *actual, *model); }
    let ghost before = *actual;
    let ghost model_before = *model;
    let entry = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 };
    let model_entry = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 20 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 };
    let enrolled = owner_scalar_enrollment_paired_exec_v1(actual, model, entry, model_entry,
        Ghost(logical::witness_storage_v1(3, 1)), Ghost(lifecycle_paired_history_v1(trace.events, trace.events.len()).writers));
    assert(enrolled.0 == Ok(AllocationReferenceV1 { slot: 0, key: entry.key }));
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::EnrollScalar { entry, result: enrolled.0 },
        model: logical::LifecycleStepV1::EnrollScalar { entry: model_entry, result: enrolled.1 },
    };
    assert(actual.stable.journal.allocations@ =~= seq![Some(enrollment_value_v1(entry)), None, None]);
    assert(actual.stable.journal.allocation_free@ =~= seq![2usize, 1]);
    let ghost mut next = trace;
    proof { next = lifecycle_reader_append_v1(trace, before, *actual, model_before, *model, event, 1); }
    Ghost(next)
}

#[verifier::spinoff_prover]
fn lifecycle_reader_register_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    reuse: bool, Ghost(trace): Ghost<LifecycleReaderTraceV1>) -> (next: Ghost<LifecycleReaderTraceV1>)
    requires lifecycle_reader_trace_v1(trace, *old(actual), *old(model)),
        lifecycle_reader_contents_v1(*old(actual), if reuse { 5nat } else { 1nat }),
    ensures lifecycle_reader_trace_v1(next@, *final(actual), *final(model)),
        lifecycle_reader_contents_v1(*final(actual), if reuse { 6nat } else { 2nat }),
        lifecycle_reader_extension_v1(trace, next@, *final(actual), *final(model), if reuse { 6nat } else { 2nat }),
        next@.events.len() == trace.events.len() + 1,
{
    hide(lifecycle_reader_trace_v1);
    proof { lifecycle_reader_reached_v1(trace, *actual, *model); }
    let ghost before = *actual;
    let ghost model_before = *model;
    let key = WriterKeyV1 { context_generation: 7, local: if reuse { 11 } else { 10 }, kind: WriterKindV1::Submission };
    let model_key = logical::WriterKeyV1 { context_generation: 7, local: if reuse { 11 } else { 10 }, kind: logical::WriterKindV1::Submission };
    let registered = owner_register_historical_exec_v1(actual, model, key, model_key,
        Ghost(logical::witness_storage_v1(3, 1)), Ghost(lifecycle_paired_history_v1(trace.events, trace.events.len()).writers));
    assert(registered.0 == Ok(WriterReferenceV1 { slot: 0, key }));
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::Register { key, result: registered.0 },
        model: logical::LifecycleStepV1::Register { key: model_key, result: registered.1 },
    };
    assert(actual.stable.journal.writers@ =~= seq![Some(WriterEntryV1::Reserved(key))]);
    assert(actual.stable.journal.free@ =~= seq![]);
    let ghost mut next = trace;
    proof { next = lifecycle_reader_append_v1(trace, before, *actual, model_before, *model, event, if reuse { 6nat } else { 2nat }); }
    Ghost(next)
}

#[verifier::spinoff_prover]
fn lifecycle_reader_begin_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    Ghost(trace): Ghost<LifecycleReaderTraceV1>) -> (next: Ghost<LifecycleReaderTraceV1>)
    requires lifecycle_reader_trace_v1(trace, *old(actual), *old(model)), lifecycle_reader_contents_v1(*old(actual), 2),
    ensures lifecycle_reader_trace_v1(next@, *final(actual), *final(model)), lifecycle_reader_contents_v1(*final(actual), 3),
        lifecycle_reader_extension_v1(trace, next@, *final(actual), *final(model), 3),
        next@.events.len() == trace.events.len() + 1,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::issued_producer_v1);
    hide(logical::producer_invariant_v1);
    proof { lifecycle_reader_reached_v1(trace, *actual, *model); }
    let ghost before = *actual;
    let ghost model_before = *model;
    let writer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } };
    let model_writer = logical::WriterReferenceV1 { slot: 0, key: logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission } };
    let (roster, model_roster) = owner_disposal_roster_fixture_v1();
    proof {
        assert(actual.stable.journal.scratch@[0].is_none()) by {
            reveal(logical::producer_invariant_v1);
        }
        reveal_with_fuel(begin_members_prefix_v1, 3);
        reveal_with_fuel(begin_allocations_prefix_v1, 3);
        reveal_with_fuel(producer_unread_writes_v1, 3);
        reveal_with_fuel(stable_unread_writes_v1, 3);
        reveal_with_fuel(begin_canonical_scan_v1, 3);
        reveal_with_fuel(begin_destination_scan_v1, 3);
        reveal_with_fuel(begin_slot_scan_v1, 3);
    }
    let begun = producer_begin_historical_exec_v1(actual, model, writer, model_writer, &roster, &model_roster,
        Ghost(logical::witness_storage_v1(3, 1)), Ghost(lifecycle_paired_history_v1(trace.events, trace.events.len()).writers));
    assert(begun.0 == Ok(()) && begun.1 == Ok(()));
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::Begin { writer, roster: roster@, result: begun.0 },
        model: logical::LifecycleStepV1::Begin { writer: model_writer, roster: model_roster@, result: begun.1 },
    };
    assert(actual.stable.journal.writers@ =~= seq![Some(WriterEntryV1::Pending { key: writer.key, head: Some(0usize), count: 1usize })]);
    assert(actual.stable.journal.members@ =~= seq![Some(MemberEntryV1 {
        writer, allocation: roster@[0].allocation, prior_lineage: 0u64, attempt_epoch: 1u64, next: None }), None, None]);
    assert(actual.stable.journal.member_free@ =~= seq![2usize, 1]);
    assert(actual.stable.journal.allocations@ =~= seq![Some(AllocationEntryV1 {
        key: roster@[0].allocation.key, device: roster@[0].device, byte_extent: 16u64,
        attempt_epoch: 1u64, content_lineage: 0u64, pending_member: Some(0usize) }), None, None]);
    let ghost mut next = trace;
    proof { next = lifecycle_reader_append_v1(trace, before, *actual, model_before, *model, event, 3); }
    Ghost(next)
}

#[verifier::spinoff_prover]
fn lifecycle_reader_acquire_producer_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    Ghost(trace): Ghost<LifecycleReaderTraceV1>) -> (next: Ghost<LifecycleReaderTraceV1>)
    requires lifecycle_reader_trace_v1(trace, *old(actual), *old(model)), lifecycle_reader_contents_v1(*old(actual), 3),
    ensures lifecycle_reader_trace_v1(next@, *final(actual), *final(model)), lifecycle_reader_contents_v1(*final(actual), 4),
        lifecycle_reader_extension_v1(trace, next@, *final(actual), *final(model), 4),
        next@.events.len() == trace.events.len() + 1,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::lifecycle_invariant_v1);
    proof { lifecycle_reader_reached_v1(trace, *actual, *model); }
    let ghost before = *actual;
    let ghost model_before = *model;
    let consumer = WriterKeyV1 { context_generation: 7, local: 40, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 40, kind: logical::WriterKindV1::Submission };
    let request = ContextProducerReadV1 {
        producer: WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } },
        read: ContextAllocationReadV1 {
            allocation: AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
            byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0 } };
    let model_request = logical::ProducerReadV1 {
        producer: logical::WriterReferenceV1 { slot: 0, key: logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission } },
        read: logical::AllocationReadV1 {
            allocation: logical::AllocationReferenceV1 { slot: 0, key: logical::AllocationKeyV1 { context_generation: 7, local: 20 } },
            device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
            byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0 } };
    let requests = vec![request];
    let model_requests = vec![model_request];
    let mut output = vec![None];
    let mut model_output = vec![None];
    let ghost original = output@;
    let ghost model_original = model_output@;
    proof {
        assert(producer_requests_view(requests@) =~= model_requests@);
        assert(producer_output_view(output@) =~= model_output@);
        reveal_with_fuel(producer_acquire_scan_v1, 3);
        reveal_with_fuel(producer_acquired_reservations_v1, 3);
        reveal_with_fuel(producer_request_count_v1, 3);
    }
    let acquired = producer_acquire_historical_exec_v1(actual, model, consumer, model_consumer,
        &requests, &model_requests, &mut output, &mut model_output);
    assert(acquired.0 == Ok(()) && acquired.1 == Ok(()));
    assert(output@[0] == Some(ContextProducerReadReferenceV1 { slot: 0, incarnation: 1, consumer }));
    assert(actual.reservations@ =~= seq![Some(ReservationV1 { reference: output@[0].unwrap(), request }), None]);
    assert(actual.counts@ =~= seq![1usize, 0, 0]);
    assert(actual.free@ =~= seq![1usize]);
    assert(requests@ =~= seq![request]);
    assert(model_requests@ =~= seq![model_request]);
    assert(original =~= seq![None]);
    assert(model_original =~= seq![None]);
    assert(output@ =~= seq![Some(ContextProducerReadReferenceV1 { slot: 0, incarnation: 1, consumer })]);
    assert(model_output@ =~= seq![Some(producer_reference_view(output@[0].unwrap()))]);
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::AcquireProducer { consumer, requests: requests@, original, output: output@, result: acquired.0 },
        model: logical::LifecycleStepV1::AcquireProducer { consumer: model_consumer, requests: model_requests@,
            original: model_original, output: model_output@, result: acquired.1 },
    };
    let ghost mut next = trace;
    proof { next = lifecycle_reader_append_v1(trace, before, *actual, model_before, *model, event, 4); }
    Ghost(next)
}

#[verifier::spinoff_prover]
fn lifecycle_reader_settle_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    Ghost(trace): Ghost<LifecycleReaderTraceV1>) -> (next: Ghost<LifecycleReaderTraceV1>)
    requires lifecycle_reader_trace_v1(trace, *old(actual), *old(model)), lifecycle_reader_contents_v1(*old(actual), 4),
    ensures lifecycle_reader_trace_v1(next@, *final(actual), *final(model)), lifecycle_reader_contents_v1(*final(actual), 5),
        lifecycle_reader_extension_v1(trace, next@, *final(actual), *final(model), 5),
        next@.events.len() == trace.events.len() + 1,
{
    hide(lifecycle_reader_trace_v1);
    proof { lifecycle_reader_reached_v1(trace, *actual, *model); }
    let ghost before = *actual;
    let ghost model_before = *model;
    let writer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } };
    let model_writer = logical::WriterReferenceV1 { slot: 0, key: logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission } };
    proof {
        reveal_with_fuel(owner_retained_scan_v1, 3);
        reveal_with_fuel(settlement_scratch_scan_v1, 3);
        reveal_with_fuel(settlement_cursor_v1, 3);
        reveal_with_fuel(settlement_allocations_prefix_v1, 3);
        reveal_with_fuel(settlement_members_prefix_v1, 3);
    }
    let settled = owner_settlement_historical_exec_v1(actual, model, writer, model_writer, writer, model_writer,
        1, 3, true, Ghost(logical::witness_storage_v1(3, 1)),
        Ghost(lifecycle_paired_history_v1(trace.events, trace.events.len()).writers));
    assert(settled.0 == Ok(()) && settled.1 == Ok(()));
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::SettleSuccess { writer, evidence: writer, writer_storage: 1, member_storage: 3, result: settled.0 },
        model: logical::LifecycleStepV1::SettleSuccess { writer: model_writer, evidence: model_writer,
            writer_storage: 1, member_storage: 3, result: settled.1 },
    };
    assert(actual.stable.journal.writers@ =~= seq![None]);
    assert(actual.stable.journal.free@ =~= seq![0usize]);
    assert(actual.stable.journal.members@ =~= seq![None, None, None]);
    assert(actual.stable.journal.member_free@ =~= seq![2usize, 1, 0]);
    assert(actual.stable.journal.allocations@ =~= seq![Some(AllocationEntryV1 {
        key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        attempt_epoch: 1, content_lineage: 1, pending_member: None }), None, None]);
    let ghost mut next = trace;
    proof { next = lifecycle_reader_append_v1(trace, before, *actual, model_before, *model, event, 5); }
    Ghost(next)
}

#[verifier::spinoff_prover]
fn lifecycle_reader_reused_producer_query_v1(actual: &ContextProducerReadJournalV1, model: &logical::ProducerReadContentsV1,
    Ghost(trace): Ghost<LifecycleReaderTraceV1>) -> (next: Ghost<LifecycleReaderTraceV1>)
    requires lifecycle_reader_trace_v1(trace, *actual, *model), lifecycle_reader_contents_v1(*actual, 6),
    ensures lifecycle_reader_trace_v1(next@, *actual, *model), next@.events.len() == trace.events.len() + 1,
        lifecycle_reader_extension_v1(trace, next@, *actual, *model, 13),
        match next@.events.last() {
            LifecyclePairedEventV1::Observe { actual_answer, model_answer, .. } =>
                actual_answer == LifecycleAnswerV1::Status(Ok(ContextProducerReadStatusV1::Success))
                    && model_answer == LifecycleAnswerV1::Status(Ok(ContextProducerReadStatusV1::Success)),
            _ => false,
        },
{
    hide(lifecycle_reader_trace_v1);
    proof { lifecycle_reader_reached_v1(trace, *actual, *model); }
    let reference = actual.reservations[0].unwrap().reference;
    let model_reference = model.reservations[0].unwrap().reference;
    let observed = producer_query_historical_exec_v1(actual, model, reference, model_reference);
    assert(observed.0 == Ok(ContextProducerReadStatusV1::Success));
    assert(actual.stable.journal.writers@[0] == Some(WriterEntryV1::Reserved(WriterKeyV1 {
        context_generation: 7, local: 11, kind: WriterKindV1::Submission })));
    assert(actual.reservations@[0].unwrap().request.producer.key.local == 10);
    let ghost event = LifecyclePairedEventV1::Observe {
        query: LifecycleQueryV1::ProducerStatus(reference),
        actual_answer: LifecycleAnswerV1::Status(observed.0),
        model_answer: LifecycleAnswerV1::Status(producer_status_result_from(observed.1)),
    };
    let ghost mut next = trace;
    proof { next = lifecycle_reader_append_v1(trace, *actual, *actual, *model, *model, event, 13); }
    Ghost(next)
}

#[verifier::spinoff_prover]
fn lifecycle_reader_acquire_stable_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    reuse: bool, Ghost(trace): Ghost<LifecycleReaderTraceV1>) -> (next: Ghost<LifecycleReaderTraceV1>)
    requires lifecycle_reader_trace_v1(trace, *old(actual), *old(model)),
        lifecycle_reader_contents_v1(*old(actual), if reuse { 9nat } else { 6nat }),
    ensures lifecycle_reader_trace_v1(next@, *final(actual), *final(model)),
        lifecycle_reader_contents_v1(*final(actual), if reuse { 10nat } else { 7nat }),
        lifecycle_reader_extension_v1(trace, next@, *final(actual), *final(model), if reuse { 10nat } else { 7nat }),
        next@.events.len() == trace.events.len() + 1,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::lifecycle_invariant_v1);
    proof { lifecycle_reader_reached_v1(trace, *actual, *model); }
    let ghost before = *actual;
    let ghost model_before = *model;
    let consumer = WriterKeyV1 { context_generation: 7, local: 50, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 50, kind: logical::WriterKindV1::Submission };
    let request = ContextAllocationReadV1 {
        allocation: AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 1 };
    let model_request = logical::AllocationReadV1 {
        allocation: logical::AllocationReferenceV1 { slot: 0, key: logical::AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 1 };
    let requests = vec![request];
    let model_requests = vec![model_request];
    let mut output = vec![None];
    let mut model_output = vec![None];
    let ghost original = output@;
    let ghost model_original = model_output@;
    proof {
        assert(stable_requests_view(requests@) =~= model_requests@);
        assert(stable_output_view(output@) =~= model_output@);
        reveal_with_fuel(stable_acquire_scan_v1, 3);
        reveal_with_fuel(stable_acquired_leases_v1, 3);
        reveal_with_fuel(stable_read_slot_count_v1, 3);
    }
    let acquired = producer_stable_acquire_historical_exec_v1(actual, model, consumer, model_consumer,
        &requests, &model_requests, &mut output, &mut model_output);
    assert(acquired.0 == Ok(()) && acquired.1 == Ok(()));
    assert(output@[0] == Some(ContextReadLeaseReferenceV1 { slot: 0, incarnation: if reuse { 2u64 } else { 1u64 }, consumer }));
    assert(actual.stable.leases@ =~= seq![Some(ReadLeaseV1 { reference: output@[0].unwrap(), request }), None]);
    assert(actual.stable.readers@ =~= seq![1usize, 0, 0]);
    assert(actual.stable.free_reads@ =~= seq![1usize]);
    assert(requests@ =~= seq![request]);
    assert(model_requests@ =~= seq![model_request]);
    assert(original =~= seq![None]);
    assert(model_original =~= seq![None]);
    assert(output@ =~= seq![Some(ContextReadLeaseReferenceV1 {
        slot: 0, incarnation: if reuse { 2u64 } else { 1u64 }, consumer })]);
    assert(model_output@ =~= seq![Some(read_reference_view(output@[0].unwrap()))]);
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::AcquireStable { consumer, requests: requests@, original, output: output@, result: acquired.0 },
        model: logical::LifecycleStepV1::AcquireStable { consumer: model_consumer, requests: model_requests@,
            original: model_original, output: model_output@, result: acquired.1 },
    };
    let ghost mut next = trace;
    proof { next = lifecycle_reader_append_v1(trace, before, *actual, model_before, *model, event, if reuse { 10nat } else { 7nat }); }
    Ghost(next)
}

#[verifier::spinoff_prover]
fn lifecycle_reader_release_producer_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    Ghost(trace): Ghost<LifecycleReaderTraceV1>) -> (next: Ghost<LifecycleReaderTraceV1>)
    requires lifecycle_reader_trace_v1(trace, *old(actual), *old(model)), lifecycle_reader_contents_v1(*old(actual), 7),
    ensures lifecycle_reader_trace_v1(next@, *final(actual), *final(model)), lifecycle_reader_contents_v1(*final(actual), 8),
        lifecycle_reader_extension_v1(trace, next@, *final(actual), *final(model), 8),
        next@.events.len() == trace.events.len() + 1,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::lifecycle_invariant_v1);
    proof { lifecycle_reader_reached_v1(trace, *actual, *model); }
    let ghost before = *actual;
    let ghost model_before = *model;
    let reference = actual.reservations[0].unwrap().reference;
    let model_reference = model.reservations[0].unwrap().reference;
    let references = vec![reference];
    let model_references = vec![model_reference];
    proof {
        assert(producer_references_view(references@) =~= model_references@);
        assert(references@ =~= seq![reference]);
        assert(model_references@ =~= seq![model_reference]);
        reveal_with_fuel(producer_release_scan_v1, 3);
        reveal_with_fuel(producer_released_reservations_v1, 3);
        reveal_with_fuel(producer_request_count_v1, 3);
    }
    let released = producer_release_historical_exec_v1(actual, model, reference.consumer, model_reference.consumer,
        &references, &model_references, &ContextReadQuiescenceEvidenceV1 { consumer: reference.consumer }, model_reference.consumer, 2);
    assert(released.0 == Ok(()) && released.1 == Ok(()));
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::ReleaseProducer { consumer: reference.consumer, references: references@,
            evidence: reference.consumer, capacity: 2, result: released.0 },
        model: logical::LifecycleStepV1::ReleaseProducer { consumer: model_reference.consumer, references: model_references@,
            evidence: model_reference.consumer, capacity: 2, result: released.1 },
    };
    assert(actual.reservations@ =~= seq![None, None]);
    assert(actual.counts@ =~= seq![0usize, 0, 0]);
    assert(actual.free@ =~= seq![1usize, 0]);
    let ghost mut next = trace;
    proof { next = lifecycle_reader_append_v1(trace, before, *actual, model_before, *model, event, 8); }
    Ghost(next)
}

#[verifier::spinoff_prover]
fn lifecycle_reader_release_stable_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    reused: bool, Ghost(trace): Ghost<LifecycleReaderTraceV1>) -> (next: Ghost<LifecycleReaderTraceV1>)
    requires lifecycle_reader_trace_v1(trace, *old(actual), *old(model)),
        lifecycle_reader_contents_v1(*old(actual), if reused { 10nat } else { 8nat }),
    ensures lifecycle_reader_trace_v1(next@, *final(actual), *final(model)),
        lifecycle_reader_contents_v1(*final(actual), if reused { 11nat } else { 9nat }),
        lifecycle_reader_extension_v1(trace, next@, *final(actual), *final(model), if reused { 11nat } else { 9nat }),
        next@.events.len() == trace.events.len() + 1,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::lifecycle_invariant_v1);
    proof { lifecycle_reader_reached_v1(trace, *actual, *model); }
    let ghost before = *actual;
    let ghost model_before = *model;
    let reference = actual.stable.leases[0].unwrap().reference;
    let model_reference = model.stable.leases[0].unwrap().reference;
    let references = vec![reference];
    let model_references = vec![model_reference];
    proof {
        assert(stable_references_view(references@) =~= model_references@);
        assert(references@ =~= seq![reference]);
        assert(model_references@ =~= seq![model_reference]);
        reveal_with_fuel(stable_release_scan_v1, 3);
        reveal_with_fuel(stable_released_leases_v1, 3);
        reveal_with_fuel(stable_read_slot_count_v1, 3);
    }
    let released = producer_stable_release_historical_exec_v1(actual, model, reference.consumer, model_reference.consumer,
        &references, &model_references, &ContextReadQuiescenceEvidenceV1 { consumer: reference.consumer }, model_reference.consumer, 2);
    assert(released.0 == Ok(()) && released.1 == Ok(()));
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::ReleaseStable { consumer: reference.consumer, references: references@,
            evidence: reference.consumer, capacity: 2, result: released.0 },
        model: logical::LifecycleStepV1::ReleaseStable { consumer: model_reference.consumer, references: model_references@,
            evidence: model_reference.consumer, capacity: 2, result: released.1 },
    };
    assert(actual.stable.leases@ =~= seq![None, None]);
    assert(actual.stable.readers@ =~= seq![0usize, 0, 0]);
    assert(actual.stable.free_reads@ =~= seq![1usize, 0]);
    let ghost mut next = trace;
    proof { next = lifecycle_reader_append_v1(trace, before, *actual, model_before, *model, event, if reused { 11nat } else { 9nat }); }
    Ghost(next)
}

#[verifier::spinoff_prover]
fn lifecycle_reader_abort_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    Ghost(trace): Ghost<LifecycleReaderTraceV1>) -> (next: Ghost<LifecycleReaderTraceV1>)
    requires lifecycle_reader_trace_v1(trace, *old(actual), *old(model)), lifecycle_reader_contents_v1(*old(actual), 11),
    ensures lifecycle_reader_trace_v1(next@, *final(actual), *final(model)), lifecycle_reader_contents_v1(*final(actual), 12),
        lifecycle_reader_extension_v1(trace, next@, *final(actual), *final(model), 12),
        next@.events.len() == trace.events.len() + 1,
{
    hide(lifecycle_reader_trace_v1);
    proof { lifecycle_reader_reached_v1(trace, *actual, *model); }
    let ghost before = *actual;
    let ghost model_before = *model;
    let reference = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission } };
    let model_reference = logical::WriterReferenceV1 { slot: 0, key: logical::WriterKeyV1 { context_generation: 7, local: 11, kind: logical::WriterKindV1::Submission } };
    let aborted = owner_abort_historical_exec_v1(actual, model, reference, model_reference, 1,
        Ghost(logical::witness_storage_v1(3, 1)), Ghost(lifecycle_paired_history_v1(trace.events, trace.events.len()).writers));
    assert(aborted.0 == Ok(()) && aborted.1 == Ok(()));
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::Abort { reference, capacity: 1, result: aborted.0 },
        model: logical::LifecycleStepV1::Abort { reference: model_reference, capacity: 1, result: aborted.1 },
    };
    assert(actual.stable.journal.writers@ =~= seq![None]);
    assert(actual.stable.journal.free@ =~= seq![0usize]);
    let ghost mut next = trace;
    proof { next = lifecycle_reader_append_v1(trace, before, *actual, model_before, *model, event, 12); }
    Ghost(next)
}

#[verifier::spinoff_prover]
proof fn lifecycle_reader_complete_v1(trace: LifecycleReaderTraceV1,
    actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1)
    requires lifecycle_reader_trace_v1(trace, actual, model), lifecycle_reader_contents_v1(actual, 12),
    ensures logical::lifecycle_invariant_v1(model, logical::witness_storage_v1(3, 1),
            lifecycle_paired_history_v1(trace.events, trace.events.len())),
        lifecycle_paired_history_v1(trace.events, trace.events.len()).stable.len() == 2,
        lifecycle_paired_history_v1(trace.events, trace.events.len()).producer.len() == 1,
        lifecycle_paired_history_v1(trace.events, trace.events.len()).stable[0].incarnation
            < lifecycle_paired_history_v1(trace.events, trace.events.len()).stable[1].incarnation,
        forall|i: int| 0 <= i < trace.events.len() ==> lifecycle_event_answers_v1(#[trigger] trace.events[i]),
        forall|i: int| 0 <= i < trace.events.len() ==> lifecycle_event_domain_v1(trace.actual[i], #[trigger] trace.events[i]),
{
    let storage = logical::witness_storage_v1(3, 1);
    lifecycle_constructor_origin_trace_v1(trace.origin, trace.actual, trace.model, trace.events, storage);
    lifecycle_reached_operation_domains_v1(trace.origin, trace.actual, trace.model, trace.events, storage);
    lifecycle_reader_reached_v1(trace, actual, model);
    let history = lifecycle_paired_history_v1(trace.events, trace.events.len());
    assert(history.stable.len() == 2 && history.producer.len() == 1);
    lifecycle_paired_history_nonreissue_v1(trace.origin, trace.actual, trace.model, trace.events,
        storage, trace.events.len(), 0, 1);
}

#[verifier::spinoff_prover]
fn lifecycle_reader_mixed_reuse_witness_v1() -> (result: bool)
    ensures result,
{
    hide(lifecycle_reader_expected_event_v1);
    hide(lifecycle_reader_trace_v1);
    hide(lifecycle_event_answers_v1);
    hide(lifecycle_event_domain_v1);
    hide(logical::lifecycle_invariant_v1);
    let (mut actual, mut model, initial) = lifecycle_reader_origin_v1();
    let ghost origin = initial@.origin;
    let trace = lifecycle_reader_enroll_v1(&mut actual, &mut model, initial);
    let trace = lifecycle_reader_register_v1(&mut actual, &mut model, false, trace);
    let trace = lifecycle_reader_begin_v1(&mut actual, &mut model, trace);
    let trace = lifecycle_reader_acquire_producer_v1(&mut actual, &mut model, trace);
    let trace = lifecycle_reader_settle_v1(&mut actual, &mut model, trace);
    let trace = lifecycle_reader_register_v1(&mut actual, &mut model, true, trace);
    let trace = lifecycle_reader_reused_producer_query_v1(&actual, &model, trace);
    let trace = lifecycle_reader_acquire_stable_v1(&mut actual, &mut model, false, trace);
    let first = actual.stable.leases[0].unwrap().reference;
    let trace = lifecycle_reader_release_producer_v1(&mut actual, &mut model, trace);
    let trace = lifecycle_reader_release_stable_v1(&mut actual, &mut model, false, trace);
    let trace = lifecycle_reader_acquire_stable_v1(&mut actual, &mut model, true, trace);
    let second = actual.stable.leases[0].unwrap().reference;
    assert(first.slot == second.slot && first.incarnation == 1 && second.incarnation == 2);
    let trace = lifecycle_reader_release_stable_v1(&mut actual, &mut model, true, trace);
    let trace = lifecycle_reader_abort_v1(&mut actual, &mut model, trace);
    proof {
        assert(trace@.origin == origin);
        assert(trace@.events.len() == 13);
        assert(trace@.events == seq![
            lifecycle_reader_expected_event_v1(1), lifecycle_reader_expected_event_v1(2),
            lifecycle_reader_expected_event_v1(3), lifecycle_reader_expected_event_v1(4),
            lifecycle_reader_expected_event_v1(5), lifecycle_reader_expected_event_v1(6),
            lifecycle_reader_expected_event_v1(13), lifecycle_reader_expected_event_v1(7),
            lifecycle_reader_expected_event_v1(8), lifecycle_reader_expected_event_v1(9),
            lifecycle_reader_expected_event_v1(10), lifecycle_reader_expected_event_v1(11),
            lifecycle_reader_expected_event_v1(12)]);
        lifecycle_reader_complete_v1(trace@, actual, model);
        assert(actual.stable.journal.registration_watermark == 11);
        assert(actual.stable.journal.reserved_count == 0);
        assert(actual.stable.leases@ == seq![None, None] && actual.reservations@ == seq![None, None]);
    }
    true
}

}
