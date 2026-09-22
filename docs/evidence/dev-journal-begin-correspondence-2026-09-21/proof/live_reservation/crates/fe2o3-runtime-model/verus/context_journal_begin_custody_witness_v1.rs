verus! {

proof fn begin_one_reservation_partition_v1(entry: logical::ProducerReservationV1)
    ensures logical::slot_partition_v1(seq![Some(entry), None], seq![1usize]),
{
    let entries = seq![Some(entry), None];
    let free = seq![1usize];
    assert forall|s: int| 0 <= s < entries.len() implies
        (#[trigger] entries[s]).is_none() == free.contains(s as usize) by {
        if s == 1 { assert(free[0] == s); }
        else if free.contains(s as usize) {
            let i = choose|i: int| 0 <= i < free.len() && free[i] == s as usize;
            assert(i == 0);
        }
    }
}

proof fn begin_two_allocations_partition_v1(first: logical::AllocationEntryV1, second: logical::AllocationEntryV1)
    ensures logical::slot_partition_v1(seq![Some(first), Some(second), None], seq![2usize]),
{
    let entries = seq![Some(first), Some(second), None];
    let free = seq![2usize];
    assert forall|s: int| 0 <= s < entries.len() implies
        (#[trigger] entries[s]).is_none() == free.contains(s as usize) by {
        if s == 2 { assert(free[0] == s); }
        else if free.contains(s as usize) {
            let i = choose|i: int| 0 <= i < free.len() && free[i] == s as usize;
            assert(i == 0);
        }
    }
}

// Separate concrete setup from transition inspection at the default solver limit.
#[verifier::spinoff_prover]
fn begin_historical_live_exercise_v1(actual: &mut JournalContentsV1, model: &mut logical::ProducerReadContentsV1,
    writer: WriterReferenceV1, model_writer: logical::WriterReferenceV1,
    roster: &[AllocationWriteV1], model_roster: &[logical::AllocationWriteV1],
    reservation: logical::ProducerReservationV1,
    Ghost(storage): Ghost<logical::StorageCapacitiesV1>, Ghost(history): Ghost<Seq<logical::WriterReferenceV1>>)
    -> (result: bool)
    requires represents(*old(actual), old(model).stable.journal),
        logical::issued_producer_v1(*old(model), storage, history),
        writer_reference_view(writer) == model_writer, begin_roster_view(roster@) == model_roster@,
        begin_preflight_decision_v1(*old(actual), writer, roster@) == Ok(0),
        logical::begin_issued_decision_v1(*old(model), model_writer, model_roster@) == Ok(()),
        begin_historical_guards_pass_v1(*old(model), model_roster@),
        roster.len() == 1, roster@[0].allocation.slot == 1, writer.slot == 1,
        reservation == logical::issued_fixture_entry_v1(0).1,
        old(model).reservations@[0] == Some(reservation),
        old(model).counts@ == seq![1usize, 0, 0], old(model).stable.readers@ == seq![0usize, 0, 0],
        logical::producer_status_v1(old(model).stable.journal, reservation.request) == Some(logical::ProducerStatusV1::Pending),
        old(actual).writers@.len() == 2, old(actual).allocations@.len() == 3,
        old(actual).allocations@[1].is_some(), old(actual).allocations@[1].unwrap().attempt_epoch == 0,
        old(actual).members@.len() == 3,
        old(actual).member_free@ == seq![2usize, 1],
    ensures result,
{
    proof {
        reveal_with_fuel(begin_members_prefix_v1, 3);
        reveal_with_fuel(begin_allocations_prefix_v1, 3);
        reveal_with_fuel(logical::producer_unread_scan_v1, 3);
        reveal_with_fuel(logical::unread_scan_v1, 3);
    }
    let request = reservation.request;
    let allocation = AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 1 } };
    let ghost before = *actual;
    let ghost model_before = *model;
    let result = begin_guarded_issued_historical_exec_v1(actual, model, writer, model_writer,
        roster, model_roster, Ghost(storage), Ghost(history));
    assert(result.0 == Some(Ok(())) && result.1 == Ok(()));
    assert(actual.allocations@[1].unwrap().attempt_epoch == 1);
    assert(actual.allocations@[1].unwrap().pending_member == Some(1));
    assert(actual.members@[1].unwrap().writer == writer);
    assert(actual.allocations@[0] == before.allocations@[0]);
    assert(actual.members@[0] == before.members@[0] && actual.writers@[0] == before.writers@[0]);
    assert(model.reservations@ == model_before.reservations@ && model.counts@ == seq![1usize, 0, 0]);
    assert(model.reservations@[0] == Some(reservation));
    assert(model.free@ == model_before.free@ && model.next_incarnation == model_before.next_incarnation);
    proof {
        assert(model.stable.journal.allocations@[0] == model_before.stable.journal.allocations@[0]);
        assert(model.stable.journal.members@[0] == model_before.stable.journal.members@[0]);
        assert(model.stable.journal.writers@[0] == model_before.stable.journal.writers@[0]);
    }
    assert(logical::producer_status_v1(model.stable.journal, request) == Some(logical::ProducerStatusV1::Pending));
    assert(logical::issued_producer_v1(*model, storage, history));

    let empty_roster = vec![];
    let model_empty_roster = vec![];
    proof { assert(begin_roster_view(empty_roster@) =~= model_empty_roster@); }
    let ghost pending = *actual;
    let ghost model_pending = *model;
    let replay = begin_guarded_issued_historical_exec_v1(actual, model, writer, model_writer,
        &empty_roster, &model_empty_roster, Ghost(storage), Ghost(history));
    assert(replay.0 == Some(Err(ReadErrorV1::InvalidReference)));
    assert(replay.1 == Err(logical::ReadErrorV1::InvalidReference));
    assert(*actual == pending && *model == model_pending);

    // Combined unread rejection precedes even an invalid writer, and skips actual Begin.
    let bad_writer = WriterReferenceV1 { slot: usize::MAX, key: writer.key };
    let model_bad_writer = logical::WriterReferenceV1 { slot: usize::MAX, key: model_writer.key };
    let blocked = vec![AllocationWriteV1 { allocation, device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    let model_blocked = vec![logical::AllocationWriteV1 {
        allocation: request.read.allocation, device: request.read.device, byte_extent: 16 }];
    proof { assert(begin_roster_view(blocked@) =~= model_blocked@); }
    let ghost blocked_before = *actual;
    let ghost model_blocked_before = *model;
    let blocked_result = begin_guarded_issued_historical_exec_v1(actual, model, bad_writer, model_bad_writer,
        &blocked, &model_blocked, Ghost(storage), Ghost(history));
    assert(blocked_result.0.is_none() && blocked_result.1 == Err(logical::ReadErrorV1::AllocationBusy));
    assert(*actual == blocked_before && *model == model_blocked_before);
    assert(represents(*actual, model.stable.journal));
    true
}

// Synthetic live reservation and a distinct Reserved writer; not actual constructor/acquire reachability.
#[verifier::spinoff_prover]
fn begin_historical_live_custody_witness_v1() -> (result: bool)
    ensures result,
{
    let ghost storage = logical::witness_storage_v1(3, 2);
    let mut model = match logical::issued_producer_constructor_exec_v1(7, 3, 2, 2, Ghost(storage)) {
        Ok(contents) => contents, Err(_) => return false,
    };
    let ghost empty = model.stable;
    let (model_entry, reservation) = logical::issued_fixture_entry_exec_v1(0);
    let request = reservation.request;
    let model_writer = logical::WriterReferenceV1 { slot: 1,
        key: logical::WriterKeyV1 { context_generation: 7, local: 20, kind: logical::WriterKindV1::Submission } };
    let ghost history = seq![request.producer, model_writer];
    let producer = WriterReferenceV1 { slot: 0,
        key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } };
    let writer = WriterReferenceV1 { slot: 1,
        key: WriterKeyV1 { context_generation: 7, local: 20, kind: WriterKindV1::Submission } };
    let protected = AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 1 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        attempt_epoch: 1, content_lineage: 0, pending_member: Some(0) };
    let destination = AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32,
        attempt_epoch: 0, content_lineage: 0, pending_member: None };
    let model_destination = logical::AllocationEntryV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 30 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32,
        attempt_epoch: 0, content_lineage: 0, pending_member: None };
    let allocation = AllocationReferenceV1 { slot: 0, key: protected.key };
    let member = MemberEntryV1 { writer: producer, allocation, prior_lineage: 0, attempt_epoch: 1, next: None };
    let mut actual = JournalContentsV1 { context_generation: 7, allocation_capacity: 3, writer_capacity: 2,
        registration_watermark: 20, reserved_count: 1,
        writers: vec![Some(WriterEntryV1::Pending { key: producer.key, head: Some(0), count: 1 }),
            Some(WriterEntryV1::Reserved(writer.key))], free: vec![],
        allocations: vec![Some(protected), Some(destination), None], allocation_free: vec![2usize],
        members: vec![Some(member), None, None], member_free: vec![2usize, 1],
        scratch: vec![None, None, None] };
    model.stable.journal.allocations.set(0, Some(model_entry));
    model.stable.journal.allocations.set(1, Some(model_destination));
    model.stable.journal.members.set(0, Some(logical::MemberEntryV1 { writer: request.producer,
        allocation: request.read.allocation, prior_lineage: 0, attempt_epoch: 1, next: None }));
    model.stable.journal.writers.set(0, Some(logical::WriterEntryV1::Pending {
        key: request.producer.key, head: Some(0), count: 1 }));
    model.stable.journal.writers.set(1, Some(logical::WriterEntryV1::Reserved(model_writer.key)));
    let _a = model.stable.journal.allocation_free.pop();
    let _a = model.stable.journal.allocation_free.pop();
    let _m = model.stable.journal.member_free.pop();
    let _w = model.stable.journal.free.pop();
    let _w = model.stable.journal.free.pop();
    model.stable.journal.registration_watermark = 20;
    model.stable.journal.reserved_count = 1;
    model.reservations.set(0, None);
    let _r = model.free.pop();
    model.counts.set(0, 1);
    model.next_incarnation = 2;
    proof {
        assert(model.stable.journal.allocations@ =~= seq![Some(model_entry), Some(model_destination), None]);
        assert(model.stable.journal.allocation_free@ =~= seq![2usize]);
        assert(model.stable.journal.member_free@ =~= seq![2usize, 1]);
        assert(model.stable.journal.free@ =~= Seq::<usize>::empty());
        assert(model.reservations@ =~= seq![Some(reservation), None]);
        assert(model.free@ =~= seq![1usize]);
        assert(model.counts@ =~= seq![1usize, 0, 0]);
        logical::reader_allocation_frame_preserves_v1(empty, model.stable);
        reveal(logical::chain_link_v1);
        assert(logical::chain_link_v1(model.stable.journal, request.producer, seq![0usize], 0));
        assert forall|m: int| 0 <= m < model.stable.journal.members@.len()
            && (#[trigger] model.stable.journal.members@[m]).is_some()
            && logical::same_producer_v1(model.stable.journal.members@[m].unwrap().writer, request.producer)
            implies seq![0usize].contains(m as usize) by { assert(m == 0); }
        assert(logical::retained_chain_v1(model.stable.journal, request.producer, seq![0usize]));
        reveal(logical::allocation_custody_v1);
        reveal(logical::member_custody_v1);
        reveal(logical::writer_custody_v1);
        begin_two_allocations_partition_v1(model_entry, model_destination);
        assert(logical::slot_partition_v1(model.stable.journal.allocations@, model.stable.journal.allocation_free@));
        assert(logical::slot_partition_v1(model.stable.journal.members@, model.stable.journal.member_free@));
        assert(logical::slot_partition_v1(model.stable.journal.writers@, model.stable.journal.free@));
        assert forall|a: int| 0 <= a < model.stable.journal.allocations@.len() implies
            #[trigger] logical::allocation_custody_v1(model.stable.journal, a) by {}
        assert forall|m: int| 0 <= m < model.stable.journal.members@.len() implies
            #[trigger] logical::member_custody_v1(model.stable.journal, m) by {}
        assert forall|w: int| 0 <= w < model.stable.journal.writers@.len() implies
            #[trigger] logical::writer_custody_v1(model.stable.journal, w) by {}
        assert(logical::pending_custody_v1(model.stable.journal));
        reveal(logical::producer_entry_valid_v1);
        reveal_with_fuel(logical::producer_count_v1, 3);
        assert(logical::producer_entry_valid_v1(model.stable.journal, reservation, 0, 2));
        assert forall|a: int| 0 <= a < model.counts@.len() implies
            model.counts@[a] == logical::producer_count_v1(model.reservations@, a as usize) by {
            if a == 0 { assert(logical::producer_count_v1(model.reservations@, 0) == 1); }
            else { logical::producer_count_zero_v1(model.reservations@, a as usize); }
        }
        begin_one_reservation_partition_v1(reservation);
        assert forall|s: int| 0 <= s < model.reservations@.len() && (#[trigger] model.reservations@[s]).is_some()
            implies logical::producer_entry_valid_v1(model.stable.journal, model.reservations@[s].unwrap(), s,
                model.next_incarnation) by { assert(s == 0); }
        assert(logical::producer_arena_v1(model.stable.journal, model.reservations@,
            model.free@, model.counts@, model.next_incarnation));
        assert(logical::producer_invariant_v1(model));
        reveal_with_fuel(logical::occupied_prefix_v1, 3);
        reveal_with_fuel(logical::reserved_prefix_v1, 3);
        assert(logical::issued_producer_v1(model, storage, history));
        assert(journal_view(actual).writers =~= model.stable.journal.writers@);
        assert(journal_view(actual).allocations =~= model.stable.journal.allocations@);
        assert(journal_view(actual).members =~= model.stable.journal.members@);
        assert(journal_view(actual).scratch =~= model.stable.journal.scratch@);
        assert(journal_view(actual).free =~= model.stable.journal.free@);
        assert(journal_view(actual).allocation_free =~= model.stable.journal.allocation_free@);
        assert(journal_view(actual).member_free =~= model.stable.journal.member_free@);
        assert(represents(actual, model.stable.journal));
    }
    assert(logical::producer_status_v1(model.stable.journal, request) == Some(logical::ProducerStatusV1::Pending));
    let target = AllocationReferenceV1 { slot: 1, key: destination.key };
    let model_target = logical::AllocationReferenceV1 { slot: 1, key: model_destination.key };
    let roster = vec![AllocationWriteV1 { allocation: target, device: destination.device, byte_extent: 32 }];
    let model_roster = vec![logical::AllocationWriteV1 { allocation: model_target, device: model_destination.device, byte_extent: 32 }];
    proof {
        reveal_with_fuel(begin_canonical_scan_v1, 3);
        reveal_with_fuel(begin_destination_scan_v1, 3);
        reveal_with_fuel(begin_slot_scan_v1, 3);
        reveal_with_fuel(logical::producer_unread_scan_v1, 3);
        reveal_with_fuel(logical::unread_scan_v1, 3);
        assert(model.stable.readers@ =~= seq![0usize, 0, 0]);
        assert(begin_roster_view(roster@) =~= model_roster@);
        begin_preflight_correspondence(actual, model.stable.journal, writer, roster@);
    }
    begin_historical_live_exercise_v1(&mut actual, &mut model, writer, model_writer, &roster, &model_roster,
        reservation, Ghost(storage), Ghost(history))
}

}
