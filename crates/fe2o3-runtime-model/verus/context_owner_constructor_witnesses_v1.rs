verus! {

proof fn owner_constructor_first_failure_v1(before: ConstructorObservationsV1, after: ConstructorObservationsV1,
    requests: Seq<(u8, usize)>, failed: nat, success: bool)
    requires constructor_trace_v1(before, after, requests, success), failed < requests.len(),
        forall|i: int| 0 <= i < failed ==> #[trigger] constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + i) as nat),
        !constructor_outcome_v1(before.outcomes@, before.attempts@.len() + failed),
    ensures !success, after.attempts@.len() == before.attempts@.len() + failed + 1,
{
    let count = (after.attempts@.len() - before.attempts@.len()) as nat;
    if count <= failed {
        assert(!success && count > 0);
        let last = count - 1;
        assert(0 <= last < failed);
        assert(constructor_outcome_v1(before.outcomes@, (before.attempts@.len() + last) as nat));
        assert(!constructor_outcome_v1(before.outcomes@, (after.attempts@.len() - 1) as nat));
        assert(false);
    }
    if count > failed + 1 || success {
        assert(failed < count && (failed + 1 < count || success));
        assert(constructor_outcome_v1(before.outcomes@, before.attempts@.len() + failed));
        assert(false);
    }
}

#[verifier::spinoff_prover]
fn owner_constructor_success_fixture_v1(prefixed: bool)
    -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1))
    ensures producer_represents(result.0, result.1),
        constructor_producer_initialized_v1(result.0, 7, 3, 1, 2),
        logical::producer_constructor_relation_v1(7, 3, 1, 2, Ok(result.1)),
        logical::issued_producer_v1(result.1, logical::witness_storage_v1(3, 1), Seq::empty()),
{
    let mut actual_observations = ConstructorObservationsV1 {
        outcomes: vec![true, true, true, true, true, true, true, true, true, true, true, true, true],
        attempts: vec![],
    };
    let mut model_observations = logical::ConstructorObservationsV1 {
        outcomes: vec![true, true, true, true, true, true, true, true, true, true, true, true, true],
        attempts: vec![],
    };
    if prefixed {
        actual_observations.outcomes.push(true);
        model_observations.outcomes.push(true);
        actual_observations.attempts.push((99u8, 42usize));
        model_observations.attempts.push((99u8, 42usize));
    }
    let ghost before = actual_observations;
    let ghost storage = logical::witness_storage_v1(3, 1);
    let constructed = constructor_producer_paired_exec_v1(&mut actual_observations, &mut model_observations,
        7, 3, 1, 2, Ghost(storage));
    assert(constructed.0.is_ok() && constructed.1.is_ok());
    assert(actual_observations.attempts@ == before.attempts@ + constructor_requests_v1(3, 1, 2));
    assert(actual_observations.attempts@.len() == if prefixed { 14nat } else { 13nat });
    assert(actual_observations.outcomes@ == before.outcomes@);
    assert(actual_observations.attempts@ == model_observations.attempts@);
    let actual = constructed.0.unwrap();
    let model = match constructed.1 { Ok(value) => value, Err(_) => { assert(false); unreached() } };
    (actual, model)
}

