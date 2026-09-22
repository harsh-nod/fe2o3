use super::*;
use alloc::{format, string::String};

type Journal = ContextReadLeasedJournalV1;
type Error = ContextVersionJournalErrorV1;

mod acquire_shared;

mod begin_guards {
    use super::*;
    type Owner = ContextReadLeasedJournalV1;
    const OWNER: &str = "stable";

    fn protect(owner: &mut Owner, destination: ContextAllocationWriteV1) {
        let request = read_view(owner, destination);
        owner
            .acquire_reads(guard_key(20), &[request], &mut [None])
            .unwrap();
    }

    include!("guards_tests_shared.rs");
}

#[test]
fn invalid_lookup_precedes_missing_stable_count_storage() {
    let (mut owner, requests) = fixture(4);
    owner.readers.clear();
    let mut invalid = requests[0].allocation;
    invalid.key.context_generation += 1;
    let before = snapshot(&owner);
    assert_eq!(
        owner.reader_count(invalid),
        Err(Error::InvalidAllocationReference)
    );
    assert_eq!(snapshot(&owner), before);
}

#[test]
fn invalid_backlink_precedes_missing_stable_count_storage() {
    let (mut owner, requests) = fixture(4);
    let allocation = requests[0].allocation;
    owner.guard_break_backlink_for_test_v1(allocation);
    owner.readers.clear();
    let before = snapshot(&owner);
    assert_eq!(owner.reader_count(allocation), Err(Error::InvalidState));
    assert_eq!(
        owner.baseline_reader_count_v1(allocation),
        Err(Error::InvalidState)
    );
    assert_eq!(snapshot(&owner), before);
}

fn consumer(local: u64) -> ContextWriterKeyV1 {
    ContextWriterKeyV1 {
        context_generation: 7,
        local,
        kind: ContextWriterKindV1::Submission,
    }
}

fn fixture(reads: usize) -> (Journal, [ContextAllocationReadV1; 3]) {
    let mut journal = Journal::new(7, 4, 4, reads).unwrap();
    let requests = core::array::from_fn(|index| {
        let device = ContextJournalDeviceKeyV1 {
            context_generation: 7,
            local: 1,
        };
        let allocation = journal
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: index as u64 + 2,
                },
                device,
                64,
            )
            .unwrap();
        ContextAllocationReadV1 {
            allocation,
            device,
            byte_extent: 64,
            byte_offset: 8,
            byte_len: 16,
            attempt_epoch: 0,
            content_lineage: 0,
        }
    });
    (journal, requests)
}

fn member(read: ContextAllocationReadV1) -> ContextAllocationWriteV1 {
    ContextAllocationWriteV1 {
        allocation: read.allocation,
        device: read.device,
        byte_extent: read.byte_extent,
    }
}

fn snapshot(journal: &Journal) -> String {
    journal.reset_access_count_for_test_v1();
    format!("{journal:?}")
}

fn storage_identity(journal: &Journal) -> [(usize, usize); 3] {
    [
        (journal.leases.as_ptr() as usize, journal.leases.capacity()),
        (
            journal.free_reads.as_ptr() as usize,
            journal.free_reads.capacity(),
        ),
        (
            journal.readers.as_ptr() as usize,
            journal.readers.capacity(),
        ),
    ]
}

fn assert_reader_invariant(journal: &Journal) {
    assert!(!journal.leases.is_empty());
    assert!(journal.leases.len() <= CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1);
    assert_eq!(journal.readers.len(), journal.allocation_capacity());
    assert_ne!(journal.next_incarnation, 0);
    let mut free = alloc::vec![false; journal.leases.len()];
    for &slot in &journal.free_reads {
        assert!(slot < free.len());
        assert!(!free[slot], "duplicate free slot");
        free[slot] = true;
    }
    let mut counts = alloc::vec![0; journal.readers.len()];
    let mut incarnations = alloc::collections::BTreeSet::new();
    for (slot, lease) in journal.leases.iter().enumerate() {
        assert_eq!(lease.is_none(), free[slot]);
        if let Some(lease) = lease {
            assert_eq!(lease.reference.slot, slot);
            assert_eq!(
                lease.reference.consumer.context_generation,
                journal.context_generation()
            );
            assert!((1..u64::MAX).contains(&lease.reference.consumer.local));
            assert!((1..journal.next_incarnation).contains(&lease.reference.incarnation));
            assert!(incarnations.insert(lease.reference.incarnation));
            assert_eq!(journal.lookup_read(lease.reference), Ok(lease.request));
            counts[lease.request.allocation.slot] += 1;
        }
    }
    assert_eq!(journal.readers, counts);
    assert_eq!(journal.retained_read_count(), incarnations.len());
    assert_eq!(
        journal.remaining_read_slots() + incarnations.len(),
        journal.leases.len()
    );
}

