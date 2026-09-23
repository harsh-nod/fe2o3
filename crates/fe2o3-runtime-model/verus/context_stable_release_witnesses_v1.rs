verus! {

// Acquisitions establish the live leases; the underlying fixture storage is synthetic.
#[verifier::spinoff_prover]
fn stable_release_live_fixture_v1() -> (result: (ContextReadLeasedJournalV1, logical::ReadContentsV1,
    ContextAllocationReadV1, logical::AllocationReadV1))
    ensures stable_represents(result.0, result.1), logical::reader_invariant_v1(result.1),
        result.0.journal.context_generation == 7,
        allocation_read_view(result.2) == result.3,
        result.2.allocation.slot == 0,
        stable_read_decision_v1(result.0.journal, result.2) == Ok(()),
        result.0.readers@ == seq![3usize], result.0.free_reads@ == seq![3usize],
        result.0.next_incarnation == 4,
        result.0.leases@.len() == 4, result.0.leases@[3].is_none(),
        forall|i: int| 0 <= i < 3 ==> (#[trigger] result.0.leases@[i]) == Some(ReadLeaseV1 {
            reference: ContextReadLeaseReferenceV1 { slot: i as usize, incarnation: (i + 1) as u64,
                consumer: WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Synchronous } },
            request: if i == 1 { ContextAllocationReadV1 { byte_len: 8, ..result.2 } } else { result.2 },
        }),
{
    let (mut actual, mut model, request, model_request) = stable_acquire_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Synchronous };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Synchronous };
    let requests = vec![request, ContextAllocationReadV1 { byte_len: 8, ..request }];
    let model_requests = vec![model_request, logical::AllocationReadV1 { byte_len: 8, ..model_request }];
    let mut output = vec![None, None];
    let mut model_output = vec![None, None];
    proof {
        reveal_with_fuel(stable_acquire_scan_v1, 4);
        reveal_with_fuel(stable_acquired_leases_v1, 4);
        reveal_with_fuel(stable_read_slot_count_v1, 4);
        assert(stable_requests_view(requests@) =~= model_requests@);
        assert(stable_output_view(output@) =~= model_output@);
    }
    let acquired = stable_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &requests, &model_requests, &mut output, &mut model_output);
    assert(acquired.0 == Ok(()) && acquired.1 == Ok(()));
    let repeated = vec![request];
    let model_repeated = vec![model_request];
    let mut again = vec![None];
    let mut model_again = vec![None];
    proof {
        assert(stable_requests_view(repeated@) =~= model_repeated@);
        assert(stable_output_view(again@) =~= model_again@);
    }
    let acquired = stable_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &repeated, &model_repeated, &mut again, &mut model_again);
    assert(acquired.0 == Ok(()) && acquired.1 == Ok(()));
    (actual, model, request, model_request)
}

