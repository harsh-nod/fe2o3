verus! {

// Synthetic represented storage with retained NoEffect producer reads, not constructor/settlement reachability.
#[verifier::spinoff_prover]
fn producer_stable_fixture_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1,
    ContextAllocationReadV1, logical::AllocationReadV1))
    ensures producer_represents(result.0, result.1), logical::producer_invariant_v1(result.1),
        result.0.stable.journal.context_generation == 7,
        result.0.stable.leases@ == seq![None, None, None, None],
        result.0.stable.free_reads@ == seq![3usize, 2, 1, 0],
        result.0.stable.readers@ == seq![0usize], result.0.stable.next_incarnation == 1,
        result.0.reservations@[0].is_some(), result.0.reservations@[1].is_some(),
        result.0.free@ == seq![3usize, 2], result.0.counts@ == seq![2usize], result.0.next_incarnation == 3,
        allocation_read_view(result.2) == result.3,
        result.2.allocation.slot == 0, result.2.allocation.key.local == 2,
        result.2.byte_offset == 0, result.2.byte_len == 4,
        stable_read_decision_v1(result.0.stable.journal, result.2) == Ok(()),
        stable_read_decision_v1(result.0.stable.journal, ContextAllocationReadV1 { byte_len: 8, ..result.2 }) == Ok(()),
{
    let mut model = match logical::producer_constructor_exec_v1(7, 1, 1, 4) {
        Ok(contents) => contents, Err(_) => { assert(false); unreached() },
    };
    let ghost empty = model.stable;
    let entry = AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 2 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 1 }, byte_extent: 16,
        attempt_epoch: 1, content_lineage: 0, pending_member: None };
    let model_entry = logical::AllocationEntryV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 2 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 1 }, byte_extent: 16,
        attempt_epoch: 1, content_lineage: 0, pending_member: None };
    model.stable.journal.allocations.set(0, Some(model_entry));
    let _slot = model.stable.journal.allocation_free.pop();
    model.stable.journal.registration_watermark = 10;
    let request = ContextAllocationReadV1 { allocation: AllocationReferenceV1 { slot: 0, key: entry.key },
        device: entry.device, byte_extent: 16, byte_offset: 0, byte_len: 4, attempt_epoch: 1, content_lineage: 0 };
    let model_request = logical::AllocationReadV1 {
        allocation: logical::AllocationReferenceV1 { slot: 0, key: model_entry.key }, device: model_entry.device,
        byte_extent: 16, byte_offset: 0, byte_len: 4, attempt_epoch: 1, content_lineage: 0 };
    let producer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } };
    let model_producer = logical::WriterReferenceV1 { slot: 0, key: logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission } };
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Submission };
    model.reservations.set(0, Some(logical::ProducerReservationV1 {
        reference: logical::ProducerReadReferenceV1 { slot: 0, incarnation: 1, consumer: model_consumer },
        request: logical::ProducerReadV1 { producer: model_producer, read: model_request } }));
    model.reservations.set(1, Some(logical::ProducerReservationV1 {
        reference: logical::ProducerReadReferenceV1 { slot: 1, incarnation: 2, consumer: model_consumer },
        request: logical::ProducerReadV1 { producer: model_producer, read: model_request } }));
    let _first = model.free.pop();
    let _second = model.free.pop();
    model.counts.set(0, 2);
    model.next_incarnation = 3;
    let actual = ContextProducerReadJournalV1 {
        stable: ContextReadLeasedJournalV1 {
            journal: JournalContentsV1 { context_generation: 7, allocation_capacity: 1, writer_capacity: 1,
                registration_watermark: 10, reserved_count: 0, writers: vec![None], free: vec![0usize],
                allocations: vec![Some(entry)], allocation_free: vec![], members: vec![None],
                member_free: vec![0usize], scratch: vec![None] },
            leases: vec![None, None, None, None], free_reads: vec![3usize, 2, 1, 0],
            readers: vec![0usize], next_incarnation: 1 },
        reservations: vec![Some(ReservationV1 { reference: ContextProducerReadReferenceV1 { slot: 0, incarnation: 1, consumer },
            request: ContextProducerReadV1 { producer, read: request } }),
            Some(ReservationV1 { reference: ContextProducerReadReferenceV1 { slot: 1, incarnation: 2, consumer },
            request: ContextProducerReadV1 { producer, read: request } }), None, None],
        free: vec![3usize, 2], counts: vec![2usize], next_incarnation: 3,
    };
    proof {
        logical::reader_allocation_frame_preserves_v1(empty, model.stable);
        reveal(logical::allocation_custody_v1);
        reveal(logical::member_custody_v1);
        reveal(logical::writer_custody_v1);
        reveal(logical::producer_entry_valid_v1);
        reveal_with_fuel(logical::producer_count_v1, 5);
        assert(logical::pending_custody_v1(model.stable.journal));
        assert(model.free@ =~= seq![3usize, 2]);
        assert forall|s: int| 0 <= s < model.reservations@.len() implies
            (#[trigger] model.reservations@[s]).is_none() == model.free@.contains(s as usize) by {
            if s >= 2 { assert(model.free@[3 - s] == s); }
        }
        assert(logical::slot_partition_v1(model.reservations@, model.free@));
        assert(logical::producer_count_v1(model.reservations@, 0) == 2);
        assert(logical::producer_entry_valid_v1(model.stable.journal, model.reservations@[0].unwrap(), 0, 3));
        assert(logical::producer_entry_valid_v1(model.stable.journal, model.reservations@[1].unwrap(), 1, 3));
        assert(logical::producer_arena_v1(model.stable.journal, model.reservations@, model.free@, model.counts@, model.next_incarnation));
        assert(logical::producer_invariant_v1(model));
        assert(journal_view(actual.stable.journal).writers =~= model.stable.journal.writers@);
        assert(journal_view(actual.stable.journal).allocations =~= model.stable.journal.allocations@);
        assert(journal_view(actual.stable.journal).members =~= model.stable.journal.members@);
        assert(journal_view(actual.stable.journal).scratch =~= model.stable.journal.scratch@);
        assert(actual.stable.journal.free@ =~= model.stable.journal.free@);
        assert(actual.stable.journal.allocation_free@ =~= model.stable.journal.allocation_free@);
        assert(actual.stable.journal.member_free@ =~= model.stable.journal.member_free@);
        assert(actual.stable.leases@.map(|_i, entry| read_lease_slot_view(entry)) =~= model.stable.leases@);
        assert(actual.stable.free_reads@ =~= model.stable.free_reads@);
        assert(actual.stable.readers@ =~= model.stable.readers@);
        assert(actual.reservations@.map(|_i, entry| producer_reservation_slot_view(entry)) =~= model.reservations@);
        assert(actual.free@ =~= model.free@);
        assert(actual.counts@ =~= model.counts@);
    }
    (actual, model, request, model_request)
}

