verus! {

// Constructor-origin storage; the supplied capacities are explicit observations,
// not claims about the allocator or the Context's error/quarantine handling.
#[verifier::spinoff_prover]
fn completion_mixed_prefix_witness_v1(stable_capacity: bool, producer_capacity: bool)
    -> (result: bool)
    ensures result,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::lifecycle_invariant_v1);
    let (actual, model, trace) = lifecycle_mixed_acquired_fixture_v1();
    proof { lifecycle_reader_reached_v1(trace@, actual, model); }
    let stable = actual.stable.leases[0].unwrap().reference;
    let model_stable = model.stable.leases[0].unwrap().reference;
    let producer = actual.reservations[0].unwrap().reference;
    let model_producer = model.reservations[0].unwrap().reference;
    let consumer = stable.consumer;
    let inputs = CompletionJournalInputsV1 {
        consumer, model_consumer: model_stable.consumer,
        stable_present: true, producer_present: true,
        stable: vec![stable], model_stable: vec![model_stable],
        producer: vec![producer], model_producer: vec![model_producer],
        stable_free: if stable_capacity { 2 } else { 1 },
        producer_free: if producer_capacity { 2 } else { 1 },
        writer: None, model_writer: None, writer_free: 1, member_free: 3,
    };
    proof {
        assert(stable_references_view(inputs.stable@) =~= inputs.model_stable@);
        assert(producer_references_view(inputs.producer@) =~= inputs.model_producer@);
        reveal_with_fuel(logical::release_scan_v1, 3);
        reveal_with_fuel(logical::producer_release_scan_v1, 3);
        reveal_with_fuel(logical::released_leases_v1, 3);
    }
    let mut pair = CompletionJournalPairV1 { actual, model, inputs, last_model_result: Ok(()) };
    let completed = pair.complete_journal_prefix_v1(consumer, SubmissionWriterOutcomeV1::Success);
    assert(completed == if stable_capacity && producer_capacity { Ok(()) } else { Err(ReadErrorV1::InvalidState) });
    assert(pair.actual.stable.free_reads@ == if stable_capacity { seq![1usize, 0] } else { seq![1usize] });
    assert(pair.actual.free@ == if stable_capacity && producer_capacity { seq![1usize, 0] } else { seq![1usize] });
    assert(pair.actual.stable.readers@ == if stable_capacity { seq![0usize, 0, 0] } else { seq![0usize, 1, 0] });
    assert(pair.actual.counts@ == if stable_capacity && producer_capacity { seq![0usize, 0, 0] } else { seq![1usize, 0, 0] });
    true
}

#[verifier::spinoff_prover]
fn completion_writer_only_witness_v1(outcome: SubmissionWriterOutcomeV1, capacity: bool)
    -> (result: bool)
    ensures result,
{
    hide(lifecycle_reader_trace_v1);
    hide(logical::producer_invariant_v1);
    hide(logical::lifecycle_invariant_v1);
    let (actual, model, trace) = lifecycle_mixed_fixture_v1();
    proof { lifecycle_reader_reached_v1(trace@, actual, model); }
    let writer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 {
        context_generation: 7, local: 11, kind: WriterKindV1::Submission } };
    let model_writer = logical::WriterReferenceV1 { slot: 0, key: logical::WriterKeyV1 {
        context_generation: 7, local: 11, kind: logical::WriterKindV1::Submission } };
    let inputs = CompletionJournalInputsV1 {
        consumer: writer.key, model_consumer: model_writer.key,
        stable_present: false, producer_present: false,
        stable: vec![], model_stable: vec![], producer: vec![], model_producer: vec![],
        stable_free: 2, producer_free: 2, writer: Some(writer), model_writer: Some(model_writer),
        writer_free: if capacity { 1 } else { 0 }, member_free: 3,
    };
    proof {
        assert(stable_references_view(inputs.stable@) =~= inputs.model_stable@);
        assert(producer_references_view(inputs.producer@) =~= inputs.model_producer@);
        reveal_with_fuel(logical::retained_scan_v1, 3);
        reveal_with_fuel(logical::settlement_scratch_scan_v1, 3);
        assert(model.stable.journal.scratch@[0].is_none()) by { reveal(logical::producer_invariant_v1); }
        assert(logical::unknown_decision_v1(model.stable.journal, model_writer) == Ok(()));
        assert(logical::settlement_preflight_decision_v1(model.stable.journal, model_writer, model_writer,
            inputs.writer_free, 3) == if capacity { Ok((Some(0usize), 1usize)) } else { Err(logical::ReadErrorV1::InvalidState) });
    }
    let mut pair = CompletionJournalPairV1 { actual, model, inputs, last_model_result: Ok(()) };
    let completed = pair.complete_journal_prefix_v1(writer.key, outcome);
    match outcome {
        SubmissionWriterOutcomeV1::Unknown => {
            assert(completed == Ok(()));
            assert(pair.actual.stable.journal.free@ == seq![]);
            assert(pair.model.stable.journal.writers@[0] == Some(logical::WriterEntryV1::Unknown {
                key: model_writer.key, head: Some(0usize), count: 1usize }));
            proof { writer_entry_slot_round_trip(pair.actual.stable.journal.writers@[0], pair.model.stable.journal.writers@[0]); }
            assert(pair.actual.stable.journal.writers@[0] == Some(WriterEntryV1::Unknown {
                key: writer.key, head: Some(0usize), count: 1usize }));
        },
        _ => {
            assert(completed == if capacity { Ok(()) } else { Err(ReadErrorV1::InvalidState) });
            assert(pair.actual.stable.journal.free@ == if capacity { seq![0usize] } else { seq![] });
        },
    }
    true
}

}
