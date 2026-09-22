verus! {

// Acquisition is paired from a represented synthetic Pending fixture, not Rust construction reachability.
#[verifier::spinoff_prover]
fn producer_release_live_fixture_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1))
    ensures producer_represents(result.0, result.1), logical::producer_invariant_v1(result.1),
        result.0.free@ == seq![3usize, 2], result.0.counts@ == seq![2usize],
        result.0.next_incarnation == 3, result.0.reservations@.len() == 4,
        forall|i: int| 0 <= i < 2 ==> (#[trigger] result.0.reservations@[i]).is_some()
            && result.0.reservations@[i].unwrap().reference == (ContextProducerReadReferenceV1 {
                slot: i as usize, incarnation: (i + 1) as u64,
                consumer: WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission } })
            && result.0.reservations@[i].unwrap().request.read.allocation.slot == 0
            && producer_lookup_decision_v1(result.0, result.0.reservations@[i].unwrap().reference)
                == Ok(result.0.reservations@[i].unwrap().request),
{
    let (mut actual, mut model, request, model_request) = producer_acquire_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Submission };
    let requests = vec![request, ContextProducerReadV1 { read: ContextAllocationReadV1 { byte_len: 12, ..request.read }, ..request }];
    let model_requests = vec![model_request, logical::ProducerReadV1 { read: logical::AllocationReadV1 { byte_len: 12, ..model_request.read }, ..model_request }];
    let mut output = vec![None, None];
    let mut model_output = vec![None, None];
    proof {
        assert(producer_requests_view(requests@) =~= model_requests@);
        assert(producer_output_view(output@) =~= model_output@);
        reveal_with_fuel(producer_acquire_scan_v1, 4);
        reveal_with_fuel(producer_acquired_reservations_v1, 4);
        reveal_with_fuel(producer_request_count_v1, 4);
    }
    let acquired = producer_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &requests, &model_requests, &mut output, &mut model_output);
    assert(acquired.0 == Ok(()) && acquired.1 == Ok(()));
    (actual, model)
}

#[verifier::spinoff_prover]
fn producer_release_live_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = producer_release_live_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Submission };
    let first = actual.reservations[0].unwrap().reference;
    let second = actual.reservations[1].unwrap().reference;
    let model_first = model.reservations[0].unwrap().reference;
    let model_second = model.reservations[1].unwrap().reference;
    proof {
        reveal_with_fuel(producer_release_scan_v1, 4);
        reveal_with_fuel(producer_released_reservations_v1, 4);
        reveal_with_fuel(producer_request_count_v1, 4);
    }
    let ghost before = actual;
    let ghost model_before = model;
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer };
    let late = vec![first, ContextProducerReadReferenceV1 { incarnation: 9, ..second }];
    let model_late = vec![model_first, logical::ProducerReadReferenceV1 { incarnation: 9, ..model_second }];
    proof { assert(producer_references_view(late@) =~= model_late@); }
    let rejected = producer_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &late, &model_late, &evidence, model_consumer, 4);
    assert(rejected.0 == Err(ReadErrorV1::InvalidReference) && rejected.1 == Err(logical::ReadErrorV1::InvalidReference));
    assert(actual == before && logical::producer_contents_frame_v1(model_before, model));
    let selected = vec![first];
    let model_selected = vec![model_first];
    proof { assert(producer_references_view(selected@) =~= model_selected@); }
    let short = producer_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &selected, &model_selected, &evidence, model_consumer, 2);
    assert(short.0 == Err(ReadErrorV1::InvalidState) && short.1 == Err(logical::ReadErrorV1::InvalidState));
    assert(actual == before && logical::producer_contents_frame_v1(model_before, model));
    let released = producer_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &selected, &model_selected, &evidence, model_consumer, 4);
    assert(released.0 == Ok(()) && released.1 == Ok(()));
    assert(actual.reservations@[0].is_none() && actual.reservations@[1] == before.reservations@[1]);
    assert(actual.counts@ == seq![1usize] && actual.next_incarnation == 3);
    assert(actual.free@ == seq![3usize, 2, 0]);
    assert(actual.stable == before.stable && logical::producer_invariant_v1(model));
    let ghost after = actual;
    let replay = producer_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &selected, &model_selected, &evidence, model_consumer, 4);
    assert(replay.0 == Err(ReadErrorV1::InvalidReference));
    assert(actual == after);
    let remaining = vec![second];
    let model_remaining = vec![model_second];
    proof { assert(producer_references_view(remaining@) =~= model_remaining@); }
    let released = producer_release_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &remaining, &model_remaining, &evidence, model_consumer, 4);
    assert(released.0 == Ok(()) && actual.counts@ == seq![0usize]);
    assert(actual.free@ == seq![3usize, 2, 0, 1] && actual.next_incarnation == 3);
    true
}