#[verifier::spinoff_prover]
fn producer_stable_acquired_fixture_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1))
    ensures producer_represents(result.0, result.1), logical::producer_invariant_v1(result.1),
        result.0.stable.free_reads@ == seq![3usize, 2], result.0.stable.readers@ == seq![2usize],
        result.0.stable.next_incarnation == 3, result.0.stable.leases@.len() == 4,
        result.0.reservations@[0].is_some(), result.0.reservations@[1].is_some(),
        result.0.free@ == seq![3usize, 2], result.0.counts@ == seq![2usize], result.0.next_incarnation == 3,
        forall|i: int| 0 <= i < 2 ==> (#[trigger] result.0.stable.leases@[i]).is_some()
            && result.0.stable.leases@[i].unwrap().reference == (ContextReadLeaseReferenceV1 {
                slot: i as usize, incarnation: (i + 1) as u64,
                consumer: WriterKeyV1 { context_generation: 7, local: 5, kind: WriterKindV1::Synchronous } })
            && result.0.stable.leases@[i].unwrap().request.allocation.slot == 0
            && stable_lease_decision_v1(result.0.stable, result.0.stable.leases@[i].unwrap().reference)
                == Ok(result.0.stable.leases@[i].unwrap().request),
{
    let (mut actual, mut model, request, model_request) = producer_stable_fixture_v1();
    let ghost original = actual;
    let consumer = WriterKeyV1 { context_generation: 7, local: 5, kind: WriterKindV1::Synchronous };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 5, kind: logical::WriterKindV1::Synchronous };
    let requests = vec![request, ContextAllocationReadV1 { byte_len: 8, ..request }];
    let model_requests = vec![model_request, logical::AllocationReadV1 { byte_len: 8, ..model_request }];
    let bad = vec![request, ContextAllocationReadV1 { byte_len: 0, ..request }];
    let model_bad = vec![model_request, logical::AllocationReadV1 { byte_len: 0, ..model_request }];
    let mut output = vec![None, None];
    let mut model_output = vec![None, None];
    proof {
        assert(stable_requests_view(requests@) =~= model_requests@);
        assert(stable_requests_view(bad@) =~= model_bad@);
        assert(stable_output_view(output@) =~= model_output@);
        reveal_with_fuel(stable_acquire_scan_v1, 4);
        reveal_with_fuel(stable_acquired_leases_v1, 4);
        reveal_with_fuel(stable_read_slot_count_v1, 4);
    }
    let late = producer_stable_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &bad, &model_bad, &mut output, &mut model_output);
    assert(late.0 == Err(ReadErrorV1::InvalidExtent));
    assert(actual == original && output@ == seq![None, None]);
    let acquired = producer_stable_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &requests, &model_requests, &mut output, &mut model_output);
    assert(acquired.0 == Ok(()) && acquired.1 == Ok(()));
    assert(actual.stable.readers@ == seq![2usize] && actual.stable.next_incarnation == 3);
    assert(output@[0].unwrap().slot == 0 && output@[1].unwrap().slot == 1);
    assert(output@[0].unwrap().incarnation == 1 && output@[1].unwrap().incarnation == 2);
    assert(producer_reservations_frame_v1(original, actual));
    (actual, model)
}

