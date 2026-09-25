verus! {

// Synthetic actual contents paired with the logical constructor, not allocation refinement.
#[verifier::spinoff_prover]
fn owner_enrollment_fixture_v1() -> (result: Option<(ContextProducerReadJournalV1, logical::ProducerReadContentsV1)>)
    ensures result.is_some(), producer_represents(result.unwrap().0, result.unwrap().1),
        logical::producer_invariant_v1(result.unwrap().1),
        logical::producer_constructor_relation_v1(7, 3, 1, 2, Ok(result.unwrap().1)),
        result.unwrap().0.stable.journal.context_generation == 7,
        result.unwrap().0.stable.journal.allocation_capacity == 3,
        result.unwrap().0.stable.journal.allocations@ == seq![None, None, None],
        result.unwrap().0.stable.journal.allocation_free@ == seq![2usize, 1, 0],
{
    let actual = ContextProducerReadJournalV1 {
        stable: ContextReadLeasedJournalV1 { journal: enrollment_three_vacant_fixture_v1(),
            leases: vec![None, None], free_reads: vec![1usize, 0], readers: vec![0usize, 0, 0], next_incarnation: 1 },
        reservations: vec![None, None], free: vec![1usize, 0], counts: vec![0usize, 0, 0], next_incarnation: 1,
    };
    let model = match logical::producer_constructor_exec_v1(7, 3, 1, 2) {
        Ok(value) => value, Err(_) => return None,
    };
    proof {
        enrollment_fixture_representation_v1(actual.stable.journal, model.stable.journal);
        assert(actual.stable.leases@.map(|_i, entry| read_lease_slot_view(entry)) =~= model.stable.leases@);
        assert(actual.stable.free_reads@ =~= model.stable.free_reads@);
        assert(actual.stable.readers@ =~= model.stable.readers@);
        assert(actual.reservations@.map(|_i, entry| producer_reservation_slot_view(entry)) =~= model.reservations@);
        assert(actual.free@ =~= model.free@);
        assert(actual.counts@ =~= model.counts@);
    }
    Some((actual, model))
}

#[verifier::spinoff_prover]
fn owner_enrollment_success_and_rollback_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_enrollment_fixture_v1().unwrap();
    actual.stable.journal.allocation_free = vec![2usize, 0, 1];
    model.stable.journal.allocation_free = vec![2usize, 0, 1];
    let entries = vec![
        EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 },
        EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 30 },
            device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 }];
    let model_entries = vec![
        logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 20 },
            device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 },
        logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 30 },
            device: logical::DeviceKeyV1 { context_generation: 7, local: 3 }, byte_extent: 32 }];
    let mut output = vec![None, None];
    let mut model_output = vec![None, None];
    let ghost before = actual;
    proof {
        assert(actual.stable.journal.allocation_free@ =~= model.stable.journal.allocation_free@);
        assert(entries_view(entries@) =~= model_entries@);
        assert(references_view(output@) =~= model_output@);
        assert forall|s: int| 0 <= s < 3 implies #[trigger] model.stable.journal.allocation_free@.contains(s as usize) by {
            if s == 0 { assert(model.stable.journal.allocation_free@[1] == s); }
            else if s == 1 { assert(model.stable.journal.allocation_free@[2] == s); }
            else { assert(model.stable.journal.allocation_free@[0] == s); }
        }
        assert(logical::slot_partition_v1(model.stable.journal.allocations@, model.stable.journal.allocation_free@));
        reveal(logical::allocation_custody_v1);
        reveal(logical::member_custody_v1);
        reveal(logical::writer_custody_v1);
        assert(logical::producer_invariant_v1(model));
        reveal_with_fuel(enrollment_roster_scan_v1, 4);
        reveal_with_fuel(enrollment_prefix_v1, 4);
    }
    let enrolled = producer_enrollment_historical_exec_v1(&mut actual, &mut model, &entries, &model_entries,
        &mut output, &mut model_output);
    assert(enrolled.0 == Ok(()) && enrolled.1 == Ok(()));
    assert(logical::producer_invariant_v1(model));
    assert(output@[0] == Some(AllocationReferenceV1 { slot: 1, key: entries@[0].key }));
    assert(output@[1] == Some(AllocationReferenceV1 { slot: 0, key: entries@[1].key }));
    assert(actual.stable.journal.allocations@[1] == Some(enrollment_value_v1(entries@[0])));
    assert(actual.stable.journal.allocations@[0] == Some(enrollment_value_v1(entries@[1])));
    assert(actual.stable.journal.allocation_free@ == seq![2usize]);
    assert(actual.reservations == before.reservations && actual.stable.leases == before.stable.leases);
    assert(actual.stable.journal.writers == before.stable.journal.writers);
    actual.stable.journal.allocation_free.push(2usize);
    model.stable.journal.allocation_free.push(2usize);
    let next = vec![EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 40 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 4 }, byte_extent: 64 }];
    let model_next = vec![logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 40 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 4 }, byte_extent: 64 }];
    let mut rejected = vec![None];
    let mut model_rejected = vec![None];
    let ghost before = actual;
    let ghost model_before = model;
    proof {
        assert(entries_view(next@) =~= model_next@);
        assert(references_view(rejected@) =~= model_rejected@);
        let plan = enrollment_plan_output_v1(actual.stable.journal, next@);
        assert(plan[0].unwrap().slot == 2);
        assert(!enrollment_retained_clear_v1(actual.stable.journal, plan, 1)) by {
            if enrollment_retained_clear_v1(actual.stable.journal, plan, 1) {
                assert(actual.stable.journal.allocation_free@[0] != plan[0].unwrap().slot);
            }
        }
    }
    let rejected_result = producer_enrollment_historical_exec_v1(&mut actual, &mut model, &next, &model_next,
        &mut rejected, &mut model_rejected);
    assert(rejected_result.0 == Err(EnrollmentErrorV1::InvalidState));
    assert(actual == before && model == model_before);
    assert(rejected@ == seq![None] && model_rejected@ == seq![None]);
    true
}