#[test]
fn whole_reader_state_survives_base_lifecycle_and_slot_reuse() {
    let mut journal = Journal::new(7, 4, 8, 8).unwrap();
    let storage = storage_identity(&journal);
    let check = |journal: &Journal| {
        assert_reader_invariant(journal);
        assert_eq!(storage_identity(journal), storage);
    };
    check(&journal);
    let device = ContextJournalDeviceKeyV1 {
        context_generation: 7,
        local: 1,
    };
    let entries = [2, 3, 4].map(|local| ContextAllocationEnrollmentV1 {
        key: ContextAllocationKeyV1 {
            context_generation: 7,
            local,
        },
        device,
        byte_extent: 64,
    });
    let mut output = [None; 3];
    journal.enroll_allocations(&entries, &mut output).unwrap();
    check(&journal);
    let [a, b, c] = output.map(Option::unwrap);
    let read = |allocation, offset| ContextAllocationReadV1 {
        allocation,
        device,
        byte_extent: 64,
        byte_offset: offset,
        byte_len: 8,
        attempt_epoch: 0,
        content_lineage: 0,
    };
    let requests = [read(a, 0), read(a, 8), read(c, 0)];
    let mut leases = [None; 3];
    journal
        .acquire_reads(consumer(10), &requests, &mut leases)
        .unwrap();
    let leases = leases.map(Option::unwrap);
    check(&journal);
    let unused = journal.register_writer(consumer(20)).unwrap();
    check(&journal);
    journal.abort_reserved(unused).unwrap();
    check(&journal);
    for local in 21..24 {
        let writer = journal.register_writer(consumer(local)).unwrap();
        check(&journal);
        let before = snapshot(&journal);
        assert_eq!(
            journal.begin_write(writer, &[member(requests[0])]),
            Err(Error::AllocationBusy)
        );
        assert_eq!(journal.retire_allocations(&[a]), Err(Error::AllocationBusy));
        assert_eq!(snapshot(&journal), before);
        check(&journal);
        let target = ContextAllocationWriteV1 {
            allocation: b,
            device,
            byte_extent: 64,
        };
        journal.begin_write(writer, &[target]).unwrap();
        check(&journal);
        match local {
            21 => journal
                .settle_success(writer, &ContextWriterSuccessEvidenceV1 { writer })
                .unwrap(),
            22 => journal
                .settle_no_effect(writer, &ContextWriterNoEffectEvidenceV1 { writer })
                .unwrap(),
            _ => {
                journal.mark_unknown(writer).unwrap();
                check(&journal);
                journal
                    .dispose_unknown(
                        writer,
                        &ContextWriterDisposalEvidenceV1 {
                            writer,
                            allocations: &[target],
                        },
                    )
                    .unwrap();
            }
        }
        check(&journal);
        for (reference, request) in leases.iter().zip(requests) {
            assert_eq!(journal.lookup_read(*reference), Ok(request));
        }
    }
    let fresh_b = journal
        .enroll_allocation(
            ContextAllocationKeyV1 {
                context_generation: 7,
                local: 40,
            },
            device,
            64,
        )
        .unwrap();
    assert_eq!(fresh_b.slot, b.slot);
    assert_eq!(journal.reader_count(fresh_b), Ok(0));
    check(&journal);
    release(&mut journal, leases[0]);
    check(&journal);
    assert_eq!(journal.reader_count(a), Ok(1));
    release(&mut journal, leases[1]);
    check(&journal);
    let writer = journal.register_writer(consumer(24)).unwrap();
    check(&journal);
    journal.begin_write(writer, &[member(requests[0])]).unwrap();
    check(&journal);
    journal
        .settle_success(writer, &ContextWriterSuccessEvidenceV1 { writer })
        .unwrap();
    check(&journal);
    let state = journal.lookup_allocation(a).unwrap();
    let reference = acquire(
        &mut journal,
        consumer(25),
        ContextAllocationReadV1 {
            attempt_epoch: state.attempt_epoch,
            content_lineage: state.content_lineage,
            ..requests[0]
        },
    );
    check(&journal);
    assert_eq!(reference.slot, leases[1].slot);
    assert_ne!(reference.incarnation, leases[1].incarnation);
    assert_eq!(journal.lookup_read(leases[1]), Err(Error::InvalidReference));
    release(&mut journal, reference);
    check(&journal);
    journal.retire_allocations(&[a]).unwrap();
    check(&journal);
    let fresh_a = journal
        .enroll_allocation(
            ContextAllocationKeyV1 {
                context_generation: 7,
                local: 41,
            },
            device,
            64,
        )
        .unwrap();
    assert_eq!(fresh_a.slot, a.slot);
    assert_eq!(journal.reader_count(fresh_a), Ok(0));
    check(&journal);
    release(&mut journal, leases[2]);
    check(&journal);
    journal.retire_allocations(&[c]).unwrap();
    check(&journal);
}

