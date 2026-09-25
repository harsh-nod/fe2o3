// Constructor-origin mixed acquisition after both reader arenas have already been reused.
verus! {

#[verifier::spinoff_prover]
fn lifecycle_mixed_reused_prefix_v1()
    -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1, Ghost<LifecycleReaderTraceV1>))
    ensures lifecycle_reader_trace_v1(result.2@, result.0, result.1),
        lifecycle_reader_contents_v1(result.0, 11), result.2@.events.len() == 12,
{
    hide(lifecycle_reader_expected_event_v1);
    hide(lifecycle_reader_trace_v1);
    let (mut actual, mut model, trace) = lifecycle_reader_origin_v1();
    let trace = lifecycle_reader_enroll_v1(&mut actual, &mut model, trace);
    let trace = lifecycle_reader_register_v1(&mut actual, &mut model, false, trace);
    let trace = lifecycle_reader_begin_v1(&mut actual, &mut model, trace);
    let trace = lifecycle_reader_acquire_producer_v1(&mut actual, &mut model, trace);
    let trace = lifecycle_reader_settle_v1(&mut actual, &mut model, trace);
    let trace = lifecycle_reader_register_v1(&mut actual, &mut model, true, trace);
    let trace = lifecycle_reader_reused_producer_query_v1(&actual, &model, trace);
    let trace = lifecycle_reader_acquire_stable_v1(&mut actual, &mut model, false, trace);
    let trace = lifecycle_reader_release_producer_v1(&mut actual, &mut model, trace);
    let trace = lifecycle_reader_release_stable_v1(&mut actual, &mut model, false, trace);
    let trace = lifecycle_reader_acquire_stable_v1(&mut actual, &mut model, true, trace);
    let trace = lifecycle_reader_release_stable_v1(&mut actual, &mut model, true, trace);
    (actual, model, trace)
}

spec fn lifecycle_mixed_ready_v1(owner: ContextProducerReadJournalV1) -> bool {
    let device = ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 };
    let allocation = AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } };
    let writer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission } };
    &&& owner.stable.journal.context_generation == 7
    &&& owner.stable.journal.allocation_capacity == 3 && owner.stable.journal.writer_capacity == 1
    &&& owner.stable.journal.registration_watermark == 11 && owner.stable.journal.reserved_count == 0
    &&& owner.stable.journal.writers@ == seq![Some(WriterEntryV1::Pending { key: writer.key, head: Some(0usize), count: 1usize })]
    &&& owner.stable.journal.free@ == seq![]
    &&& owner.stable.journal.allocations@ == seq![Some(AllocationEntryV1 {
        key: allocation.key, device, byte_extent: 16, attempt_epoch: 2, content_lineage: 1, pending_member: Some(0usize) }),
        Some(AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 21 }, device,
            byte_extent: 16, attempt_epoch: 0, content_lineage: 0, pending_member: None }), None]
    &&& owner.stable.journal.allocation_free@ == seq![2usize]
    &&& owner.stable.journal.members@ == seq![Some(MemberEntryV1 {
        writer, allocation, prior_lineage: 1, attempt_epoch: 2, next: None }), None, None]
    &&& owner.stable.journal.member_free@ == seq![2usize, 1]
    &&& owner.stable.journal.scratch@.len() == 3
    &&& owner.stable.leases@ == seq![None, None] && owner.stable.free_reads@ == seq![1usize, 0]
    &&& owner.stable.readers@ == seq![0usize, 0, 0] && owner.stable.next_incarnation == 3
    &&& owner.reservations@ == seq![None, None] && owner.free@ == seq![1usize, 0]
    &&& owner.counts@ == seq![0usize, 0, 0] && owner.next_incarnation == 2
}

