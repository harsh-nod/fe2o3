use super::*;

fn entry(local: u64) -> ContextAllocationEnrollmentV1 {
    ContextAllocationEnrollmentV1 {
        key: ContextAllocationKeyV1 {
            context_generation: 7,
            local,
        },
        device: ContextJournalDeviceKeyV1 {
            context_generation: 7,
            local: 11,
        },
        byte_extent: 64,
    }
}

fn enroll(journal: &mut Journal, locals: &[u64]) -> Vec<ContextAllocationReferenceV1> {
    let entries: Vec<_> = locals.iter().map(|&local| entry(local)).collect();
    let mut output = vec![None; locals.len()];
    journal.enroll_allocations(&entries, &mut output).unwrap();
    output.into_iter().map(Option::unwrap).collect()
}

fn reject_enrollment(
    journal: &mut Journal,
    entries: &[ContextAllocationEnrollmentV1],
    error: Error,
) {
    let before = snapshot(journal);
    let mut output = vec![None; entries.len()];
    assert_eq!(journal.enroll_allocations(entries, &mut output), Err(error));
    assert_eq!(snapshot(journal), before);
    assert!(output.iter().all(Option::is_none));
}

#[test]
fn batch_enrollment_preserves_key_coordinates_across_permuted_slots_and_out_of_order_batches() {
    let mut journal = Journal::new(7, 6, 2).unwrap();
    journal.allocation_free = vec![4, 0, 5, 2, 1, 3];
    let addresses = storage(&journal);
    let high = enroll(&mut journal, &[300, 400, 500]);
    assert_eq!(high.iter().map(|r| r.slot).collect::<Vec<_>>(), [3, 1, 2]);
    let low = enroll(&mut journal, &[100, 200, u64::MAX - 1]);
    for reference in high.iter().chain(&low) {
        let actual = journal.lookup_allocation(*reference).unwrap();
        assert_eq!(actual.device, entry(reference.key.local).device);
        assert_eq!(actual.byte_extent, 64);
        assert_eq!(actual.content_lineage, 0);
    }
    assert_eq!(journal.remaining_allocation_slots(), 0);
    journal.retire_allocations(&high).unwrap();
    let fresh = enroll(&mut journal, &[501, 502, 503]);
    assert_eq!(fresh.len(), 3);
    for reference in high {
        assert_eq!(
            journal.lookup_allocation(reference),
            Err(Error::InvalidAllocationReference)
        );
    }
    assert_eq!(storage(&journal), addresses);
    audit(&journal);
}

#[test]
fn batch_enrollment_rejects_each_bad_coordinate_without_partial_changes() {
    for index in 0..3 {
        for case in 0..6 {
            let mut journal = Journal::new(7, 4, 2).unwrap();
            let mut entries = [entry(10), entry(20), entry(30)];
            let error = match case {
                0 => {
                    entries[index].key.context_generation = 8;
                    Error::ForeignContext
                }
                1 => {
                    entries[index].key.local = 0;
                    Error::InvalidAllocationId
                }
                2 => {
                    entries[index].key.local = u64::MAX;
                    Error::InvalidAllocationId
                }
                3 => {
                    entries[index].device.context_generation = 8;
                    Error::ForeignContext
                }
                4 => {
                    entries[index].device.local = 0;
                    Error::InvalidDeviceId
                }
                _ => {
                    entries[index].byte_extent = 0;
                    Error::InvalidExtent
                }
            };
            reject_enrollment(&mut journal, &entries, error);
        }
    }
}

#[test]
fn batch_enrollment_rejects_capacity_replay_order_and_output_shape() {
    let mut journal = Journal::new(7, 3, 1).unwrap();
    let existing = enroll(&mut journal, &[20]);
    reject_enrollment(
        &mut journal,
        &[entry(10), entry(20)],
        Error::AllocationReplay,
    );
    reject_enrollment(
        &mut journal,
        &[entry(10), entry(10)],
        Error::NonCanonicalRoster,
    );
    reject_enrollment(
        &mut journal,
        &[entry(30), entry(10)],
        Error::NonCanonicalRoster,
    );
    reject_enrollment(
        &mut journal,
        &[entry(30), entry(40), entry(50)],
        Error::AllocationCapacity,
    );
    let before = snapshot(&journal);
    let mut output = [Some(existing[0])];
    assert_eq!(
        journal.enroll_allocations(&[entry(10)], &mut output),
        Err(Error::InvalidState)
    );
    assert_eq!(output, [Some(existing[0])]);
    assert_eq!(
        journal.enroll_allocations(&[entry(10)], &mut []),
        Err(Error::RosterCapacity)
    );
    assert_eq!(journal.enroll_allocations(&[], &mut []), Ok(()));
    assert_eq!(snapshot(&journal), before);
}