fn acquire(
    journal: &mut Journal,
    key: ContextWriterKeyV1,
    read: ContextAllocationReadV1,
) -> ContextReadLeaseReferenceV1 {
    let mut output = [None];
    journal.acquire_reads(key, &[read], &mut output).unwrap();
    output[0].unwrap()
}

fn release(journal: &mut Journal, reference: ContextReadLeaseReferenceV1) {
    journal
        .release_reads(
            reference.consumer,
            &[reference],
            &ContextReadQuiescenceEvidenceV1 {
                consumer: reference.consumer,
            },
        )
        .unwrap();
}

#[test]
fn batched_commits_preserve_exact_counts_contents_and_free_stack_order() {
    let (mut journal, requests) = fixture(8);
    let retained = acquire(&mut journal, consumer(19), requests[0]);
    let ranges = [
        ContextAllocationReadV1 {
            byte_offset: 0,
            byte_len: 4,
            ..requests[0]
        },
        ContextAllocationReadV1 {
            byte_offset: 8,
            byte_len: 4,
            ..requests[0]
        },
        ContextAllocationReadV1 {
            byte_offset: 0,
            byte_len: 4,
            ..requests[1]
        },
        ContextAllocationReadV1 {
            byte_offset: 8,
            byte_len: 4,
            ..requests[1]
        },
        requests[2],
    ];
    let storage = storage_identity(&journal);
    let free_before = journal.free_reads.clone();
    let leases_before = journal.leases.clone();
    let readers_before = journal.readers.clone();
    let next_before = journal.next_incarnation;
    journal.reset_access_count_for_test_v1();
    let base_before = format!("{:?}", journal.journal);
    let mut output = [None; 5];
    journal
        .acquire_reads(consumer(20), &ranges, &mut output)
        .unwrap();
    let references = output.map(Option::unwrap);
    let mut expected_leases = leases_before;
    let mut expected_readers = readers_before;
    for (index, (&reference, &request)) in references.iter().zip(&ranges).enumerate() {
        assert_eq!(reference.slot, free_before[free_before.len() - 1 - index]);
        assert_eq!(reference.incarnation, next_before + index as u64);
        assert_eq!(reference.consumer, consumer(20));
        expected_leases[reference.slot] = Some(ReadLeaseV1 { reference, request });
        expected_readers[request.allocation.slot] += 1;
    }
    let mut expected_free = free_before[..free_before.len() - ranges.len()].to_vec();
    assert_eq!(journal.leases, expected_leases);
    assert_eq!(journal.readers, expected_readers);
    assert_eq!(journal.free_reads, expected_free);
    assert_eq!(journal.next_incarnation, next_before + ranges.len() as u64);

    let released = [references[0], references[2], references[4]];
    journal
        .release_reads(
            consumer(20),
            &released,
            &ContextReadQuiescenceEvidenceV1 {
                consumer: consumer(20),
            },
        )
        .unwrap();
    for reference in released {
        let entry = expected_leases[reference.slot].take().unwrap();
        expected_readers[entry.request.allocation.slot] -= 1;
        expected_free.push(reference.slot);
        assert_eq!(journal.lookup_read(reference), Err(Error::InvalidReference));
    }
    assert_eq!(journal.leases, expected_leases);
    assert_eq!(journal.readers, expected_readers);
    assert_eq!(journal.free_reads, expected_free);
    assert_eq!(journal.next_incarnation, next_before + ranges.len() as u64);
    assert_eq!(journal.lookup_read(retained), Ok(requests[0]));
    assert_eq!(journal.lookup_read(references[1]), Ok(ranges[1]));
    assert_eq!(journal.lookup_read(references[3]), Ok(ranges[3]));
    assert_eq!(storage_identity(&journal), storage);
    journal.reset_access_count_for_test_v1();
    assert_eq!(format!("{:?}", journal.journal), base_before);

    let mut reused = [None; 3];
    journal
        .acquire_reads(consumer(21), &ranges[..3], &mut reused)
        .unwrap();
    let mut released_slots = released.map(|reference| reference.slot);
    released_slots.reverse();
    assert_eq!(
        reused.map(|reference| reference.unwrap().slot),
        released_slots
    );
    assert_eq!(storage_identity(&journal), storage);
}

