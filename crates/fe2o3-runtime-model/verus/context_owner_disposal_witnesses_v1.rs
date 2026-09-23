verus! {

fn owner_disposal_roster_fixture_v1() -> (result: (Vec<AllocationWriteV1>, Vec<logical::AllocationWriteV1>))
    ensures begin_roster_view(result.0@) == result.1@,
        result.0@ == seq![AllocationWriteV1 {
            allocation: AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }],
{
    let roster = vec![AllocationWriteV1 {
        allocation: AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    let model_roster = vec![logical::AllocationWriteV1 {
        allocation: logical::AllocationReferenceV1 { slot: 0, key: logical::AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    proof { assert(begin_roster_view(roster@) =~= model_roster@); }
    (roster, model_roster)
}

// The fixture starts from synthetic storage; lifecycle calls on both sides are executable.
#[verifier::spinoff_prover]
fn owner_disposal_live_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, writer, model_writer) = owner_settlement_pending_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    let (roster, model_roster) = owner_disposal_roster_fixture_v1();
    proof {
        reveal_with_fuel(owner_retained_scan_v1, 3);
        reveal_with_fuel(disposal_roster_scan_v1, 3);
        reveal_with_fuel(settlement_scratch_scan_v1, 3);
        reveal_with_fuel(retirement_prefix_v1, 3);
        reveal_with_fuel(settlement_members_prefix_v1, 3);
        reveal_with_fuel(disposal_producer_safe_v1, 3);
        reveal_with_fuel(producer_unread_writes_v1, 3);
        reveal_with_fuel(stable_unread_writes_v1, 3);
    }
    let marked = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(marked.0 == Ok(()));
    let ghost before = actual;
    let disposed = owner_disposal_paired_exec_v1(&mut actual, &mut model, writer, model_writer, writer, model_writer,
        &roster, &model_roster, 1, 3, 3, Ghost(storage), Ghost(history));
    assert(disposed.0 == Ok(()));
    assert(actual.stable.journal.writers@ =~= seq![None]);
    assert(actual.stable.journal.free@ =~= seq![0usize]);
    assert(actual.stable.journal.allocations@[0].is_none());
    assert(actual.stable.journal.allocations@[1] == before.stable.journal.allocations@[1]);
    assert(actual.stable.journal.allocation_free@ =~= seq![2usize, 0]);
    assert(actual.stable.journal.members@ =~= seq![None, None, None]);
    assert(actual.stable.journal.member_free@ =~= seq![2usize, 1, 0]);
    assert(actual.stable.journal.scratch@ == before.stable.journal.scratch@);
    assert(logical::producer_invariant_v1(model));
    let stale = actual.stable.journal.lookup_allocation(roster[0].allocation);
    assert(stale == Err(ReadErrorV1::InvalidAllocationReference));
    true
}

#[verifier::spinoff_prover]
fn owner_disposal_rejection_witness_v1(case: u8) -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, writer, model_writer) = owner_settlement_pending_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    let (roster, model_roster) = if case == 0 {
        let roster: Vec<AllocationWriteV1> = vec![];
        let model_roster: Vec<logical::AllocationWriteV1> = vec![];
        proof { assert(begin_roster_view(roster@) =~= model_roster@); }
        (roster, model_roster)
    } else { owner_disposal_roster_fixture_v1() };
    proof {
        reveal_with_fuel(owner_retained_scan_v1, 3);
        reveal_with_fuel(settlement_scratch_scan_v1, 3);
        reveal_with_fuel(disposal_roster_scan_v1, 3);
        reveal_with_fuel(disposal_producer_safe_v1, 3);
        reveal_with_fuel(producer_unread_writes_v1, 3);
        reveal_with_fuel(stable_unread_writes_v1, 3);
    }
    let marked = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(marked.0 == Ok(()));
    let evidence = if case == 1 {
        WriterReferenceV1 { slot: writer.slot, key: WriterKeyV1 { kind: WriterKindV1::Synchronous, ..writer.key } }
    } else { writer };
    let model_evidence = if case == 1 {
        logical::WriterReferenceV1 { slot: model_writer.slot,
            key: logical::WriterKeyV1 { kind: logical::WriterKindV1::Synchronous, ..model_writer.key } }
    } else { model_writer };
    let ghost before = actual;
    let ghost model_before = model;
    let rejected = owner_disposal_paired_exec_v1(&mut actual, &mut model, writer, model_writer, evidence, model_evidence,
        &roster, &model_roster, 1, 3, if case < 2 { 3 } else { 1 }, Ghost(storage), Ghost(history));
    assert(rejected.0 == Err(if case < 2 { ReadErrorV1::SettlementEvidenceMismatch } else { ReadErrorV1::InvalidState }));
    assert(actual == before && model == model_before);
    true
}

#[verifier::spinoff_prover]
fn owner_disposal_busy_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, writer, model_writer) = owner_settlement_producer_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    let (roster, model_roster) = owner_disposal_roster_fixture_v1();
    proof { reveal_with_fuel(owner_retained_scan_v1, 3); }
    let marked = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(marked.0 == Ok(()));
    let ghost before = actual;
    let ghost model_before = model;
    let disposed = owner_disposal_paired_exec_v1(&mut actual, &mut model, writer, model_writer, writer, model_writer,
        &roster, &model_roster, 1, 3, 3, Ghost(storage), Ghost(history));
    assert(disposed.0 == Err(ReadErrorV1::AllocationBusy));
    assert(actual == before && model == model_before);
    let status = actual.producer_read_status(actual.reservations[0].unwrap().reference);
    assert(status == Ok(ContextProducerReadStatusV1::Unknown));
    true
}