#[verifier::spinoff_prover]
fn producer_stable_live_witness_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1))
    ensures producer_represents(result.0, result.1), logical::producer_invariant_v1(result.1),
        result.0.stable.free_reads@ == seq![3usize, 2, 0], result.0.stable.readers@ == seq![1usize],
        result.0.stable.next_incarnation == 3, result.0.stable.leases@.len() == 4,
        result.0.stable.leases@[0].is_none(), result.0.stable.leases@[1].is_some(),
        result.0.stable.leases@[1].unwrap().reference == (ContextReadLeaseReferenceV1 {
            slot: 1, incarnation: 2, consumer: WriterKeyV1 { context_generation: 7, local: 5, kind: WriterKindV1::Synchronous } }),
        result.0.stable.leases@[1].unwrap().request.allocation.slot == 0,
        stable_lease_decision_v1(result.0.stable, result.0.stable.leases@[1].unwrap().reference)
            == Ok(result.0.stable.leases@[1].unwrap().request),
        result.0.reservations@[0].is_some(), result.0.reservations@[1].is_some(),
        result.0.free@ == seq![3usize, 2], result.0.counts@ == seq![2usize], result.0.next_incarnation == 3,
{
    let (mut actual, mut model) = producer_stable_acquired_fixture_v1();
    let ghost original = actual;
    let consumer = WriterKeyV1 { context_generation: 7, local: 5, kind: WriterKindV1::Synchronous };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 5, kind: logical::WriterKindV1::Synchronous };
    proof {
        reveal_with_fuel(stable_release_scan_v1, 4);
        reveal_with_fuel(stable_released_leases_v1, 4);
        reveal_with_fuel(stable_read_slot_count_v1, 4);
    }
    let first = vec![actual.stable.leases[0].unwrap().reference];
    let model_first = vec![model.stable.leases[0].unwrap().reference];
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer };
    proof { assert(stable_references_view(first@) =~= model_first@); }
    let ghost held = actual;
    let shortage = producer_stable_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &first, &model_first, &evidence, model_consumer, 2);
    assert(shortage.0 == Err(ReadErrorV1::InvalidState) && actual == held);
    let released = producer_stable_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &first, &model_first, &evidence, model_consumer, 4);
    assert(released.0 == Ok(()) && actual.stable.readers@ == seq![1usize]);
    assert(producer_reservations_frame_v1(original, actual));
    (actual, model)
}

