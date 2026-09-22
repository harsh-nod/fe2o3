verus! {

// Synthetic represented storage, not a proof of the fallible Rust constructor.
#[verifier::spinoff_prover]
fn stable_acquire_fixture_v1() -> (result: (ContextReadLeasedJournalV1, logical::ReadContentsV1,
    ContextAllocationReadV1, logical::AllocationReadV1))
    ensures stable_represents(result.0, result.1), logical::reader_invariant_v1(result.1),
        result.0.journal.context_generation == 7,
        result.0.leases@ == seq![None, None, None, None],
        result.0.free_reads@ == seq![3usize, 2, 1, 0],
        result.0.readers@ == seq![0usize], result.0.next_incarnation == 1,
        allocation_read_view(result.2) == result.3,
        result.2.allocation.slot == 0, result.2.allocation.key.local == 2,
        result.2.byte_offset == 0, result.2.byte_len == 4,
        stable_read_decision_v1(result.0.journal, result.2) == Ok(()),
        stable_read_decision_v1(result.0.journal, ContextAllocationReadV1 { byte_len: 8, ..result.2 }) == Ok(()),
{
    let mut model = match logical::reader_constructor_verified_v1(7, 1, 1, 4) {
        Ok(contents) => contents, Err(_) => { assert(false); unreached() },
    };
    let ghost empty = model;
    let entry = AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 2 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 1 }, byte_extent: 16,
        attempt_epoch: 0, content_lineage: 0, pending_member: None };
    let model_entry = logical::AllocationEntryV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 2 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 1 }, byte_extent: 16,
        attempt_epoch: 0, content_lineage: 0, pending_member: None };
    model.journal.allocations.set(0, Some(model_entry));
    let _slot = model.journal.allocation_free.pop();
    let actual = ContextReadLeasedJournalV1 {
        journal: JournalContentsV1 { context_generation: 7, allocation_capacity: 1, writer_capacity: 1,
            registration_watermark: 0, reserved_count: 0, writers: vec![None], free: vec![0usize],
            allocations: vec![Some(entry)], allocation_free: vec![], members: vec![None],
            member_free: vec![0usize], scratch: vec![None] },
        leases: vec![None, None, None, None], free_reads: vec![3usize, 2, 1, 0],
        readers: vec![0usize], next_incarnation: 1,
    };
    let request = ContextAllocationReadV1 { allocation: AllocationReferenceV1 { slot: 0, key: entry.key },
        device: entry.device, byte_extent: 16, byte_offset: 0, byte_len: 4, attempt_epoch: 0, content_lineage: 0 };
    let model_request = logical::AllocationReadV1 {
        allocation: logical::AllocationReferenceV1 { slot: 0, key: model_entry.key }, device: model_entry.device,
        byte_extent: 16, byte_offset: 0, byte_len: 4, attempt_epoch: 0, content_lineage: 0 };
    proof {
        logical::reader_allocation_frame_preserves_v1(empty, model);
        assert(journal_view(actual.journal).writers =~= model.journal.writers@);
        assert(journal_view(actual.journal).allocations =~= model.journal.allocations@);
        assert(journal_view(actual.journal).members =~= model.journal.members@);
        assert(journal_view(actual.journal).scratch =~= model.journal.scratch@);
        assert(actual.journal.free@ =~= model.journal.free@);
        assert(actual.journal.allocation_free@ =~= model.journal.allocation_free@);
        assert(actual.journal.member_free@ =~= model.journal.member_free@);
        assert(actual.leases@.map(|_i, entry| read_lease_slot_view(entry)) =~= model.leases@);
        assert(actual.free_reads@ =~= model.free_reads@);
        assert(actual.readers@ =~= model.readers@);
    }
    (actual, model, request, model_request)
}

