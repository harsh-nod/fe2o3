verus! {

spec fn owner_settlement_pending_fixture_contents_v1(actual: ContextProducerReadJournalV1, writer: WriterReferenceV1) -> bool {
    let first = AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } };
    writer == (WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } })
        && actual.stable.journal.context_generation == 7
        && actual.stable.journal.allocation_capacity == 3 && actual.stable.journal.writer_capacity == 1
        && actual.stable.journal.registration_watermark == 10 && actual.stable.journal.reserved_count == 0
        && actual.stable.journal.writers@ == seq![Some(WriterEntryV1::Pending { key: writer.key, head: Some(0usize), count: 1usize })]
        && actual.stable.journal.free@ == seq![] && actual.stable.journal.allocation_free@ == seq![2usize]
        && actual.stable.journal.member_free@ == seq![2usize, 1]
        && actual.stable.journal.scratch@ == seq![None, None, None]
        && actual.stable.journal.members@ == seq![Some(MemberEntryV1 {
            writer, allocation: first, prior_lineage: 0u64, attempt_epoch: 1u64, next: None }), None, None]
        && actual.stable.journal.allocations@ == seq![
            Some(AllocationEntryV1 { key: first.key, device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 },
                byte_extent: 16u64, attempt_epoch: 1u64, content_lineage: 0u64, pending_member: Some(0usize) }),
            Some(AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
                device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 },
                byte_extent: 32u64, attempt_epoch: 0u64, content_lineage: 0u64, pending_member: None }), None]
}

// Expose actual empty contents once, keeping inverse-view quantifiers out of
// the subsequent executable lifecycle checks.
#[verifier::spinoff_prover]
fn owner_settlement_empty_fixture_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1))
    ensures producer_represents(result.0, result.1), logical::producer_invariant_v1(result.1),
        logical::producer_constructor_relation_v1(7, 3, 1, 2, Ok(result.1)),
        result.0.stable.journal.context_generation == 7,
        result.0.stable.journal.allocation_capacity == 3, result.0.stable.journal.writer_capacity == 1,
        result.0.stable.journal.registration_watermark == 0, result.0.stable.journal.reserved_count == 0,
        result.0.stable.journal.writers@ == seq![None], result.0.stable.journal.free@ == seq![0usize],
        result.0.stable.journal.allocations@ == seq![None, None, None], result.0.stable.journal.allocation_free@ == seq![2usize, 1, 0],
        result.0.stable.journal.members@ == seq![None, None, None], result.0.stable.journal.member_free@ == seq![2usize, 1, 0],
        result.0.stable.journal.scratch@ == seq![None, None, None],
        result.0.counts@ == seq![0usize, 0, 0], result.0.reservations@ == seq![None, None],
        result.0.free@ == seq![1usize, 0], result.0.next_incarnation == 1,
        result.0.stable.readers@ == seq![0usize, 0, 0], result.0.stable.leases@ == seq![None, None],
        result.0.stable.free_reads@ == seq![1usize, 0], result.0.stable.next_incarnation == 1,
{
    let (mut actual, mut model) = owner_enrollment_fixture_v1().unwrap();
    proof {
        sequence_round_trip(concrete_contents(actual.stable.journal), logical_contents(model.stable.journal));
        assert forall|i: int| 0 <= i < actual.reservations@.len() implies
            (#[trigger] actual.reservations@[i]).is_none() by {
            producer_reservation_slot_round_trip(actual.reservations@[i], model.reservations@[i]);
        }
        assert forall|i: int| 0 <= i < actual.stable.leases@.len() implies
            (#[trigger] actual.stable.leases@[i]).is_none() by {
            read_lease_slot_round_trip(actual.stable.leases@[i], model.stable.leases@[i]);
        }
        assert(actual.stable.journal.writers@ =~= seq![None]);
        assert(actual.stable.journal.free@ =~= seq![0usize]);
        assert(actual.stable.journal.members@ =~= seq![None, None, None]);
        assert(actual.stable.journal.member_free@ =~= seq![2usize, 1, 0]);
        assert(actual.stable.journal.scratch@ =~= seq![None, None, None]);
        assert(actual.reservations@ =~= seq![None, None]);
        assert(actual.stable.leases@ =~= seq![None, None]);
    }
    (actual, model)
}