#[test]
fn last_admissible_incarnation_batch_never_reopens_after_release() {
    let (mut journal, requests) = fixture(3);
    journal.next_incarnation = u64::MAX - 3;
    let storage = storage_identity(&journal);
    let mut output = [None; 3];
    journal
        .acquire_reads(consumer(20), &requests, &mut output)
        .unwrap();
    let references = output.map(Option::unwrap);
    assert_eq!(
        references.map(|reference| reference.incarnation),
        [u64::MAX - 3, u64::MAX - 2, u64::MAX - 1]
    );
    assert_eq!(journal.next_incarnation, u64::MAX);
    journal
        .release_reads(
            consumer(20),
            &references,
            &ContextReadQuiescenceEvidenceV1 {
                consumer: consumer(20),
            },
        )
        .unwrap();
    assert_eq!(journal.next_incarnation, u64::MAX);
    assert_eq!(journal.readers, [0; 4]);
    let before = snapshot(&journal);
    let mut retry = [None];
    assert_eq!(
        journal.acquire_reads(consumer(21), &requests[..1], &mut retry),
        Err(Error::EpochExhausted)
    );
    assert_eq!(retry, [None]);
    assert_eq!(snapshot(&journal), before);
    assert_eq!(storage_identity(&journal), storage);
}

#[test]
fn overlapping_readers_block_every_writer_and_retirement_boundary_until_last_release() {
    let (mut journal, requests) = fixture(4);
    let storage = storage_identity(&journal);
    let first = acquire(&mut journal, consumer(20), requests[0]);
    let second = acquire(&mut journal, consumer(10), requests[0]);
    assert_eq!(journal.reader_count(requests[0].allocation), Ok(2));
    assert_eq!(
        journal
            .lookup_allocation(requests[0].allocation)
            .unwrap()
            .content_lineage,
        0
    );
    let writer = journal.register_writer(consumer(30)).unwrap();
    let before = snapshot(&journal);
    assert_eq!(
        journal.begin_write(writer, &[member(requests[0])]),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        journal.validate_allocation_retirement(&[requests[0].allocation]),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        journal.retire_allocations(&[requests[1].allocation, requests[0].allocation]),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        journal.validate_unknown_disposal(writer, &[member(requests[0])]),
        Err(Error::AllocationBusy)
    );
    assert_eq!(
        journal.dispose_unknown(
            writer,
            &ContextWriterDisposalEvidenceV1 {
                writer,
                allocations: &[member(requests[0])]
            }
        ),
        Err(Error::AllocationBusy)
    );
    assert_eq!(snapshot(&journal), before);
    release(&mut journal, second);
    assert_eq!(
        journal.begin_write(writer, &[member(requests[0])]),
        Err(Error::AllocationBusy)
    );
    release(&mut journal, first);
    journal.begin_write(writer, &[member(requests[0])]).unwrap();
    assert_eq!(journal.retained_read_count(), 0);
    assert_eq!(storage_identity(&journal), storage);
}

#[test]
fn acquisition_preflight_is_atomic_for_every_roster_position() {
    for index in 0..3 {
        for fault in 0..8 {
            let (mut journal, mut requests) = fixture(4);
            match fault {
                0 => requests[index].allocation.key.context_generation += 1,
                1 => requests[index].allocation.key.local += 20,
                2 => requests[index].device.local += 1,
                3 => requests[index].byte_extent += 1,
                4 => requests[index].byte_len = 0,
                5 => requests[index].byte_offset = u64::MAX,
                6 => requests[index].attempt_epoch += 1,
                _ => requests[index].content_lineage += 1,
            }
            let before = snapshot(&journal);
            let storage = storage_identity(&journal);
            let mut output = [None; 3];
            let expected = match fault {
                0 | 1 => Error::InvalidAllocationReference,
                2 => Error::AllocationDeviceMismatch,
                3 => Error::AllocationExtentMismatch,
                4 | 5 => Error::InvalidExtent,
                _ => Error::InvalidState,
            };
            assert_eq!(
                journal.acquire_reads(consumer(20), &requests, &mut output),
                Err(expected)
            );
            assert_eq!(output, [None; 3]);
            assert_eq!(snapshot(&journal), before);
            assert_eq!(storage_identity(&journal), storage);
        }
    }
}

