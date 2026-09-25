verus! {

// Constructor reachability/physical allocation remain separate; subsequent
// transitions below execute the shared public methods and historical leaves.
#[verifier::spinoff_prover]
fn owner_writer_register_abort_reuse_begin_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_enrollment_fixture_v1().unwrap();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost empty = Seq::<logical::WriterReferenceV1>::empty();
    proof { logical::issued_constructor_v1(model.stable.journal, 7, 3, 1, storage); }
    let key = WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission };
    let model_key = logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission };
    let registered = owner_register_historical_exec_v1(&mut actual, &mut model, key, model_key, Ghost(storage), Ghost(empty));
    assert(registered.0.is_ok() && registered.1.is_ok());
    let writer = registered.0.unwrap();
    let model_writer = match registered.1 { Ok(value) => value, Err(_) => return false };
    let ghost history = logical::registration_history_v1(empty, registered.1);
    proof { writer_reference_round_trip(writer, model_writer); }
    let found = owner_reserved_lookup_historical_exec_v1(&actual.stable.journal, &model.stable.journal, writer, model_writer);
    assert(found.0 == Ok(key) && found.1 == Ok(model_key));
    let aborted = owner_abort_historical_exec_v1(&mut actual, &mut model, writer, model_writer, 1, Ghost(storage), Ghost(history));
    assert(aborted.0 == Ok(()) && aborted.1 == Ok(()));
    assert(actual.stable.journal.registration_watermark == 10);
    let ghost rejected_before = actual;
    let replay = owner_register_historical_exec_v1(&mut actual, &mut model, key, model_key, Ghost(storage), Ghost(history));
    assert(replay.0 == Err(ReadErrorV1::WriterReplay) && actual == rejected_before);
    let next_key = WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Synchronous };
    let model_next_key = logical::WriterKeyV1 { context_generation: 7, local: 11, kind: logical::WriterKindV1::Synchronous };
    let next = owner_register_historical_exec_v1(&mut actual, &mut model, next_key, model_next_key, Ghost(storage), Ghost(history));
    assert(next.0.is_ok() && next.1.is_ok());
    let next_writer = next.0.unwrap();
    let model_next_writer = match next.1 { Ok(value) => value, Err(_) => return false };
    let ghost next_history = logical::registration_history_v1(history, next.1);
    assert(next_writer.slot == writer.slot);
    proof { writer_reference_round_trip(next_writer, model_next_writer); }
    let ghost before_stale = actual;
    let stale = owner_abort_historical_exec_v1(&mut actual, &mut model, writer, model_writer, 1, Ghost(storage), Ghost(next_history));
    assert(stale.0 == Err(ReadErrorV1::InvalidReference) && actual == before_stale);
    let roster: Vec<AllocationWriteV1> = vec![];
    let model_roster: Vec<logical::AllocationWriteV1> = vec![];
    proof { assert(begin_roster_view(roster@) =~= model_roster@); }
    let begun = producer_begin_historical_exec_v1(&mut actual, &mut model, next_writer, model_next_writer,
        &roster, &model_roster, Ghost(storage), Ghost(next_history));
    assert(begun.0 == Ok(()) && begun.1 == Ok(()));
    assert(actual.stable.journal.writers@[0] == Some(WriterEntryV1::Pending { key: next_key, head: None, count: 0 }));
    let found = owner_reserved_lookup_historical_exec_v1(&actual.stable.journal, &model.stable.journal, next_writer, model_next_writer);
    assert(found.0 == Err(ReadErrorV1::InvalidReference));
    let ghost before_abort = actual;
    let rejected = owner_abort_historical_exec_v1(&mut actual, &mut model, next_writer, model_next_writer,
        1, Ghost(storage), Ghost(next_history));
    assert(rejected.0 == Err(ReadErrorV1::InvalidReference) && actual == before_abort);
    true
}