// The initial actual storage is synthetic; subsequent transitions execute both
// public/shared owner methods and the independent historical executor.
#[verifier::spinoff_prover]
fn owner_settlement_enrolled_fixture_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1))
    ensures producer_represents(result.0, result.1),
        logical::issued_producer_v1(result.1, logical::witness_storage_v1(3, 1), Seq::empty()),
        result.0.stable.journal.context_generation == 7,
        result.0.stable.journal.allocation_capacity == 3, result.0.stable.journal.writer_capacity == 1,
        result.0.stable.journal.registration_watermark == 0, result.0.stable.journal.reserved_count == 0,
        result.0.stable.journal.writers@ == seq![None], result.0.stable.journal.free@ == seq![0usize],
        result.0.stable.journal.allocation_free@ == seq![2usize],
        result.0.stable.journal.members@ == seq![None, None, None], result.0.stable.journal.member_free@ == seq![2usize, 1, 0],
        result.0.stable.journal.scratch@ == seq![None, None, None],
        result.0.stable.journal.allocations@ == seq![
            Some(AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
                device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 },
                byte_extent: 16u64, attempt_epoch: 0u64, content_lineage: 0u64, pending_member: None }),
            Some(AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
                device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 },
                byte_extent: 32u64, attempt_epoch: 0u64, content_lineage: 0u64, pending_member: None }), None],
        result.0.counts@ == seq![0usize, 0, 0], result.0.reservations@ == seq![None, None],
        result.0.free@ == seq![1usize, 0], result.0.next_incarnation == 1,
        result.0.stable.readers@ == seq![0usize, 0, 0], result.0.stable.leases@ == seq![None, None],
        result.0.stable.free_reads@ == seq![1usize, 0], result.0.stable.next_incarnation == 1,
{
    let (mut actual, mut model) = owner_settlement_empty_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    proof { logical::issued_constructor_v1(model.stable.journal, 7, 3, 1, storage); }
    let entries = vec![
        EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 },
        EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 }];
    let model_entries = vec![
        logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 20 },
            device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 },
        logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 30 },
            device: logical::DeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 }];
    let mut output = vec![None, None];
    let mut model_output = vec![None, None];
    let ghost before = model;
    let ghost original = model_output@;
    proof {
        assert(entries_view(entries@) =~= model_entries@);
        assert(references_view(output@) =~= model_output@);
        reveal_with_fuel(enrollment_roster_scan_v1, 4);
        reveal_with_fuel(enrollment_prefix_v1, 4);
    }
    let enrolled = producer_enrollment_historical_exec_v1(&mut actual, &mut model, &entries, &model_entries,
        &mut output, &mut model_output);
    assert(enrolled.0 == Ok(()) && enrolled.1 == Ok(()));
    assert(output@[0] == Some(AllocationReferenceV1 { slot: 0, key: entries@[0].key }));
    assert(output@[1] == Some(AllocationReferenceV1 { slot: 1, key: entries@[1].key }));
    assert(actual.stable.journal.allocations@ =~= seq![Some(enrollment_value_v1(entries@[0])), Some(enrollment_value_v1(entries@[1])), None]);
    proof {
        logical::enrollment_success_admission_v1(before.stable.journal, model_entries@, original);
        logical::enrollment_preserves_issued_producer_v1(before, model, model_entries@, model_output@, 1, storage, history);
    }
    (actual, model)
}