#[test]
fn capacity_canonicality_consumer_and_incarnation_rejections_preserve_all_state() {
    for fault in 0..8 {
        let (mut journal, mut requests) = fixture(2);
        let mut key = consumer(20);
        let mut output = [None; 2];
        match fault {
            0 => {
                acquire(&mut journal, consumer(10), requests[2]);
            }
            1 => requests.swap(0, 1),
            2 => requests[1] = requests[0],
            3 => key.context_generation += 1,
            4 => key.local = 0,
            5 => key.local = u64::MAX,
            6 => journal.next_incarnation = u64::MAX,
            _ => {
                output[0] = Some(ContextReadLeaseReferenceV1 {
                    slot: 0,
                    incarnation: 1,
                    consumer: key,
                })
            }
        }
        let original = output;
        let before = snapshot(&journal);
        let expected = match fault {
            0 => Error::MemberCapacity,
            1 | 2 => Error::NonCanonicalRoster,
            3 => Error::ForeignContext,
            4 | 5 => Error::InvalidWriterId,
            6 => Error::EpochExhausted,
            _ => Error::InvalidState,
        };
        assert_eq!(
            journal.acquire_reads(key, &requests[..2], &mut output),
            Err(expected)
        );
        assert_eq!(snapshot(&journal), before);
        assert_eq!(output, original);
    }
}

#[test]
fn lease_incarnation_rejects_stale_release_after_identical_reacquisition() {
    let (mut journal, requests) = fixture(1);
    let first = acquire(&mut journal, consumer(20), requests[0]);
    release(&mut journal, first);
    let second = acquire(&mut journal, consumer(20), requests[0]);
    assert_eq!(first.slot, second.slot);
    assert_ne!(first.incarnation, second.incarnation);
    let before = snapshot(&journal);
    assert_eq!(journal.lookup_read(first), Err(Error::InvalidReference));
    assert_eq!(
        journal.release_reads(
            first.consumer,
            &[first],
            &ContextReadQuiescenceEvidenceV1 {
                consumer: first.consumer
            }
        ),
        Err(Error::InvalidReference)
    );
    assert_eq!(snapshot(&journal), before);
    assert_eq!(journal.lookup_read(second), Ok(requests[0]));
    release(&mut journal, second);
    journal
        .retire_allocations(&[requests[0].allocation])
        .unwrap();
    let fresh = journal
        .enroll_allocation(
            ContextAllocationKeyV1 {
                context_generation: 7,
                local: 40,
            },
            requests[0].device,
            64,
        )
        .unwrap();
    assert_eq!(fresh.slot, requests[0].allocation.slot);
    let mut output = [None];
    assert!(
        journal
            .acquire_reads(consumer(41), &[requests[0]], &mut output)
            .is_err()
    );
    assert_eq!(journal.reader_count(fresh), Ok(0));
}

#[test]
fn exact_version_snapshot_allows_no_effect_epoch_gap_but_not_pending_or_unknown() {
    for unknown in [false, true] {
        let (mut journal, mut requests) = fixture(2);
        let writer = journal.register_writer(consumer(20)).unwrap();
        journal.begin_write(writer, &[member(requests[0])]).unwrap();
        if unknown {
            journal.mark_unknown(writer).unwrap();
        }
        let before = snapshot(&journal);
        assert_eq!(
            journal.acquire_reads(consumer(30), &requests[..1], &mut [None]),
            Err(Error::AllocationBusy)
        );
        assert_eq!(snapshot(&journal), before);
        if !unknown {
            journal
                .settle_no_effect(writer, &ContextWriterNoEffectEvidenceV1 { writer })
                .unwrap();
            assert!(
                journal
                    .acquire_reads(consumer(30), &requests[..1], &mut [None])
                    .is_err()
            );
            requests[0].attempt_epoch = 1;
            let reference = acquire(&mut journal, consumer(30), requests[0]);
            assert_eq!(journal.lookup_read(reference).unwrap().content_lineage, 0);
            release(&mut journal, reference);
        }
    }
}