#[verifier::spinoff_prover]
fn lifecycle_mixed_fixture_v1()
    -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1, Ghost<LifecycleReaderTraceV1>))
    ensures lifecycle_reader_trace_v1(result.2@, result.0, result.1),
        lifecycle_mixed_ready_v1(result.0), result.2@.events.len() == 14,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::issued_producer_v1);
    hide(logical::producer_invariant_v1);
    let (mut actual, mut model, trace) = lifecycle_mixed_reused_prefix_v1();
    proof { lifecycle_reader_reached_v1(trace@, actual, model); }
    let ghost before = actual;
    let ghost model_before = model;
    let entry = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 21 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 };
    let model_entry = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 21 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 };
    let enrolled = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, entry, model_entry,
        Ghost(logical::witness_storage_v1(3, 1)), Ghost(lifecycle_paired_history_v1(trace@.events, trace@.events.len()).writers));
    assert(enrolled.0 == Ok(AllocationReferenceV1 { slot: 1, key: entry.key }));
    assert(actual.stable.journal.allocations@ =~= before.stable.journal.allocations@.update(1, Some(enrollment_value_v1(entry))));
    assert(actual.stable.journal.allocation_free@ =~= seq![2usize]);
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::EnrollScalar { entry, result: enrolled.0 },
        model: logical::LifecycleStepV1::EnrollScalar { entry: model_entry, result: enrolled.1 },
    };
    let ghost mut next = trace@;
    proof {
        next = lifecycle_reader_append_event_v1(trace@, before, actual, model_before, model, event);
        lifecycle_reader_reached_v1(next, actual, model);
    }
    let ghost before_begin = actual;
    let ghost model_before_begin = model;
    let writer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission } };
    let model_writer = logical::WriterReferenceV1 { slot: 0, key: logical::WriterKeyV1 { context_generation: 7, local: 11, kind: logical::WriterKindV1::Submission } };
    let (roster, model_roster) = owner_disposal_roster_fixture_v1();
    proof {
        assert(actual.stable.journal.scratch@[0].is_none()) by { reveal(logical::producer_invariant_v1); }
        reveal_with_fuel(begin_members_prefix_v1, 3);
        reveal_with_fuel(begin_allocations_prefix_v1, 3);
        reveal_with_fuel(producer_unread_writes_v1, 3);
        reveal_with_fuel(stable_unread_writes_v1, 3);
        reveal_with_fuel(begin_canonical_scan_v1, 3);
        reveal_with_fuel(begin_destination_scan_v1, 3);
        reveal_with_fuel(begin_slot_scan_v1, 3);
    }
    let begun = producer_begin_historical_exec_v1(&mut actual, &mut model, writer, model_writer, &roster, &model_roster,
        Ghost(logical::witness_storage_v1(3, 1)), Ghost(lifecycle_paired_history_v1(next.events, next.events.len()).writers));
    assert(begun.0 == Ok(()) && begun.1 == Ok(()));
    assert(actual.stable.journal.writers@ =~= seq![Some(WriterEntryV1::Pending { key: writer.key, head: Some(0usize), count: 1usize })]);
    assert(actual.stable.journal.members@ =~= seq![Some(MemberEntryV1 {
        writer, allocation: roster@[0].allocation, prior_lineage: 1, attempt_epoch: 2, next: None }), None, None]);
    assert(actual.stable.journal.member_free@ =~= seq![2usize, 1]);
    assert(actual.stable.journal.allocations@ =~= before_begin.stable.journal.allocations@.update(0, Some(AllocationEntryV1 {
        key: roster@[0].allocation.key, device: roster@[0].device, byte_extent: 16,
        attempt_epoch: 2, content_lineage: 1, pending_member: Some(0usize) })));
    assert(actual.stable.journal.allocations@ =~= seq![Some(AllocationEntryV1 {
        key: roster@[0].allocation.key, device: roster@[0].device, byte_extent: 16,
        attempt_epoch: 2, content_lineage: 1, pending_member: Some(0usize) }), Some(enrollment_value_v1(entry)), None]);
    assert(actual.stable.leases@ == seq![None, None] && actual.stable.next_incarnation == 3);
    assert(actual.reservations@ == seq![None, None] && actual.next_incarnation == 2);
    assert(actual.stable.journal.registration_watermark == 11 && actual.stable.journal.reserved_count == 0);
    assert(actual.stable.journal.free@ == seq![] && actual.stable.journal.allocation_free@ == seq![2usize]);
    assert(lifecycle_mixed_ready_v1(actual));
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::Begin { writer, roster: roster@, result: begun.0 },
        model: logical::LifecycleStepV1::Begin { writer: model_writer, roster: model_roster@, result: begun.1 },
    };
    proof { next = lifecycle_reader_append_event_v1(next, before_begin, actual, model_before_begin, model, event); }
    (actual, model, Ghost(next))
}

spec fn lifecycle_mixed_stable_request_v1() -> ContextAllocationReadV1 {
    ContextAllocationReadV1 {
        allocation: AllocationReferenceV1 { slot: 1, key: AllocationKeyV1 { context_generation: 7, local: 21 } },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        byte_offset: 0, byte_len: 8, attempt_epoch: 0, content_lineage: 0 }
}

spec fn lifecycle_mixed_pending_request_v1(bad: bool) -> ContextProducerReadV1 {
    ContextProducerReadV1 {
        producer: WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission } },
        read: ContextAllocationReadV1 {
            allocation: AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
            byte_offset: 0, byte_len: if bad { 0u64 } else { 8u64 }, attempt_epoch: 2, content_lineage: 1 } }
}