#[verifier::spinoff_prover]
fn owner_writer_enroll_register_member_begin_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_enrollment_fixture_v1().unwrap();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    proof { logical::issued_constructor_v1(model.stable.journal, 7, 3, 1, storage); }
    let entries = vec![EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    let model_entries = vec![logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 20 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    let mut output = vec![None];
    let mut model_output = vec![None];
    let ghost before = model;
    let ghost original = model_output@;
    proof {
        assert(entries_view(entries@) =~= model_entries@);
        assert(references_view(output@) =~= model_output@);
        reveal_with_fuel(enrollment_roster_scan_v1, 3);
        reveal_with_fuel(enrollment_prefix_v1, 3);
    }
    let enrolled = producer_enrollment_historical_exec_v1(&mut actual, &mut model, &entries, &model_entries, &mut output, &mut model_output);
    assert(enrolled.0 == Ok(()) && enrolled.1 == Ok(()));
    proof {
        logical::enrollment_success_admission_v1(before.stable.journal, model_entries@, original);
        logical::enrollment_preserves_issued_producer_v1(before, model, model_entries@, model_output@, 2, storage, history);
    }
    let key = WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission };
    let model_key = logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission };
    let registered = owner_register_historical_exec_v1(&mut actual, &mut model, key, model_key, Ghost(storage), Ghost(history));
    assert(registered.0.is_ok() && registered.1.is_ok());
    let writer = registered.0.unwrap();
    let model_writer = match registered.1 { Ok(value) => value, Err(_) => return false };
    let ghost history = logical::registration_history_v1(history, registered.1);
    let roster = vec![AllocationWriteV1 { allocation: output[0].unwrap(), device: entries[0].device, byte_extent: 16 }];
    let model_roster = vec![logical::AllocationWriteV1 { allocation: model_output[0].unwrap(), device: model_entries[0].device, byte_extent: 16 }];
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
    assert(actual.stable.journal.allocations@[0].unwrap().attempt_epoch == 1);
    assert(actual.stable.journal.allocations@[0].unwrap().pending_member == Some(0));
    assert(actual.stable.journal.members@[0].unwrap().writer == writer);
    assert(actual.stable.journal.writers@[0] == Some(WriterEntryV1::Pending { key, head: Some(0), count: 1 }));
    assert(logical::issued_producer_v1(model, storage, history));
    true
}

#[verifier::spinoff_prover]
fn owner_writer_raw_capacity_and_identity_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_enrollment_fixture_v1().unwrap();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    // Deliberately malformed outer storage and a non-issuable existing key.
    actual.counts = vec![];
    model.counts = vec![];
    actual.next_incarnation = 0;
    model.next_incarnation = 0;
    let key = WriterKeyV1 { context_generation: 7, local: 0, kind: WriterKindV1::Submission };
    let model_key = logical::WriterKeyV1 { context_generation: 7, local: 0, kind: logical::WriterKindV1::Submission };
    let writer = WriterReferenceV1 { slot: 0, key };
    let model_writer = logical::WriterReferenceV1 { slot: 0, key: model_key };
    actual.stable.journal.writers = vec![Some(WriterEntryV1::Reserved(key))];
    model.stable.journal.writers = vec![Some(logical::WriterEntryV1::Reserved(model_key))];
    actual.stable.journal.free = vec![];
    model.stable.journal.free = vec![];
    actual.stable.journal.reserved_count = 1;
    model.stable.journal.reserved_count = 1;
    proof {
        assert(actual.counts@ =~= model.counts@);
        assert(actual.stable.journal.free@ =~= model.stable.journal.free@);
        assert(journal_view(actual.stable.journal).writers =~= model.stable.journal.writers@);
    }
    let found = owner_reserved_lookup_historical_exec_v1(&actual.stable.journal, &model.stable.journal, writer, model_writer);
    assert(found.0 == Ok(key) && found.1 == Ok(model_key));
    let ghost before = actual;
    let no_room = owner_abort_historical_exec_v1(&mut actual, &mut model, writer, model_writer, 0, Ghost(storage), Ghost(history));
    assert(no_room.0 == Err(ReadErrorV1::InvalidState) && actual == before);
    // Observation-parameter contrast, not a claim about the literal vec![] capacity.
    let room = owner_abort_historical_exec_v1(&mut actual, &mut model, writer, model_writer, 1, Ghost(storage), Ghost(history));
    assert(room.0 == Ok(()) && room.1 == Ok(()));
    assert(actual.counts == before.counts && actual.next_incarnation == 0);
    assert(actual.stable.journal.writers@ == seq![None]);
    assert(actual.stable.journal.free@ == seq![0usize]);
    true
}

}