#[verifier::spinoff_prover]
fn owner_settlement_pending_fixture_v1() -> (result: (ContextProducerReadJournalV1,
    logical::ProducerReadContentsV1, WriterReferenceV1, logical::WriterReferenceV1))
    ensures producer_represents(result.0, result.1), writer_reference_view(result.2) == result.3,
        owner_settlement_pending_fixture_contents_v1(result.0, result.2),
        logical::issued_producer_v1(result.1, logical::witness_storage_v1(3, 1), seq![result.3]),
        result.0.counts@ == seq![0usize, 0, 0], result.0.reservations@ == seq![None, None],
        result.0.free@ == seq![1usize, 0], result.0.next_incarnation == 1,
        result.0.stable.readers@ == seq![0usize, 0, 0], result.0.stable.leases@ == seq![None, None],
        result.0.stable.free_reads@ == seq![1usize, 0], result.0.stable.next_incarnation == 1,
{
    let (mut actual, mut model) = owner_settlement_enrolled_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    let key = WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission };
    let model_key = logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission };
    let registered = owner_register_historical_exec_v1(&mut actual, &mut model, key, model_key, Ghost(storage), Ghost(history));
    assert(registered.0.is_ok() && registered.1.is_ok());
    let writer = registered.0.unwrap();
    assert(writer == (WriterReferenceV1 { slot: 0, key }));
    let model_writer = match registered.1 { Ok(value) => value, Err(_) => { assert(false); unreached() } };
    let ghost history = logical::registration_history_v1(history, registered.1);
    let roster = vec![AllocationWriteV1 {
        allocation: AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    let model_roster = vec![logical::AllocationWriteV1 {
        allocation: logical::AllocationReferenceV1 { slot: 0, key: logical::AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    proof {
        writer_reference_round_trip(writer, model_writer);
        assert(begin_roster_view(roster@) =~= model_roster@);
        reveal_with_fuel(begin_members_prefix_v1, 3);
        reveal_with_fuel(begin_allocations_prefix_v1, 3);
        reveal_with_fuel(producer_unread_writes_v1, 3);
        reveal_with_fuel(stable_unread_writes_v1, 3);
        reveal_with_fuel(begin_canonical_scan_v1, 3);
        reveal_with_fuel(begin_destination_scan_v1, 3);
        reveal_with_fuel(begin_slot_scan_v1, 3);
    }
    let begun = producer_begin_historical_exec_v1(&mut actual, &mut model, writer, model_writer,
        &roster, &model_roster, Ghost(storage), Ghost(history));
    assert(begun.0 == Ok(()) && begun.1 == Ok(()));
    assert(actual.stable.journal.context_generation == 7);
    assert(actual.stable.journal.allocation_capacity == 3 && actual.stable.journal.writer_capacity == 1);
    assert(actual.stable.journal.registration_watermark == 10 && actual.stable.journal.reserved_count == 0);
    assert(actual.stable.journal.writers@ =~= seq![Some(WriterEntryV1::Pending { key, head: Some(0usize), count: 1usize })]);
    assert(actual.stable.journal.free@ =~= seq![]);
    assert(actual.stable.journal.allocation_free@ == seq![2usize]);
    assert(actual.stable.journal.member_free@ =~= seq![2usize, 1]);
    assert(actual.stable.journal.scratch@ == seq![None, None, None]);
    assert(actual.stable.journal.members@ =~= seq![Some(MemberEntryV1 {
        writer, allocation: roster@[0].allocation, prior_lineage: 0u64, attempt_epoch: 1u64, next: None }), None, None]);
    assert(actual.stable.journal.allocations@ =~= seq![Some(AllocationEntryV1 {
        key: roster@[0].allocation.key, device: roster@[0].device, byte_extent: 16u64,
        attempt_epoch: 1u64, content_lineage: 0u64, pending_member: Some(0usize) }),
        Some(AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 },
            byte_extent: 32u64, attempt_epoch: 0u64, content_lineage: 0u64, pending_member: None }), None]);
    (actual, model, writer, model_writer)
}

spec fn owner_settlement_producer_fixture_contents_v1(actual: ContextProducerReadJournalV1, writer: WriterReferenceV1) -> bool {
    actual.counts@ == seq![1usize, 0, 0] && actual.free@ == seq![1usize] && actual.next_incarnation == 2
        && actual.reservations@ == seq![Some(ReservationV1 {
            reference: ContextProducerReadReferenceV1 { slot: 0, incarnation: 1,
                consumer: WriterKeyV1 { context_generation: 7, local: 40, kind: WriterKindV1::Submission } },
            request: ContextProducerReadV1 { producer: writer, read: ContextAllocationReadV1 {
                allocation: AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
                device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
                byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0 } } }), None]
}

#[verifier::spinoff_prover]
fn owner_settlement_producer_fixture_v1() -> (result: (ContextProducerReadJournalV1,
    logical::ProducerReadContentsV1, WriterReferenceV1, logical::WriterReferenceV1))
    ensures producer_represents(result.0, result.1), writer_reference_view(result.2) == result.3,
        owner_settlement_pending_fixture_contents_v1(result.0, result.2),
        logical::issued_producer_v1(result.1, logical::witness_storage_v1(3, 1), seq![result.3]),
        owner_settlement_producer_fixture_contents_v1(result.0, result.2),
        result.0.stable.readers@ == seq![0usize, 0, 0], result.0.stable.leases@ == seq![None, None],
        result.0.stable.free_reads@ == seq![1usize, 0], result.0.stable.next_incarnation == 1,
{
    let (mut actual, mut model, writer, model_writer) = owner_settlement_pending_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    let consumer = WriterKeyV1 { context_generation: 7, local: 40, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 40, kind: logical::WriterKindV1::Submission };
    let request = ContextProducerReadV1 { producer: writer, read: ContextAllocationReadV1 {
        allocation: AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0 } };
    let model_request = logical::ProducerReadV1 { producer: model_writer, read: logical::AllocationReadV1 {
        allocation: logical::AllocationReferenceV1 { slot: 0, key: logical::AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0 } };
    let requests = vec![request];
    let model_requests = vec![model_request];
    let mut reserved = vec![None];
    let mut model_reserved = vec![None];
    proof {
        assert(producer_requests_view(requests@) =~= model_requests@);
        assert(producer_output_view(reserved@) =~= model_reserved@);
        reveal_with_fuel(producer_acquire_scan_v1, 3);
        reveal_with_fuel(producer_acquired_reservations_v1, 3);
        reveal_with_fuel(producer_request_count_v1, 3);
    }
    let acquired = producer_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &requests, &model_requests, &mut reserved, &mut model_reserved);
    assert(acquired.0 == Ok(()) && acquired.1 == Ok(()));
    assert(reserved@[0].is_some() && reserved@[0].unwrap().slot == 0 && reserved@[0].unwrap().incarnation == 1);
    assert(reserved@[0] == Some(ContextProducerReadReferenceV1 { slot: 0, incarnation: 1, consumer }));
    assert(actual.reservations@[0].unwrap().request == request);
    assert(actual.reservations@[0].unwrap().reference == reserved@[0].unwrap());
    assert(producer_query_decision_v1(actual, reserved@[0].unwrap()) == Ok(ContextProducerReadStatusV1::Pending));
    assert(actual.counts@ =~= seq![1usize, 0, 0]);
    assert(actual.free@ =~= seq![1usize]);
    assert(actual.next_incarnation == 2);
    assert(actual.reservations@ =~= seq![Some(ReservationV1 { reference: reserved@[0].unwrap(), request }), None]);
    assert(logical::issued_producer_v1(model, storage, history));
    (actual, model, writer, model_writer)
}

#[verifier::spinoff_prover]
fn owner_settlement_read_fixture_v1() -> (result: (ContextProducerReadJournalV1,
    logical::ProducerReadContentsV1, WriterReferenceV1, logical::WriterReferenceV1))
    ensures producer_represents(result.0, result.1), writer_reference_view(result.2) == result.3,
        owner_settlement_pending_fixture_contents_v1(result.0, result.2),
        owner_settlement_producer_fixture_contents_v1(result.0, result.2),
        logical::issued_producer_v1(result.1, logical::witness_storage_v1(3, 1), seq![result.3]),
        result.0.stable.readers@ == seq![0usize, 1, 0], result.0.stable.free_reads@ == seq![1usize], result.0.stable.next_incarnation == 2,
        result.0.stable.leases@.len() == 2, result.0.stable.leases@[0].is_some(), result.0.stable.leases@[1].is_none(),
        result.0.stable.leases@[0].unwrap().request.allocation.slot == 1,
        stable_lease_decision_v1(result.0.stable, result.0.stable.leases@[0].unwrap().reference)
            == Ok(result.0.stable.leases@[0].unwrap().request),
{
    let (mut actual, mut model, writer, model_writer) = owner_settlement_producer_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    let consumer = WriterKeyV1 { context_generation: 7, local: 40, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 40, kind: logical::WriterKindV1::Submission };
    let stable_request = ContextAllocationReadV1 {
        allocation: AllocationReferenceV1 { slot: 1, key: AllocationKeyV1 { context_generation: 7, local: 30 } },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 },
        byte_extent: 32, byte_offset: 0, byte_len: 16, attempt_epoch: 0, content_lineage: 0 };
    let model_stable_request = logical::AllocationReadV1 {
        allocation: logical::AllocationReferenceV1 { slot: 1, key: logical::AllocationKeyV1 { context_generation: 7, local: 30 } },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 3 },
        byte_extent: 32, byte_offset: 0, byte_len: 16, attempt_epoch: 0, content_lineage: 0 };
    let stable_requests = vec![stable_request];
    let model_stable_requests = vec![model_stable_request];
    let mut leased = vec![None];
    let mut model_leased = vec![None];
    proof {
        assert(stable_requests_view(stable_requests@) =~= model_stable_requests@);
        assert(stable_output_view(leased@) =~= model_leased@);
        reveal_with_fuel(stable_acquire_scan_v1, 3);
        reveal_with_fuel(stable_acquired_leases_v1, 3);
        reveal_with_fuel(stable_read_slot_count_v1, 3);
    }
    let acquired = producer_stable_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &stable_requests, &model_stable_requests, &mut leased, &mut model_leased);
    assert(acquired.0 == Ok(()) && acquired.1 == Ok(()));
    assert(leased@[0].is_some() && leased@[0].unwrap().slot == 0 && leased@[0].unwrap().incarnation == 1);
    assert(actual.stable.leases@[0].unwrap().request == stable_request);
    assert(actual.stable.leases@[0].unwrap().reference == leased@[0].unwrap());
    assert(stable_lease_decision_v1(actual.stable, leased@[0].unwrap()) == Ok(stable_request));
    assert(actual.stable.readers@ =~= seq![0usize, 1, 0]);
    assert(logical::issued_producer_v1(model, storage, history));
    (actual, model, writer, model_writer)
}