#[verifier::spinoff_prover]
fn stable_acquire_live_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, request, model_request) = stable_acquire_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Synchronous };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Synchronous };
    let initial = vec![request];
    let model_initial = vec![model_request];
    let mut first = vec![None];
    let mut model_first = vec![None];
    proof {
        assert(stable_requests_view(initial@) =~= model_initial@);
        assert(stable_output_view(first@) =~= model_first@);
        reveal_with_fuel(stable_acquire_scan_v1, 4);
        reveal_with_fuel(stable_acquired_leases_v1, 4);
        reveal_with_fuel(stable_read_slot_count_v1, 4);
    }
    let result = stable_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &initial, &model_initial, &mut first, &mut model_first);
    assert(result.0 == Ok(()) && result.1 == Ok(()));
    assert(first@[0].is_some() && first@[0].unwrap().slot == 0 && first@[0].unwrap().incarnation == 1);
    let ghost retained = actual.leases@[0];
    assert(retained.is_some());
    let requests = vec![request, ContextAllocationReadV1 { byte_len: 8, ..request }];
    let model_requests = vec![model_request, logical::AllocationReadV1 { byte_len: 8, ..model_request }];
    let mut output = vec![None, None];
    let mut model_output = vec![None, None];
    proof {
        assert(stable_requests_view(requests@) =~= model_requests@);
        assert(stable_output_view(output@) =~= model_output@);
    }
    let bad = vec![request, ContextAllocationReadV1 { byte_len: 0, ..request }];
    let model_bad = vec![model_request, logical::AllocationReadV1 { byte_len: 0, ..model_request }];
    let ghost before_late = actual;
    let ghost model_before_late = model;
    proof { assert(stable_requests_view(bad@) =~= model_bad@); }
    let late = stable_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &bad, &model_bad, &mut output, &mut model_output);
    assert(late.0 == Err(ReadErrorV1::InvalidExtent) && late.1 == Err(logical::ReadErrorV1::InvalidExtent));
    assert(actual == before_late);
    assert(logical::reader_contents_frame_v1(model_before_late, model));
    assert(output@ == seq![None, None] && model_output@ == seq![None, None]);
    let result = stable_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &requests, &model_requests, &mut output, &mut model_output);
    assert(result.0 == Ok(()) && result.1 == Ok(()));
    assert(actual.leases@[0] == retained);
    assert(actual.readers@ == seq![3usize] && actual.next_incarnation == 4);
    assert(output@[0].unwrap().slot == 1 && output@[1].unwrap().slot == 2);
    assert(output@[0].unwrap().incarnation == 2 && output@[1].unwrap().incarnation == 3);
    assert(logical::reader_invariant_v1(model));
    let ghost before = actual;
    let ghost model_before = model;
    let ghost output_before = output@;
    let ghost model_output_before = model_output@;
    let rejected = stable_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &requests, &model_requests, &mut output, &mut model_output);
    assert(rejected.0 == Err(ReadErrorV1::InvalidState) && rejected.1 == Err(logical::ReadErrorV1::InvalidState));
    assert(actual == before);
    assert(logical::reader_contents_frame_v1(model_before, model));
    assert(output@ == output_before && model_output@ == model_output_before);
    true
}

// The raw contract preserves aliases; it does not promise valid lease custody here.
#[verifier::spinoff_prover]
fn stable_acquire_alias_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, _model, request, _model_request) = stable_acquire_fixture_v1();
    actual.free_reads.set(2, 1);
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Synchronous };
    let requests = vec![request, ContextAllocationReadV1 { byte_len: 8, ..request }];
    let mut output = vec![None, None];
    proof {
        reveal_with_fuel(stable_acquire_scan_v1, 4);
        reveal_with_fuel(stable_acquired_leases_v1, 4);
        reveal_with_fuel(stable_read_slot_count_v1, 4);
    }
    let result = actual.acquire_reads(consumer, &requests, &mut output);
    assert(result == Ok(()));
    assert(output@[0].unwrap().slot == 0 && output@[1].unwrap().slot == 0);
    assert(output@[0].unwrap().incarnation == 1 && output@[1].unwrap().incarnation == 2);
    assert(actual.leases@[0].unwrap().reference == output@[1].unwrap());
    assert(actual.leases@[0].unwrap().request.byte_len == 8);
    assert(actual.readers@ == seq![2usize] && actual.next_incarnation == 3);
    true
}

}
