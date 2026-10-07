verus! {

proof fn enrollment_one_reservation_partition_v1(entry: logical::ProducerReservationV1)
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

// Synthetic live custody fixture; not production Begin/acquire reachability.
#[verifier::spinoff_prover]
fn enrollment_live_custody_witness_v1() -> (result: bool)
    ensures result,
{
    proof {
        reveal_with_fuel(enrollment_roster_scan_v1, 3);
        reveal_with_fuel(enrollment_prefix_v1, 3);
    }
    let mut actual = enrollment_three_vacant_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let mut model = match logical::issued_producer_constructor_exec_v1(7, 3, 1, 2, Ghost(storage)) {
        Ok(contents) => contents, Err(_) => return false,
    };
    let ghost empty = model.stable;
    proof { enrollment_fixture_representation_v1(actual, model.stable.journal); }
    let (model_entry, reservation) = logical::issued_fixture_entry_exec_v1(0);
    let request = reservation.request;
    let ghost history = seq![request.producer];
    let producer = ContextWriterReferenceV1 { slot: 0,
        key: ContextWriterKeyV1 { context_generation: 7, local: 10, kind: ContextWriterKindV1::Submission } };
    let protected = AllocationEntryV1 {
        key: AllocationKeyV1 { context_generation: 7, local: 1 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 },
        byte_extent: 16, attempt_epoch: 1, content_lineage: 0, pending_member: Some(0),
    };
    let allocation = AllocationReferenceV1 { slot: 0, key: protected.key };
    let member = MemberEntryV1 { writer: producer, allocation, prior_lineage: 0, attempt_epoch: 1, next: None };
    actual.allocations.set(0, Some(protected));
    actual.members.set(0, Some(member));
    actual.writers.set(0, Some(WriterEntryV1::Pending { key: producer.key, head: Some(0), count: 1 }));
    let _a = actual.allocation_free.pop();
    let _m = actual.member_free.pop();
    let _w = actual.free.pop();
    actual.registration_watermark = 10;
    model.stable.journal.allocations.set(0, Some(model_entry));
    model.stable.journal.members.set(0, Some(logical::MemberEntryV1 { writer: request.producer,
        allocation: request.read.allocation, prior_lineage: 0, attempt_epoch: 1, next: None }));
    model.stable.journal.writers.set(0, Some(logical::WriterEntryV1::Pending {
        key: request.producer.key, head: Some(0), count: 1 }));
    let _a = model.stable.journal.allocation_free.pop();
    let _m = model.stable.journal.member_free.pop();
    let _w = model.stable.journal.free.pop();
    model.stable.journal.registration_watermark = 10;
    model.reservations.set(0, Some(reservation));
    let _r = model.free.pop();
    model.counts.set(0, 1);
    model.next_incarnation = 2;
    proof {
        assert(model.stable.journal.allocations@ =~= seq![Some(model_entry), None, None]);
        assert(model.stable.journal.allocation_free@ =~= seq![2usize, 1]);
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
        assert(logical::slot_partition_v1(model.stable.journal.allocations@, model.stable.journal.allocation_free@));
        assert(logical::slot_partition_v1(model.stable.journal.members@, model.stable.journal.member_free@));
        assert(logical::slot_partition_v1(model.stable.journal.writers@, model.stable.journal.free@));
        assert forall|a: int| 0 <= a < model.stable.journal.allocations@.len() implies
            #[trigger] logical::allocation_custody_v1(model.stable.journal, a) by {}
        assert forall|m: int| 0 <= m < model.stable.journal.members@.len() implies
            #[trigger] logical::member_custody_v1(model.stable.journal, m) by {}
        assert(logical::writer_custody_v1(model.stable.journal, 0));
        assert(logical::pending_custody_v1(model.stable.journal));
        reveal(logical::producer_entry_valid_v1);
        reveal_with_fuel(logical::producer_count_v1, 3);
        assert(logical::producer_entry_valid_v1(model.stable.journal, reservation, 0, 2));
        assert forall|a: int| 0 <= a < model.counts@.len() implies
            model.counts@[a] == logical::producer_count_v1(model.reservations@, a as usize) by {
            if a == 0 {
                assert(logical::producer_count_v1(model.reservations@, 0) == 1);
            } else {
                logical::producer_count_zero_v1(model.reservations@, a as usize);
            }
        }
        enrollment_one_reservation_partition_v1(reservation);
        assert forall|s: int| 0 <= s < model.reservations@.len() && (#[trigger] model.reservations@[s]).is_some()
            implies logical::producer_entry_valid_v1(model.stable.journal, model.reservations@[s].unwrap(), s,
                model.next_incarnation) by { assert(s == 0); }
        assert(logical::producer_arena_v1(model.stable.journal, model.reservations@,
            model.free@, model.counts@, model.next_incarnation));
        assert(logical::producer_invariant_v1(model));
        reveal_with_fuel(logical::occupied_prefix_v1, 2);
        reveal_with_fuel(logical::reserved_prefix_v1, 2);
        assert(logical::issued_producer_v1(model, storage, history));
        assert(journal_view(actual).writers =~= model.stable.journal.writers@);
        assert(journal_view(actual).allocations =~= model.stable.journal.allocations@);
        assert(journal_view(actual).members =~= model.stable.journal.members@);
        assert(represents(actual, model.stable.journal));
    }
    assert(logical::producer_status_v1(model.stable.journal, request) == Some(logical::ProducerStatusV1::Pending));
    let ghost before = actual;
    let ghost model_before = model;
    let entries = vec![EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 }];
    let model_entries = vec![logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 30 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 }];
    let mut output = vec![None];
    let mut model_output = vec![None];
    proof {
        assert(entries_view(entries@) =~= model_entries@);
        assert(references_view(output@) =~= model_output@);
    }
    let result = enrollment_issued_historical_exec_v1(&mut actual, &mut model, &entries, &model_entries,
        &mut output, &mut model_output, Ghost(storage), Ghost(history));
    assert(result.0 == Ok(()));
    assert(output@[0] == Some(AllocationReferenceV1 { slot: 1, key: entries@[0].key }));
    assert(actual.allocation_free@ == seq![2usize]);
    assert(actual.allocations@[0] == before.allocations@[0]);
    assert(actual.members@[0] == before.members@[0] && actual.writers@[0] == before.writers@[0]);
    assert(model.stable.journal.allocations@[0] == model_before.stable.journal.allocations@[0]);
    assert(model.stable.journal.members@[0] == model_before.stable.journal.members@[0]);
    assert(model.stable.journal.writers@[0] == model_before.stable.journal.writers@[0]);
    assert(model.reservations@ == model_before.reservations@ && model.counts@ == seq![1usize, 0, 0]);
    assert(model.reservations@[0] == Some(reservation));
    assert(model.free@ == model_before.free@ && model.next_incarnation == model_before.next_incarnation);
    assert(logical::producer_status_v1(model.stable.journal, request) == Some(logical::ProducerStatusV1::Pending));
    assert(logical::issued_producer_v1(model, storage, history));
    assert(represents(actual, model.stable.journal));
    true
}

}