#[verifier::spinoff_prover]
fn owner_settlement_live_history_witness_v1(success: bool) -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, writer, model_writer) = owner_settlement_read_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    let reference = actual.reservations[0].unwrap().reference;
    let model_reference = model.reservations[0].unwrap().reference;
    let request = actual.reservations[0].unwrap().request;
    let model_request = model.reservations[0].unwrap().request;
    let lease = actual.stable.leases[0].unwrap().reference;
    let model_lease = model.stable.leases[0].unwrap().reference;
    let stable_request = actual.stable.leases[0].unwrap().request;
    let model_stable_request = model.stable.leases[0].unwrap().request;
    let pending = producer_query_historical_exec_v1(&actual, &model, reference, model_reference);
    assert(pending.0 == Ok(ContextProducerReadStatusV1::Pending));
    let wrong = WriterReferenceV1 { slot: 1, ..writer };
    let model_wrong = logical::WriterReferenceV1 { slot: 1, ..model_writer };
    let ghost before = actual;
    let ghost model_before = model;
    proof {
        reveal_with_fuel(owner_retained_scan_v1, 3);
        reveal_with_fuel(settlement_scratch_scan_v1, 3);
        reveal_with_fuel(settlement_cursor_v1, 3);
        reveal_with_fuel(settlement_allocations_prefix_v1, 3);
        reveal_with_fuel(settlement_members_prefix_v1, 3);
    }
    let mismatch = owner_settlement_historical_exec_v1(&mut actual, &mut model, writer, model_writer,
        wrong, model_wrong, 0, 0, success, Ghost(storage), Ghost(history));
    assert(mismatch.0 == Err(ReadErrorV1::SettlementEvidenceMismatch));
    assert(actual == before && model == model_before);
    let shortage = owner_settlement_historical_exec_v1(&mut actual, &mut model, writer, model_writer,
        writer, model_writer, 0, 3, success, Ghost(storage), Ghost(history));
    assert(shortage.0 == Err(ReadErrorV1::InvalidState));
    assert(actual == before && model == model_before);
    let settled = owner_settlement_historical_exec_v1(&mut actual, &mut model, writer, model_writer,
        writer, model_writer, 1, 3, success, Ghost(storage), Ghost(history));
    assert(settled.0 == Ok(()) && settled.1 == Ok(()));
    assert(logical::issued_producer_v1(model, storage, history));
    assert(actual.stable.journal.writers@ == seq![None]);
    assert(actual.stable.journal.free@ == seq![0usize]);
    assert(actual.stable.journal.member_free@ == seq![2usize, 1, 0]);
    assert(actual.stable.journal.allocation_free@ == seq![2usize]);
    assert(actual.stable.journal.registration_watermark == 10 && actual.stable.journal.reserved_count == 0);
    assert(actual.stable.journal.allocations@[0].unwrap().attempt_epoch == 1);
    assert(actual.stable.journal.allocations@[0].unwrap().content_lineage == if success { 1u64 } else { 0u64 });
    assert(actual.stable.journal.allocations@[0].unwrap().pending_member.is_none());
    assert(actual.counts@ == seq![1usize, 0, 0] && actual.stable.readers@ == seq![0usize, 1, 0]);
    assert(actual.free@ == seq![1usize] && actual.stable.free_reads@ == seq![1usize]);
    assert(actual.next_incarnation == 2 && actual.stable.next_incarnation == 2);
    assert(actual.reservations == before.reservations && actual.stable.leases == before.stable.leases);
    proof { owner_settlement_retained_reference_transition_v1(model_before, model, model_writer, success, model_reference); }
    let status = producer_query_historical_exec_v1(&actual, &model, reference, model_reference);
    assert(status.0 == Ok(if success { ContextProducerReadStatusV1::Success } else { ContextProducerReadStatusV1::NoEffect }));
    let found = producer_lookup_historical_exec_v1(&actual, &model, reference, model_reference);
    assert(found.0 == Ok(request) && found.1 == Ok(model_request));
    let found = stable_lookup_historical_exec_v1(&actual.stable, &model.stable, lease, model_lease);
    assert(found.0 == Ok(stable_request) && found.1 == Ok(model_stable_request));
    let ghost settled_actual = actual;
    let ghost settled_model = model;
    let replay = owner_settlement_historical_exec_v1(&mut actual, &mut model, writer, model_writer,
        writer, model_writer, 1, 3, success, Ghost(storage), Ghost(history));
    assert(replay.0 == Err(ReadErrorV1::InvalidReference));
    assert(actual == settled_actual && model == settled_model);
    true
}