#[verifier::spinoff_prover]
fn producer_stable_replay_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = producer_stable_live_witness_v1();
    let ghost original = actual;
    let consumer = WriterKeyV1 { context_generation: 7, local: 5, kind: WriterKindV1::Synchronous };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 5, kind: logical::WriterKindV1::Synchronous };
    let first = vec![ContextReadLeaseReferenceV1 { slot: 0, incarnation: 1, consumer }];
    let model_first = vec![logical::ReadReferenceV1 { slot: 0, incarnation: 1, consumer: model_consumer }];
    let second = vec![actual.stable.leases[1].unwrap().reference];
    let model_second = vec![model.stable.leases[1].unwrap().reference];
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer };
    proof {
        assert(stable_references_view(first@) =~= model_first@);
        assert(stable_references_view(second@) =~= model_second@);
        reveal_with_fuel(stable_release_scan_v1, 4);
        reveal_with_fuel(stable_released_leases_v1, 4);
        reveal_with_fuel(stable_read_slot_count_v1, 4);
    }
    let ghost before_replay = actual;
    let replay = producer_stable_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &first, &model_first, &evidence, model_consumer, 4);
    assert(replay.0 == Err(ReadErrorV1::InvalidReference) && actual == before_replay);
    let last = producer_stable_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &second, &model_second, &evidence, model_consumer, 4);
    assert(last.0 == Ok(()) && actual.stable.readers@ == seq![0usize]);
    assert(actual.stable.free_reads@ == seq![3usize, 2, 0, 1] && actual.stable.next_incarnation == 3);
    assert(producer_reservations_frame_v1(original, actual));
    true
}

#[verifier::spinoff_prover]
fn producer_stable_budget_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, request, model_request) = producer_stable_fixture_v1();
    actual.stable.next_incarnation = u64::MAX;
    model.stable.next_incarnation = u64::MAX;
    let consumer = WriterKeyV1 { context_generation: 7, local: 5, kind: WriterKindV1::Synchronous };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 5, kind: logical::WriterKindV1::Synchronous };
    let requests = vec![request, request, request];
    let model_requests = vec![model_request, model_request, model_request];
    let mut output = vec![None, None, None];
    let mut model_output = vec![None, None, None];
    proof {
        assert(logical::producer_invariant_v1(model));
        assert(stable_requests_view(requests@) =~= model_requests@);
        assert(stable_output_view(output@) =~= model_output@);
    }
    assert(stable_capacity_decision_v1(actual.stable, 3) == Err(ReadErrorV1::EpochExhausted));
    let ghost before = actual;
    let rejected = producer_stable_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &requests, &model_requests, &mut output, &mut model_output);
    assert(rejected.0 == Err(ReadErrorV1::MemberCapacity));
    assert(actual == before && output@ == seq![None, None, None]);
    true
}