#[test]
fn batch_enrollment_rejects_corrupt_selected_free_slots_and_restores_output() {
    for index in 0..3 {
        for occupied in [false, true] {
            let mut journal = Journal::new(7, 4, 1).unwrap();
            let existing = enroll(&mut journal, &[100]);
            journal.allocation_free[index] = if occupied {
                existing[0].slot
            } else {
                usize::MAX
            };
            reject_enrollment(
                &mut journal,
                &[entry(10), entry(20), entry(30)],
                Error::InvalidState,
            );
        }
    }
    let mut journal = Journal::new(7, 3, 1).unwrap();
    journal.allocation_free[0] = journal.allocation_free[2];
    reject_enrollment(
        &mut journal,
        &[entry(10), entry(20), entry(30)],
        Error::InvalidState,
    );
    reject_enrollment(&mut journal, &[entry(10)], Error::InvalidState);
    let mut journal = Journal::new(7, 2, 1).unwrap();
    journal.allocation_free = vec![0, 1, 0];
    reject_enrollment(&mut journal, &[entry(10)], Error::InvalidState);
}

#[test]
fn batch_retirement_rejects_pending_and_unknown_at_every_position() {
    for index in 0..3 {
        for unknown in [false, true] {
            let mut journal = Journal::new(7, 3, 1).unwrap();
            let references = enroll(&mut journal, &[10, 20, 30]);
            let writer = journal.register_writer(key(40)).unwrap();
            journal
                .begin_write(
                    writer,
                    &[ContextAllocationWriteV1 {
                        allocation: references[index],
                        device: entry(10).device,
                        byte_extent: 64,
                    }],
                )
                .unwrap();
            if unknown {
                journal.mark_unknown(writer).unwrap();
            }
            let before = snapshot(&journal);
            assert_eq!(
                journal.validate_allocation_retirement(&references),
                Err(Error::AllocationBusy)
            );
            assert_eq!(
                journal.retire_allocations(&references),
                Err(Error::AllocationBusy)
            );
            assert_eq!(snapshot(&journal), before);
            audit(&journal);
        }
    }
}

#[test]
fn batch_retirement_rejects_stale_foreign_duplicate_and_insufficient_return_storage() {
    for index in 0..3 {
        let mut journal = Journal::new(7, 3, 1).unwrap();
        let mut references = enroll(&mut journal, &[10, 20, 30]);
        references[index].key.context_generation = 8;
        let before = snapshot(&journal);
        assert_eq!(
            journal.retire_allocations(&references),
            Err(Error::InvalidAllocationReference)
        );
        assert_eq!(snapshot(&journal), before);
    }
    let mut journal = Journal::new(7, 3, 1).unwrap();
    let references = enroll(&mut journal, &[10, 20, 30]);
    let before = snapshot(&journal);
    assert_eq!(
        journal.retire_allocations(&[references[0], references[0]]),
        Err(Error::NonCanonicalRoster)
    );
    assert_eq!(snapshot(&journal), before);
    journal.allocation_free = Vec::new();
    let before = snapshot(&journal);
    assert_eq!(
        journal.retire_allocations(&references),
        Err(Error::InvalidState)
    );
    assert_eq!(snapshot(&journal), before);
}

#[test]
fn capacity_one_fresh_identity_reuse_never_revives_old_references() {
    let mut journal = Journal::new(7, 1, 1).unwrap();
    let addresses = storage(&journal);
    let mut old = Vec::new();
    for local in 1..=100 {
        let current = enroll(&mut journal, &[local]);
        for &reference in &old {
            assert_eq!(
                journal.lookup_allocation(reference),
                Err(Error::InvalidAllocationReference)
            );
        }
        journal.retire_allocations(&current).unwrap();
        old.push(current[0]);
        assert_eq!(journal.remaining_allocation_slots(), 1);
        audit(&journal);
    }
    assert_eq!(storage(&journal), addresses);
    let before = snapshot(&journal);
    journal.retire_allocations(&[]).unwrap();
    assert_eq!(snapshot(&journal), before);
}
