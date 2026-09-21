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

// Deliberately quadratic and immutable: this oracle neither sorts references
// nor uses staged allocation entries to recognize corrupt free-stack aliases.
fn enrollment_oracle(
    journal: &Journal,
    entries: &[ContextAllocationEnrollmentV1],
    output: &[Option<ContextAllocationReferenceV1>],
) -> Result<Vec<Option<ContextAllocationReferenceV1>>, Error> {
    if entries.len() > journal.allocation_capacity || output.len() != entries.len() {
        return Err(Error::RosterCapacity);
    }
    if output.iter().any(Option::is_some) {
        return Err(Error::InvalidState);
    }
    for (index, value) in entries.iter().enumerate() {
        if value.key.context_generation != journal.context_generation
            || value.device.context_generation != journal.context_generation
        {
            return Err(Error::ForeignContext);
        }
        if !issuable_context_id(value.key.local) {
            return Err(Error::InvalidAllocationId);
        }
        if !issuable_context_id(value.device.local) {
            return Err(Error::InvalidDeviceId);
        }
        if value.byte_extent == 0 {
            return Err(Error::InvalidExtent);
        }
        if index != 0 && entries[index - 1].key >= value.key {
            return Err(Error::NonCanonicalRoster);
        }
    }
    if entries.is_empty() {
        return Ok(Vec::new());
    }
    if journal.allocation_free.len() > journal.allocation_capacity {
        return Err(Error::InvalidState);
    }
    if journal
        .allocations
        .iter()
        .flatten()
        .any(|old| entries.iter().any(|new| new.key == old.key))
    {
        return Err(Error::AllocationReplay);
    }
    let remaining = journal
        .allocation_free
        .len()
        .checked_sub(entries.len())
        .ok_or(Error::AllocationCapacity)?;
    let selected = &journal.allocation_free[remaining..];
    if selected
        .iter()
        .any(|&slot| journal.allocations.get(slot) != Some(&None))
        || selected.iter().enumerate().any(|(index, slot)| {
            selected[..index].contains(slot) || journal.allocation_free[..remaining].contains(slot)
        })
    {
        return Err(Error::InvalidState);
    }
    Ok(entries
        .iter()
        .zip(selected.iter().rev())
        .map(|(entry, &slot)| {
            Some(ContextAllocationReferenceV1 {
                slot,
                key: entry.key,
            })
        })
        .collect())
}

fn check_enrollment_oracle(
    journal: &mut Journal,
    entries: &[ContextAllocationEnrollmentV1],
    mut output: Vec<Option<ContextAllocationReferenceV1>>,
) {
    let decision = enrollment_oracle(journal, entries, &output);
    let mut expected = snapshot(journal);
    let original_output = output.clone();
    let output_storage = (output.as_ptr(), output.capacity());
    if let Ok(references) = &decision {
        for (entry, reference) in entries.iter().zip(references) {
            expected.allocations[reference.unwrap().slot] = Some(AllocationEntryV1 {
                key: entry.key,
                device: entry.device,
                byte_extent: entry.byte_extent,
                attempt_epoch: 0,
                content_lineage: 0,
                pending_member: None,
            });
        }
        expected
            .allocation_free
            .truncate(expected.allocation_free.len() - entries.len());
    }
    assert_eq!(
        journal.enroll_allocations(entries, &mut output),
        decision.as_ref().map(|_| ()).map_err(|e| *e)
    );
    assert_eq!(output, decision.unwrap_or(original_output));
    assert_eq!((output.as_ptr(), output.capacity()), output_storage);
    assert_eq!(snapshot(journal), expected);
}

#[test]
fn enrollment_matches_immutable_oracle_for_small_malformed_arenas() {
    let mut cases = 0;
    // Four states per allocation: vacant, unrelated, replay, or same local ID
    // in another generation. Include oversized stacks and two invalid indices.
    for encoding in 0usize..64 {
        for free_len in 0u32..=4 {
            for free_encoding in 0usize..5usize.pow(free_len) {
                for count in 0..=3 {
                    let mut journal = Journal::new(7, 3, 1).unwrap();
                    let mut state = encoding;
                    for (index, value) in journal.allocations.iter_mut().enumerate() {
                        let kind = state % 4;
                        state /= 4;
                        if kind != 0 {
                            let mut metadata =
                                entry(if kind == 1 { 100 + index as u64 } else { 10 });
                            if kind == 3 {
                                metadata.key.context_generation = 8;
                            }
                            *value = Some(AllocationEntryV1 {
                                key: metadata.key,
                                device: metadata.device,
                                byte_extent: metadata.byte_extent,
                                attempt_epoch: 17,
                                content_lineage: 9,
                                pending_member: None,
                            });
                        }
                    }
                    let mut digits = free_encoding;
                    journal.allocation_free = (0..free_len)
                        .map(|_| {
                            let slot = [0, 1, 2, 3, usize::MAX][digits % 5];
                            digits /= 5;
                            slot
                        })
                        .collect();
                    let entries: Vec<_> = (0..count).map(|i| entry(10 + i as u64 * 10)).collect();
                    check_enrollment_oracle(&mut journal, &entries, vec![None; count]);
                    cases += 1;
                }
            }
        }
    }
    assert_eq!(cases, 199_936);
}

