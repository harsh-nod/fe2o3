// Concrete raw states demonstrate admitted and rejected paths, not reachability.
verus! {

#[verifier::spinoff_prover]
fn owner_settlement_nonempty_raw_witness_v1(success: bool) -> (result: bool)
    ensures result,
{
    proof {
        reveal_with_fuel(owner_retained_scan_v1, 4);
        reveal_with_fuel(settlement_scratch_scan_v1, 4);
        reveal_with_fuel(settlement_cursor_v1, 4);
        reveal_with_fuel(settlement_members_prefix_v1, 4);
        reveal_with_fuel(settlement_allocations_prefix_v1, 4);
    }
    let writer = WriterReferenceV1 { slot: 0,
        key: WriterKeyV1 { context_generation: 7, local: 0, kind: WriterKindV1::Submission } };
    let device = ContextJournalDeviceKeyV1 { context_generation: 7, local: 1 };
    let a = AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 10 } };
    let b = AllocationReferenceV1 { slot: 1, key: AllocationKeyV1 { context_generation: 7, local: 11 } };
    let dirty = BeginMemberPlanV1 { member_slot: usize::MAX, allocation: a, prior_lineage: u64::MAX, attempt_epoch: 0 };
    let journal = JournalContentsV1 {
        context_generation: 7, allocation_capacity: 3, writer_capacity: 2,
        registration_watermark: u64::MAX, reserved_count: usize::MAX,
        writers: vec![Some(WriterEntryV1::Pending { key: writer.key, head: Some(1), count: 2 })],
        free: vec![usize::MAX], allocation_free: vec![usize::MAX],
        allocations: vec![
            Some(AllocationEntryV1 { key: a.key, device, byte_extent: 16, attempt_epoch: 7, content_lineage: 3, pending_member: Some(1) }),
            Some(AllocationEntryV1 { key: b.key, device, byte_extent: 32, attempt_epoch: 9, content_lineage: 5, pending_member: Some(0) })],
        members: vec![
            Some(MemberEntryV1 { writer, allocation: b, prior_lineage: 5, attempt_epoch: 9, next: None }),
            Some(MemberEntryV1 { writer, allocation: a, prior_lineage: 3, attempt_epoch: 7, next: Some(0) })],
        member_free: vec![usize::MAX], scratch: vec![None, None, Some(dirty)],
    };
    let stable = ContextReadLeasedJournalV1 { journal, leases: vec![], free_reads: vec![usize::MAX],
        readers: vec![], next_incarnation: 0 };
    let mut owner = ContextProducerReadJournalV1 { stable, reservations: vec![], free: vec![usize::MAX],
        counts: vec![], next_incarnation: 0 };
    let ghost before = owner;
    let no_room = owner.settle_success_observed_v1(writer, &ContextWriterSuccessEvidenceV1 { writer }, 2, 2);
    assert(no_room == Err(ReadErrorV1::InvalidState) && owner == before);
    let outcome = if success {
        owner.settle_success_observed_v1(writer, &ContextWriterSuccessEvidenceV1 { writer }, 2, 3)
    } else {
        owner.settle_no_effect_observed_v1(writer, &ContextWriterNoEffectEvidenceV1 { writer }, 2, 3)
    };
    assert(outcome == Ok(()));
    assert(owner_writer_producer_frame_v1(before, owner));
    assert(owner.stable.journal.registration_watermark == u64::MAX);
    assert(owner.stable.journal.reserved_count == usize::MAX);
    assert(owner.stable.journal.writers@ == seq![None]);
    assert(owner.stable.journal.free@ == seq![usize::MAX, 0usize]);
    assert(owner.stable.journal.member_free@ == seq![usize::MAX, 1usize, 0usize]);
    assert(owner.stable.journal.members@ == seq![None, None]);
    assert(owner.stable.journal.scratch@ == before.stable.journal.scratch@);
    assert(owner.stable.journal.allocations@[0].unwrap().attempt_epoch == 7);
    assert(owner.stable.journal.allocations@[1].unwrap().attempt_epoch == 9);
    assert(owner.stable.journal.allocations@[0].unwrap().content_lineage == if success { 7u64 } else { 3u64 });
    assert(owner.stable.journal.allocations@[1].unwrap().content_lineage == if success { 9u64 } else { 5u64 });
    assert(owner.stable.journal.allocations@[0].unwrap().pending_member.is_none());
    assert(owner.stable.journal.allocations@[1].unwrap().pending_member.is_none());
    let ghost settled = owner;
    let replay = owner.settle_no_effect_observed_v1(writer, &ContextWriterNoEffectEvidenceV1 { writer }, 0, 0);
    assert(replay == Err(ReadErrorV1::InvalidReference) && owner == settled);
    true
}

}
