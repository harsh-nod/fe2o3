verus! {

// Concrete verification fixture, not refinement of the fallible Rust constructor.
fn enrollment_three_vacant_fixture_v1() -> (journal: JournalContentsV1)
    ensures journal.context_generation == 7, journal.allocation_capacity == 3, journal.writer_capacity == 1,
        journal.registration_watermark == 0, journal.reserved_count == 0,
        journal.writers@ == seq![None], journal.free@ == seq![0usize],
        journal.allocations@ == seq![None, None, None], journal.allocation_free@ == seq![2usize, 1, 0],
        journal.members@ == seq![None, None, None], journal.member_free@ == seq![2usize, 1, 0],
        journal.scratch@ == seq![None, None, None],
{
    JournalContentsV1 { context_generation: 7, allocation_capacity: 3, writer_capacity: 1,
        registration_watermark: 0, reserved_count: 0,
        writers: vec![None], free: vec![0usize], allocations: vec![None, None, None],
        allocation_free: vec![2usize, 1, 0], members: vec![None, None, None],
        member_free: vec![2usize, 1, 0], scratch: vec![None, None, None] }
}

proof fn enrollment_fixture_representation_v1(actual: JournalContentsV1, model: logical::JournalContentsV1)
    requires logical::constructor_contents_initialized_v1(model, 7, 3, 1),
        actual.context_generation == 7, actual.allocation_capacity == 3, actual.writer_capacity == 1,
        actual.registration_watermark == 0, actual.reserved_count == 0,
        actual.writers@ == seq![None], actual.free@ == seq![0usize],
        actual.allocations@ == seq![None, None, None], actual.allocation_free@ == seq![2usize, 1, 0],
        actual.members@ == seq![None, None, None], actual.member_free@ == seq![2usize, 1, 0],
        actual.scratch@ == seq![None, None, None],
    ensures represents(actual, model),
{
    assert(journal_view(actual).writers =~= model.writers@);
    assert(journal_view(actual).free =~= model.free@);
    assert(journal_view(actual).allocations =~= model.allocations@);
    assert(journal_view(actual).allocation_free =~= model.allocation_free@);
    assert(journal_view(actual).members =~= model.members@);
    assert(journal_view(actual).member_free =~= model.member_free@);
    assert(journal_view(actual).scratch =~= model.scratch@);
}

#[verifier::spinoff_prover]
fn enrollment_raw_correspondence_witness_v1() -> (result: bool)
    ensures result,
{
    proof {
        reveal_with_fuel(enrollment_roster_scan_v1, 4);
        reveal_with_fuel(enrollment_prefix_v1, 4);
    }
    let mut actual = enrollment_three_vacant_fixture_v1();
    let mut model = match logical::constructor_contents_exec_v1(7, 3, 1) {
        Ok(journal) => journal, Err(_) => return false,
    };
    proof { enrollment_fixture_representation_v1(actual, model); }
    actual.allocation_free = vec![2usize, 0, 1];
    model.allocation_free = vec![2usize, 0, 1];
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
    proof {
        assert(entries_view(entries@) =~= model_entries@);
        assert(references_view(output@) =~= model_output@);
    }
    let result = enrollment_historical_exec_v1(&mut actual, &mut model, &entries, &model_entries, &mut output, &mut model_output);
    assert(result.0 == Ok(()));
    assert(output@[0] == Some(AllocationReferenceV1 { slot: 1, key: entries@[0].key }));
    assert(output@[1] == Some(AllocationReferenceV1 { slot: 0, key: entries@[1].key }));
    assert(actual.allocations@[1] == Some(enrollment_value_v1(entries@[0])));
    assert(actual.allocations@[0] == Some(enrollment_value_v1(entries@[1])));
    assert(actual.allocations@[2].is_none());
    assert(actual.allocation_free@ == seq![2usize]);

    // Deliberately malformed raw journals exercise a late rollback without custody premises.
    actual.allocation_free.push(2usize);
    model.allocation_free.push(2usize);
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
        assert(!enrollment_replay_v1(actual, next@));
        assert(enrollment_plan_output_v1(actual, next@)[0].unwrap().slot == 2);
        let plan = enrollment_plan_output_v1(actual, next@);
        assert(actual.allocation_free@[0] == 2);
        assert(plan.len() == 1 && plan[0].unwrap().slot == 2);
        assert(!enrollment_retained_clear_v1(actual, plan, 1)) by {
            if enrollment_retained_clear_v1(actual, plan, 1) {
                assert(actual.allocation_free@[0] != plan[0].unwrap().slot);
            }
        }
    }
    let result = enrollment_historical_exec_v1(&mut actual, &mut model, &next, &model_next, &mut rejected, &mut model_rejected);
    assert(result.0 == Err(EnrollmentErrorV1::InvalidState));
    assert(actual == before && model == model_before);
    assert(rejected@ == seq![None] && model_rejected@ == seq![None]);

    let invalid = vec![EnrollmentV1 { key: AllocationKeyV1 { context_generation: 8, local: 0 },
        device: ContextJournalDeviceKeyV1 { context_generation: 9, local: 0 }, byte_extent: 0 }];
    let model_invalid = vec![logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 8, local: 0 },
        device: logical::DeviceKeyV1 { context_generation: 9, local: 0 }, byte_extent: 0 }];
    let mut dirty = vec![Some(AllocationReferenceV1 { slot: usize::MAX,
        key: AllocationKeyV1 { context_generation: 7, local: 91 } })];
    let mut model_dirty = vec![Some(logical::AllocationReferenceV1 { slot: usize::MAX,
        key: logical::AllocationKeyV1 { context_generation: 7, local: 91 } })];
    let ghost old_dirty = dirty@;
    let ghost old_model_dirty = model_dirty@;
    proof {
        assert(entries_view(invalid@) =~= model_invalid@);
        assert(references_view(dirty@) =~= model_dirty@);
    }
    let result = enrollment_historical_exec_v1(&mut actual, &mut model, &invalid, &model_invalid, &mut dirty, &mut model_dirty);
    assert(result.0 == Err(EnrollmentErrorV1::InvalidState));
    assert(actual == before && model == model_before);
    assert(dirty@ == old_dirty && model_dirty@ == old_model_dirty);
    true
}

