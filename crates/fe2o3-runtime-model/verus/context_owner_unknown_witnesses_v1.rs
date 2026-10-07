verus! {

// Paired acquisition from a concrete synthetic Pending fixture, not Rust constructor reachability.
#[verifier::spinoff_prover]
fn owner_unknown_retained_fixture_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1,
    WriterReferenceV1, logical::WriterReferenceV1))
    ensures producer_represents(result.0, result.1), logical::producer_invariant_v1(result.1),
        writer_reference_view(result.2) == result.3,
        result.2.key.local == 10, result.2.slot == 0,
        owner_retained_header_v1(result.0.stable.journal, result.2, true) == Ok((Some(0usize), 1usize, false)),
        owner_retained_chain_v1(result.0.stable.journal, result.2, Some(0usize), 1usize) == Ok(()),
        result.0.reservations@.len() == 4, result.0.reservations@[0].is_some(),
        result.0.reservations@[0].unwrap().reference.slot == 0,
        result.0.reservations@[0].unwrap().request.producer == result.2,
        producer_status_decision_v1(result.0.stable.journal, result.0.reservations@[0].unwrap().request)
            == Ok(ContextProducerReadStatusV1::Pending),
        producer_query_decision_v1(result.0, result.0.reservations@[0].unwrap().reference)
            == Ok(ContextProducerReadStatusV1::Pending),
{
    let (mut actual, mut model, request, model_request) = producer_acquire_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Submission };
    let requests = vec![request];
    let model_requests = vec![model_request];
    let mut output = vec![None];
    let mut model_output = vec![None];
    proof {
        assert(producer_requests_view(requests@) =~= model_requests@);
        assert(producer_output_view(output@) =~= model_output@);
        reveal_with_fuel(producer_acquire_scan_v1, 3);
        reveal_with_fuel(producer_acquired_reservations_v1, 3);
        reveal_with_fuel(producer_request_count_v1, 3);
        reveal_with_fuel(owner_retained_scan_v1, 3);
    }
    let acquired = producer_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &requests, &model_requests, &mut output, &mut model_output);
    assert(acquired.0 == Ok(()));
    (actual, model, request.producer, model_request.producer)
}

#[verifier::spinoff_prover]
fn owner_unknown_live_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, writer, model_writer) = owner_unknown_retained_fixture_v1();
    let ghost before = actual;
    let reference = actual.reservations[0].unwrap().reference;
    let marked = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(marked.0 == Ok(()) && marked.1 == Ok(()));
    assert(actual.reservations == before.reservations && actual.counts == before.counts);
    proof {
        owner_retained_chain_frame_v1(before.stable.journal, actual.stable.journal, writer, Some(0usize), 1usize);
        let request = before.reservations@[0].unwrap().request;
        assert(allocation_lookup_decision_v1(actual.stable.journal, request.read.allocation)
            == allocation_lookup_decision_v1(before.stable.journal, request.read.allocation));
        assert(writer_lookup_decision_v1(actual.stable.journal, writer) == Ok(ContextWriterStateV1::Unknown { member_count: 1 }));
        assert(producer_status_decision_v1(actual.stable.journal, request) == Ok(ContextProducerReadStatusV1::Unknown));
    }
    let queried = actual.producer_read_status(reference);
    assert(queried == Ok(ContextProducerReadStatusV1::Unknown));
    let ghost unknown = actual;
    let ghost model_unknown = model;
    let repeated = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(repeated.0 == Ok(()) && actual == unknown && model == model_unknown);
    true
}

#[verifier::spinoff_prover]
fn owner_unknown_raw_rejection_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, writer, model_writer) = owner_unknown_retained_fixture_v1();
    let marked = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(marked.0 == Ok(()));
    actual.stable.journal.members = vec![];
    model.stable.journal.members = vec![];
    proof { assert(journal_view(actual.stable.journal).members =~= model.stable.journal.members@); }
    let ghost before = actual;
    let ghost model_before = model;
    let stale = WriterReferenceV1 { key: WriterKeyV1 { local: 11, ..writer.key }, ..writer };
    let model_stale = logical::WriterReferenceV1 { key: logical::WriterKeyV1 { local: 11, ..model_writer.key }, ..model_writer };
    let rejected = producer_unknown_historical_exec_v1(&mut actual, &mut model, stale, model_stale);
    assert(rejected.0 == Err(ReadErrorV1::InvalidReference) && actual == before && model == model_before);
    let rejected = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(rejected.0 == Err(ReadErrorV1::InvalidState) && actual == before && model == model_before);
    true
}

#[verifier::spinoff_prover]
fn owner_unknown_raw_storage_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, writer, model_writer) = owner_unknown_retained_fixture_v1();
    actual.stable.readers = vec![];
    model.stable.readers = vec![];
    actual.stable.free_reads = vec![usize::MAX];
    model.stable.free_reads = vec![usize::MAX];
    actual.counts = vec![];
    model.counts = vec![];
    actual.free = vec![usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX];
    model.free = vec![usize::MAX, usize::MAX, usize::MAX, usize::MAX, usize::MAX];
    actual.next_incarnation = 0;
    model.next_incarnation = 0;
    actual.stable.next_incarnation = u64::MAX;
    model.stable.next_incarnation = u64::MAX;
    proof {
        assert(actual.stable.readers@ =~= model.stable.readers@);
        assert(actual.stable.free_reads@ =~= model.stable.free_reads@);
        assert(actual.counts@ =~= model.counts@);
        assert(actual.free@ =~= model.free@);
    }
    let ghost before = actual;
    let marked = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(marked.0 == Ok(()) && marked.1 == Ok(()));
    assert(actual.counts == before.counts && actual.free == before.free && actual.next_incarnation == 0);
    assert(actual.stable.readers == before.stable.readers && actual.stable.free_reads == before.stable.free_reads);
    true
}

#[verifier::spinoff_prover]
fn owner_unknown_zero_id_empty_witness_v1() -> (result: bool)
    ensures result,
{
    let writer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 0, local: 0, kind: WriterKindV1::Synchronous } };
    let mut actual = ContextProducerReadJournalV1 {
        stable: ContextReadLeasedJournalV1 {
            journal: JournalContentsV1 { context_generation: 0, allocation_capacity: 0, writer_capacity: 0,
                registration_watermark: u64::MAX, reserved_count: usize::MAX,
                writers: vec![Some(WriterEntryV1::Pending { key: writer.key, head: None, count: 0 })],
                free: vec![], allocations: vec![], allocation_free: vec![], members: vec![], member_free: vec![], scratch: vec![] },
            leases: vec![], free_reads: vec![], readers: vec![], next_incarnation: 0 },
        reservations: vec![], free: vec![], counts: vec![], next_incarnation: 0,
    };
    let marked = actual.mark_unknown(writer);
    assert(marked == Ok(()));
    let ghost before = actual;
    let repeated = actual.mark_unknown(writer);
    assert(repeated == Ok(()) && actual == before);
    true
}

}