#[verifier::spinoff_prover]
fn stable_release_live_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, request, model_request) = stable_release_live_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Synchronous };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Synchronous };
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer };
    assert(actual.leases@ =~= seq![
        Some(ReadLeaseV1 { reference: ContextReadLeaseReferenceV1 { slot: 0, incarnation: 1, consumer }, request }),
        Some(ReadLeaseV1 { reference: ContextReadLeaseReferenceV1 { slot: 1, incarnation: 2, consumer },
            request: ContextAllocationReadV1 { byte_len: 8, ..request } }),
        Some(ReadLeaseV1 { reference: ContextReadLeaseReferenceV1 { slot: 2, incarnation: 3, consumer }, request }),
        None,
    ]);
    let first = actual.leases[0].unwrap().reference;
    let model_first = model.leases[0].unwrap().reference;
    let second = actual.leases[2].unwrap().reference;
    let model_second = model.leases[2].unwrap().reference;
    let ghost retained_entry = actual.leases@[1];
    let repeated = vec![request];
    let model_repeated = vec![model_request];
    proof {
        reveal_with_fuel(stable_acquire_scan_v1, 4);
        reveal_with_fuel(stable_acquired_leases_v1, 4);
        reveal_with_fuel(stable_read_slot_count_v1, 4);
        reveal_with_fuel(stable_release_scan_v1, 4);
        reveal_with_fuel(stable_released_leases_v1, 4);
        assert(stable_requests_view(repeated@) =~= model_repeated@);
    }
    assert(first.slot == 0 && first.incarnation == 1 && second.slot == 2 && second.incarnation == 3);
    let looked = stable_lookup_historical_exec_v1(&actual, &model, first, model_first);
    assert(looked.0 == Ok(request) && looked.1 == Ok(model_request));
    let refs = vec![first, second];
    let model_refs = vec![model_first, model_second];
    proof { assert(stable_references_view(refs@) =~= model_refs@); }
    let ghost before = actual;
    let ghost model_before = model;
    // The same represented state rejects a shortage observation and accepts adequate headroom.
    let short = stable_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &refs, &model_refs, &evidence, model_consumer, 1);
    assert(short.0 == Err(ReadErrorV1::InvalidState) && short.1 == Err(logical::ReadErrorV1::InvalidState));
    assert(actual == before && logical::reader_contents_frame_v1(model_before, model));
    let duplicate = vec![first, first];
    let model_duplicate = vec![model_first, model_first];
    proof { assert(stable_references_view(duplicate@) =~= model_duplicate@); }
    let rejected = stable_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &duplicate, &model_duplicate, &evidence, model_consumer, 4);
    assert(rejected.0 == Err(ReadErrorV1::NonCanonicalRoster));
    assert(actual == before);
    let released = stable_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &refs, &model_refs, &evidence, model_consumer, 4);
    assert(released.0 == Ok(()) && released.1 == Ok(()));
    assert(actual.leases@[0].is_none() && actual.leases@[2].is_none());
    assert(actual.leases@[1] == retained_entry);
    assert(actual.readers@ == seq![1usize] && actual.next_incarnation == 4);
    assert(actual.free_reads@ == seq![3usize, 0, 2]);
    let ghost released_state = actual;
    let rejected = stable_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &refs, &model_refs, &evidence, model_consumer, 4);
    assert(rejected.0 == Err(ReadErrorV1::InvalidState));
    assert(actual == released_state);
    // A one-member stale release reaches lookup instead of failing the two-member arena check.
    let stale = vec![first];
    let model_stale = vec![model_first];
    proof { assert(stable_references_view(stale@) =~= model_stale@); }
    let rejected = stable_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &stale, &model_stale, &evidence, model_consumer, 4);
    assert(rejected.0 == Err(ReadErrorV1::InvalidReference));
    assert(actual == released_state);
    let mut fresh = vec![None];
    let mut model_fresh = vec![None];
    proof { assert(stable_output_view(fresh@) =~= model_fresh@); }
    let acquired = stable_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &repeated, &model_repeated, &mut fresh, &mut model_fresh);
    assert(acquired.0 == Ok(()) && acquired.1 == Ok(()));
    assert(fresh@[0].unwrap().slot == 2 && fresh@[0].unwrap().incarnation == 4);
    assert(actual.leases@[1] == retained_entry);
    assert(logical::reader_invariant_v1(model));
    true
}

// Raw undercount is outside the reader invariant, but rejection still preserves the owner.
#[verifier::spinoff_prover]
fn stable_release_late_undercount_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, _model, request, _model_request) = stable_acquire_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Synchronous };
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer };
    let requests = vec![request, ContextAllocationReadV1 { byte_len: 8, ..request }];
    let mut output = vec![None, None];
    proof {
        reveal_with_fuel(stable_acquire_scan_v1, 4);
        reveal_with_fuel(stable_acquired_leases_v1, 4);
        reveal_with_fuel(stable_read_slot_count_v1, 4);
        reveal_with_fuel(stable_release_scan_v1, 4);
    }
    let acquired = actual.acquire_reads(consumer, &requests, &mut output);
    assert(acquired == Ok(()));
    actual.readers.set(0, 1);
    let refs = vec![output[0].unwrap(), output[1].unwrap()];
    let ghost before = actual;
    let rejected = actual.release_reads_observed_v1(consumer, &refs, &evidence, 4);
    assert(rejected == Err(ReadErrorV1::InvalidState));
    assert(actual == before);
    true
}

// The raw release contract preserves, rather than sanitizes, a malformed free prefix.
#[verifier::spinoff_prover]
fn stable_release_raw_prefix_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, _model, request, _model_request) = stable_acquire_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Synchronous };
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer };
    let requests = vec![request, ContextAllocationReadV1 { byte_len: 8, ..request }];
    let mut output = vec![None, None];
    proof {
        reveal_with_fuel(stable_acquire_scan_v1, 4);
        reveal_with_fuel(stable_acquired_leases_v1, 4);
        reveal_with_fuel(stable_read_slot_count_v1, 4);
        reveal_with_fuel(stable_release_scan_v1, 4);
        reveal_with_fuel(stable_released_leases_v1, 4);
    }
    let acquired = actual.acquire_reads(consumer, &requests, &mut output);
    assert(acquired == Ok(()));
    actual.free_reads.set(0, 0);
    actual.free_reads.set(1, 0);
    let ghost retained = actual.leases@[1];
    let refs = vec![output[0].unwrap()];
    let released = actual.release_reads_observed_v1(consumer, &refs, &evidence, 4);
    assert(released == Ok(()));
    assert(actual.free_reads@ == seq![0usize, 0, 0]);
    assert(actual.leases@[0].is_none() && actual.leases@[1] == retained);
    assert(actual.readers@ == seq![1usize] && actual.next_incarnation == 3);
    true
}

}