#[test]
fn release_roster_is_atomic_and_binds_exact_consumer_and_each_lease() {
    for fault in 0..7 {
        let (mut journal, requests) = fixture(4);
        let mut output = [None; 3];
        journal
            .acquire_reads(consumer(20), &requests, &mut output)
            .unwrap();
        let mut references = output.map(Option::unwrap);
        let mut evidence = ContextReadQuiescenceEvidenceV1 {
            consumer: consumer(20),
        };
        match fault {
            0..=2 => references[fault].incarnation += 100,
            3 => references[1] = references[0],
            4 => references.swap(0, 2),
            5 => references[1].consumer.kind = ContextWriterKindV1::Synchronous,
            _ => evidence.consumer.local += 1,
        }
        let before = snapshot(&journal);
        let expected = match fault {
            0..=2 | 5 => Error::InvalidReference,
            3 | 4 => Error::NonCanonicalRoster,
            _ => Error::SettlementEvidenceMismatch,
        };
        assert_eq!(
            journal.release_reads(consumer(20), &references, &evidence),
            Err(expected)
        );
        assert_eq!(snapshot(&journal), before);
        journal
            .release_reads(
                consumer(20),
                &output.map(Option::unwrap),
                &ContextReadQuiescenceEvidenceV1 {
                    consumer: consumer(20),
                },
            )
            .unwrap();
        assert_eq!(journal.retained_read_count(), 0);
    }
}

#[test]
fn repeated_multireader_trace_matches_independent_counts_without_storage_growth() {
    let (mut journal, requests) = fixture(8);
    let storage = storage_identity(&journal);
    let mut expected = [0; 3];
    let mut held = Vec::new();
    let mut random = 17u64;
    for step in 0..4000 {
        random = random.wrapping_mul(6364136223846793005).wrapping_add(1);
        let index = (random as usize) % 3;
        if held.len() < 8 && (held.is_empty() || random & 4 == 0) {
            let reference = acquire(&mut journal, consumer(20 + step), requests[index]);
            held.push((index, reference));
            expected[index] += 1;
        } else {
            let at = random as usize % held.len();
            let (index, reference) = held.swap_remove(at);
            release(&mut journal, reference);
            expected[index] -= 1;
        }
        for index in 0..3 {
            assert_eq!(
                journal.reader_count(requests[index].allocation),
                Ok(expected[index])
            );
        }
        assert_eq!(journal.retained_read_count(), held.len());
        for &(index, reference) in &held {
            assert_eq!(journal.lookup_read(reference), Ok(requests[index]));
        }
        for slot in 0..journal.leases.len() {
            let free = journal
                .free_reads
                .iter()
                .filter(|&&entry| entry == slot)
                .count();
            let occupied = held
                .iter()
                .filter(|(_, reference)| reference.slot == slot)
                .count();
            assert_eq!(free + occupied, 1);
            assert_eq!(journal.leases[slot].is_some(), occupied == 1);
        }
        assert_reader_invariant(&journal);
        assert_eq!(storage_identity(&journal), storage);
    }
}

#[test]
fn same_allocation_range_batches_and_reacquisitions_have_exact_counts_and_release_order() {
    let (mut journal, requests) = fixture(8);
    let mut ranges = [requests[0]; 3];
    ranges[1].byte_offset = 12;
    ranges[2].byte_offset = 32;
    let key = consumer(20);
    let mut output = [None; 3];
    journal.acquire_reads(key, &ranges, &mut output).unwrap();
    let first = output.map(Option::unwrap);
    let repeated = acquire(&mut journal, key, ranges[0]);
    assert_eq!(journal.reader_count(ranges[0].allocation), Ok(4));
    for (reference, request) in first.into_iter().zip(ranges) {
        assert_eq!(journal.lookup_read(reference), Ok(request));
    }
    let writer = journal.register_writer(consumer(30)).unwrap();
    let before = snapshot(&journal);
    let evidence = ContextReadQuiescenceEvidenceV1 { consumer: key };
    assert_eq!(
        journal.release_reads(key, &[repeated, first[0]], &evidence),
        Err(Error::NonCanonicalRoster)
    );
    assert_eq!(snapshot(&journal), before);
    journal
        .release_reads(key, &[first[0], repeated, first[1]], &evidence)
        .unwrap();
    assert_eq!(journal.reader_count(ranges[0].allocation), Ok(1));
    assert_eq!(journal.lookup_read(first[2]), Ok(ranges[2]));
    assert_eq!(
        journal.begin_write(writer, &[member(ranges[0])]),
        Err(Error::AllocationBusy)
    );
    assert_eq!(journal.lookup_read(repeated), Err(Error::InvalidReference));
    release(&mut journal, first[2]);
    journal.begin_write(writer, &[member(ranges[0])]).unwrap();
}