// Total represented-state correspondence also applies outside valid custody.
#[verifier::spinoff_prover]
fn owner_settlement_raw_history_witness_v1(success: bool) -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_enrollment_fixture_v1().unwrap();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    actual.counts = vec![];
    model.counts = vec![];
    actual.next_incarnation = 0;
    model.next_incarnation = 0;
    let key = WriterKeyV1 { context_generation: 7, local: 0, kind: WriterKindV1::Submission };
    let model_key = logical::WriterKeyV1 { context_generation: 7, local: 0, kind: logical::WriterKindV1::Submission };
    let writer = WriterReferenceV1 { slot: 0, key };
    let model_writer = logical::WriterReferenceV1 { slot: 0, key: model_key };
    actual.stable.journal.writers = vec![Some(WriterEntryV1::Pending { key, head: None, count: 0 })];
    model.stable.journal.writers = vec![Some(logical::WriterEntryV1::Pending { key: model_key, head: None, count: 0 })];
    actual.stable.journal.free = vec![];
    model.stable.journal.free = vec![];
    proof {
        assert(actual.counts@ =~= model.counts@);
        assert(actual.stable.journal.free@ =~= model.stable.journal.free@);
        assert(journal_view(actual.stable.journal).writers =~= model.stable.journal.writers@);
        assert(!logical::producer_invariant_v1(model));
        assert(!logical::issued_producer_v1(model, storage, history));
    }
    let ghost before = actual;
    let ghost model_before = model;
    let rejected = owner_settlement_historical_exec_v1(&mut actual, &mut model, writer, model_writer,
        writer, model_writer, 0, 3, success, Ghost(storage), Ghost(history));
    assert(rejected.0 == Err(ReadErrorV1::InvalidState));
    assert(actual == before && model == model_before);
    let settled = owner_settlement_historical_exec_v1(&mut actual, &mut model, writer, model_writer,
        writer, model_writer, 1, 3, success, Ghost(storage), Ghost(history));
    assert(settled.0 == Ok(()) && settled.1 == Ok(()));
    assert(actual.stable.journal.writers@ == seq![None]);
    assert(actual.stable.journal.free@ == seq![0usize]);
    assert(actual.counts == before.counts && actual.next_incarnation == 0);
    assert(actual.stable.journal.allocations@ == before.stable.journal.allocations@);
    assert(actual.stable.journal.members@ == before.stable.journal.members@);
    true
}

}