fn lifecycle_mixed_requests_v1(bad: bool) -> (result: (ContextAllocationReadV1, logical::AllocationReadV1,
    ContextProducerReadV1, logical::ProducerReadV1))
    ensures result.0 == lifecycle_mixed_stable_request_v1(), result.2 == lifecycle_mixed_pending_request_v1(bad),
        allocation_read_view(result.0) == result.1, producer_read_view(result.2) == result.3,
{
    let stable = ContextAllocationReadV1 {
        allocation: AllocationReferenceV1 { slot: 1, key: AllocationKeyV1 { context_generation: 7, local: 21 } },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        byte_offset: 0, byte_len: 8, attempt_epoch: 0, content_lineage: 0 };
    let model_stable = logical::AllocationReadV1 {
        allocation: logical::AllocationReferenceV1 { slot: 1, key: logical::AllocationKeyV1 { context_generation: 7, local: 21 } },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        byte_offset: 0, byte_len: 8, attempt_epoch: 0, content_lineage: 0 };
    let pending = ContextProducerReadV1 {
        producer: WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission } },
        read: ContextAllocationReadV1 {
            allocation: AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
            byte_offset: 0, byte_len: if bad { 0 } else { 8 }, attempt_epoch: 2, content_lineage: 1 } };
    let model_pending = logical::ProducerReadV1 {
        producer: logical::WriterReferenceV1 { slot: 0, key: logical::WriterKeyV1 { context_generation: 7, local: 11, kind: logical::WriterKindV1::Submission } },
        read: logical::AllocationReadV1 {
            allocation: logical::AllocationReferenceV1 { slot: 0, key: logical::AllocationKeyV1 { context_generation: 7, local: 20 } },
            device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
            byte_offset: 0, byte_len: if bad { 0 } else { 8 }, attempt_epoch: 2, content_lineage: 1 } };
    (stable, model_stable, pending, model_pending)
}

#[verifier::spinoff_prover]
fn lifecycle_mixed_record_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    consumer: WriterKeyV1, model_consumer: logical::WriterKeyV1,
    stable: &[ContextAllocationReadV1], model_stable: &[logical::AllocationReadV1],
    pending: &[ContextProducerReadV1], model_pending: &[logical::ProducerReadV1],
    stable_output: &mut [Option<ContextReadLeaseReferenceV1>], model_stable_output: &mut Vec<Option<logical::ReadReferenceV1>>,
    producer_output: &mut [Option<ContextProducerReadReferenceV1>], model_producer_output: &mut Vec<Option<logical::ProducerReadReferenceV1>>,
    Ghost(trace): Ghost<LifecycleReaderTraceV1>,
) -> (result: (Result<(), ReadErrorV1>, Ghost<LifecycleReaderTraceV1>))
    requires lifecycle_reader_trace_v1(trace, *old(actual), *old(model)),
        writer_key_view(consumer) == model_consumer, stable_requests_view(stable@) == model_stable@,
        producer_requests_view(pending@) == model_pending@,
        stable_output_view(old(stable_output)@) == old(model_stable_output)@,
        producer_output_view(old(producer_output)@) == old(model_producer_output)@,
        stable@.len() <= u64::MAX, pending@.len() <= u64::MAX,
    ensures lifecycle_reader_trace_v1(result.1@, *final(actual), *final(model)),
        result.1@.origin == trace.origin, result.1@.actual == trace.actual.push(*final(actual)),
        result.1@.model == trace.model.push(*final(model)), result.1@.events.len() == trace.events.len() + 1,
        result.1@.events.take(trace.events.len() as int) == trace.events,
        mixed_acquire_relation_v1(*old(actual), *final(actual), consumer, stable@, pending@,
            old(stable_output)@, final(stable_output)@, old(producer_output)@, final(producer_output)@, result.0),
        match result.1@.events.last() {
            LifecyclePairedEventV1::Mutation { actual: step, .. } => step == LifecycleActualStepV1::AcquireMixed {
                consumer, stable: stable@, pending: pending@, stable_original: old(stable_output)@,
                stable_output: final(stable_output)@, producer_original: old(producer_output)@,
                producer_output: final(producer_output)@, result: result.0 },
            _ => false,
        },
{
    hide(lifecycle_reader_trace_v1);
    proof { lifecycle_reader_reached_v1(trace, *actual, *model); }
    let ghost before = *actual;
    let ghost model_before = *model;
    let ghost stable_before = stable_output@;
    let ghost producer_before = producer_output@;
    let ghost model_stable_before = model_stable_output@;
    let ghost model_producer_before = model_producer_output@;
    let _stable_count = stable.len();
    let _pending_count = pending.len();
    let _stable_output_count = stable_output.len();
    let _producer_output_count = producer_output.len();
    let acquired = mixed_acquire_paired_exec_v1(actual, model, consumer, model_consumer,
        stable, model_stable, pending, model_pending, stable_output, model_stable_output, producer_output, model_producer_output);
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::AcquireMixed { consumer, stable: stable@, pending: pending@,
            stable_original: stable_before, stable_output: stable_output@,
            producer_original: producer_before, producer_output: producer_output@, result: acquired.0 },
        model: logical::LifecycleStepV1::AcquireMixed { consumer: model_consumer, stable: model_stable@, pending: model_pending@,
            stable_original: model_stable_before, stable_output: model_stable_output@,
            producer_original: model_producer_before, producer_output: model_producer_output@, result: acquired.1 },
    };
    let ghost mut next = trace;
    proof {
        if let LifecyclePairedEventV1::Mutation { actual, model } = event {
            assert(lifecycle_step_inputs_v1(actual, model));
        }
        next = trace;
        assert(next.events.take(trace.events.len() as int) =~= trace.events);
    }
    (acquired.0, Ghost(next))
}