// Literal lifecycle metadata and malformed free-prefix/epoch values exercise the raw contract only.
#[verifier::spinoff_prover]
fn producer_release_raw_status_witness_v1(phase: u8) -> (result: bool)
    requires phase < 4,
    ensures result,
{
    let (mut actual, _model, request, _model_request) = producer_acquire_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission };
    let requests = vec![request, ContextProducerReadV1 { read: ContextAllocationReadV1 { byte_len: 12, ..request.read }, ..request }];
    let mut output = vec![None, None];
    proof {
        reveal_with_fuel(producer_acquire_scan_v1, 4);
        reveal_with_fuel(producer_acquired_reservations_v1, 4);
        reveal_with_fuel(producer_request_count_v1, 4);
        reveal_with_fuel(producer_release_scan_v1, 4);
        reveal_with_fuel(producer_released_reservations_v1, 4);
    }
    let acquired = actual.acquire_producer_reads(consumer, &requests, &mut output);
    assert(acquired == Ok(()));
    let first = output[0].unwrap();
    let second = output[1].unwrap();
    let producer = WriterReferenceV1 { slot: if phase < 2 { 0 } else { usize::MAX },
        key: WriterKeyV1 { context_generation: 7, local: 0, kind: WriterKindV1::Submission } };
    let writer = if phase == 0 { WriterEntryV1::Pending { key: producer.key, head: Some(usize::MAX), count: usize::MAX } }
        else if phase == 1 { WriterEntryV1::Unknown { key: producer.key, head: Some(usize::MAX), count: usize::MAX } }
        else { WriterEntryV1::Reserved(WriterKeyV1 { local: 99, ..producer.key }) };
    let first_request = ContextProducerReadV1 { producer, read: ContextAllocationReadV1 {
        byte_extent: 16, attempt_epoch: 1, content_lineage: 0, ..request.read } };
    let second_request = ContextProducerReadV1 { producer,
        read: ContextAllocationReadV1 { byte_len: 12, ..first_request.read } };
    actual.reservations.set(0, Some(ReservationV1 { reference: first, request: first_request }));
    actual.reservations.set(1, Some(ReservationV1 { reference: second, request: second_request }));
    actual.stable.journal.writers = vec![Some(writer)];
    actual.stable.journal.allocations = vec![Some(AllocationEntryV1 {
        key: request.read.allocation.key, device: request.read.device, byte_extent: 16, attempt_epoch: 1,
        content_lineage: 0, pending_member: if phase < 2 { Some(0) } else { None },
    })];
    actual.stable.journal.members = vec![Some(MemberEntryV1 { writer: producer, allocation: request.read.allocation,
        prior_lineage: 0, attempt_epoch: 1, next: None })];
    actual.free = vec![usize::MAX, usize::MAX];
    actual.next_incarnation = 0;
    let status = actual.status(&first_request);
    assert(status == Ok(if phase == 0 { ContextProducerReadStatusV1::Pending }
        else if phase == 1 { ContextProducerReadStatusV1::Unknown }
        else if phase == 2 { ContextProducerReadStatusV1::Success }
        else { ContextProducerReadStatusV1::NoEffect }));
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer };
    let references = vec![first, second];
    let ghost before = actual;
    let short = actual.release_producer_reads_observed_v1(consumer, &references, &evidence, 3);
    assert(short == Err(ReadErrorV1::InvalidState) && actual == before);
    let released = actual.release_producer_reads_observed_v1(consumer, &references, &evidence, 4);
    assert(released == Ok(()));
    assert(actual.reservations@[0].is_none() && actual.reservations@[1].is_none());
    assert(actual.counts@ == seq![0usize] && actual.next_incarnation == 0);
    assert(actual.free@ == seq![usize::MAX, usize::MAX, 0, 1]);
    assert(actual.stable == before.stable);
    true
}

#[verifier::spinoff_prover]
fn producer_release_malformed_prefix_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, _model, _request, _model_request) = producer_acquire_fixture_v1();
    actual.counts.clear();
    actual.free.push(usize::MAX);
    let consumer = WriterKeyV1 { context_generation: 7, local: 0, kind: WriterKindV1::Synchronous };
    let mut evidence = ContextReadQuiescenceEvidenceV1 { consumer: WriterKeyV1 { local: 1, ..consumer } };
    let empty = vec![];
    let ghost before = actual;
    let result = actual.release_producer_reads_observed_v1(consumer, &empty, &evidence, 0);
    assert(result == Err(ReadErrorV1::SettlementEvidenceMismatch) && actual == before);
    evidence.consumer = consumer;
    let result = actual.release_producer_reads_observed_v1(consumer, &empty, &evidence, 0);
    assert(result == Err(ReadErrorV1::RosterCapacity) && actual == before);
    let references = vec![ContextProducerReadReferenceV1 { slot: usize::MAX, incarnation: 0, consumer }];
    let result = actual.release_producer_reads_observed_v1(consumer, &references, &evidence, 0);
    assert(result == Err(ReadErrorV1::InvalidState) && actual == before);
    true
}

}
