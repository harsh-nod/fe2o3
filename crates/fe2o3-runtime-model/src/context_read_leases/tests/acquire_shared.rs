use super::*;

mod performance;

#[test]
fn shared_acquire_preserves_capacity_zero_count_and_error_priority() {
    for next in [0, 1, u64::MAX - 1, u64::MAX] {
        for count in [0, 1, 2, 4, 5, usize::MAX] {
            let (mut owner, _) = fixture(4);
            owner.next_incarnation = next;
            let before = snapshot(&owner);
            let expected = if count > 4 {
                Err(Error::MemberCapacity)
            } else if next == 0 || next.checked_add(count as u64).is_none() {
                Err(Error::EpochExhausted)
            } else {
                Ok(())
            };
            assert_eq!(owner.baseline_validate_read_capacity_v1(count), expected);
            assert_eq!(owner.validate_read_capacity(count), expected);
            assert_eq!(snapshot(&owner), before);
        }
    }
}

#[test]
fn shared_acquire_matches_synchronous_length_order_and_alias_overwrites() {
    for alias in [false, true] {
        let mut observed = None;
        for candidate in [false, true] {
            let (mut owner, requests) = fixture(4);
            let mut roster = [requests[0]; 2];
            roster[0].byte_offset = 0;
            roster[0].byte_len = 1;
            roster[1].byte_offset = 0;
            roster[1].byte_len = 2;
            if alias {
                let len = owner.free_reads.len();
                owner.free_reads[len - 2] = owner.free_reads[len - 1];
            }
            let mut consumer = consumer(20);
            consumer.kind = ContextWriterKindV1::Synchronous;
            let storage = owner.guard_owner_storage_v1();
            let mut output = [None; 2];
            let result = if candidate {
                owner.acquire_reads(consumer, &roster, &mut output)
            } else {
                owner.baseline_acquire_reads_v1(consumer, &roster, &mut output)
            };
            assert_eq!(result, Ok(()));
            assert_eq!(owner.guard_owner_storage_v1(), storage);
            assert_eq!(owner.readers[roster[0].allocation.slot], 2);
            assert_eq!(
                output[0].unwrap().consumer.kind,
                ContextWriterKindV1::Synchronous
            );
            if alias {
                assert_eq!(output[0].unwrap().slot, output[1].unwrap().slot);
                assert_eq!(
                    owner.leases[output[1].unwrap().slot].unwrap().reference,
                    output[1].unwrap()
                );
            } else {
                assert_reader_invariant(&owner);
            }
            let state = (snapshot(&owner), output);
            if let Some(before) = &observed {
                assert_eq!(&state, before);
            }
            observed = Some(state);
        }
    }
}

#[test]
fn shared_read_validation_keeps_endpoint_and_lookup_error_precedence() {
    for fault in 0..4 {
        let (mut owner, requests) = fixture(4);
        let mut request = requests[0];
        request.byte_offset = 48;
        request.byte_len = 16;
        let expected = match fault {
            0 => Ok(()),
            1 => {
                request.byte_offset += 1;
                Err(Error::InvalidExtent)
            }
            2 => {
                owner.guard_break_backlink_for_test_v1(request.allocation);
                owner.readers.clear();
                request.device.local += 1;
                request.byte_len = 0;
                Err(Error::InvalidState)
            }
            _ => {
                request.allocation.slot = usize::MAX;
                owner.readers.clear();
                request.byte_len = 0;
                Err(Error::InvalidAllocationReference)
            }
        };
        let before = snapshot(&owner);
        assert_eq!(owner.baseline_validate_read_v1(&request), expected);
        assert_eq!(owner.validate_read(&request), expected);
        assert_eq!(snapshot(&owner), before);
    }
}

#[test]
fn shared_acquire_matches_late_faults_and_preserves_owner_and_output() {
    for fault in 0..8 {
        let mut observed = None;
        for candidate in [false, true] {
            let (mut owner, mut requests) = fixture(4);
            let mut output = [None; 3];
            let expected = match fault {
                0 => {
                    output[2] = Some(ContextReadLeaseReferenceV1 {
                        slot: 0,
                        incarnation: 1,
                        consumer: consumer(20),
                    });
                    Error::InvalidState
                }
                1 => {
                    let index = owner.free_reads.len() - requests.len();
                    owner.free_reads[index] = usize::MAX;
                    Error::InvalidState
                }
                2 => {
                    let slot = owner.free_reads[owner.free_reads.len() - requests.len()];
                    owner.leases[slot] = Some(ReadLeaseV1 {
                        reference: ContextReadLeaseReferenceV1 {
                            slot,
                            incarnation: 1,
                            consumer: consumer(19),
                        },
                        request: requests[2],
                    });
                    Error::InvalidState
                }
                3 | 4 => {
                    requests[1] = requests[0];
                    requests[1].byte_len -= if fault == 3 { 0 } else { 1 };
                    Error::NonCanonicalRoster
                }
                5 => {
                    owner.guard_break_backlink_for_test_v1(requests[0].allocation);
                    owner.readers.clear();
                    requests[0].byte_len = 0;
                    Error::InvalidState
                }
                6 => {
                    requests[0].allocation.slot = usize::MAX;
                    owner.readers.clear();
                    Error::InvalidAllocationReference
                }
                _ => {
                    requests[2].byte_offset = 49;
                    Error::InvalidExtent
                }
            };
            let before = snapshot(&owner);
            let before_output = output;
            let storage = owner.guard_owner_storage_v1();
            let result = if candidate {
                owner.acquire_reads(consumer(20), &requests, &mut output)
            } else {
                owner.baseline_acquire_reads_v1(consumer(20), &requests, &mut output)
            };
            assert_eq!(result, Err(expected), "fault {fault}");
            assert_eq!(snapshot(&owner), before);
            assert_eq!(output, before_output);
            assert_eq!(owner.guard_owner_storage_v1(), storage);
            let state = (before, output, result);
            if let Some(prior) = &observed {
                assert_eq!(&state, prior);
            }
            observed = Some(state);
        }
    }
}

#[test]
fn shared_acquire_orders_allocation_ids_independently_of_slots() {
    let mut observed = None;
    for candidate in [false, true] {
        let mut owner = Journal::new(7, 4, 4, 4).unwrap();
        let device = ContextJournalDeviceKeyV1 {
            context_generation: 7,
            local: 1,
        };
        let mut requests = [9, 2, 7].map(|local| ContextAllocationReadV1 {
            allocation: owner
                .enroll_allocation(
                    ContextAllocationKeyV1 {
                        context_generation: 7,
                        local,
                    },
                    device,
                    64,
                )
                .unwrap(),
            device,
            byte_extent: 64,
            byte_offset: 0,
            byte_len: 16,
            attempt_epoch: 0,
            content_lineage: 0,
        });
        requests.sort_by_key(|request| request.allocation.key.local);
        assert_eq!(requests.map(|request| request.allocation.slot), [1, 2, 0]);
        let mut output = [None; 3];
        let storage = owner.guard_owner_storage_v1();
        let result = if candidate {
            owner.acquire_reads(consumer(20), &requests, &mut output)
        } else {
            owner.baseline_acquire_reads_v1(consumer(20), &requests, &mut output)
        };
        assert_eq!(result, Ok(()));
        assert_reader_invariant(&owner);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
        let state = (snapshot(&owner), output);
        if let Some(prior) = &observed {
            assert_eq!(&state, prior);
        }
        observed = Some(state);
    }
}