spec fn lifecycle_mixed_held_v1(owner: ContextProducerReadJournalV1) -> bool {
    let consumer = WriterKeyV1 { context_generation: 7, local: 60, kind: WriterKindV1::Submission };
    &&& owner.stable.leases@ == seq![Some(ReadLeaseV1 {
        reference: ContextReadLeaseReferenceV1 { slot: 0, incarnation: 3, consumer }, request: lifecycle_mixed_stable_request_v1() }), None]
    &&& owner.stable.free_reads@ == seq![1usize] && owner.stable.readers@ == seq![0usize, 1, 0]
    &&& owner.stable.next_incarnation == 4
    &&& owner.reservations@ == seq![Some(ReservationV1 {
        reference: ContextProducerReadReferenceV1 { slot: 0, incarnation: 2, consumer }, request: lifecycle_mixed_pending_request_v1(false) }), None]
    &&& owner.free@ == seq![1usize] && owner.counts@ == seq![1usize, 0, 0]
    &&& owner.next_incarnation == 3
}

#[verifier::spinoff_prover]
fn lifecycle_mixed_acquired_fixture_v1()
    -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1, Ghost<LifecycleReaderTraceV1>))
    ensures lifecycle_reader_trace_v1(result.2@, result.0, result.1), result.2@.events.len() == 15,
        lifecycle_mixed_ready_v1(result.2@.actual[14]), lifecycle_mixed_held_v1(result.0),
        result.0.stable.journal == result.2@.actual[14].stable.journal,
        match result.2@.events.last() {
            LifecyclePairedEventV1::Mutation { actual: LifecycleActualStepV1::AcquireMixed { result, .. }, .. } => result == Ok(()),
            _ => false,
        },
{
    hide(lifecycle_reader_trace_v1);
    let (mut actual, mut model, trace) = lifecycle_mixed_fixture_v1();
    let ghost before = actual;
    let (stable, model_stable, pending, model_pending) = lifecycle_mixed_requests_v1(false);
    let consumer = WriterKeyV1 { context_generation: 7, local: 60, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 60, kind: logical::WriterKindV1::Submission };
    let stable_requests = vec![stable];
    let model_stable_requests = vec![model_stable];
    let pending_requests = vec![pending];
    let model_pending_requests = vec![model_pending];
    let mut stable_output = vec![None];
    let mut model_stable_output = vec![None];
    let mut producer_output = vec![None];
    let mut model_producer_output = vec![None];
    proof {
        assert(stable_requests_view(stable_requests@) =~= model_stable_requests@);
        assert(producer_requests_view(pending_requests@) =~= model_pending_requests@);
        assert(stable_output_view(stable_output@) =~= model_stable_output@);
        assert(producer_output_view(producer_output@) =~= model_producer_output@);
        reveal_with_fuel(stable_acquire_scan_v1, 3);
        reveal_with_fuel(producer_acquire_scan_v1, 3);
        reveal_with_fuel(stable_acquired_leases_v1, 3);
        reveal_with_fuel(producer_acquired_reservations_v1, 3);
        reveal_with_fuel(stable_read_slot_count_v1, 3);
        reveal_with_fuel(producer_request_count_v1, 3);
    }
    let (acquired, next) = lifecycle_mixed_record_v1(&mut actual, &mut model, consumer, model_consumer,
        &stable_requests, &model_stable_requests, &pending_requests, &model_pending_requests,
        &mut stable_output, &mut model_stable_output, &mut producer_output, &mut model_producer_output, trace);
    assert(acquired == Ok(()));
    proof {
        let middle = choose|middle: ContextProducerReadJournalV1|
            mixed_stable_commit_v1(before, middle, consumer, stable_requests@, seq![None], stable_output@)
            && mixed_producer_commit_v1(middle, actual, consumer, pending_requests@, seq![None], producer_output@);
        assert(stable_output@[0] == Some(ContextReadLeaseReferenceV1 { slot: 0, incarnation: 3, consumer }));
        assert(producer_output@[0] == Some(ContextProducerReadReferenceV1 { slot: 0, incarnation: 2, consumer }));
        assert(actual.stable.leases@ =~= seq![Some(ReadLeaseV1 { reference: stable_output@[0].unwrap(), request: stable }), None]);
        assert(actual.reservations@ =~= seq![Some(ReservationV1 { reference: producer_output@[0].unwrap(), request: pending }), None]);
        assert(actual.stable.free_reads@ =~= seq![1usize] && actual.free@ =~= seq![1usize]);
        assert(actual.stable.readers@ =~= seq![0usize, 1, 0] && actual.counts@ =~= seq![1usize, 0, 0]);
        reveal(lifecycle_reader_trace_v1);
        assert(trace@.actual[14] == before);
    }
    (actual, model, next)
}