#[verifier::spinoff_prover]
fn owner_enrollment_raw_storage_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_enrollment_fixture_v1().unwrap();
    actual.stable.readers = vec![];
    model.stable.readers = vec![];
    actual.counts = vec![];
    model.counts = vec![];
    actual.next_incarnation = 0;
    model.next_incarnation = 0;
    actual.stable.next_incarnation = u64::MAX;
    model.stable.next_incarnation = u64::MAX;
    let entries = vec![EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    let model_entries = vec![logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 20 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    let mut output = vec![None];
    let mut model_output = vec![None];
    let ghost before = actual;
    proof {
        assert(actual.stable.readers@ =~= model.stable.readers@);
        assert(actual.counts@ =~= model.counts@);
        assert(entries_view(entries@) =~= model_entries@);
        assert(references_view(output@) =~= model_output@);
        reveal_with_fuel(enrollment_roster_scan_v1, 3);
    }
    let result = producer_enrollment_historical_exec_v1(&mut actual, &mut model, &entries, &model_entries,
        &mut output, &mut model_output);
    assert(result.0 == Ok(()));
    assert(actual.counts == before.counts && actual.stable.readers == before.stable.readers);
    actual.stable.journal.allocation_free = vec![usize::MAX, usize::MAX, usize::MAX, usize::MAX];
    model.stable.journal.allocation_free = vec![usize::MAX, usize::MAX, usize::MAX, usize::MAX];
    let empty: Vec<EnrollmentV1> = vec![];
    let model_empty: Vec<logical::EnrollmentV1> = vec![];
    let mut empty_output = vec![];
    let mut model_empty_output = vec![];
    let ghost before = actual;
    proof {
        assert(actual.stable.journal.allocation_free@ =~= model.stable.journal.allocation_free@);
        assert(entries_view(empty@) =~= model_empty@);
        assert(references_view(empty_output@) =~= model_empty_output@);
    }
    let empty_result = producer_enrollment_historical_exec_v1(&mut actual, &mut model, &empty, &model_empty,
        &mut empty_output, &mut model_empty_output);
    assert(empty_result.0 == Ok(()) && actual == before);
    true
}

}