#[verifier::spinoff_prover]
fn owner_disposal_raw_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, writer, model_writer) = owner_settlement_pending_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    let (roster, model_roster) = owner_disposal_roster_fixture_v1();
    proof {
        reveal_with_fuel(owner_retained_scan_v1, 3);
        reveal_with_fuel(disposal_roster_scan_v1, 3);
        reveal_with_fuel(settlement_scratch_scan_v1, 3);
        reveal_with_fuel(retirement_prefix_v1, 3);
        reveal_with_fuel(disposal_producer_safe_v1, 3);
        reveal_with_fuel(producer_unread_writes_v1, 3);
        reveal_with_fuel(stable_unread_writes_v1, 3);
    }
    let marked = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(marked.0 == Ok(()));
    actual.stable.journal.reserved_count = usize::MAX;
    model.stable.journal.reserved_count = usize::MAX;
    actual.stable.journal.registration_watermark = 0;
    model.stable.journal.registration_watermark = 0;
    actual.stable.readers = vec![0usize];
    model.stable.readers = vec![0usize];
    actual.counts = vec![0usize];
    model.counts = vec![0usize];
    let plan = BeginMemberPlanV1 { member_slot: usize::MAX, allocation: roster[0].allocation,
        prior_lineage: u64::MAX, attempt_epoch: 0 };
    let model_plan = logical::BeginMemberPlanV1 { member_slot: usize::MAX, allocation: model_roster[0].allocation,
        prior_lineage: u64::MAX, attempt_epoch: 0 };
    actual.stable.journal.scratch.set(2, Some(plan));
    model.stable.journal.scratch.set(2, Some(model_plan));
    proof {
        assert(actual.stable.journal.scratch@.map(|_i, item| begin_plan_slot_view(item)) =~= model.stable.journal.scratch@);
        assert(actual.stable.readers@ =~= model.stable.readers@);
        assert(actual.counts@ =~= model.counts@);
        assert(producer_represents(actual, model));
        assert(!logical::producer_invariant_v1(model));
    }
    let ghost before = actual;
    let disposed = owner_disposal_paired_exec_v1(&mut actual, &mut model, writer, model_writer, writer, model_writer,
        &roster, &model_roster, 1, 3, 3, Ghost(storage), Ghost(history));
    assert(disposed.0 == Ok(()));
    assert(actual.stable.journal.allocations@[0].is_none());
    assert(actual.stable.journal.reserved_count == usize::MAX);
    assert(actual.stable.journal.registration_watermark == 0);
    assert(actual.stable.journal.scratch@ == before.stable.journal.scratch@);
    true
}