#[verifier::spinoff_prover]
fn enrollment_issued_correspondence_witness_v1() -> (result: bool)
    ensures result,
{
    proof {
        reveal_with_fuel(enrollment_roster_scan_v1, 3);
        reveal_with_fuel(enrollment_prefix_v1, 3);
    }
    let mut actual = enrollment_three_vacant_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    let mut model = match logical::issued_producer_constructor_exec_v1(7, 3, 1, 2, Ghost(storage)) {
        Ok(contents) => contents, Err(_) => return false,
    };
    proof { enrollment_fixture_representation_v1(actual, model.stable.journal); }
    let entries = vec![EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 20 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    let model_entries = vec![logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 20 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 2 }, byte_extent: 16 }];
    let mut output = vec![None];
    let mut model_output = vec![None];
    proof {
        assert(entries_view(entries@) =~= model_entries@);
        assert(references_view(output@) =~= model_output@);
    }
    let result = enrollment_issued_historical_exec_v1(&mut actual, &mut model, &entries, &model_entries,
        &mut output, &mut model_output, Ghost(storage), Ghost(history));
    assert(result.0 == Ok(()));
    assert(output@[0] == Some(AllocationReferenceV1 { slot: 0, key: entries@[0].key }));
    assert(actual.allocations@[0] == Some(enrollment_value_v1(entries@[0])));
    assert(logical::issued_producer_v1(model, storage, history));
    let ghost before = actual;
    let ghost model_before = model;
    let mut rejected = vec![None];
    let mut model_rejected = vec![None];
    proof { assert(references_view(rejected@) =~= model_rejected@); }
    let result = enrollment_issued_historical_exec_v1(&mut actual, &mut model, &entries, &model_entries,
        &mut rejected, &mut model_rejected, Ghost(storage), Ghost(history));
    assert(result.0 == Err(EnrollmentErrorV1::AllocationReplay));
    assert(actual == before && model == model_before);
    assert(rejected@ == seq![None] && model_rejected@ == seq![None]);
    assert(logical::issued_producer_v1(model, storage, history));
    true
}

}