#[verifier::spinoff_prover]
proof fn lifecycle_mixed_trace_audit_v1(trace: LifecycleReaderTraceV1,
    actual: ContextProducerReadJournalV1, model: logical::ProducerReadContentsV1)
    requires lifecycle_reader_trace_v1(trace, actual, model),
    ensures logical::lifecycle_invariant_v1(model, logical::witness_storage_v1(3, 1),
            lifecycle_paired_history_v1(trace.events, trace.events.len())),
        lifecycle_paired_history_v1(trace.events, trace.events.len()).stable.len() + 1 == actual.stable.next_incarnation,
        lifecycle_paired_history_v1(trace.events, trace.events.len()).producer.len() + 1 == actual.next_incarnation,
        forall|i: int| 0 <= i < trace.events.len() ==> lifecycle_event_answers_v1(#[trigger] trace.events[i]),
        forall|i: int| 0 <= i < trace.events.len() ==> lifecycle_event_domain_v1(trace.actual[i], #[trigger] trace.events[i]),
{
    let storage = logical::witness_storage_v1(3, 1);
    lifecycle_constructor_origin_trace_v1(trace.origin, trace.actual, trace.model, trace.events, storage);
    lifecycle_reached_operation_domains_v1(trace.origin, trace.actual, trace.model, trace.events, storage);
    lifecycle_reader_reached_v1(trace, actual, model);
}

#[verifier::spinoff_prover]
fn lifecycle_mixed_late_failure_witness_v1() -> (result: bool)
    ensures result,
{
    hide(lifecycle_reader_trace_v1);
    let (mut actual, mut model, trace) = lifecycle_mixed_fixture_v1();
    let ghost before = actual;
    let (stable, model_stable, pending, model_pending) = lifecycle_mixed_requests_v1(true);
    let consumer = WriterKeyV1 { context_generation: 7, local: 60, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 60, kind: logical::WriterKindV1::Submission };
    let stable_requests = vec![stable];
    let model_stable_requests = vec![model_stable];
    let pending_requests = vec![pending];
    let model_pending_requests = vec![model_pending];
    let mut stable_output = vec![None];
    let mut model_stable_output = vec![None];
    let mut producer_output = vec![None];
    let mut model_producer_output = vec![None];
    proof {
        assert(stable_requests_view(stable_requests@) =~= model_stable_requests@);
        assert(producer_requests_view(pending_requests@) =~= model_pending_requests@);
        assert(stable_output_view(stable_output@) =~= model_stable_output@);
        assert(producer_output_view(producer_output@) =~= model_producer_output@);
        reveal_with_fuel(stable_acquire_scan_v1, 3);
        reveal_with_fuel(producer_acquire_scan_v1, 3);
        assert(stable_acquire_decision_v1(actual.stable, consumer, stable_requests@, stable_output@) == Ok(()));
    }
    let (rejected, next) = lifecycle_mixed_record_v1(&mut actual, &mut model, consumer, model_consumer,
        &stable_requests, &model_stable_requests, &pending_requests, &model_pending_requests,
        &mut stable_output, &mut model_stable_output, &mut producer_output, &mut model_producer_output, trace);
    assert(rejected == Err(ReadErrorV1::InvalidExtent));
    assert(actual == before && stable_output@ == seq![None] && producer_output@ == seq![None]);
    proof {
        lifecycle_mixed_trace_audit_v1(next@, actual, model);
        let history = lifecycle_paired_history_v1(next@.events, next@.events.len());
        assert(next@.events.len() == 15 && history.stable.len() == 2 && history.producer.len() == 1);
    }
    true
}

#[verifier::spinoff_prover]
fn lifecycle_mixed_capacity_witness_v1() -> (result: bool)
    ensures result,
{
    hide(lifecycle_reader_trace_v1);
    let (mut actual, mut model, trace) = lifecycle_mixed_acquired_fixture_v1();
    proof { lifecycle_reader_reached_v1(trace@, actual, model); }
    let ghost before = actual;
    let (stable, model_stable, pending, model_pending) = lifecycle_mixed_requests_v1(false);
    let consumer = WriterKeyV1 { context_generation: 7, local: 60, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 60, kind: logical::WriterKindV1::Submission };
    let stable_requests = vec![stable];
    let model_stable_requests = vec![model_stable];
    let pending_requests = vec![pending];
    let model_pending_requests = vec![model_pending];
    let mut stable_output = vec![Some(actual.stable.leases[0].unwrap().reference)];
    let mut model_stable_output = vec![Some(model.stable.leases[0].unwrap().reference)];
    let mut producer_output = vec![Some(actual.reservations[0].unwrap().reference)];
    let mut model_producer_output = vec![Some(model.reservations[0].unwrap().reference)];
    let ghost stable_before = stable_output@;
    let ghost producer_before = producer_output@;
    proof {
        assert(stable_requests_view(stable_requests@) =~= model_stable_requests@);
        assert(producer_requests_view(pending_requests@) =~= model_pending_requests@);
        assert(stable_output_view(stable_output@) =~= model_stable_output@);
        assert(producer_output_view(producer_output@) =~= model_producer_output@);
    }
    let (rejected, next) = lifecycle_mixed_record_v1(&mut actual, &mut model, consumer, model_consumer,
        &stable_requests, &model_stable_requests, &pending_requests, &model_pending_requests,
        &mut stable_output, &mut model_stable_output, &mut producer_output, &mut model_producer_output, trace);
    assert(rejected == Err(ReadErrorV1::MemberCapacity));
    assert(actual == before && stable_output@ == stable_before && producer_output@ == producer_before);
    proof {
        lifecycle_mixed_trace_audit_v1(next@, actual, model);
        let history = lifecycle_paired_history_v1(next@.events, next@.events.len());
        assert(next@.events.len() == 16 && history.stable.len() == 3 && history.producer.len() == 2);
        reveal(lifecycle_reader_trace_v1);
        lifecycle_paired_history_nonreissue_v1(next@.origin, next@.actual, next@.model, next@.events,
            logical::witness_storage_v1(3, 1), next@.events.len(), 0, 1);
        lifecycle_paired_history_nonreissue_v1(next@.origin, next@.actual, next@.model, next@.events,
            logical::witness_storage_v1(3, 1), next@.events.len(), 1, 2);
        assert(history.stable[0].incarnation < history.stable[1].incarnation < history.stable[2].incarnation);
        assert(history.producer[0].incarnation < history.producer[1].incarnation);
    }
    true
}

#[verifier::spinoff_prover]
fn lifecycle_mixed_terminal_fixture_v1(success: bool)
    -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1, Ghost<LifecycleReaderTraceV1>))
    ensures lifecycle_reader_trace_v1(result.2@, result.0, result.1), result.2@.events.len() == 18,
        lifecycle_mixed_held_v1(result.0), result.0.stable.journal.context_generation == 7,
        stable_read_decision_v1(result.0.stable.journal, lifecycle_mixed_stable_request_v1()) == Ok(()),
        result.0.stable.journal.writer_capacity == 1, result.0.stable.journal.registration_watermark == 12,
        result.0.stable.journal.writers@ == seq![Some(WriterEntryV1::Reserved(WriterKeyV1 {
            context_generation: 7, local: 12, kind: WriterKindV1::Submission }))],
        producer_query_decision_v1(result.0, result.0.reservations@[0].unwrap().reference)
            == Ok(if success { ContextProducerReadStatusV1::Success } else { ContextProducerReadStatusV1::NoEffect }),
{
    hide(lifecycle_reader_trace_v1);
    let (mut actual, mut model, trace) = lifecycle_mixed_acquired_fixture_v1();
    proof { lifecycle_reader_reached_v1(trace@, actual, model); }
    let ghost before = actual;
    let ghost model_before = model;
    let writer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission } };
    let model_writer = logical::WriterReferenceV1 { slot: 0, key: logical::WriterKeyV1 { context_generation: 7, local: 11, kind: logical::WriterKindV1::Submission } };
    proof {
        reveal_with_fuel(owner_retained_scan_v1, 3);
        reveal_with_fuel(settlement_scratch_scan_v1, 3);
        reveal_with_fuel(settlement_cursor_v1, 3);
        reveal_with_fuel(settlement_allocations_prefix_v1, 3);
        reveal_with_fuel(settlement_members_prefix_v1, 3);
    }
    let settled = owner_settlement_historical_exec_v1(&mut actual, &mut model, writer, model_writer, writer, model_writer,
        1, 3, success, Ghost(logical::witness_storage_v1(3, 1)),
        Ghost(lifecycle_paired_history_v1(trace@.events, trace@.events.len()).writers));
    assert(settled.0 == Ok(()) && settled.1 == Ok(()));
    assert(actual.stable.journal.writers@ =~= seq![None]);
    assert(actual.stable.journal.free@ =~= seq![0usize]);
    assert(actual.stable.journal.members@ =~= seq![None, None, None]);
    assert(actual.stable.journal.member_free@ =~= seq![2usize, 1, 0]);
    assert(actual.stable.journal.allocations@ =~= before.stable.journal.allocations@.update(0, Some(AllocationEntryV1 {
        key: AllocationKeyV1 { context_generation: 7, local: 20 }, device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 },
        byte_extent: 16, attempt_epoch: 2, content_lineage: if success { 2u64 } else { 1u64 }, pending_member: None })));
    let ghost event = if success { LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::SettleSuccess { writer, evidence: writer, writer_storage: 1, member_storage: 3, result: settled.0 },
        model: logical::LifecycleStepV1::SettleSuccess { writer: model_writer, evidence: model_writer,
            writer_storage: 1, member_storage: 3, result: settled.1 },
    } } else { LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::SettleNoEffect { writer, evidence: writer, writer_storage: 1, member_storage: 3, result: settled.0 },
        model: logical::LifecycleStepV1::SettleNoEffect { writer: model_writer, evidence: model_writer,
            writer_storage: 1, member_storage: 3, result: settled.1 },
    } };
    let ghost mut next = trace@;
    proof {
        next = lifecycle_reader_append_event_v1(next, before, actual, model_before, model, event);
        lifecycle_reader_reached_v1(next, actual, model);
    }
    let ghost before_register = actual;
    let ghost model_before_register = model;
    let key = WriterKeyV1 { context_generation: 7, local: 12, kind: WriterKindV1::Submission };
    let model_key = logical::WriterKeyV1 { context_generation: 7, local: 12, kind: logical::WriterKindV1::Submission };
    let registered = owner_register_historical_exec_v1(&mut actual, &mut model, key, model_key,
        Ghost(logical::witness_storage_v1(3, 1)), Ghost(lifecycle_paired_history_v1(next.events, next.events.len()).writers));
    assert(registered.0 == Ok(WriterReferenceV1 { slot: 0, key }));
    assert(actual.stable.journal.writers@ =~= seq![Some(WriterEntryV1::Reserved(key))]);
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::Register { key, result: registered.0 },
        model: logical::LifecycleStepV1::Register { key: model_key, result: registered.1 },
    };
    proof {
        next = lifecycle_reader_append_event_v1(next, before_register, actual, model_before_register, model, event);
        lifecycle_reader_reached_v1(next, actual, model);
    }
    let reference = actual.reservations[0].unwrap().reference;
    let model_reference = model.reservations[0].unwrap().reference;
    let observed = producer_query_historical_exec_v1(&actual, &model, reference, model_reference);
    assert(observed.0 == Ok(if success { ContextProducerReadStatusV1::Success } else { ContextProducerReadStatusV1::NoEffect }));
    assert(actual.reservations@[0].unwrap().request.producer.key.local == 11);
    let ghost event = LifecyclePairedEventV1::Observe { query: LifecycleQueryV1::ProducerStatus(reference),
        actual_answer: LifecycleAnswerV1::Status(observed.0), model_answer: LifecycleAnswerV1::Status(producer_status_result_from(observed.1)) };
    proof { next = lifecycle_reader_append_event_v1(next, actual, actual, model, model, event); }
    (actual, model, Ghost(next))
}