#[verifier::spinoff_prover]
fn owner_disposal_unreached_count_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, writer, model_writer) = owner_settlement_pending_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    actual.stable.readers = vec![];
    model.stable.readers = vec![];
    actual.counts = vec![];
    model.counts = vec![];
    let roster = vec![AllocationWriteV1 {
        allocation: AllocationReferenceV1 { slot: usize::MAX, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    let model_roster = vec![logical::AllocationWriteV1 {
        allocation: logical::AllocationReferenceV1 { slot: usize::MAX, key: logical::AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    proof {
        assert(actual.stable.readers@ =~= model.stable.readers@);
        assert(actual.counts@ =~= model.counts@);
        assert(producer_represents(actual, model));
        assert(begin_roster_view(roster@) =~= model_roster@);
        assert(!logical::producer_invariant_v1(model));
    }
    let ghost before = actual;
    let ghost model_before = model;
    let rejected = owner_disposal_paired_exec_v1(&mut actual, &mut model, writer, model_writer, writer, model_writer,
        &roster, &model_roster, 0, 0, 0, Ghost(storage), Ghost(history));
    assert(rejected.0 == Err(ReadErrorV1::InvalidAllocationReference));
    assert(actual == before && model == model_before);
    true
}

spec fn owner_disposal_resolved_writer_v1(local: u64) -> WriterReferenceV1 {
    WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local, kind: WriterKindV1::Submission } }
}

spec fn owner_disposal_resolved_model_writer_v1(local: u64) -> logical::WriterReferenceV1 {
    logical::WriterReferenceV1 { slot: 0,
        key: logical::WriterKeyV1 { context_generation: 7, local, kind: logical::WriterKindV1::Submission } }
}

// These fixtures expose concrete transition inputs, not accumulated lifecycle relations.
spec fn owner_disposal_resolved_custody_v1(actual: ContextProducerReadJournalV1,
    model: logical::ProducerReadContentsV1, success: bool, registered: bool) -> bool
{
    &&& producer_represents(actual, model)
    &&& logical::issued_producer_v1(model, logical::witness_storage_v1(3, 1),
        if registered { seq![owner_disposal_resolved_model_writer_v1(10), owner_disposal_resolved_model_writer_v1(11)] }
        else { seq![owner_disposal_resolved_model_writer_v1(10)] })
    &&& actual.stable.journal.context_generation == 7
    &&& actual.stable.journal.allocation_capacity == 3 && actual.stable.journal.writer_capacity == 1
    &&& actual.stable.journal.allocations@.len() == 3
    &&& actual.stable.journal.allocations@[0] == Some(AllocationEntryV1 {
        key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        attempt_epoch: 1, content_lineage: if success { 1u64 } else { 0u64 }, pending_member: None })
    &&& actual.stable.journal.allocations@[1] == Some(AllocationEntryV1 {
        key: AllocationKeyV1 { context_generation: 7, local: 30 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32,
        attempt_epoch: 0, content_lineage: 0, pending_member: None })
    &&& actual.stable.journal.scratch@ == seq![None, None, None]
    &&& actual.counts@ == seq![1usize, 0, 0] && actual.stable.readers@ == seq![0usize, 1, 0]
    &&& actual.reservations@.len() == 2 && actual.reservations@[0].is_some() && actual.reservations@[1].is_none()
    &&& actual.reservations@[0].unwrap().request.producer == owner_disposal_resolved_writer_v1(10)
    &&& actual.reservations@[0].unwrap().request.read.allocation.slot == 0
    &&& producer_query_decision_v1(actual, actual.reservations@[0].unwrap().reference)
        == Ok(if success { ContextProducerReadStatusV1::Success } else { ContextProducerReadStatusV1::NoEffect })
    &&& producer_lookup_decision_v1(actual, actual.reservations@[0].unwrap().reference)
        == Ok(actual.reservations@[0].unwrap().request)
    &&& actual.stable.leases@.len() == 2 && actual.stable.leases@[0].is_some() && actual.stable.leases@[1].is_none()
    &&& actual.stable.leases@[0].unwrap().request.allocation.slot == 1
    &&& stable_lease_decision_v1(actual.stable, actual.stable.leases@[0].unwrap().reference)
        == Ok(actual.stable.leases@[0].unwrap().request)
    &&& actual.free@ == seq![1usize] && actual.stable.free_reads@ == seq![1usize]
    &&& actual.next_incarnation == 2 && actual.stable.next_incarnation == 2
}

#[verifier::spinoff_prover]
fn owner_disposal_resolved_settled_fixture_v1(success: bool)
    -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1))
    ensures owner_disposal_resolved_custody_v1(result.0, result.1, success, false),
        result.0.stable.journal.registration_watermark == 10, result.0.stable.journal.reserved_count == 0,
        result.0.stable.journal.writers@ == seq![None], result.0.stable.journal.free@ == seq![0usize],
        result.0.stable.journal.allocations@[2].is_none(), result.0.stable.journal.allocation_free@ == seq![2usize],
        result.0.stable.journal.members@ == seq![None, None, None], result.0.stable.journal.member_free@ == seq![2usize, 1, 0],
{
    let (mut actual, mut model, writer, model_writer) = owner_settlement_read_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    proof {
        reveal_with_fuel(owner_retained_scan_v1, 3);
        reveal_with_fuel(settlement_scratch_scan_v1, 3);
        reveal_with_fuel(settlement_cursor_v1, 3);
        reveal_with_fuel(settlement_allocations_prefix_v1, 3);
        reveal_with_fuel(settlement_members_prefix_v1, 3);
    }
    let settled = owner_settlement_historical_exec_v1(&mut actual, &mut model, writer, model_writer,
        writer, model_writer, 1, 3, success, Ghost(storage), Ghost(history));
    assert(settled.0 == Ok(()));
    let reference = actual.reservations[0].unwrap().reference;
    let request = actual.reservations[0].unwrap().request;
    let lease = actual.stable.leases[0].unwrap().reference;
    let read = actual.stable.leases[0].unwrap().request;
    let status = actual.producer_read_status(reference);
    let lookup = actual.lookup_producer_read(reference);
    let stable_lookup = actual.stable.lookup_read(lease);
    assert(status == Ok(if success { ContextProducerReadStatusV1::Success } else { ContextProducerReadStatusV1::NoEffect }));
    assert(lookup == Ok(request) && stable_lookup == Ok(read));
    (actual, model)
}

#[verifier::spinoff_prover]
fn owner_disposal_resolved_registered_fixture_v1(success: bool)
    -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1))
    ensures owner_disposal_resolved_custody_v1(result.0, result.1, success, true),
        result.0.stable.journal.registration_watermark == 11, result.0.stable.journal.reserved_count == 1,
        result.0.stable.journal.writers@ == seq![Some(WriterEntryV1::Reserved(owner_disposal_resolved_writer_v1(11).key))],
        result.0.stable.journal.free@ == seq![], result.0.stable.journal.allocation_free@ == seq![],
        result.0.stable.journal.members@ == seq![None, None, None], result.0.stable.journal.member_free@ == seq![2usize, 1, 0],
        result.0.stable.journal.allocations@[2] == Some(AllocationEntryV1 {
            key: AllocationKeyV1 { context_generation: 7, local: 50 },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32,
            attempt_epoch: 0, content_lineage: 0, pending_member: None }),
{
    let (mut actual, mut model) = owner_disposal_resolved_settled_fixture_v1(success);
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![owner_disposal_resolved_model_writer_v1(10)];
    let entry = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 50 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let model_entry = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 50 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let enrolled = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, entry, model_entry, Ghost(storage), Ghost(history));
    assert(enrolled.0 == Ok(AllocationReferenceV1 { slot: 2, key: entry.key }));
    let key = WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission };
    let model_key = logical::WriterKeyV1 { context_generation: 7, local: 11, kind: logical::WriterKindV1::Submission };
    let registered = owner_register_historical_exec_v1(&mut actual, &mut model, key, model_key, Ghost(storage), Ghost(history));
    assert(registered.0 == Ok(WriterReferenceV1 { slot: 0, key }));
    (actual, model)
}

