use super::*;
use alloc::{format, string::String};

type Journal = ContextReadLeasedJournalV1;
type Error = ContextVersionJournalErrorV1;

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
            assert!(
                journal
                    .acquire_reads(consumer(20), &requests, &mut output)
                    .is_err()
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
        assert!(
            journal
                .acquire_reads(key, &requests[..2], &mut output)
                .is_err()
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
        assert!(
            journal
                .release_reads(consumer(20), &references, &evidence)
                .is_err()
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