#[verifier::spinoff_prover]
fn owner_constructor_failure_witness_v1(failed: u8) -> (result: bool)
    requires failed < 13,
    ensures result,
{
    let mut actual = ConstructorObservationsV1 {
        outcomes: vec![true, true, true, true, true, true, true, true, true, true, true, true, true], attempts: vec![],
    };
    let mut model = logical::ConstructorObservationsV1 {
        outcomes: vec![true, true, true, true, true, true, true, true, true, true, true, true, true], attempts: vec![],
    };
    actual.outcomes.set(failed as usize, false);
    model.outcomes.set(failed as usize, false);
    let ghost before = actual;
    let constructed = constructor_producer_paired_exec_v1(&mut actual, &mut model, 7, 3, 1, 2,
        Ghost(logical::witness_storage_v1(3, 1)));
    proof {
        assert forall|i: int| 0 <= i < failed implies #[trigger] constructor_outcome_v1(before.outcomes@, i as nat) by {};
        owner_constructor_first_failure_v1(before, actual, constructor_requests_v1(3, 1, 2), failed as nat, constructed.0.is_ok());
    }
    assert(constructed.0 == Err(ReadErrorV1::StorageAllocationFailed));
    assert(constructed.1 == Err(logical::ConstructorErrorV1::StorageAllocationFailed));
    assert(actual.attempts@.len() == failed + 1);
    assert(actual.attempts@ == constructor_requests_v1(3, 1, 2).take(failed as int + 1));
    assert(actual.attempts@ == model.attempts@);
    true
}

#[verifier::spinoff_prover]
fn owner_constructor_exhausted_witness_v1(empty: bool) -> (result: bool)
    ensures result,
{
    let mut actual = ConstructorObservationsV1 {
        outcomes: if empty { vec![] } else { vec![true, true, true, true] }, attempts: vec![],
    };
    let mut model = logical::ConstructorObservationsV1 {
        outcomes: if empty { vec![] } else { vec![true, true, true, true] }, attempts: vec![],
    };
    let ghost before = actual;
    let constructed = constructor_producer_paired_exec_v1(&mut actual, &mut model, 7, 3, 1, 2,
        Ghost(logical::witness_storage_v1(3, 1)));
    proof {
        let failed = if empty { 0nat } else { 4nat };
        assert forall|i: int| 0 <= i < failed implies #[trigger] constructor_outcome_v1(before.outcomes@, i as nat) by {};
        owner_constructor_first_failure_v1(before, actual, constructor_requests_v1(3, 1, 2), failed, constructed.0.is_ok());
    }
    assert(constructed.0 == Err(ReadErrorV1::StorageAllocationFailed));
    assert(constructed.1 == Err(logical::ConstructorErrorV1::StorageAllocationFailed));
    assert(actual.attempts@.len() == if empty { 1nat } else { 5nat });
    assert(actual.attempts@ == constructor_requests_v1(3, 1, 2).take(if empty { 1 } else { 5 }));
    assert(actual.attempts@ == model.attempts@);
    true
}

#[verifier::spinoff_prover]
fn owner_constructor_validation_witness_v1(bad_reads: bool) -> (result: bool)
    ensures result,
{
    let mut actual = ConstructorObservationsV1 { outcomes: vec![], attempts: vec![(99u8, 42usize)] };
    let mut model = logical::ConstructorObservationsV1 { outcomes: vec![], attempts: vec![(99u8, 42usize)] };
    let ghost before = actual;
    let ghost model_before = model;
    let journal = constructor_journal_paired_exec_v1(&mut actual, &mut model, 0, 0, 0);
    assert(journal.0 == Err(ReadErrorV1::InvalidContextGeneration));
    assert(journal.1 == Err(logical::ConstructorErrorV1::InvalidContextGeneration));
    let stable = constructor_stable_paired_exec_v1(&mut actual, &mut model, 0, 0, 0, if bad_reads { 0 } else { 2 });
    assert(stable.0 == Err(if bad_reads { ReadErrorV1::InvalidCapacity } else { ReadErrorV1::InvalidContextGeneration }));
    assert(stable.1 == Err(if bad_reads { logical::ConstructorErrorV1::InvalidCapacity } else { logical::ConstructorErrorV1::InvalidContextGeneration }));
    let producer = constructor_producer_paired_exec_v1(&mut actual, &mut model, 0, 0, 0, if bad_reads { 0 } else { 2 },
        Ghost(logical::witness_storage_v1(3, 1)));
    assert(producer.0 == Err(if bad_reads { ReadErrorV1::InvalidCapacity } else { ReadErrorV1::InvalidContextGeneration }));
    assert(producer.1 == Err(if bad_reads { logical::ConstructorErrorV1::InvalidCapacity } else { logical::ConstructorErrorV1::InvalidContextGeneration }));
    assert(actual.outcomes@ == before.outcomes@ && actual.attempts@ == before.attempts@);
    assert(model.outcomes@ == model_before.outcomes@ && model.attempts@ == model_before.attempts@);
    true
}