#[verifier::spinoff_prover]
fn owner_disposal_resolved_unknown_fixture_v1(success: bool)
    -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1))
    ensures owner_disposal_resolved_custody_v1(result.0, result.1, success, true),
        result.0.stable.journal.registration_watermark == 11, result.0.stable.journal.reserved_count == 0,
        result.0.stable.journal.writers@ == seq![Some(WriterEntryV1::Unknown {
            key: owner_disposal_resolved_writer_v1(11).key, head: Some(0usize), count: 1usize })],
        result.0.stable.journal.free@ == seq![], result.0.stable.journal.allocation_free@ == seq![],
        result.0.stable.journal.members@ == seq![Some(MemberEntryV1 {
            writer: owner_disposal_resolved_writer_v1(11),
            allocation: AllocationReferenceV1 { slot: 2, key: AllocationKeyV1 { context_generation: 7, local: 50 } },
            prior_lineage: 0, attempt_epoch: 1, next: None }), None, None],
        result.0.stable.journal.member_free@ == seq![2usize, 1],
        result.0.stable.journal.allocations@[2] == Some(AllocationEntryV1 {
            key: AllocationKeyV1 { context_generation: 7, local: 50 },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32,
            attempt_epoch: 1, content_lineage: 0, pending_member: Some(0usize) }),
{
    let (mut actual, mut model) = owner_disposal_resolved_registered_fixture_v1(success);
    let ghost before = actual;
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![owner_disposal_resolved_model_writer_v1(10), owner_disposal_resolved_model_writer_v1(11)];
    let writer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission } };
    let model_writer = logical::WriterReferenceV1 { slot: 0,
        key: logical::WriterKeyV1 { context_generation: 7, local: 11, kind: logical::WriterKindV1::Submission } };
    let roster = vec![AllocationWriteV1 {
        allocation: AllocationReferenceV1 { slot: 2, key: AllocationKeyV1 { context_generation: 7, local: 50 } },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 }];
    let model_roster = vec![logical::AllocationWriteV1 {
        allocation: logical::AllocationReferenceV1 { slot: 2, key: logical::AllocationKeyV1 { context_generation: 7, local: 50 } },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 }];
    proof {
        assert(begin_roster_view(roster@) =~= model_roster@);
        reveal_with_fuel(begin_members_prefix_v1, 3);
        reveal_with_fuel(begin_allocations_prefix_v1, 3);
        reveal_with_fuel(producer_unread_writes_v1, 3);
        reveal_with_fuel(stable_unread_writes_v1, 3);
        reveal_with_fuel(begin_canonical_scan_v1, 3);
        reveal_with_fuel(begin_destination_scan_v1, 3);
        reveal_with_fuel(begin_slot_scan_v1, 3);
        reveal_with_fuel(owner_retained_scan_v1, 3);
    }
    let begun = producer_begin_historical_exec_v1(&mut actual, &mut model, writer, model_writer,
        &roster, &model_roster, Ghost(storage), Ghost(history));
    assert(begun.0 == Ok(()));
    let ghost pending = model;
    let marked = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(marked.0 == Ok(()));
    proof {
        let pre = logical::issuance_contents_projection_v1(pending.stable.journal, storage, history);
        let post = logical::issuance_contents_projection_v1(model.stable.journal, storage, history);
        logical::issuance::prefix_update_v1(pre.writers, 0, post.writers[0], 1);
        assert(logical::issuance::issuance_invariant_v1(post));
        assert(logical::issued_producer_v1(model, storage, history));
    }
    assert(actual.stable.journal.allocations@[0] == before.stable.journal.allocations@[0]);
    assert(actual.stable.journal.allocations@[1] == before.stable.journal.allocations@[1]);
    assert(actual.stable.journal.scratch@ == before.stable.journal.scratch@);
    assert(actual.reservations == before.reservations && actual.stable.leases == before.stable.leases);
    let reference = actual.reservations[0].unwrap().reference;
    let request = actual.reservations[0].unwrap().request;
    let lease = actual.stable.leases[0].unwrap().reference;
    let read = actual.stable.leases[0].unwrap().request;
    let status = actual.producer_read_status(reference);
    let lookup = actual.lookup_producer_read(reference);
    let stable_lookup = actual.stable.lookup_read(lease);
    assert(status == Ok(if success { ContextProducerReadStatusV1::Success } else { ContextProducerReadStatusV1::NoEffect }));
    assert(lookup == Ok(request) && stable_lookup == Ok(read));
    (actual, model)
}

