verus! {

// Synthetic mirrored storage and a logical constructor; not fallible Rust constructor reachability.
#[verifier::spinoff_prover]
fn producer_acquire_fixture_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1,
    ContextProducerReadV1, logical::ProducerReadV1))
    ensures producer_represents(result.0, result.1), logical::producer_invariant_v1(result.1),
        result.0.stable.journal.context_generation == 7,
        result.0.reservations@ == seq![None, None, None, None],
        result.0.free@ == seq![3usize, 2, 1, 0],
        result.0.counts@ == seq![0usize], result.0.next_incarnation == 1,
        result.0.stable.leases@ == seq![None, None, None, None],
        result.0.stable.free_reads@ == seq![3usize, 2, 1, 0],
        result.0.stable.readers@ == seq![0usize], result.0.stable.next_incarnation == 1,
        producer_read_view(result.2) == result.3,
        result.2.producer.key.local == 10,
        result.2.read.allocation.slot == 0, result.2.read.allocation.key.local == 1,
        result.2.read.byte_offset == 0, result.2.read.byte_len == 8,
        producer_status_decision_v1(result.0.stable.journal, result.2) == Ok(ContextProducerReadStatusV1::Pending),
        producer_status_decision_v1(result.0.stable.journal, ContextProducerReadV1 {
            read: ContextAllocationReadV1 { byte_len: 12, ..result.2.read }, ..result.2
        }) == Ok(ContextProducerReadStatusV1::Pending),
{

    let mut model = match logical::producer_constructor_exec_v1(7, 1, 1, 4) { Ok(value) => value, Err(_) => { assert(false); unreached() } };
    let ghost empty = model.stable;
    let key = logical::AllocationKeyV1 { context_generation: 7, local: 1 };
    let device = logical::DeviceKeyV1 { context_generation: 7, local: 2 };
    let allocation = logical::AllocationReferenceV1 { slot: 0, key };
    let producer = logical::WriterReferenceV1 { slot: 0, key: logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission } };
    let _allocation_slot = model.stable.journal.allocation_free.pop();
    let _member_slot = model.stable.journal.member_free.pop();
    let _writer_slot = model.stable.journal.free.pop();
    model.stable.journal.registration_watermark = 10;
    model.stable.journal.allocations.set(0, Some(logical::AllocationEntryV1 {
        key, device, byte_extent: 16, attempt_epoch: 1, content_lineage: 0, pending_member: Some(0),
    }));
    model.stable.journal.members.set(0, Some(logical::MemberEntryV1 {
        writer: producer, allocation, prior_lineage: 0, attempt_epoch: 1, next: None,
    }));
    model.stable.journal.writers.set(0, Some(logical::WriterEntryV1::Pending { key: producer.key, head: Some(0), count: 1 }));
    let request = logical::ProducerReadV1 { read: logical::AllocationReadV1 {
        allocation, device, byte_extent: 16, byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0,
    }, producer };
    proof {
        logical::reader_allocation_frame_preserves_v1(empty, model.stable);
        reveal(logical::chain_link_v1);
        assert(logical::chain_link_v1(model.stable.journal, producer, seq![0usize], 0));
        assert forall|m: int| 0 <= m < model.stable.journal.members@.len()
            && (#[trigger] model.stable.journal.members@[m]).is_some()
            && logical::same_producer_v1(model.stable.journal.members@[m].unwrap().writer, producer)
            implies seq![0usize].contains(m as usize) by { assert(m == 0); }
        assert(logical::retained_chain_v1(model.stable.journal, producer, seq![0usize]));
        reveal(logical::allocation_custody_v1);
        reveal(logical::member_custody_v1);
        reveal(logical::writer_custody_v1);
        assert(logical::pending_custody_v1(model.stable.journal));
        assert(logical::producer_arena_v1(model.stable.journal, model.reservations@, model.free@, model.counts@, model.next_incarnation));
        assert(model.free@ =~= seq![3usize, 2usize, 1usize, 0usize]);
    }

    let actual_producer = WriterReferenceV1 { slot: 0,
        key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } };
    let actual_allocation = AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 1 } };
    let actual_device = ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 };
    let actual_request = ContextProducerReadV1 { producer: actual_producer, read: ContextAllocationReadV1 {
        allocation: actual_allocation, device: actual_device, byte_extent: 16, byte_offset: 0, byte_len: 8,
        attempt_epoch: 1, content_lineage: 0 } };
    let actual = ContextProducerReadJournalV1 {
        stable: ContextReadLeasedJournalV1 {
            journal: JournalContentsV1 { context_generation: 7, allocation_capacity: 1, writer_capacity: 1,
                registration_watermark: 10, reserved_count: 0,
                writers: vec![Some(WriterEntryV1::Pending { key: actual_producer.key, head: Some(0), count: 1 })],
                free: vec![],
                allocations: vec![Some(AllocationEntryV1 { key: actual_allocation.key, device: actual_device,
                    byte_extent: 16, attempt_epoch: 1, content_lineage: 0, pending_member: Some(0) })],
                allocation_free: vec![],
                members: vec![Some(MemberEntryV1 { writer: actual_producer, allocation: actual_allocation,
                    prior_lineage: 0, attempt_epoch: 1, next: None })],
                member_free: vec![], scratch: vec![None] },
            leases: vec![None, None, None, None], free_reads: vec![3usize, 2, 1, 0],
            readers: vec![0usize], next_incarnation: 1 },
        reservations: vec![None, None, None, None], free: vec![3usize, 2, 1, 0],
        counts: vec![0usize], next_incarnation: 1,
    };
    proof {
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
    (actual, model, actual_request, request)
}

