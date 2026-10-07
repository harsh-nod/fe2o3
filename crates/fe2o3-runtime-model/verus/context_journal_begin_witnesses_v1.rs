// Concrete raw contents, not constructor reachability or globally valid custody.
verus! {

#[verifier::spinoff_prover]
fn begin_raw_alias_witness_v1() -> (result: bool)
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
    let first = AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 1 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        attempt_epoch: 3, content_lineage: 9, pending_member: None };
    let second = AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 2 },
        device: first.device, byte_extent: 32, attempt_epoch: 5, content_lineage: 11, pending_member: None };
    let first_ref = AllocationReferenceV1 { slot: 0, key: first.key };
    let second_ref = AllocationReferenceV1 { slot: 1, key: second.key };
    let dirty = BeginMemberPlanV1 { member_slot: usize::MAX, allocation: first_ref,
        prior_lineage: u64::MAX, attempt_epoch: 0 };
    let mut journal = JournalContentsV1 { context_generation: 7, allocation_capacity: 2, writer_capacity: 1,
        registration_watermark: 41, reserved_count: 1,
        writers: vec![Some(WriterEntryV1::Reserved(writer.key))], free: vec![],
        allocations: vec![Some(first), Some(second)], allocation_free: vec![],
        members: vec![None, None], member_free: vec![0usize, 0, 0], scratch: vec![None, None, Some(dirty)] };
    let roster = vec![AllocationWriteV1 { allocation: first_ref, device: first.device, byte_extent: 16 },
        AllocationWriteV1 { allocation: second_ref, device: second.device, byte_extent: 32 }];
    let ghost original = journal;
    let admitted = begin_exec_v1(&mut journal, writer, &roster);
    assert(admitted == Ok(()));
    assert(journal.members@[0] == Some(MemberEntryV1 { writer, allocation: second_ref,
        prior_lineage: 11, attempt_epoch: 6, next: None }));
    assert(journal.members@[1].is_none());
    assert(journal.allocations@[0].unwrap().attempt_epoch == 4);
    assert(journal.allocations@[1].unwrap().attempt_epoch == 6);
    assert(journal.allocations@[0].unwrap().pending_member == Some(0));
    assert(journal.allocations@[1].unwrap().pending_member == Some(0));
    assert(journal.member_free@ == seq![0usize]);
    assert(journal.scratch@ == original.scratch@);
    assert(journal.writers@[0] == Some(WriterEntryV1::Pending { key: writer.key, head: Some(0), count: 2 }));
    let ghost committed = journal;
    let rejected = begin_exec_v1(&mut journal, writer, &roster);
    assert(rejected == Err(ReadErrorV1::InvalidReference));
    assert(journal == committed);

    journal.writers.set(0, Some(WriterEntryV1::Reserved(writer.key)));
    journal.reserved_count = usize::MAX;
    journal.allocation_capacity = 0;
    journal.writer_capacity = 0;
    journal.registration_watermark = u64::MAX;
    let empty = vec![];
    let ghost before_empty = journal;
    let admitted_empty = begin_exec_v1(&mut journal, writer, &empty);
    assert(admitted_empty == Ok(()));
    assert(journal.reserved_count == usize::MAX - 1);
    assert(journal.writers@[0] == Some(WriterEntryV1::Pending { key: writer.key, head: None, count: 0 }));
    assert(journal.allocations@ == before_empty.allocations@ && journal.members@ == before_empty.members@);
    assert(journal.scratch@ == before_empty.scratch@ && journal.member_free@ == before_empty.member_free@);
    true
}

#[verifier::spinoff_prover]
fn begin_raw_repeated_destination_witness_v1() -> (result: bool)
    ensures result,
{
    proof {
        reveal_with_fuel(begin_canonical_scan_v1, 3);
        reveal_with_fuel(begin_members_prefix_v1, 3);
        reveal_with_fuel(begin_allocations_prefix_v1, 3);
    }
    let writer = WriterReferenceV1 { slot: 0,
        key: WriterKeyV1 { context_generation: 7, local: 41, kind: WriterKindV1::Synchronous } };
    let entry = AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 1 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        attempt_epoch: 3, content_lineage: 99, pending_member: None };
    let reference = AllocationReferenceV1 { slot: 0, key: entry.key };
    let destination = AllocationWriteV1 { allocation: reference, device: entry.device, byte_extent: 16 };
    let mut journal = JournalContentsV1 { context_generation: 7, allocation_capacity: 2, writer_capacity: 1,
        registration_watermark: 41, reserved_count: 1,
        writers: vec![Some(WriterEntryV1::Reserved(writer.key))], free: vec![],
        allocations: vec![Some(entry)], allocation_free: vec![],
        members: vec![None, None], member_free: vec![1usize, 0], scratch: vec![None, None] };
    let roster = vec![destination, destination];
    assert(begin_preflight_decision_v1(journal, writer, roster@) == Err(ReadErrorV1::NonCanonicalRoster));
    let ghost rejected_before = journal;
    let rejected = begin_exec_v1(&mut journal, writer, &roster);
    assert(rejected == Err(ReadErrorV1::NonCanonicalRoster));
    assert(journal == rejected_before);
    assert(begin_storage_ready_v1(journal, roster@));
    let ghost before = journal;
    begin_stage_exec_v1(&mut journal, &roster);
    begin_commit_exec_v1(&mut journal, writer, &roster, 0, Ghost(before));
    assert(journal.allocations@[0].unwrap().attempt_epoch == 4);
    assert(journal.allocations@[0].unwrap().content_lineage == 99);
    assert(journal.allocations@[0].unwrap().pending_member == Some(1));
    assert(journal.members@[0] == Some(MemberEntryV1 { writer, allocation: reference,
        prior_lineage: 99, attempt_epoch: 4, next: Some(1) }));
    assert(journal.members@[1] == Some(MemberEntryV1 { writer, allocation: reference,
        prior_lineage: 99, attempt_epoch: 4, next: None }));
    assert(journal.member_free@ == Seq::<usize>::empty());
    assert(journal.scratch@ == seq![None, None]);
    true
}

}