#[verifier::spinoff_prover]
fn owner_disposal_resolved_reuse_witness_v1(success: bool) -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_disposal_resolved_unknown_fixture_v1(success);
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![owner_disposal_resolved_model_writer_v1(10), owner_disposal_resolved_model_writer_v1(11)];
    let reference = actual.reservations[0].unwrap().reference;
    let model_reference = model.reservations[0].unwrap().reference;
    let request = actual.reservations[0].unwrap().request;
    let model_request = model.reservations[0].unwrap().request;
    let lease = actual.stable.leases[0].unwrap().reference;
    let model_lease = model.stable.leases[0].unwrap().reference;
    let read = actual.stable.leases[0].unwrap().request;
    let model_read = model.stable.leases[0].unwrap().request;
    let writer = WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 11, kind: WriterKindV1::Submission } };
    let model_writer = logical::WriterReferenceV1 { slot: 0,
        key: logical::WriterKeyV1 { context_generation: 7, local: 11, kind: logical::WriterKindV1::Submission } };
    assert(writer.slot == request.producer.slot && writer.key != request.producer.key);
    let roster = vec![AllocationWriteV1 {
        allocation: AllocationReferenceV1 { slot: 2, key: AllocationKeyV1 { context_generation: 7, local: 50 } },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 }];
    let model_roster = vec![logical::AllocationWriteV1 {
        allocation: logical::AllocationReferenceV1 { slot: 2, key: logical::AllocationKeyV1 { context_generation: 7, local: 50 } },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 }];
    proof {
        assert(begin_roster_view(roster@) =~= model_roster@);
        reveal_with_fuel(owner_retained_scan_v1, 3);
        reveal_with_fuel(disposal_roster_scan_v1, 3);
        reveal_with_fuel(settlement_scratch_scan_v1, 3);
        reveal_with_fuel(retirement_prefix_v1, 3);
        reveal_with_fuel(producer_unread_writes_v1, 3);
        reveal_with_fuel(stable_unread_writes_v1, 3);
        owner_disposal_invariant_domain_v1(actual, model, roster@);
    }
    let ghost before = actual;
    let disposed = owner_disposal_paired_exec_v1(&mut actual, &mut model, writer, model_writer, writer, model_writer,
        &roster, &model_roster, 1, 3, 3, Ghost(storage), Ghost(history));
    assert(disposed.0 == Ok(()));
    assert(actual.stable.journal.writers@ =~= seq![None]);
    assert(actual.stable.journal.allocations@[2].is_none());
    assert(actual.reservations == before.reservations && actual.stable.leases == before.stable.leases);
    assert(logical::producer_invariant_v1(model));
    let status = producer_query_historical_exec_v1(&actual, &model, reference, model_reference);
    let lookup = producer_lookup_historical_exec_v1(&actual, &model, reference, model_reference);
    let stable_lookup = stable_lookup_historical_exec_v1(&actual.stable, &model.stable, lease, model_lease);
    assert(status.0 == Ok(if success { ContextProducerReadStatusV1::Success } else { ContextProducerReadStatusV1::NoEffect }));
    assert(status.1 == Ok(if success { logical::ProducerStatusV1::Success } else { logical::ProducerStatusV1::NoEffect }));
    assert(lookup.0 == Ok(request) && lookup.1 == Ok(model_request));
    assert(stable_lookup.0 == Ok(read) && stable_lookup.1 == Ok(model_read));
    true
}

