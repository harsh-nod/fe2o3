// Concrete raw pair. Allocation failure and constructor reachability are out of scope.
verus! {

#[verifier::spinoff_prover]
fn begin_historical_raw_witness_v1(aliased: bool) -> (result: bool)
    ensures result,
{
    proof {
        reveal_with_fuel(begin_canonical_scan_v1, 3);
        reveal_with_fuel(begin_destination_scan_v1, 3);
        reveal_with_fuel(begin_slot_scan_v1, 3);
        reveal_with_fuel(begin_members_prefix_v1, 3);
        reveal_with_fuel(begin_allocations_prefix_v1, 3);
    }
    let writer = WriterReferenceV1 { slot: 0,
        key: WriterKeyV1 { context_generation: 7, local: 41, kind: WriterKindV1::Synchronous } };
    let model_writer = logical::WriterReferenceV1 { slot: 0,
        key: logical::WriterKeyV1 { context_generation: 7, local: 41, kind: logical::WriterKindV1::Synchronous } };
    let first = AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 1 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        attempt_epoch: 3, content_lineage: 9, pending_member: None };
    let second = AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 2 },
        device: first.device, byte_extent: 32, attempt_epoch: 5, content_lineage: 11, pending_member: None };
    let model_first = logical::AllocationEntryV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 1 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        attempt_epoch: 3, content_lineage: 9, pending_member: None };
    let model_second = logical::AllocationEntryV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 2 },
        device: model_first.device, byte_extent: 32, attempt_epoch: 5, content_lineage: 11, pending_member: None };
    let first_ref = AllocationReferenceV1 { slot: 0, key: first.key };
    let second_ref = AllocationReferenceV1 { slot: 1, key: second.key };
    let model_first_ref = logical::AllocationReferenceV1 { slot: 0, key: model_first.key };
    let model_second_ref = logical::AllocationReferenceV1 { slot: 1, key: model_second.key };
    let dirty = BeginMemberPlanV1 { member_slot: usize::MAX, allocation: first_ref,
        prior_lineage: u64::MAX, attempt_epoch: 0 };
    let model_dirty = logical::BeginMemberPlanV1 { member_slot: usize::MAX, allocation: model_first_ref,
        prior_lineage: u64::MAX, attempt_epoch: 0 };
    let mut actual = JournalContentsV1 { context_generation: 7, allocation_capacity: 2, writer_capacity: 1,
        registration_watermark: 41, reserved_count: 1,
        writers: vec![Some(WriterEntryV1::Reserved(writer.key))], free: vec![],
        allocations: vec![Some(first), Some(second)], allocation_free: vec![],
        members: vec![None, None, None], member_free: if aliased { vec![0usize, 0, 0] } else { vec![2usize, 1, 0] },
        scratch: vec![None, None, Some(dirty)] };
    let mut model = logical::JournalContentsV1 { context_generation: 7, allocation_capacity: 2, writer_capacity: 1,
        registration_watermark: 41, reserved_count: 1,
        writers: vec![Some(logical::WriterEntryV1::Reserved(model_writer.key))], free: vec![],
        allocations: vec![Some(model_first), Some(model_second)], allocation_free: vec![],
        members: vec![None, None, None], member_free: if aliased { vec![0usize, 0, 0] } else { vec![2usize, 1, 0] },
        scratch: vec![None, None, Some(model_dirty)] };
    let roster = vec![AllocationWriteV1 { allocation: first_ref, device: first.device, byte_extent: 16 },
        AllocationWriteV1 { allocation: second_ref, device: second.device, byte_extent: 32 }];
    let model_roster = vec![logical::AllocationWriteV1 { allocation: model_first_ref, device: model_first.device, byte_extent: 16 },
        logical::AllocationWriteV1 { allocation: model_second_ref, device: model_second.device, byte_extent: 32 }];
    proof {
        assert(journal_view(actual).writers =~= model.writers@);
        assert(journal_view(actual).allocations =~= model.allocations@);
        assert(journal_view(actual).members =~= model.members@);
        assert(journal_view(actual).scratch =~= model.scratch@);
        assert(begin_roster_view(roster@) =~= model_roster@);
        assert(represents(actual, model));
    }
    let ghost before = actual;
    let admitted = begin_historical_exec_v1(&mut actual, &mut model, writer, model_writer, &roster, &model_roster);
    assert(admitted.0 == Ok(()) && admitted.1 == Ok(()));
    assert(actual.allocations@[0].unwrap().attempt_epoch == 4);
    assert(actual.allocations@[1].unwrap().attempt_epoch == 6);
    assert(actual.scratch@ == before.scratch@);
    if aliased {
        assert(actual.members@[0] == Some(MemberEntryV1 { writer, allocation: second_ref,
            prior_lineage: 11, attempt_epoch: 6, next: None }));
        assert(actual.members@[1].is_none() && actual.members@[2].is_none());
        assert(actual.member_free@ == seq![0usize]);
        assert(actual.allocations@[0].unwrap().pending_member == Some(0));
        assert(actual.allocations@[1].unwrap().pending_member == Some(0));
    } else {
        assert(actual.members@[0].unwrap().next == Some(1));
        assert(actual.members@[1].unwrap().allocation == second_ref);
        assert(actual.member_free@ == seq![2usize]);
    }
    let ghost committed = actual;
    let ghost model_committed = model;
    let rejected = begin_historical_exec_v1(&mut actual, &mut model, writer, model_writer, &roster, &model_roster);
    assert(rejected.0 == Err(ReadErrorV1::InvalidReference));
    assert(actual == committed && model == model_committed);

    actual.writers.set(0, Some(WriterEntryV1::Reserved(writer.key)));
    model.writers.set(0, Some(logical::WriterEntryV1::Reserved(model_writer.key)));
    actual.reserved_count = usize::MAX;
    model.reserved_count = usize::MAX;
    let swapped = vec![roster[1], roster[0]];
    let model_swapped = vec![model_roster[1], model_roster[0]];
    proof {
        assert(journal_view(actual).writers =~= model.writers@);
        assert(begin_roster_view(swapped@) =~= model_swapped@);
    }
    let ghost before_rejection = actual;
    let ghost model_before_rejection = model;
    let noncanonical = begin_historical_exec_v1(&mut actual, &mut model, writer, model_writer, &swapped, &model_swapped);
    assert(noncanonical.0 == Err(ReadErrorV1::NonCanonicalRoster));
    assert(actual == before_rejection && model == model_before_rejection);

    actual.allocation_capacity = 0;
    actual.writer_capacity = 0;
    actual.registration_watermark = u64::MAX;
    model.allocation_capacity = 0;
    model.writer_capacity = 0;
    model.registration_watermark = u64::MAX;
    let empty = vec![];
    let model_empty = vec![];
    proof { assert(begin_roster_view(empty@) =~= model_empty@); }
    let ghost before_empty = actual;
    let admitted_empty = begin_historical_exec_v1(&mut actual, &mut model, writer, model_writer, &empty, &model_empty);
    assert(admitted_empty.0 == Ok(()) && admitted_empty.1 == Ok(()));
    assert(actual.reserved_count == usize::MAX - 1);
    assert(actual.writers@[0] == Some(WriterEntryV1::Pending { key: writer.key, head: None, count: 0 }));
    assert(actual.allocations@ == before_empty.allocations@ && actual.members@ == before_empty.members@);
    assert(actual.scratch@ == before_empty.scratch@ && actual.member_free@ == before_empty.member_free@);
    assert(represents(actual, model));
    true
}

}