#[verifier::spinoff_prover]
fn lifecycle_mixed_stable_release_v1(actual: &mut ContextProducerReadJournalV1, model: &mut logical::ProducerReadContentsV1,
    Ghost(trace): Ghost<LifecycleReaderTraceV1>) -> (result: Ghost<LifecycleReaderTraceV1>)
    requires lifecycle_reader_trace_v1(trace, *old(actual), *old(model)), lifecycle_mixed_held_v1(*old(actual)),
        stable_read_decision_v1(old(actual).stable.journal, lifecycle_mixed_stable_request_v1()) == Ok(()),
    ensures lifecycle_reader_trace_v1(result@, *final(actual), *final(model)), result@.events.len() == trace.events.len() + 1,
        result@.origin == trace.origin, result@.actual == trace.actual.push(*final(actual)),
        result@.model == trace.model.push(*final(model)),
        result@.events == trace.events.push(LifecyclePairedEventV1::Mutation {
            actual: LifecycleActualStepV1::ReleaseStable {
                consumer: old(actual).stable.leases@[0].unwrap().reference.consumer,
                references: seq![old(actual).stable.leases@[0].unwrap().reference],
                evidence: old(actual).stable.leases@[0].unwrap().reference.consumer, capacity: 2, result: Ok(()) },
            model: logical::LifecycleStepV1::ReleaseStable {
                consumer: old(model).stable.leases@[0].unwrap().reference.consumer,
                references: seq![old(model).stable.leases@[0].unwrap().reference],
                evidence: old(model).stable.leases@[0].unwrap().reference.consumer, capacity: 2, result: Ok(()) },
        }),
        final(actual).stable.leases@ == seq![None, None], final(actual).stable.readers@ == seq![0usize, 0, 0],
        final(actual).stable.free_reads@ == seq![1usize, 0], final(actual).stable.next_incarnation == 4,
        producer_reservations_frame_v1(*old(actual), *final(actual)), final(actual).stable.journal == old(actual).stable.journal,
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
        assert(references@ =~= seq![reference]);
        assert(model_references@ =~= seq![model_reference]);
        assert(stable_references_view(references@) =~= model_references@);
        reveal_with_fuel(stable_release_scan_v1, 3);
        reveal_with_fuel(stable_released_leases_v1, 3);
        reveal_with_fuel(stable_read_slot_count_v1, 3);
    }
    let released = producer_stable_release_historical_exec_v1(actual, model, reference.consumer, model_reference.consumer,
        &references, &model_references, &ContextReadQuiescenceEvidenceV1 { consumer: reference.consumer }, model_reference.consumer, 2);
    assert(released.0 == Ok(()) && released.1 == Ok(()));
    assert(actual.stable.leases@ =~= seq![None, None]);
    assert(actual.stable.readers@ =~= seq![0usize, 0, 0]);
    assert(actual.stable.free_reads@ =~= seq![1usize, 0]);
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::ReleaseStable { consumer: reference.consumer, references: references@,
            evidence: reference.consumer, capacity: 2, result: released.0 },
        model: logical::LifecycleStepV1::ReleaseStable { consumer: model_reference.consumer, references: model_references@,
            evidence: model_reference.consumer, capacity: 2, result: released.1 },
    };
    let ghost mut next = trace;
    proof {
        next = lifecycle_reader_append_event_v1(next, before, *actual, model_before, *model, event);
    }
    Ghost(next)
}