#[verifier::spinoff_prover]
fn producer_acquire_live_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, request, model_request) = producer_acquire_fixture_v1();
    let capacity = producer_capacity_historical_exec_v1(&actual, &model, 0);
    assert(capacity.0 == Ok(()) && capacity.1 == Ok(()));
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Submission };
    let initial = vec![request];
    let model_initial = vec![model_request];
    let mut first = vec![None];
    let mut model_first = vec![None];
    proof {
        assert(producer_requests_view(initial@) =~= model_initial@);
        assert(producer_output_view(first@) =~= model_first@);
        reveal_with_fuel(producer_acquire_scan_v1, 4);
        reveal_with_fuel(producer_acquired_reservations_v1, 4);
        reveal_with_fuel(producer_request_count_v1, 4);
    }
    let result = producer_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &initial, &model_initial, &mut first, &mut model_first);
    assert(result.0 == Ok(()) && result.1 == Ok(()));
    assert(first@[0].is_some() && first@[0].unwrap().slot == 0 && first@[0].unwrap().incarnation == 1);
    let ghost retained = actual.reservations@[0];
    assert(retained.is_some());
    let requests = vec![request, ContextProducerReadV1 { read: ContextAllocationReadV1 { byte_len: 12, ..request.read }, ..request }];
    let model_requests = vec![model_request, logical::ProducerReadV1 { read: logical::AllocationReadV1 { byte_len: 12, ..model_request.read }, ..model_request }];
    let mut output = vec![None, None];
    let mut model_output = vec![None, None];
    proof {
        assert(producer_requests_view(requests@) =~= model_requests@);
        assert(producer_output_view(output@) =~= model_output@);
    }
    let bad = vec![request, ContextProducerReadV1 { read: ContextAllocationReadV1 { byte_len: 0, ..request.read }, ..request }];
    let model_bad = vec![model_request, logical::ProducerReadV1 { read: logical::AllocationReadV1 { byte_len: 0, ..model_request.read }, ..model_request }];
    let ghost before_late = actual;
    let ghost model_before_late = model;
    proof { assert(producer_requests_view(bad@) =~= model_bad@); }
    let late = producer_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &bad, &model_bad, &mut output, &mut model_output);
    assert(late.0 == Err(ReadErrorV1::InvalidExtent) && late.1 == Err(logical::ReadErrorV1::InvalidExtent));
    assert(actual == before_late);
    assert(logical::producer_contents_frame_v1(model_before_late, model));
    assert(output@ == seq![None, None] && model_output@ == seq![None, None]);
    let result = producer_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &requests, &model_requests, &mut output, &mut model_output);
    assert(result.0 == Ok(()) && result.1 == Ok(()));
    assert(actual.reservations@[0] == retained);
    assert(actual.counts@ == seq![3usize] && actual.next_incarnation == 4);
    assert(output@[0].unwrap().slot == 1 && output@[1].unwrap().slot == 2);
    assert(output@[0].unwrap().incarnation == 2 && output@[1].unwrap().incarnation == 3);
    assert(logical::producer_invariant_v1(model));
    let ghost before = actual;
    let ghost model_before = model;
    let ghost output_before = output@;
    let ghost model_output_before = model_output@;
    let rejected = producer_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &requests, &model_requests, &mut output, &mut model_output);
    assert(rejected.0 == Err(ReadErrorV1::InvalidState) && rejected.1 == Err(logical::ReadErrorV1::InvalidState));
    assert(actual == before);
    assert(logical::producer_contents_frame_v1(model_before, model));
    assert(output@ == output_before && model_output@ == model_output_before);
    true
}

// The raw contract preserves aliases; it does not promise valid producer custody here.
#[verifier::spinoff_prover]
fn producer_acquire_alias_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, _model, request, _model_request) = producer_acquire_fixture_v1();
    actual.free.set(2, 1);
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let requests = vec![request, ContextProducerReadV1 { read: ContextAllocationReadV1 { byte_len: 12, ..request.read }, ..request }];
    let mut output = vec![None, None];
    proof {
        reveal_with_fuel(producer_acquire_scan_v1, 4);
        reveal_with_fuel(producer_acquired_reservations_v1, 4);
        reveal_with_fuel(producer_request_count_v1, 4);
    }
    let result = actual.acquire_producer_reads(consumer, &requests, &mut output);
    assert(result == Ok(()));
    assert(output@[0].unwrap().slot == 0 && output@[1].unwrap().slot == 0);
    assert(output@[0].unwrap().incarnation == 1 && output@[1].unwrap().incarnation == 2);
    assert(actual.reservations@[0].unwrap().reference == output@[1].unwrap());
    assert(actual.reservations@[0].unwrap().request.read.byte_len == 12);
    assert(actual.counts@ == seq![2usize] && actual.next_incarnation == 3);
    true
}


