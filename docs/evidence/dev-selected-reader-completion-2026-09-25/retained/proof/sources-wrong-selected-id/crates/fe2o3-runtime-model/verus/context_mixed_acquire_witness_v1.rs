verus! {

// Synthetic represented storage, not production constructor-origin reachability.
#[verifier::spinoff_prover]
fn mixed_acquire_fixture_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1,
    ContextAllocationReadV1, logical::AllocationReadV1, ContextProducerReadV1, logical::ProducerReadV1))
    ensures producer_represents(result.0, result.1), logical::producer_invariant_v1(result.1),
        allocation_read_view(result.2) == result.3, producer_read_view(result.4) == result.5,
        result.0.stable.journal.context_generation == 7,
        result.0.stable.leases@ == seq![None, None, None, None],
        result.0.stable.free_reads@ == seq![3usize, 2, 1, 0],
        result.0.stable.readers@ == seq![0usize, 0], result.0.stable.next_incarnation == 1,
        result.0.reservations@ == seq![None, None, None, None],
        result.0.free@ == seq![3usize, 2, 1, 0],
        result.0.counts@ == seq![0usize, 0], result.0.next_incarnation == 1,
        result.2.allocation.slot == 1, result.2.allocation.key.local == 2, result.2.byte_offset == 0, result.2.byte_len == 4,
        result.4.read.allocation.slot == 0, result.4.read.allocation.key.local == 1,
        result.4.read.byte_offset == 0, result.4.read.byte_len == 8, result.4.producer.key.local == 10,
        stable_read_decision_v1(result.0.stable.journal, result.2) == Ok(()),
        producer_status_decision_v1(result.0.stable.journal, result.4) == Ok(ContextProducerReadStatusV1::Pending),
        producer_status_decision_v1(result.0.stable.journal, ContextProducerReadV1 {
            read: ContextAllocationReadV1 { byte_len: 12, ..result.4.read }, ..result.4
        }) == Ok(ContextProducerReadStatusV1::Pending),
{
    let mut model = match logical::producer_constructor_exec_v1(7, 2, 1, 4) {
        Ok(value) => value, Err(_) => { assert(false); unreached() },
    };
    let ghost empty = model.stable;
    let device = ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 };
    let allocation = AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 1 } };
    let stable_allocation = AllocationReferenceV1 { slot: 1, key: AllocationKeyV1 { context_generation: 7, local: 2 } };
    let producer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } };
    let stable = ContextAllocationReadV1 { allocation: stable_allocation, device, byte_extent: 16,
        byte_offset: 0, byte_len: 4, attempt_epoch: 0, content_lineage: 0 };
    let pending = ContextProducerReadV1 { producer, read: ContextAllocationReadV1 { allocation, device, byte_extent: 16,
        byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0 } };
    let model_device = logical::DeviceKeyV1 { context_generation: 7, local: 2 };
    let model_allocation = logical::AllocationReferenceV1 { slot: 0, key: logical::AllocationKeyV1 { context_generation: 7, local: 1 } };
    let model_stable_allocation = logical::AllocationReferenceV1 { slot: 1, key: logical::AllocationKeyV1 { context_generation: 7, local: 2 } };
    let model_producer = logical::WriterReferenceV1 { slot: 0,
        key: logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission } };
    let model_stable = logical::AllocationReadV1 { allocation: model_stable_allocation, device: model_device, byte_extent: 16,
        byte_offset: 0, byte_len: 4, attempt_epoch: 0, content_lineage: 0 };
    let model_pending = logical::ProducerReadV1 { producer: model_producer, read: logical::AllocationReadV1 {
        allocation: model_allocation, device: model_device, byte_extent: 16,
        byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0 } };
    let _first = model.stable.journal.allocation_free.pop();
    let _second = model.stable.journal.allocation_free.pop();
    let _member = model.stable.journal.member_free.pop();
    let _writer = model.stable.journal.free.pop();
    model.stable.journal.registration_watermark = 10;
    model.stable.journal.allocations.set(0, Some(logical::AllocationEntryV1 {
        key: model_allocation.key, device: model_device, byte_extent: 16, attempt_epoch: 1, content_lineage: 0, pending_member: Some(0) }));
    model.stable.journal.allocations.set(1, Some(logical::AllocationEntryV1 {
        key: model_stable_allocation.key, device: model_device, byte_extent: 16, attempt_epoch: 0, content_lineage: 0, pending_member: None }));
    model.stable.journal.members.set(0, Some(logical::MemberEntryV1 {
        writer: model_producer, allocation: model_allocation, prior_lineage: 0, attempt_epoch: 1, next: None }));
    model.stable.journal.writers.set(0, Some(logical::WriterEntryV1::Pending { key: model_producer.key, head: Some(0), count: 1 }));
    proof {
        logical::reader_allocation_frame_preserves_v1(empty, model.stable);
        reveal(logical::chain_link_v1);
        assert(logical::chain_link_v1(model.stable.journal, model_producer, seq![0usize], 0));
        assert forall|m: int| 0 <= m < model.stable.journal.members@.len()
            && (#[trigger] model.stable.journal.members@[m]).is_some()
            && logical::same_producer_v1(model.stable.journal.members@[m].unwrap().writer, model_producer)
            implies seq![0usize].contains(m as usize) by { assert(m == 0); }
        assert(logical::retained_chain_v1(model.stable.journal, model_producer, seq![0usize]));
        reveal(logical::allocation_custody_v1);
        reveal(logical::member_custody_v1);
        reveal(logical::writer_custody_v1);
        assert(logical::pending_custody_v1(model.stable.journal));
        assert(logical::producer_arena_v1(model.stable.journal, model.reservations@, model.free@, model.counts@, model.next_incarnation));
    }
    let actual = ContextProducerReadJournalV1 {
        stable: ContextReadLeasedJournalV1 {
            journal: JournalContentsV1 { context_generation: 7, allocation_capacity: 2, writer_capacity: 1,
                registration_watermark: 10, reserved_count: 0,
                writers: vec![Some(WriterEntryV1::Pending { key: producer.key, head: Some(0), count: 1 })], free: vec![],
                allocations: vec![Some(AllocationEntryV1 { key: allocation.key, device, byte_extent: 16,
                    attempt_epoch: 1, content_lineage: 0, pending_member: Some(0) }),
                    Some(AllocationEntryV1 { key: stable_allocation.key, device, byte_extent: 16,
                    attempt_epoch: 0, content_lineage: 0, pending_member: None })], allocation_free: vec![],
                members: vec![Some(MemberEntryV1 { writer: producer, allocation, prior_lineage: 0, attempt_epoch: 1, next: None }), None],
                member_free: vec![1usize], scratch: vec![None, None] },
            leases: vec![None, None, None, None], free_reads: vec![3usize, 2, 1, 0],
            readers: vec![0usize, 0], next_incarnation: 1 },
        reservations: vec![None, None, None, None], free: vec![3usize, 2, 1, 0],
        counts: vec![0usize, 0], next_incarnation: 1,
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
    (actual, model, stable, model_stable, pending, model_pending)
}

#[verifier::spinoff_prover]
fn mixed_acquire_late_rejection_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, stable, model_stable, pending, model_pending) = mixed_acquire_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Submission };
    let stable_requests = vec![stable];
    let model_stable_requests = vec![model_stable];
    let bad = vec![pending, ContextProducerReadV1 { read: ContextAllocationReadV1 { byte_len: 0, ..pending.read }, ..pending }];
    let model_bad = vec![model_pending, logical::ProducerReadV1 { read: logical::AllocationReadV1 { byte_len: 0, ..model_pending.read }, ..model_pending }];
    let mut stable_output = vec![None];
    let mut model_stable_output = vec![None];
    let mut producer_output = vec![None, None];
    let mut model_producer_output = vec![None, None];
    let ghost before = actual;
    proof {
        assert(stable_requests_view(stable_requests@) =~= model_stable_requests@);
        assert(producer_requests_view(bad@) =~= model_bad@);
        assert(stable_output_view(stable_output@) =~= model_stable_output@);
        assert(producer_output_view(producer_output@) =~= model_producer_output@);
        reveal_with_fuel(stable_acquire_scan_v1, 4);
        reveal_with_fuel(producer_acquire_scan_v1, 4);
    }
    let rejected = mixed_acquire_paired_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &stable_requests, &model_stable_requests, &bad, &model_bad,
        &mut stable_output, &mut model_stable_output, &mut producer_output, &mut model_producer_output);
    assert(rejected.0 == Err(ReadErrorV1::InvalidExtent));
    assert(actual == before && stable_output@ == seq![None] && producer_output@ == seq![None, None]);
    true
}