#[verifier::spinoff_prover]
fn owner_disposal_empty_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_settlement_empty_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    proof { logical::issued_constructor_v1(model.stable.journal, 7, 3, 1, storage); }
    let key = WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission };
    let model_key = logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission };
    let registered = owner_register_historical_exec_v1(&mut actual, &mut model, key, model_key, Ghost(storage), Ghost(history));
    assert(registered.0.is_ok() && registered.1.is_ok());
    let writer = registered.0.unwrap();
    let model_writer = match registered.1 { Ok(value) => value, Err(_) => return false };
    let ghost history = logical::registration_history_v1(history, registered.1);
    let roster: Vec<AllocationWriteV1> = vec![];
    let model_roster: Vec<logical::AllocationWriteV1> = vec![];
    proof {
        writer_reference_round_trip(writer, model_writer);
        assert(begin_roster_view(roster@) =~= model_roster@);
    }
    let begun = producer_begin_historical_exec_v1(&mut actual, &mut model, writer, model_writer,
        &roster, &model_roster, Ghost(storage), Ghost(history));
    assert(begun.0 == Ok(()));
    let marked = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(marked.0 == Ok(()));
    actual.stable.journal.scratch = vec![];
    model.stable.journal.scratch = vec![];
    proof { assert(actual.stable.journal.scratch@.map(|_i, item| begin_plan_slot_view(item)) =~= model.stable.journal.scratch@); }
    let ghost before = actual;
    let disposed = owner_disposal_paired_exec_v1(&mut actual, &mut model, writer, model_writer, writer, model_writer,
        &roster, &model_roster, 1, 3, 3, Ghost(storage), Ghost(history));
    assert(disposed.0 == Ok(()));
    assert(actual.stable.journal.writers@ =~= seq![None]);
    assert(actual.stable.journal.free@ =~= seq![0usize]);
    assert(actual.stable.journal.members@ == before.stable.journal.members@);
    assert(actual.stable.journal.allocations@ == before.stable.journal.allocations@);
    assert(actual.stable.journal.member_free@ == before.stable.journal.member_free@);
    assert(actual.stable.journal.allocation_free@ == before.stable.journal.allocation_free@);
    assert(actual.stable.journal.scratch@.len() == 0);
    assert(actual.stable.journal.registration_watermark == 10);
    true
}

}