// Each lifecycle fixture calls the constructor-derived predecessor, never a synthetic owner fixture.
#[verifier::spinoff_prover]
fn owner_constructor_enrolled_fixture_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1))
    ensures producer_represents(result.0, result.1),
        logical::issued_producer_v1(result.1, logical::witness_storage_v1(3, 1), Seq::empty()),
        result.0.stable.journal.context_generation == 7,
        result.0.stable.journal.allocation_capacity == 3, result.0.stable.journal.writer_capacity == 1,
        result.0.stable.journal.registration_watermark == 0, result.0.stable.journal.reserved_count == 0,
        result.0.stable.journal.writers@ == seq![None], result.0.stable.journal.free@ == seq![0usize],
        result.0.stable.journal.allocation_free@ == seq![2usize],
        result.0.stable.journal.members@ == seq![None, None, None], result.0.stable.journal.member_free@ == seq![2usize, 1, 0],
        result.0.stable.journal.scratch@ == seq![None, None, None],
        result.0.stable.journal.allocations@ == seq![
            Some(AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
                device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 },
                byte_extent: 16u64, attempt_epoch: 0u64, content_lineage: 0u64, pending_member: None }),
            Some(AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
                device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 },
                byte_extent: 32u64, attempt_epoch: 0u64, content_lineage: 0u64, pending_member: None }), None],
        result.0.counts@ == seq![0usize, 0, 0], result.0.reservations@ == seq![None, None],
        result.0.free@ == seq![1usize, 0], result.0.next_incarnation == 1,
        result.0.stable.readers@ == seq![0usize, 0, 0], result.0.stable.leases@ == seq![None, None],
        result.0.stable.free_reads@ == seq![1usize, 0], result.0.stable.next_incarnation == 1,
{
    let (mut actual, mut model) = owner_constructor_success_fixture_v1(false);
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    let first = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 };
    let model_first = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 20 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 };
    let enrolled = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, first, model_first, Ghost(storage), Ghost(history));
    assert(enrolled.0 == Ok(AllocationReferenceV1 { slot: 0, key: first.key }));
    let second = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 };
    let model_second = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 30 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 };
    let enrolled = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, second, model_second, Ghost(storage), Ghost(history));
    assert(enrolled.0 == Ok(AllocationReferenceV1 { slot: 1, key: second.key }));
    proof {
        assert(actual.stable.journal.writers@ =~= seq![None]);
        assert(actual.stable.journal.free@ =~= seq![0usize]);
        assert(actual.stable.journal.allocations@ =~= seq![Some(enrollment_value_v1(first)), Some(enrollment_value_v1(second)), None]);
        assert(actual.stable.journal.allocation_free@ =~= seq![2usize]);
        assert(actual.stable.journal.members@ =~= seq![None, None, None]);
        assert(actual.stable.journal.member_free@ =~= seq![2usize, 1, 0]);
        assert(actual.stable.journal.scratch@ =~= seq![None, None, None]);
        assert(actual.counts@ =~= seq![0usize, 0, 0]);
        assert(actual.reservations@ =~= seq![None, None]);
        assert(actual.free@ =~= seq![1usize, 0]);
        assert(actual.stable.readers@ =~= seq![0usize, 0, 0]);
        assert(actual.stable.leases@ =~= seq![None, None]);
        assert(actual.stable.free_reads@ =~= seq![1usize, 0]);
    }
    (actual, model)
}