#[test]
fn success_then_no_effect_requires_the_exact_epoch_and_nonzero_lineage_snapshot() {
    let (mut journal, requests) = fixture(2);
    let writer = journal.register_writer(consumer(20)).unwrap();
    journal.begin_write(writer, &[member(requests[0])]).unwrap();
    journal
        .settle_success(writer, &ContextWriterSuccessEvidenceV1 { writer })
        .unwrap();
    let current = ContextAllocationReadV1 {
        attempt_epoch: 1,
        content_lineage: 1,
        ..requests[0]
    };
    let lease = acquire(&mut journal, consumer(25), current);
    release(&mut journal, lease);
    let writer = journal.register_writer(consumer(30)).unwrap();
    journal.begin_write(writer, &[member(requests[0])]).unwrap();
    journal
        .settle_no_effect(writer, &ContextWriterNoEffectEvidenceV1 { writer })
        .unwrap();
    assert_eq!(
        journal.validate_read(&requests[0]),
        Err(Error::InvalidState)
    );
    assert_eq!(journal.validate_read(&current), Err(Error::InvalidState));
    let current = ContextAllocationReadV1 {
        attempt_epoch: 2,
        ..current
    };
    let lease = acquire(&mut journal, consumer(35), current);
    assert_eq!(journal.lookup_read(lease), Ok(current));
    release(&mut journal, lease);
}

#[test]
fn constructor_contents_and_reader_capacity_error_priority_are_exact() {
    let journal = Journal::new(7, 3, 5, 8).unwrap();
    assert_eq!(journal.leases, [None; 8]);
    assert_eq!(journal.free_reads, [7, 6, 5, 4, 3, 2, 1, 0]);
    assert_eq!(journal.readers, [0; 3]);
    assert_eq!(journal.next_incarnation, 1);
    assert_eq!(journal.context_generation(), 7);
    for reads in [0, CONTEXT_VERSION_JOURNAL_MAX_ENTRIES_V1 + 1] {
        assert_eq!(
            Journal::new(0, 0, 0, reads).unwrap_err(),
            Error::InvalidCapacity
        );
    }
    assert_eq!(
        Journal::new(0, 0, 0, 1).unwrap_err(),
        Error::InvalidContextGeneration
    );
    assert_eq!(
        Journal::new(7, 0, 0, 1).unwrap_err(),
        Error::InvalidCapacity
    );
}

#[test]
fn acquisition_simultaneous_faults_follow_header_then_member_order() {
    for fault in 0..8 {
        let (mut journal, mut requests) = fixture(1);
        let mut key = consumer(20);
        let mut output = [None; 2];
        let occupied = ContextReadLeaseReferenceV1 {
            slot: 0,
            incarnation: 1,
            consumer: key,
        };
        requests[0].device.local += 1;
        requests[1].allocation.key.local += 20;
        journal.next_incarnation = 0;
        let (count, output_count, expected) = match fault {
            0 => {
                key.context_generation += 1;
                key.local = 0;
                (0, 1, Error::ForeignContext)
            }
            1 => {
                key.local = 0;
                (0, 1, Error::InvalidWriterId)
            }
            2 => {
                output[0] = Some(occupied);
                (0, 1, Error::RosterCapacity)
            }
            3 => {
                output[0] = Some(occupied);
                (2, 1, Error::RosterCapacity)
            }
            4 => {
                output[0] = Some(occupied);
                (2, 2, Error::InvalidState)
            }
            5 => (2, 2, Error::MemberCapacity),
            6 => (1, 1, Error::EpochExhausted),
            _ => {
                journal.next_incarnation = 1;
                // The earlier device fault must precede a later allocation fault.
                journal.free_reads.push(usize::MAX);
                (2, 2, Error::AllocationDeviceMismatch)
            }
        };
        let before = snapshot(&journal);
        let original = output;
        let storage = storage_identity(&journal);
        assert_eq!(
            journal.acquire_reads(key, &requests[..count], &mut output[..output_count]),
            Err(expected)
        );
        assert_eq!(snapshot(&journal), before);
        assert_eq!(output, original);
        assert_eq!(storage_identity(&journal), storage);
    }
}