#[verifier::spinoff_prover]
fn lifecycle_mixed_settlement_witness_v1(success: bool) -> (result: bool)
    ensures result,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::lifecycle_invariant_v1);
    let (mut actual, mut model, trace) = lifecycle_mixed_terminal_fixture_v1(success);
    let ghost terminal_trace = trace@;
    proof {
        assert(terminal_trace.actual.len() == 19 && terminal_trace.model.len() == 19) by {
            reveal(lifecycle_reader_trace_v1);
        }
    }
    let trace = lifecycle_mixed_stable_release_v1(&mut actual, &mut model, trace);
    let ghost mut next = trace@;
    proof { lifecycle_reader_reached_v1(next, actual, model); }
    let ghost before_release = actual;
    let ghost model_before_release = model;
    let reference = actual.reservations[0].unwrap().reference;
    let model_reference = model.reservations[0].unwrap().reference;
    let references = vec![reference];
    let model_references = vec![model_reference];
    proof {
        assert(producer_references_view(references@) =~= model_references@);
        reveal_with_fuel(producer_release_scan_v1, 3);
        reveal_with_fuel(producer_released_reservations_v1, 3);
        reveal_with_fuel(producer_request_count_v1, 3);
    }
    let released = producer_release_historical_exec_v1(&mut actual, &mut model, reference.consumer, model_reference.consumer,
        &references, &model_references, &ContextReadQuiescenceEvidenceV1 { consumer: reference.consumer }, model_reference.consumer, 2);
    assert(released.0 == Ok(()) && released.1 == Ok(()));
    assert(actual.reservations@ =~= seq![None, None]);
    assert(actual.counts@ =~= seq![0usize, 0, 0]);
    assert(actual.free@ =~= seq![1usize, 0]);
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::ReleaseProducer { consumer: reference.consumer, references: references@,
            evidence: reference.consumer, capacity: 2, result: released.0 },
        model: logical::LifecycleStepV1::ReleaseProducer { consumer: model_reference.consumer, references: model_references@,
            evidence: model_reference.consumer, capacity: 2, result: released.1 },
    };
    proof {
        next = lifecycle_reader_append_event_v1(next, before_release, actual, model_before_release, model, event);
        lifecycle_mixed_trace_audit_v1(next, actual, model);
        let history = lifecycle_paired_history_v1(next.events, next.events.len());
        assert(next.events.len() == 20 && history.stable.len() == 3 && history.producer.len() == 2);
        assert(next.origin == terminal_trace.origin);
        assert(next.events.take(18) =~= terminal_trace.events);
        assert(next.actual.take(19) =~= terminal_trace.actual);
        assert(next.model.take(19) =~= terminal_trace.model);
        assert(actual.stable.next_incarnation == 4 && actual.next_incarnation == 3);
        assert(producer_total_retained_v1(actual) == 0);
    }
    true
}