// A matching raw Pending entry is accepted even when its ID is not issuable.
#[verifier::spinoff_prover]
fn producer_acquire_zero_producer_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, _model, request, _model_request) = producer_acquire_fixture_v1();
    let producer = WriterReferenceV1 { slot: 0,
        key: WriterKeyV1 { context_generation: 7, local: 0, kind: WriterKindV1::Submission } };
    let request = ContextProducerReadV1 { producer, ..request };
    actual.stable.journal.writers = vec![Some(WriterEntryV1::Pending {
        key: producer.key, head: Some(0), count: 1 })];
    actual.stable.journal.allocations = vec![Some(AllocationEntryV1 {
        key: request.read.allocation.key, device: request.read.device,
        byte_extent: request.read.byte_extent, attempt_epoch: request.read.attempt_epoch,
        content_lineage: request.read.content_lineage, pending_member: Some(0) })];
    actual.stable.journal.members = vec![Some(MemberEntryV1 {
        writer: producer, allocation: request.read.allocation, prior_lineage: request.read.content_lineage,
        attempt_epoch: request.read.attempt_epoch, next: None })];
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let requests = vec![request];
    let mut output = vec![None];
    proof {
        reveal_with_fuel(producer_acquire_scan_v1, 3);
        reveal_with_fuel(producer_acquired_reservations_v1, 3);
        reveal_with_fuel(producer_request_count_v1, 3);
    }
    let result = actual.acquire_producer_reads(consumer, &requests, &mut output);
    assert(result == Ok(()));
    assert(output@[0].unwrap().slot == 0 && output@[0].unwrap().incarnation == 1);
    assert(actual.reservations@[0].unwrap().request.producer.key.local == 0);
    assert(actual.counts@ == seq![1usize] && actual.next_incarnation == 2);
    true
}

// Header-prefix errors do not require valid budget arithmetic or count storage.
#[verifier::spinoff_prover]
fn producer_acquire_bad_budget_prefix_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, _model, request, _model_request) = producer_acquire_fixture_v1();
    actual.free.push(0);
    actual.counts.clear();
    let foreign = WriterKeyV1 { context_generation: 8, local: 20, kind: WriterKindV1::Submission };
    let requests = vec![request];
    let mut output = vec![None];
    let ghost before = actual;
    let result = actual.acquire_producer_reads(foreign, &requests, &mut output);
    assert(result == Err(ReadErrorV1::ForeignContext));
    assert(actual == before && output@ == seq![None]);
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let sentinel = ContextProducerReadReferenceV1 { slot: usize::MAX, incarnation: 0, consumer };
    output.set(0, Some(sentinel));
    proof {
        assert(output@[0].is_some());
        assert(exists|i: int| 0 <= i < output@.len() && output@[i].is_some());
        assert(producer_acquire_prefix_v1(actual, consumer, requests.len(), output@) == Err(ReadErrorV1::InvalidState));
    }
    let result = actual.acquire_producer_reads(consumer, &requests, &mut output);
    assert(result == Err(ReadErrorV1::InvalidState));
    assert(actual == before && output@ == seq![Some(sentinel)]);
    true
}

// Unequal raw arenas qualify length arithmetic, not stable-lease custody.
#[verifier::spinoff_prover]
fn producer_acquire_raw_budget_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, _model, _request, _model_request) = producer_acquire_fixture_v1();
    actual.reservations.truncate(2);
    actual.free = vec![1usize, 0];
    let _slot = actual.stable.free_reads.pop();
    let retained = actual.retained_producer_read_count();
    let total = actual.retained_read_count();
    let remaining = actual.remaining_read_slots();
    assert(retained == 0 && total == 1 && remaining == 1);
    let result = actual.validate_producer_read_capacity(1);
    assert(result == Ok(()));
    actual.next_incarnation = 0;
    let result = actual.validate_producer_read_capacity(2);
    assert(result == Err(ReadErrorV1::MemberCapacity));
    let result = actual.validate_producer_read_capacity(1);
    assert(result == Err(ReadErrorV1::EpochExhausted));
    actual.next_incarnation = u64::MAX;
    let result = actual.validate_producer_read_capacity(0);
    assert(result == Ok(()));
    let result = actual.validate_producer_read_capacity(1);
    assert(result == Err(ReadErrorV1::EpochExhausted));
    actual.stable.next_incarnation = 0;
    let ghost before = actual;
    let result = actual.validate_read_capacity(2);
    assert(result == Err(ReadErrorV1::MemberCapacity));
    let result = actual.validate_read_capacity(1);
    assert(result == Err(ReadErrorV1::EpochExhausted));
    assert(actual == before);
    true
}

}