#[verifier::spinoff_prover]
fn mixed_acquire_acquired_fixture_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1,
    ContextAllocationReadV1, logical::AllocationReadV1, ContextProducerReadV1, logical::ProducerReadV1))
    ensures producer_represents(result.0, result.1), logical::producer_invariant_v1(result.1),
        allocation_read_view(result.2) == result.3, producer_read_view(result.4) == result.5,
        result.0.stable.journal.context_generation == 7, producer_remaining_v1(result.0) == 1,
        result.0.stable.readers@ == seq![0usize, 1], result.0.counts@ == seq![2usize, 0],
        result.0.stable.next_incarnation == 2, result.0.next_incarnation == 3,
{
    let (mut actual, mut model, stable, model_stable, pending, model_pending) = mixed_acquire_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Submission };
    let stable_requests = vec![stable];
    let model_stable_requests = vec![model_stable];
    let pending_requests = vec![pending, ContextProducerReadV1 { read: ContextAllocationReadV1 { byte_len: 12, ..pending.read }, ..pending }];
    let model_pending_requests = vec![model_pending, logical::ProducerReadV1 { read: logical::AllocationReadV1 { byte_len: 12, ..model_pending.read }, ..model_pending }];
    let mut stable_output = vec![None];
    let mut model_stable_output = vec![None];
    let mut producer_output = vec![None, None];
    let mut model_producer_output = vec![None, None];
    let ghost before = actual;
    proof {
        assert(stable_requests_view(stable_requests@) =~= model_stable_requests@);
        assert(producer_requests_view(pending_requests@) =~= model_pending_requests@);
        assert(stable_output_view(stable_output@) =~= model_stable_output@);
        assert(producer_output_view(producer_output@) =~= model_producer_output@);
        reveal_with_fuel(stable_acquire_scan_v1, 4);
        reveal_with_fuel(producer_acquire_scan_v1, 4);
        reveal_with_fuel(stable_read_slot_count_v1, 4);
        reveal_with_fuel(producer_request_count_v1, 4);
    }
    let acquired = mixed_acquire_paired_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &stable_requests, &model_stable_requests, &pending_requests, &model_pending_requests,
        &mut stable_output, &mut model_stable_output, &mut producer_output, &mut model_producer_output);
    assert(acquired.0 == Ok(()) && acquired.1 == Ok(()));
    let ghost middle = choose|middle: ContextProducerReadJournalV1|
        mixed_stable_commit_v1(before, middle, consumer, stable_requests@, seq![None], stable_output@)
        && mixed_producer_commit_v1(middle, actual, consumer, pending_requests@, seq![None, None], producer_output@);
    assert(actual.stable.readers@ == seq![0usize, 1] && actual.counts@ == seq![2usize, 0]);
    assert(actual.stable.next_incarnation == 2 && actual.next_incarnation == 3);
    assert(producer_total_retained_v1(actual) == 3);
    assert(stable_output@[0] == Some(ContextReadLeaseReferenceV1 { slot: 0, incarnation: 1, consumer }));
    assert(producer_output@[0] == Some(ContextProducerReadReferenceV1 { slot: 0, incarnation: 1, consumer }));
    assert(producer_output@[1] == Some(ContextProducerReadReferenceV1 { slot: 1, incarnation: 2, consumer }));
    (actual, model, stable, model_stable, pending, model_pending)
}

#[verifier::spinoff_prover]
fn mixed_acquire_capacity_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, stable, model_stable, pending, model_pending) = mixed_acquire_acquired_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Submission };
    let stable_requests = vec![stable];
    let model_stable_requests = vec![model_stable];
    let ghost held = actual;
    let mut full_stable = vec![None];
    let mut model_full_stable = vec![None];
    let one_pending = vec![pending];
    let model_one_pending = vec![model_pending];
    let mut full_producer = vec![None];
    let mut model_full_producer = vec![None];
    proof {
        assert(stable_requests_view(stable_requests@) =~= model_stable_requests@);
        assert(producer_requests_view(one_pending@) =~= model_one_pending@);
        assert(stable_output_view(full_stable@) =~= model_full_stable@);
        assert(producer_output_view(full_producer@) =~= model_full_producer@);
    }
    let full = mixed_acquire_paired_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &stable_requests, &model_stable_requests, &one_pending, &model_one_pending,
        &mut full_stable, &mut model_full_stable, &mut full_producer, &mut model_full_producer);
    assert(full.0 == Err(ReadErrorV1::MemberCapacity));
    assert(actual == held && full_stable@ == seq![None] && full_producer@ == seq![None]);
    true
}

}
