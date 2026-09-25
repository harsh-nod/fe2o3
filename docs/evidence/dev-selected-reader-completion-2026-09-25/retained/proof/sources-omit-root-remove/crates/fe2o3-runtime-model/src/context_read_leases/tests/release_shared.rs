use super::*;

mod performance;

#[test]
fn shared_release_preserves_a_malformed_existing_free_prefix() {
    let mut previous = None;
    for candidate in [false, true] {
        let (mut owner, requests) = fixture(4);
        let mut output = [None; 2];
        owner
            .acquire_reads(consumer(20), &requests[..2], &mut output)
            .unwrap();
        let references = output.map(Option::unwrap);
        let retained = owner.leases[references[1].slot];
        owner.free_reads.fill(0);
        let storage = owner.guard_owner_storage_v1();
        let evidence = ContextReadQuiescenceEvidenceV1 {
            consumer: consumer(20),
        };
        let result = if candidate {
            owner.release_reads(consumer(20), &references[..1], &evidence)
        } else {
            owner.baseline_release_reads_v1(consumer(20), &references[..1], &evidence)
        };
        assert_eq!(result, Ok(()));
        assert_eq!(owner.free_reads, [0, 0, 0]);
        assert_eq!(owner.leases[references[1].slot], retained);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
        let state = snapshot(&owner);
        if let Some(prior) = &previous {
            assert_eq!(&state, prior);
        }
        previous = Some(state);
    }
}

#[test]
fn shared_lookup_keeps_full_reference_identity_ahead_of_read_validation() {
    for fault in 0..8 {
        let (mut owner, requests) = fixture(4);
        let mut output = [None];
        owner
            .acquire_reads(consumer(20), &requests[..1], &mut output)
            .unwrap();
        let mut reference = output[0].unwrap();
        owner.leases[reference.slot]
            .as_mut()
            .unwrap()
            .request
            .device
            .local += 1;
        match fault {
            0 => {}
            1 => reference.slot = usize::MAX,
            2 => reference.slot = 3,
            3 => reference.incarnation += 1,
            4 => reference.consumer.context_generation += 1,
            5 => reference.consumer.local += 1,
            6 => reference.consumer.kind = ContextWriterKindV1::Synchronous,
            _ => {
                owner.leases[reference.slot]
                    .as_mut()
                    .unwrap()
                    .reference
                    .slot += 1
            }
        }
        let expected = if fault == 0 {
            Error::AllocationDeviceMismatch
        } else {
            Error::InvalidReference
        };
        let before = snapshot(&owner);
        assert_eq!(owner.lookup_read(reference), Err(expected));
        assert_eq!(owner.baseline_lookup_read_v1(reference), Err(expected));
        assert_eq!(snapshot(&owner), before);
    }
}

#[test]
fn shared_release_synchronous_subset_preserves_unrelated_custody_and_incarnation() {
    let mut previous = None;
    for candidate in [false, true] {
        let (mut owner, requests) = fixture(4);
        let mut key = consumer(20);
        key.kind = ContextWriterKindV1::Synchronous;
        let mut output = [None; 3];
        owner.acquire_reads(key, &requests, &mut output).unwrap();
        let references = output.map(Option::unwrap);
        let retained = owner.leases[references[1].slot];
        let next = owner.next_incarnation;
        let storage = owner.guard_owner_storage_v1();
        assert_eq!(owner.lookup_read(references[0]), Ok(requests[0]));
        let evidence = ContextReadQuiescenceEvidenceV1 { consumer: key };
        let roster = [references[0], references[2]];
        let result = if candidate {
            owner.release_reads(key, &roster, &evidence)
        } else {
            owner.baseline_release_reads_v1(key, &roster, &evidence)
        };
        assert_eq!(result, Ok(()));
        assert_eq!(owner.leases[references[1].slot], retained);
        assert_eq!(owner.next_incarnation, next);
        assert_eq!(owner.free_reads, [3, 0, 2]);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
        assert_reader_invariant(&owner);
        let state = snapshot(&owner);
        if let Some(prior) = &previous {
            assert_eq!(&state, prior);
        }
        previous = Some(state);
    }
}