#[verifier::spinoff_prover]
fn owner_constructor_pending_fixture_v1() -> (result: (ContextProducerReadJournalV1,
    logical::ProducerReadContentsV1, WriterReferenceV1, logical::WriterReferenceV1))
    ensures producer_represents(result.0, result.1), writer_reference_view(result.2) == result.3,
        owner_settlement_pending_fixture_contents_v1(result.0, result.2),
        logical::issued_producer_v1(result.1, logical::witness_storage_v1(3, 1), seq![result.3]),
        result.0.counts@ == seq![0usize, 0, 0], result.0.reservations@ == seq![None, None],
        result.0.free@ == seq![1usize, 0], result.0.next_incarnation == 1,
        result.0.stable.readers@ == seq![0usize, 0, 0], result.0.stable.leases@ == seq![None, None],
        result.0.stable.free_reads@ == seq![1usize, 0], result.0.stable.next_incarnation == 1,
{
    let (mut actual, mut model) = owner_constructor_enrolled_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    let key = WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission };
    let model_key = logical::WriterKeyV1 { context_generation: 7, local: 10, kind: logical::WriterKindV1::Submission };
    let registered = owner_register_historical_exec_v1(&mut actual, &mut model, key, model_key, Ghost(storage), Ghost(history));
    assert(registered.0 == Ok(WriterReferenceV1 { slot: 0, key }));
    let writer = registered.0.unwrap();
    let model_writer = match registered.1 { Ok(value) => value, Err(_) => { assert(false); unreached() } };
    let ghost history = seq![model_writer];
    let (roster, model_roster) = owner_disposal_roster_fixture_v1();
    proof {
        writer_reference_round_trip(writer, model_writer);
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
    assert(actual.stable.journal.writers@ =~= seq![Some(WriterEntryV1::Pending { key, head: Some(0usize), count: 1usize })]);
    assert(actual.stable.journal.free@ =~= seq![]);
    assert(actual.stable.journal.member_free@ =~= seq![2usize, 1]);
    assert(actual.stable.journal.members@ =~= seq![Some(MemberEntryV1 {
        writer, allocation: roster@[0].allocation, prior_lineage: 0u64, attempt_epoch: 1u64, next: None }), None, None]);
    assert(actual.stable.journal.allocations@ =~= seq![Some(AllocationEntryV1 {
        key: roster@[0].allocation.key, device: roster@[0].device, byte_extent: 16u64,
        attempt_epoch: 1u64, content_lineage: 0u64, pending_member: Some(0usize) }),
        Some(AllocationEntryV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 },
            byte_extent: 32u64, attempt_epoch: 0u64, content_lineage: 0u64, pending_member: None }), None]);
    (actual, model, writer, model_writer)
}