#[test]
fn read_validation_simultaneous_faults_preserve_exact_precedence() {
    for fault in 0..6 {
        let (mut journal, requests) = fixture(2);
        let writer = journal.register_writer(consumer(20)).unwrap();
        journal.begin_write(writer, &[member(requests[0])]).unwrap();
        let mut request = requests[0];
        request.content_lineage = 99;
        request.attempt_epoch = 99;
        let expected = match fault {
            0 => {
                request.allocation.key.local += 99;
                request.device.local += 1;
                Error::InvalidAllocationReference
            }
            1 => {
                request.device.local += 1;
                request.byte_extent += 1;
                Error::AllocationDeviceMismatch
            }
            2 => {
                request.byte_extent += 1;
                request.byte_len = 0;
                Error::AllocationExtentMismatch
            }
            3 => {
                request.byte_len = 0;
                Error::InvalidExtent
            }
            4 => {
                request.byte_offset = u64::MAX;
                Error::InvalidExtent
            }
            _ => Error::AllocationBusy,
        };
        let before = snapshot(&journal);
        assert_eq!(journal.validate_read(&request), Err(expected));
        assert_eq!(snapshot(&journal), before);
    }
}

#[test]
fn acquisition_group_counts_and_selected_slots_are_checked_before_commit() {
    for fault in 0..5 {
        let (mut journal, requests) = fixture(4);
        let mut ranges = [requests[0]; 2];
        ranges[1].byte_offset += 1;
        match fault {
            0 => journal.readers[requests[0].allocation.slot] = usize::MAX,
            1 => journal.readers[requests[0].allocation.slot] = 3,
            2 => *journal.free_reads.last_mut().unwrap() = usize::MAX,
            3 => {
                let slot = journal.free_reads[0];
                journal.leases[slot] = Some(ReadLeaseV1 {
                    reference: ContextReadLeaseReferenceV1 {
                        slot,
                        incarnation: 1,
                        consumer: consumer(10),
                    },
                    request: requests[2],
                });
                *journal.free_reads.last_mut().unwrap() = slot;
            }
            _ => {
                ranges[1] = ranges[0];
                ranges[1].device.local += 1;
            }
        }
        let before = snapshot(&journal);
        let storage = storage_identity(&journal);
        let mut output = [None; 2];
        assert_eq!(
            journal.acquire_reads(consumer(20), &ranges, &mut output),
            Err(if fault == 4 {
                Error::AllocationDeviceMismatch
            } else {
                Error::InvalidState
            })
        );
        assert_eq!(snapshot(&journal), before);
        assert_eq!(output, [None; 2]);
        assert_eq!(storage_identity(&journal), storage);
    }
}

#[test]
fn release_simultaneous_faults_follow_evidence_roster_headroom_then_member_order() {
    for fault in 0..9 {
        let (mut journal, requests) = fixture(4);
        let first = acquire(&mut journal, consumer(20), requests[0]);
        let second = acquire(&mut journal, consumer(20), requests[1]);
        let mut references = [first, second];
        let mut evidence = ContextReadQuiescenceEvidenceV1 {
            consumer: consumer(20),
        };
        let (count, expected) = match fault {
            0 => {
                evidence.consumer.local += 1;
                (0, Error::SettlementEvidenceMismatch)
            }
            1 => (0, Error::RosterCapacity),
            2 => {
                journal.free_reads.push(usize::MAX);
                references[0].consumer.local += 1;
                (2, Error::InvalidState)
            }
            3 => {
                references[0].consumer.local += 1;
                references[0].slot = usize::MAX;
                (2, Error::InvalidReference)
            }
            4 => {
                journal.leases[first.slot]
                    .as_mut()
                    .unwrap()
                    .request
                    .content_lineage += 1;
                references[1].incarnation += 1;
                (2, Error::InvalidState)
            }
            5 => {
                references[1] = first;
                journal.readers[requests[0].allocation.slot] = 1;
                (2, Error::NonCanonicalRoster)
            }
            6 => {
                journal.readers[requests[1].allocation.slot] = 0;
                (2, Error::InvalidState)
            }
            7 => {
                let request = &mut journal.leases[second.slot].as_mut().unwrap().request;
                *request = requests[0];
                request.byte_offset += 1;
                (2, Error::InvalidState)
            }
            _ => {
                journal.free_reads = Vec::new();
                references[0].incarnation += 1;
                (2, Error::InvalidState)
            }
        };
        let before = snapshot(&journal);
        let storage = storage_identity(&journal);
        assert_eq!(
            journal.release_reads(consumer(20), &references[..count], &evidence),
            Err(expected)
        );
        assert_eq!(snapshot(&journal), before);
        assert_eq!(storage_identity(&journal), storage);
    }
}