#[verifier::spinoff_prover]
fn lifecycle_mixed_unknown_witness_v1() -> (result: bool)
    ensures result,
{
    hide(lifecycle_reader_trace_v1);
    let (mut actual, mut model, trace) = lifecycle_mixed_acquired_fixture_v1();
    proof { lifecycle_reader_reached_v1(trace@, actual, model); }
    let ghost before = actual;
    let ghost model_before = model;
    let writer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission } };
    let model_writer = logical::WriterReferenceV1 { slot: 0, key: logical::WriterKeyV1 { context_generation: 7, local: 11, kind: logical::WriterKindV1::Submission } };
    proof { reveal_with_fuel(owner_retained_scan_v1, 3); }
    let unknown = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(unknown.0 == Ok(()) && unknown.1 == Ok(()));
    let ghost event = LifecyclePairedEventV1::Mutation {
        actual: LifecycleActualStepV1::Unknown { writer, result: unknown.0 },
        model: logical::LifecycleStepV1::Unknown { writer: model_writer, result: unknown.1 },
    };
    let ghost mut next = trace@;
    proof {
        next = lifecycle_reader_append_event_v1(next, before, actual, model_before, model, event);
        lifecycle_reader_reached_v1(next, actual, model);
    }
    let reference = actual.reservations[0].unwrap().reference;
    let model_reference = model.reservations[0].unwrap().reference;
    let observed = producer_query_historical_exec_v1(&actual, &model, reference, model_reference);
    assert(observed.0 == Ok(ContextProducerReadStatusV1::Unknown));
    assert(stable_read_decision_v1(actual.stable.journal, lifecycle_mixed_stable_request_v1()) == Ok(()));
    assert(lifecycle_mixed_held_v1(actual));
    assert(actual.stable.journal.free@ == seq![] && actual.stable.journal.members@ == before.stable.journal.members@);
    let ghost event = LifecyclePairedEventV1::Observe { query: LifecycleQueryV1::ProducerStatus(reference),
        actual_answer: LifecycleAnswerV1::Status(observed.0), model_answer: LifecycleAnswerV1::Status(producer_status_result_from(observed.1)) };
    proof {
        next = lifecycle_reader_append_event_v1(next, actual, actual, model, model, event);
        lifecycle_mixed_trace_audit_v1(next, actual, model);
        assert(next.events.len() == 17 && producer_total_retained_v1(actual) == 2);
    }
    true
}

}
