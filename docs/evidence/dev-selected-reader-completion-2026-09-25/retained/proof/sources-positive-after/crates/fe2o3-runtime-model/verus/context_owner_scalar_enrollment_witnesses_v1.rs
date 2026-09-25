verus! {

#[verifier::spinoff_prover]
fn owner_scalar_enrollment_live_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model, writer, model_writer) = owner_settlement_read_fixture_v1();
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = seq![model_writer];
    let reference = actual.reservations[0].unwrap().reference;
    let request = actual.reservations[0].unwrap().request;
    let lease = actual.stable.leases[0].unwrap().reference;
    let read = actual.stable.leases[0].unwrap().request;
    let entry = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 50 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: u64::MAX };
    let model_entry = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 50 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: u64::MAX };
    let ghost before = actual;
    let enrolled = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, entry, model_entry,
        Ghost(storage), Ghost(history));
    assert(enrolled.0 == Ok(AllocationReferenceV1 { slot: 2, key: entry.key }));
    assert(actual.stable.journal.allocation_free@ =~= seq![]);
    assert(actual.stable.journal.allocations@[2] == Some(enrollment_value_v1(entry)));
    assert(logical::issued_producer_v1(model, storage, history));
    assert(actual.reservations == before.reservations && actual.stable.leases == before.stable.leases);
    assert(actual.counts == before.counts && actual.stable.readers == before.stable.readers);
    assert(actual.next_incarnation == before.next_incarnation && actual.stable.next_incarnation == before.stable.next_incarnation);
    let status = actual.producer_read_status(reference);
    let lookup = actual.lookup_producer_read(reference);
    let stable_lookup = actual.stable.lookup_read(lease);
    assert(status == Ok(ContextProducerReadStatusV1::Pending));
    assert(lookup == Ok(request) && stable_lookup == Ok(read));
    let ghost before_replay = actual;
    let ghost model_before_replay = model;
    let replay = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, entry, model_entry,
        Ghost(storage), Ghost(history));
    assert(replay.0 == Err(ReadErrorV1::AllocationReplay));
    assert(actual == before_replay && model == model_before_replay);
    let entry = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 60 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 1 };
    let model_entry = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 60 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 1 };
    let full = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, entry, model_entry,
        Ghost(storage), Ghost(history));
    assert(full.0 == Err(ReadErrorV1::AllocationCapacity));
    assert(actual == before_replay && model == model_before_replay);
    true
}

// This success is outside initial custody, and singleton batch admission would
// reject its logical capacity and aliased free prefix.
#[verifier::spinoff_prover]
fn owner_scalar_enrollment_raw_witness_v1() -> (result: bool)
    ensures result,
{
    let (mut actual, mut model) = owner_settlement_empty_fixture_v1();
    actual.stable.journal.allocation_capacity = 0;
    model.stable.journal.allocation_capacity = 0;
    actual.stable.journal.allocation_free = vec![usize::MAX, 2usize, 2, 2];
    model.stable.journal.allocation_free = vec![usize::MAX, 2usize, 2, 2];
    actual.counts = vec![];
    model.counts = vec![];
    actual.next_incarnation = 0;
    model.next_incarnation = 0;
    proof {
        assert(actual.stable.journal.allocation_free@ =~= model.stable.journal.allocation_free@);
        assert(actual.counts@ =~= model.counts@);
        assert(producer_represents(actual, model));
        assert(!logical::producer_invariant_v1(model));
    }
    let ghost storage = logical::witness_storage_v1(3, 1);
    let ghost history = Seq::<logical::WriterReferenceV1>::empty();
    let invalid = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 0 },
        device: ContextJournalDeviceKeyV1 { context_generation: 8, local: 0 }, byte_extent: 0 };
    let model_invalid = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 0 },
        device: logical::DeviceKeyV1 { context_generation: 8, local: 0 }, byte_extent: 0 };
    let ghost before = actual;
    let ghost model_before = model;
    let rejected = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, invalid, model_invalid,
        Ghost(storage), Ghost(history));
    assert(rejected.0 == Err(ReadErrorV1::InvalidAllocationId));
    assert(actual == before && model == model_before);
    let entry = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 50 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let model_entry = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 50 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let enrolled = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, entry, model_entry,
        Ghost(storage), Ghost(history));
    assert(enrolled.0 == Ok(AllocationReferenceV1 { slot: 2, key: entry.key }));
    assert(actual.stable.journal.allocation_free@ =~= seq![usize::MAX, 2usize, 2]);
    assert(actual.counts == before.counts && actual.next_incarnation == 0);
    let ghost before = actual;
    let ghost model_before = model;
    let replay = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, entry, model_entry,
        Ghost(storage), Ghost(history));
    assert(replay.0 == Err(ReadErrorV1::AllocationReplay));
    assert(actual == before && model == model_before);
    let different = EnrollmentV1 { key: AllocationKeyV1 { context_generation: 7, local: 60 },
        device: ContextJournalDeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let model_different = logical::EnrollmentV1 { key: logical::AllocationKeyV1 { context_generation: 7, local: 60 },
        device: logical::DeviceKeyV1 { context_generation: 7, local: 9 }, byte_extent: 32 };
    let occupied = owner_scalar_enrollment_paired_exec_v1(&mut actual, &mut model, different, model_different,
        Ghost(storage), Ghost(history));
    assert(occupied.0 == Err(ReadErrorV1::InvalidState));
    assert(actual == before && model == model_before);
    true
}

}
