verus! {

#[verifier::spinoff_prover]
fn owner_retirement_live_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, _writer, model_writer) = owner_settlement_read_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    let reference = actual.reservations[0].unwrap().reference;
    let request = actual.reservations[0].unwrap().request;
    let lease = actual.stable.leases[0].unwrap().reference;
    let read = actual.stable.leases[0].unwrap().request;
    let entry = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 50 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let model_entry = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 50 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let enrolled = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, entry, model_entry, Ghost(storage), Ghost(history));
    assert(enrolled.0 == Ok(AllocationReferenceV1 { slot: 2, key: entry.key }));
    let retired_reference = AllocationReferenceV1 { slot: 2, key: entry.key };
    let model_retired_reference = logical::AllocationReferenceV1 { slot: 2, key: model_entry.key };
    let roster = vec![retired_reference];
    let model_roster = vec![model_retired_reference];
    proof {
        assert(retirement_roster_view_v1(roster@) =~= model_roster@);
        owner_retirement_invariant_domain_v1(actual, model, roster@);
        reveal_with_fuel(retirement_scan_v1, 3);
        reveal_with_fuel(retirement_stable_unread_v1, 3);
        reveal_with_fuel(retirement_producer_unread_v1, 3);
        reveal_with_fuel(retirement_prefix_v1, 3);
    }
    let retired = owner_retirement_paired_exec_v1(&mut actual, &mut model, &roster, &model_roster, 3, Ghost(storage), Ghost(history));
    assert(retired.0 == Ok(()));
    assert(actual.stable.journal.allocations@[2].is_none());
    assert(actual.stable.journal.allocation_free@ =~= seq![2usize]);
    assert(logical::issued_producer_v1(model, storage, history));
    let status = actual.producer_read_status(reference);
    let lookup = actual.lookup_producer_read(reference);
    let stable_lookup = actual.stable.lookup_read(lease);
    assert(status == Ok(ContextProducerReadStatusV1::Pending));
    assert(lookup == Ok(request) && stable_lookup == Ok(read));
    true
}

#[verifier::spinoff_prover]
fn owner_retirement_busy_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, _writer, model_writer) = owner_settlement_read_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    let blocked = vec![AllocationReferenceV1 { slot: 1, key: AllocationKeyV1 { context_generation: 7, local: 30 } }];
    let model_blocked = vec![logical::AllocationReferenceV1 { slot: 1,
        key: logical::AllocationKeyV1 { context_generation: 7, local: 30 } }];
    proof {
        assert(retirement_roster_view_v1(blocked@) =~= model_blocked@);
        owner_retirement_invariant_domain_v1(actual, model, blocked@);
    }
    let ghost before = actual;
    let ghost model_before = model;
    let rejected = owner_retirement_paired_exec_v1(&mut actual, &mut model, &blocked, &model_blocked, 3, Ghost(storage), Ghost(history));
    assert(rejected.0 == Err(ReadErrorV1::AllocationBusy));
    assert(actual == before && model == model_before);
    true
}