#[verifier::spinoff_prover]
fn owner_constructor_acquired_fixture_v1() -> (result: (ContextProducerReadJournalV1,
    logical::ProducerReadContentsV1, WriterReferenceV1, logical::WriterReferenceV1))
    ensures producer_represents(result.0, result.1), writer_reference_view(result.2) == result.3,
        owner_settlement_pending_fixture_contents_v1(result.0, result.2),
        owner_settlement_producer_fixture_contents_v1(result.0, result.2),
        logical::issued_producer_v1(result.1, logical::witness_storage_v1(3, 1), seq![result.3]),
        result.0.stable.readers@ == seq![0usize, 0, 0], result.0.stable.leases@ == seq![None, None],
        result.0.stable.free_reads@ == seq![1usize, 0], result.0.stable.next_incarnation == 1,
{
    let (mut actual, mut model, writer, model_writer) = owner_constructor_pending_fixture_v1();
    let consumer = WriterKeyV1 { context_generation: 7, local: 40, kind: WriterKindV1::Submission };
    let model_consumer = logical::WriterKeyV1 { context_generation: 7, local: 40, kind: logical::WriterKindV1::Submission };
    let request = ContextProducerReadV1 { producer: writer, read: ContextAllocationReadV1 {
        allocation: AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0 } };
    let model_request = logical::ProducerReadV1 { producer: model_writer, read: logical::AllocationReadV1 {
        allocation: logical::AllocationReferenceV1 { slot: 0, key: logical::AllocationKeyV1 { context_generation: 7, local: 20 } },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
        byte_offset: 0, byte_len: 8, attempt_epoch: 1, content_lineage: 0 } };
    let requests = vec![request];
    let model_requests = vec![model_request];
    let mut reserved = vec![None];
    let mut model_reserved = vec![None];
    proof {
        assert(producer_requests_view(requests@) =~= model_requests@);
        assert(producer_output_view(reserved@) =~= model_reserved@);
        reveal_with_fuel(producer_acquire_scan_v1, 3);
        reveal_with_fuel(producer_acquired_reservations_v1, 3);
        reveal_with_fuel(producer_request_count_v1, 3);
    }
    let acquired = producer_acquire_historical_exec_v1(&mut actual, &mut model, consumer, model_consumer,
        &requests, &model_requests, &mut reserved, &mut model_reserved);
    assert(acquired.0 == Ok(()) && acquired.1 == Ok(()));
    assert(reserved@[0] == Some(ContextProducerReadReferenceV1 { slot: 0, incarnation: 1, consumer }));
    assert(actual.reservations@ =~= seq![Some(ReservationV1 { reference: reserved@[0].unwrap(), request }), None]);
    assert(actual.counts@ =~= seq![1usize, 0, 0]);
    assert(actual.free@ =~= seq![1usize]);
    (actual, model, writer, model_writer)
}

#[verifier::spinoff_prover]
fn owner_constructor_settle_release_witness_v1(success: bool) -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, writer, model_writer) = owner_constructor_acquired_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    let reference = actual.reservations[0].unwrap().reference;
    let model_reference = model.reservations[0].unwrap().reference;
    let request = actual.reservations[0].unwrap().request;
    let model_request = model.reservations[0].unwrap().request;
    let pending = producer_query_historical_exec_v1(&actual, &model, reference, model_reference);
    assert(pending.0 == Ok(ContextProducerReadStatusV1::Pending));
    let ghost before = model;
    proof {
        reveal_with_fuel(owner_retained_scan_v1, 3);
        reveal_with_fuel(settlement_scratch_scan_v1, 3);
        reveal_with_fuel(settlement_cursor_v1, 3);
        reveal_with_fuel(settlement_allocations_prefix_v1, 3);
        reveal_with_fuel(settlement_members_prefix_v1, 3);
    }
    let settled = owner_settlement_historical_exec_v1(&mut actual, &mut model, writer, model_writer,
        writer, model_writer, 1, 3, success, Ghost(storage), Ghost(history));
    assert(settled.0 == Ok(()) && settled.1 == Ok(()));
    proof { owner_settlement_retained_reference_transition_v1(before, model, model_writer, success, model_reference); }
    let status = producer_query_historical_exec_v1(&actual, &model, reference, model_reference);
    let found = producer_lookup_historical_exec_v1(&actual, &model, reference, model_reference);
    assert(status.0 == Ok(if success { ContextProducerReadStatusV1::Success } else { ContextProducerReadStatusV1::NoEffect }));
    assert(found.0 == Ok(request) && found.1 == Ok(model_request));
    let references = vec![reference];
    let model_references = vec![model_reference];
    proof {
        assert(producer_references_view(references@) =~= model_references@);
        reveal_with_fuel(producer_release_scan_v1, 3);
        reveal_with_fuel(producer_released_reservations_v1, 3);
        reveal_with_fuel(producer_request_count_v1, 3);
    }
    let released = producer_release_historical_exec_v1(&mut actual, &mut model, reference.consumer, model_reference.consumer,
        &references, &model_references, &ContextReadQuiescenceEvidenceV1 { consumer: reference.consumer }, model_reference.consumer, 2);
    assert(released.0 == Ok(()) && released.1 == Ok(()));
    assert(actual.reservations@ =~= seq![None, None]);
    assert(actual.counts@ =~= seq![0usize, 0, 0]);
    assert(actual.free@ =~= seq![1usize, 0]);
    assert(actual.next_incarnation == 2);
    assert(logical::issued_producer_v1(model, storage, history));
    let stale = producer_lookup_historical_exec_v1(&actual, &model, reference, model_reference);
    assert(stale.0 == Err(ReadErrorV1::InvalidReference) && stale.1 == Err(logical::ReadErrorV1::InvalidReference));
    true
}