#[verifier::spinoff_prover]
fn producer_stable_raw_prefix_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, _model, request, _model_request) = producer_stable_fixture_v1();
    actual.free = vec![0usize, 0, 0, 0, 0];
    actual.counts = vec![];
    actual.stable.readers = vec![];
    actual.next_incarnation = 0;
    let ghost before = actual;
    let consumer = WriterKeyV1 { context_generation: 7, local: 5, kind: WriterKindV1::Synchronous };
    let requests = vec![request];
    let empty = vec![];
    let mut output = vec![None];
    let foreign = actual.acquire_reads(WriterKeyV1 { context_generation: 0, ..consumer }, &requests, &mut output);
    assert(foreign == Err(ReadErrorV1::ForeignContext) && actual == before);
    let invalid = actual.acquire_reads(WriterKeyV1 { local: 0, ..consumer }, &requests, &mut output);
    assert(invalid == Err(ReadErrorV1::InvalidWriterId) && actual == before);
    let roster = actual.acquire_reads(consumer, &empty, &mut output);
    assert(roster == Err(ReadErrorV1::RosterCapacity) && actual == before && output@ == seq![None]);
    output.set(0, Some(ContextReadLeaseReferenceV1 { slot: 0, incarnation: 1, consumer }));
    assert(output@[0].is_some());
    let ghost occupied = output@;
    let dirty = actual.acquire_reads(consumer, &requests, &mut output);
    assert(dirty == Err(ReadErrorV1::InvalidState) && actual == before && output@ == occupied);
    true
}

#[verifier::spinoff_prover]
fn producer_stable_raw_irrelevance_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, _model, request, _model_request) = producer_stable_fixture_v1();
    actual.counts = vec![];
    actual.next_incarnation = 0;
    let ghost before = actual;
    let consumer = WriterKeyV1 { context_generation: 7, local: 5, kind: WriterKindV1::Synchronous };
    let requests = vec![request];
    let mut output = vec![None];
    proof {
        reveal_with_fuel(stable_acquire_scan_v1, 3);
        reveal_with_fuel(stable_acquired_leases_v1, 3);
        reveal_with_fuel(stable_read_slot_count_v1, 3);
        reveal_with_fuel(stable_release_scan_v1, 3);
        reveal_with_fuel(stable_released_leases_v1, 3);
    }
    let acquired = actual.acquire_reads(consumer, &requests, &mut output);
    assert(acquired == Ok(()) && producer_reservations_frame_v1(before, actual));
    actual.free = vec![0usize, 0, 0, 0, 0];
    let ghost malformed = actual;
    let references = vec![output[0].unwrap()];
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer };
    let released = actual.release_reads_observed_v1(consumer, &references, &evidence, 4);
    assert(released == Ok(()) && producer_reservations_frame_v1(malformed, actual));
    assert(actual.stable.readers@ == seq![0usize]);
    true
}

#[verifier::spinoff_prover]
fn producer_stable_raw_alias_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, _model, request, _model_request) = producer_stable_fixture_v1();
    actual.stable.free_reads.set(2, 1);
    let ghost before = actual;
    let consumer = WriterKeyV1 { context_generation: 7, local: 5, kind: WriterKindV1::Synchronous };
    let requests = vec![request, ContextAllocationReadV1 { byte_len: 8, ..request }];
    let mut output = vec![None, None];
    proof {
        reveal_with_fuel(stable_acquire_scan_v1, 4);
        reveal_with_fuel(stable_acquired_leases_v1, 4);
        reveal_with_fuel(stable_read_slot_count_v1, 4);
    }
    let acquired = actual.acquire_reads(consumer, &requests, &mut output);
    assert(acquired == Ok(()));
    assert(output@[0].unwrap().slot == 0 && output@[1].unwrap().slot == 0);
    assert(output@[0].unwrap().incarnation == 1 && output@[1].unwrap().incarnation == 2);
    assert(actual.stable.leases@[0].unwrap().reference == output@[1].unwrap());
    assert(actual.stable.leases@[0].unwrap().request.byte_len == 8);
    assert(actual.stable.readers@ == seq![2usize] && actual.stable.next_incarnation == 3);
    assert(producer_reservations_frame_v1(before, actual));
    true
}

}