#[test]
fn shared_release_late_lookup_error_precedes_missing_count_storage_without_partial_release() {
    for backlink in [false, true] {
        let mut previous = None;
        for candidate in [false, true] {
            let (mut owner, requests) = fixture(4);
            let mut output = [None; 3];
            owner
                .acquire_reads(consumer(20), &requests, &mut output)
                .unwrap();
            let references = output.map(Option::unwrap);
            owner.readers.truncate(requests[2].allocation.slot);
            let expected = if backlink {
                owner.guard_break_backlink_for_test_v1(requests[2].allocation);
                Error::InvalidState
            } else {
                owner.leases[references[2].slot]
                    .as_mut()
                    .unwrap()
                    .request
                    .device
                    .local += 1;
                Error::AllocationDeviceMismatch
            };
            let before = snapshot(&owner);
            let storage = owner.guard_owner_storage_v1();
            let evidence = ContextReadQuiescenceEvidenceV1 {
                consumer: consumer(20),
            };
            let result = if candidate {
                owner.release_reads(consumer(20), &references, &evidence)
            } else {
                owner.baseline_release_reads_v1(consumer(20), &references, &evidence)
            };
            assert_eq!(result, Err(expected));
            assert_eq!(snapshot(&owner), before);
            assert_eq!(owner.guard_owner_storage_v1(), storage);
            if let Some(prior) = &previous {
                assert_eq!(&before, prior);
            }
            previous = Some(before);
        }
    }
}

#[test]
fn shared_release_distinguishes_equal_contents_with_different_physical_headroom() {
    let mut contents = None;
    for capacity in [0, 2] {
        for candidate in [false, true] {
            let (mut owner, requests) = fixture(2);
            let mut output = [None; 2];
            owner
                .acquire_reads(consumer(20), &requests[..2], &mut output)
                .unwrap();
            let references = output.map(Option::unwrap);
            owner.free_reads = alloc::vec![0usize; capacity].into_boxed_slice().into_vec();
            owner.free_reads.clear();
            assert_eq!(owner.free_reads.capacity(), capacity);
            assert_reader_invariant(&owner);
            let before = snapshot(&owner);
            if let Some(prior) = &contents {
                assert_eq!(&before, prior);
            }
            contents = Some(before.clone());
            let storage = owner.guard_owner_storage_v1();
            let evidence = ContextReadQuiescenceEvidenceV1 {
                consumer: consumer(20),
            };
            let result = if candidate {
                owner.release_reads(consumer(20), &references, &evidence)
            } else {
                owner.baseline_release_reads_v1(consumer(20), &references, &evidence)
            };
            assert_eq!(
                result,
                if capacity == 0 {
                    Err(Error::InvalidState)
                } else {
                    Ok(())
                }
            );
            if capacity == 0 {
                assert_eq!(snapshot(&owner), before);
            } else {
                assert_eq!(owner.free_reads, [0, 1]);
                assert_reader_invariant(&owner);
            }
            assert_eq!(owner.guard_owner_storage_v1(), storage);
        }
    }
}

#[test]
fn shared_release_preserves_logical_and_physical_headroom_precedence() {
    for fault in 0..3 {
        for candidate in [false, true] {
            let (mut owner, requests) = fixture(3);
            let mut output = [None; 2];
            owner
                .acquire_reads(consumer(20), &requests[..2], &mut output)
                .unwrap();
            let mut references = output.map(Option::unwrap);
            let capacity = match fault {
                1 => 1,
                2 => 4,
                _ => 3,
            };
            owner.free_reads = alloc::vec![2usize; capacity].into_boxed_slice().into_vec();
            owner.free_reads.truncate(if fault == 2 { 2 } else { 1 });
            if fault != 0 {
                references[0].incarnation += 1;
            }
            assert_eq!(owner.free_reads.capacity(), capacity);
            let before = snapshot(&owner);
            let storage = owner.guard_owner_storage_v1();
            let evidence = ContextReadQuiescenceEvidenceV1 {
                consumer: consumer(20),
            };
            let result = if candidate {
                owner.release_reads(consumer(20), &references, &evidence)
            } else {
                owner.baseline_release_reads_v1(consumer(20), &references, &evidence)
            };
            assert_eq!(
                result,
                if fault == 0 {
                    Ok(())
                } else {
                    Err(Error::InvalidState)
                }
            );
            if fault != 0 {
                assert_eq!(snapshot(&owner), before);
            } else {
                assert_eq!(owner.free_reads, [2, 0, 1]);
                assert_reader_invariant(&owner);
            }
            assert_eq!(owner.guard_owner_storage_v1(), storage);
        }
    }
}