#[verifier::spinoff_prover]
fn owner_constructor_unknown_fixture_v1() -> (result: (ContextProducerReadJournalV1,
    logical::ProducerReadContentsV1, WriterReferenceV1, logical::WriterReferenceV1))
    ensures producer_represents(result.0, result.1), logical::producer_invariant_v1(result.1),
        writer_reference_view(result.2) == result.3,
        result.2 == (WriterReferenceV1 { slot: 0, key: WriterKeyV1 { context_generation: 7, local: 10, kind: WriterKindV1::Submission } }),
        result.0.stable.journal.context_generation == 7,
        result.0.stable.journal.allocation_capacity == 3, result.0.stable.journal.writer_capacity == 1,
        result.0.stable.journal.writers@ == seq![Some(WriterEntryV1::Unknown { key: result.2.key, head: Some(0usize), count: 1usize })],
        result.0.stable.journal.free@ == seq![], result.0.stable.journal.allocation_free@ == seq![2usize],
        result.0.stable.journal.member_free@ == seq![2usize, 1], result.0.stable.journal.scratch@ == seq![None, None, None],
        result.0.stable.journal.members@ == seq![Some(MemberEntryV1 {
            writer: result.2, allocation: AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
            prior_lineage: 0, attempt_epoch: 1, next: None }), None, None],
        result.0.stable.journal.allocations@.len() == 3,
        result.0.stable.journal.allocations@[0] == Some(AllocationEntryV1 {
            key: AllocationKeyV1 { context_generation: 7, local: 20 },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16,
            content_lineage: 0, attempt_epoch: 1, pending_member: Some(0usize) }),
        result.0.counts@ == seq![0usize, 0, 0], result.0.stable.readers@ == seq![0usize, 0, 0],
{
    let (mut actual, mut model, writer, model_writer) = owner_constructor_pending_fixture_v1();
    proof { reveal_with_fuel(owner_retained_scan_v1, 3); }
    let marked = producer_unknown_historical_exec_v1(&mut actual, &mut model, writer, model_writer);
    assert(marked.0 == Ok(()) && marked.1 == Ok(()));
    assert(actual.stable.journal.writers@ =~= seq![Some(WriterEntryV1::Unknown { key: writer.key, head: Some(0usize), count: 1usize })]);
    (actual, model, writer, model_writer)
}

#[verifier::spinoff_prover]
fn owner_constructor_unknown_disposal_witness_v1() -> (result: bool)
    ensures result,
{
    hide(logical::producer_invariant_v1);
    let (mut actual, mut model, writer, model_writer) = owner_constructor_unknown_fixture_v1();
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
    let disposed = owner_disposal_paired_exec_v1(&mut actual, &mut model, writer, model_writer, writer, model_writer,
        &roster, &model_roster, 1, 3, 3, Ghost(storage), Ghost(history));
    assert(disposed.0 == Ok(()) && disposed.1 == Ok(()));
    assert(actual.stable.journal.writers@ =~= seq![None]);
    assert(actual.stable.journal.free@ =~= seq![0usize]);
    assert(actual.stable.journal.allocations@[0].is_none());
    assert(actual.stable.journal.allocation_free@ =~= seq![2usize, 0]);
    assert(actual.stable.journal.members@ =~= seq![None, None, None]);
    assert(actual.stable.journal.member_free@ =~= seq![2usize, 1, 0]);
    assert(logical::producer_invariant_v1(model));
    true
}

}