#[test]
fn enrollment_oracle_covers_independent_capacity_shape_and_header_precedence() {
    for arena_len in 0..=4 {
        for capacity in 0..=4 {
            for free in [vec![], vec![0], vec![2, 1, 0], vec![usize::MAX, 0]] {
                for case in 0..10 {
                    let mut journal = Journal::new(7, 4, 1).unwrap();
                    journal.allocations.truncate(arena_len);
                    journal.allocation_capacity = capacity;
                    journal.allocation_free = free.clone();
                    let mut entries = vec![entry(10), entry(20)];
                    let mut output = vec![None; 2];
                    match case {
                        0 => {}
                        1 => {
                            entries.clear();
                            output.clear();
                        }
                        2 => {
                            output.pop();
                        }
                        3 => {
                            output[0] = Some(ContextAllocationReferenceV1 {
                                slot: usize::MAX,
                                key: entry(1).key,
                            });
                        }
                        4 => {
                            entries[0].key.context_generation = 8;
                            entries[0].key.local = 0;
                        }
                        5 => {
                            entries[0].key.local = 0;
                            entries[0].device.local = 0;
                        }
                        6 => {
                            entries[0].device.local = 0;
                            entries[0].byte_extent = 0;
                        }
                        7 => {
                            entries[0].byte_extent = 0;
                            entries[1].key.local = 0;
                        }
                        8 => {
                            entries[1].key = entries[0].key;
                        }
                        _ => {
                            entries[1].key.local = 1;
                            entries[1].byte_extent = 0;
                        }
                    }
                    check_enrollment_oracle(&mut journal, &entries, output);
                }
            }
        }
    }
}

#[test]
fn enrollment_preserves_replay_scan_access_counts() {
    for replay in [None, Some(0), Some(2), Some(4)] {
        let mut journal = Journal::new(7, 5, 1).unwrap();
        if let Some(slot) = replay {
            journal.allocations[slot] = Some(AllocationEntryV1 {
                key: entry(10).key,
                device: entry(10).device,
                byte_extent: 64,
                attempt_epoch: 0,
                content_lineage: 0,
                pending_member: None,
            });
        }
        journal.indexed_accesses.set(0);
        let result = journal.enroll_allocations(&[entry(10)], &mut [None]);
        assert_eq!(
            result,
            replay.map_or(Ok(()), |_| Err(Error::AllocationReplay))
        );
        assert_eq!(
            journal.indexed_accesses.get(),
            replay.map_or(5, |slot| slot + 1)
        );
    }
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
fn batch_enrollment_restores_exact_output_for_every_small_free_stack_permutation() {
    fn check(order: &[usize]) -> usize {
        for count in 0..=order.len() {
            let mut journal = Journal::new(7, order.len() + 1, 2).unwrap();
            let existing = enroll(&mut journal, &[1000]);
            assert_eq!(existing[0].slot, 0);
            journal.allocation_free.copy_from_slice(order);
            let entries: Vec<_> = (0..count)
                .map(|index| {
                    let mut value = entry(10 + index as u64 * 10);
                    value.device.local += index as u64;
                    value.byte_extent += index as u64;
                    value
                })
                .collect();
            let mut expected = snapshot(&journal);
            let expected_output: Vec<_> = entries
                .iter()
                .zip(order.iter().rev())
                .map(|(entry, &slot)| {
                    expected.allocations[slot] = Some(AllocationEntryV1 {
                        key: entry.key,
                        device: entry.device,
                        byte_extent: entry.byte_extent,
                        attempt_epoch: 0,
                        content_lineage: 0,
                        pending_member: None,
                    });
                    Some(ContextAllocationReferenceV1 {
                        slot,
                        key: entry.key,
                    })
                })
                .collect();
            expected.allocation_free.truncate(order.len() - count);
            let mut output = vec![None; count];
            let output_storage = (output.as_ptr(), output.capacity());
            assert_eq!(journal.enroll_allocations(&entries, &mut output), Ok(()));
            assert_eq!(output, expected_output, "free stack {order:?}");
            assert_eq!((output.as_ptr(), output.capacity()), output_storage);
            assert_eq!(snapshot(&journal), expected, "free stack {order:?}");
            audit(&journal);
        }
        order.len() + 1
    }

    fn permutations(order: &mut [usize], offset: usize) -> usize {
        if offset == order.len() {
            return check(order);
        }
        let mut cases = 0;
        for index in offset..order.len() {
            order.swap(offset, index);
            cases += permutations(order, offset + 1);
            order.swap(offset, index);
        }
        cases
    }

    assert_eq!(permutations(&mut [1, 2, 3, 4, 5], 0), 720);
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