#[verifier::spinoff_prover]
fn owner_retirement_nonmonotone_fixture_v1() -> (result: (ContextProducerReadJournalV1, logical::ProducerReadContentsV1))
    ensures producer_represents(result.0, result.1),
        logical::issued_producer_v1(result.1, logical::witness_storage_v1(3, 1), Seq::empty()),
        result.0.stable.journal.context_generation == 7, result.0.stable.journal.allocation_capacity == 3,
        result.0.stable.journal.allocations@ == seq![None, None, None],
        result.0.stable.journal.allocation_free@ == seq![0usize, 2, 1],
{
    let (mut actual, mut model) = owner_settlement_enrolled_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    let entry = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 25 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let model_entry = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 25 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let enrolled = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, entry, model_entry, Ghost(storage), Ghost(history));
    assert(enrolled.0 == Ok(AllocationReferenceV1 { slot: 2, key: entry.key }));
    let roster = vec![AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } },
        AllocationReferenceV1 { slot: 2, key: entry.key },
        AllocationReferenceV1 { slot: 1, key: AllocationKeyV1 { context_generation: 7, local: 30 } }];
    let model_roster = vec![logical::AllocationReferenceV1 { slot: 0, key: logical::AllocationKeyV1 { context_generation: 7, local: 20 } },
        logical::AllocationReferenceV1 { slot: 2, key: model_entry.key },
        logical::AllocationReferenceV1 { slot: 1, key: logical::AllocationKeyV1 { context_generation: 7, local: 30 } }];
    proof {
        assert(retirement_roster_view_v1(roster@) =~= model_roster@);
        owner_retirement_invariant_domain_v1(actual, model, roster@);
        reveal_with_fuel(retirement_scan_v1, 5);
        reveal_with_fuel(retirement_stable_unread_v1, 5);
        reveal_with_fuel(retirement_producer_unread_v1, 5);
        reveal_with_fuel(retirement_prefix_v1, 5);
    }
    let retired = owner_retirement_paired_exec_v1(&mut actual, &mut model, &roster, &model_roster, 3, Ghost(storage), Ghost(history));
    assert(retired.0 == Ok(()));
    assert(actual.stable.journal.allocations@ =~= seq![None, None, None]);
    assert(actual.stable.journal.allocation_free@ =~= seq![0usize, 2, 1]);
    assert(logical::issued_producer_v1(model, storage, history));
    (actual, model)
}

#[verifier::spinoff_prover]
fn owner_retirement_reuse_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_retirement_nonmonotone_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    let replacement = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 60 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let model_replacement = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 60 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let reused = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, replacement, model_replacement, Ghost(storage), Ghost(history));
    assert(reused.0 == Ok(AllocationReferenceV1 { slot: 1, key: replacement.key }));
    let stale = actual.stable.journal.lookup_allocation(AllocationReferenceV1 {
        slot: 1, key: AllocationKeyV1 { context_generation: 7, local: 30 } });
    assert(stale == Err(ReadErrorV1::InvalidAllocationReference));
    true
}

// A failed first lookup must not inspect malformed count storage in the suffix.
#[verifier::spinoff_prover]
fn owner_retirement_raw_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_settlement_enrolled_fixture_v1();
    actual.stable.journal.allocation_capacity = 0;
    model.stable.journal.allocation_capacity = 0;
    actual.stable.readers = vec![];
    model.stable.readers = vec![];
    actual.counts = vec![];
    model.counts = vec![];
    let roster = vec![AllocationReferenceV1 { slot: usize::MAX, key: AllocationKeyV1 { context_generation: 7, local: 10 } },
        AllocationReferenceV1 { slot: 0, key: AllocationKeyV1 { context_generation: 7, local: 20 } }];
    let model_roster = vec![logical::AllocationReferenceV1 { slot: usize::MAX, key: logical::AllocationKeyV1 { context_generation: 7, local: 10 } },
        logical::AllocationReferenceV1 { slot: 0, key: logical::AllocationKeyV1 { context_generation: 7, local: 20 } }];
    proof {
        assert(actual.stable.readers@ =~= model.stable.readers@);
        assert(actual.counts@ =~= model.counts@);
        assert(producer_represents(actual, model));
        assert(!logical::producer_invariant_v1(model));
        assert(retirement_roster_view_v1(roster@) =~= model_roster@);
        assert(retirement_producer_safe_v1(actual, roster@, 0));
    }
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    let ghost before = actual;
    let ghost model_before = model;
    let rejected = owner_retirement_paired_exec_v1(&mut actual, &mut model, &roster, &model_roster, 0, Ghost(storage), Ghost(history));
    assert(rejected.0 == Err(ReadErrorV1::InvalidAllocationReference));
    assert(actual == before && model == model_before);
    let empty: Vec<AllocationReferenceV1> = vec![];
    let model_empty: Vec<logical::AllocationReferenceV1> = vec![];
    proof { assert(retirement_roster_view_v1(empty@) =~= model_empty@); }
    let rejected = owner_retirement_paired_exec_v1(&mut actual, &mut model, &empty, &model_empty, 0, Ghost(storage), Ghost(history));
    assert(rejected.0 == Err(ReadErrorV1::InvalidState));
    assert(actual == before && model == model_before);
    true
}

}
