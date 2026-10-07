use super::*;

mod performance;

struct Case {
    owner: ContextProducerReadJournalV1,
    requests: Vec<ContextProducerReadV1>,
    output: Vec<Option<ContextProducerReadReferenceV1>>,
    consumer: ContextWriterKeyV1,
    retained: ContextProducerReadReferenceV1,
    stable: ContextReadLeaseReferenceV1,
}

fn setup(k: usize, capacity: usize, distinct: bool, fault: &str) -> Case {
    assert!(k >= 2 && capacity >= k + 2);
    let allocations = if distinct { k } else { 1 };
    let mut owner = ContextProducerReadJournalV1::new(7, allocations + 1, 2, capacity).unwrap();
    let device = ContextJournalDeviceKeyV1 {
        context_generation: 7,
        local: 1,
    };
    let extent = 2 * k as u64 + 64;
    let mut sources = Vec::new();
    for id in (1..=allocations).rev() {
        sources.push(
            owner
                .enroll_allocation(
                    ContextAllocationKeyV1 {
                        context_generation: 7,
                        local: id as u64,
                    },
                    device,
                    extent,
                )
                .unwrap(),
        );
    }
    sources.sort_unstable_by_key(|reference| reference.key.local);
    let other = owner
        .enroll_allocation(
            ContextAllocationKeyV1 {
                context_generation: 7,
                local: allocations as u64 + 1,
            },
            device,
            extent,
        )
        .unwrap();
    let producer = owner.register_writer(key(10)).unwrap();
    let writes: Vec<_> = sources
        .iter()
        .map(|&allocation| ContextAllocationWriteV1 {
            allocation,
            device,
            byte_extent: extent,
        })
        .collect();
    owner.begin_write(producer, &writes).unwrap();
    let mut requests: Vec<_> = (0..k)
        .map(|index| ContextProducerReadV1 {
            producer,
            read: ContextAllocationReadV1 {
                allocation: sources[if distinct { index } else { 0 }],
                device,
                byte_extent: extent,
                byte_offset: 2 * index as u64,
                byte_len: 3,
                attempt_epoch: 1,
                content_lineage: 0,
            },
        })
        .collect();
    let mut retained = [None];
    owner
        .acquire_producer_reads(key(15), &requests[..1], &mut retained)
        .unwrap();
    let mut stable = [None];
    owner
        .acquire_reads(
            key(16),
            &[ContextAllocationReadV1 {
                allocation: other,
                device,
                byte_extent: extent,
                byte_offset: 0,
                byte_len: 4,
                attempt_epoch: 0,
                content_lineage: 0,
            }],
            &mut stable,
        )
        .unwrap();
    let retained = retained[0].unwrap();
    let stable = stable[0].unwrap();
    assert_eq!(retained.slot, stable.slot);
    let mut output = alloc::vec![None; k];
    let last_free = owner.free.len() - k;
    match fault {
        "none" => {}
        "allocation" => requests[k - 1].read.allocation.slot = usize::MAX,
        "device" => requests[k - 1].read.device.context_generation += 1,
        "producer" => requests[k - 1].producer.key.local += 1,
        "canonical" => requests[k - 1] = requests[k - 2],
        "count" => {
            owner.counts[requests[k - 1].read.allocation.slot] = if distinct {
                usize::MAX
            } else {
                capacity - k + 1
            }
        }
        "free" => owner.free[last_free] = usize::MAX,
        "occupied" => owner.free[last_free] = retained.slot,
        "alias" => owner.free[last_free] = owner.free[last_free + 1],
        "output" => output[k - 1] = Some(retained),
        "epoch" => owner.next_incarnation = 0,
        "budget" => owner.free.truncate(k),
        _ => unreachable!(),
    }
    Case {
        owner,
        requests,
        output,
        consumer: key(20),
        retained,
        stable,
    }
}

fn expected(k: usize, fault: &str) -> (Result<(), Error>, usize) {
    match fault {
        "none" | "alias" => (Ok(()), 3 * k),
        "allocation" => (Err(Error::InvalidAllocationReference), 3 * (k - 1) + 1),
        "device" => (Err(Error::AllocationDeviceMismatch), 3 * (k - 1) + 2),
        "producer" => (Err(Error::InvalidState), 3 * (k - 1) + 2),
        "canonical" => (Err(Error::NonCanonicalRoster), 3 * k),
        "count" | "free" | "occupied" => (Err(Error::InvalidState), 3 * k),
        "output" => (Err(Error::InvalidState), 0),
        "epoch" => (Err(Error::EpochExhausted), 0),
        "budget" => (Err(Error::MemberCapacity), 0),
        _ => unreachable!(),
    }
}

struct Reset {
    reservations: Vec<(usize, Option<ReservationV1>)>,
    counts: Vec<(usize, usize)>,
    free_prefix: usize,
    free_tail: Vec<usize>,
    incarnation: u64,
    output: Vec<Option<ContextProducerReadReferenceV1>>,
}

impl Reset {
    fn new(case: &Case) -> Self {
        let owner = &case.owner;
        let free_prefix = owner.free.len().saturating_sub(case.requests.len());
        let free_tail = owner.free[free_prefix..].to_vec();
        let reservations = free_tail
            .iter()
            .filter_map(|&slot| owner.reservations.get(slot).map(|&entry| (slot, entry)))
            .collect();
        let counts = case
            .requests
            .iter()
            .filter_map(|request| {
                let slot = request.read.allocation.slot;
                owner.counts.get(slot).map(|&count| (slot, count))
            })
            .collect();
        Self {
            reservations,
            counts,
            free_prefix,
            free_tail,
            incarnation: owner.next_incarnation,
            output: case.output.clone(),
        }
    }

    fn restore(&self, case: &mut Case) {
        for &(slot, entry) in &self.reservations {
            case.owner.reservations[slot] = entry;
        }
        for &(slot, count) in &self.counts {
            case.owner.counts[slot] = count;
        }
        case.owner.free.truncate(self.free_prefix);
        case.owner.free.extend_from_slice(&self.free_tail);
        case.owner.next_incarnation = self.incarnation;
        case.output.copy_from_slice(&self.output);
        case.owner.reset_access_count_for_test_v1();
    }
}

fn run(case: &mut Case, candidate: bool) -> Result<(), Error> {
    if candidate {
        case.owner
            .acquire_producer_reads(case.consumer, &case.requests, &mut case.output)
    } else {
        case.owner.baseline_acquire_producer_reads_v1(
            case.consumer,
            &case.requests,
            &mut case.output,
        )
    }
}

fn qualify(case: &mut Case, fault: &str) {
    let reset = Reset::new(case);
    let before = snapshot(&case.owner);
    let storage = case.owner.guard_owner_storage_v1();
    let output_storage = (case.output.as_ptr(), case.output.capacity());
    let wanted = expected(case.requests.len(), fault);
    let mut frozen = None;
    for candidate in [false, true] {
        assert_eq!(run(case, candidate), wanted.0);
        assert_eq!(case.owner.guard_accesses_for_test_v1(), wanted.1);
        let after = snapshot(&case.owner);
        if wanted.0.is_err() {
            assert_eq!(after, before);
            assert_eq!(case.output, reset.output);
        }
        if candidate {
            let (state, output) = frozen.as_ref().unwrap();
            assert_eq!(&after, state);
            assert_eq!(&case.output, output);
        } else {
            frozen = Some((after, case.output.clone()));
        }
        assert!(case.owner.lookup_producer_read(case.retained).is_ok());
        assert!(case.owner.lookup_read(case.stable).is_ok());
        assert_eq!(case.owner.guard_owner_storage_v1(), storage);
        assert_eq!(
            (case.output.as_ptr(), case.output.capacity()),
            output_storage
        );
        reset.restore(case);
        assert_eq!(snapshot(&case.owner), before);
        assert_eq!(case.output, reset.output);
        assert_eq!(case.owner.guard_owner_storage_v1(), storage);
    }
}

#[test]
fn shared_admission_matches_frozen_state_output_storage_custody_and_accesses() {
    for k in [2, 8, 64] {
        for capacity in [k + 2, 1024] {
            for distinct in [false, true] {
                for fault in [
                    "none",
                    "allocation",
                    "device",
                    "producer",
                    "canonical",
                    "count",
                    "free",
                    "occupied",
                    "alias",
                    "output",
                    "epoch",
                    "budget",
                ] {
                    qualify(&mut setup(k, capacity, distinct, fault), fault);
                }
            }
        }
    }
}

#[test]
fn shared_admission_duplicate_destination_keeps_sequential_overwrite_semantics() {
    let mut case = setup(2, 4, false, "alias");
    let incarnation = case.owner.next_incarnation;
    let allocation = case.requests[0].read.allocation.slot;
    assert_eq!(run(&mut case, true), Ok(()));
    let first = case.output[0].unwrap();
    let second = case.output[1].unwrap();
    assert_eq!(first.slot, second.slot);
    assert_eq!(
        (first.incarnation, second.incarnation),
        (incarnation, incarnation + 1)
    );
    assert_eq!(
        case.owner.lookup_producer_read(first),
        Err(Error::InvalidReference)
    );
    assert_eq!(
        case.owner.lookup_producer_read(second),
        Ok(case.requests[1])
    );
    assert_eq!(case.owner.counts[allocation], 3);
    assert_eq!(case.owner.next_incarnation, incarnation + 2);
}

#[test]
fn shared_capacity_matches_frozen_combined_budget_and_epoch_boundaries() {
    let mut case = setup(2, 8, false, "none");
    for incarnation in [0, 1, u64::MAX - 1, u64::MAX] {
        case.owner.next_incarnation = incarnation;
        assert_eq!(case.owner.retained_producer_read_count(), 1);
        assert_eq!(
            case.owner.retained_producer_read_count(),
            case.owner.baseline_retained_producer_read_count_v1()
        );
        assert_eq!(case.owner.retained_read_count(), 2);
        assert_eq!(
            case.owner.retained_read_count(),
            case.owner.baseline_retained_read_count_v1()
        );
        assert_eq!(case.owner.remaining_read_slots(), 6);
        assert_eq!(
            case.owner.remaining_read_slots(),
            case.owner.baseline_remaining_read_slots_v1()
        );
        for count in [0, 1, 6, 7, usize::MAX] {
            let before = snapshot(&case.owner);
            let expected = if count > 6 {
                Err(Error::MemberCapacity)
            } else if incarnation == 0 || incarnation.checked_add(count as u64).is_none() {
                Err(Error::EpochExhausted)
            } else {
                Ok(())
            };
            assert_eq!(case.owner.validate_producer_read_capacity(count), expected);
            assert_eq!(
                case.owner
                    .baseline_validate_producer_read_capacity_v1(count),
                expected
            );
            assert_eq!(
                case.owner.validate_read_capacity(count),
                if count > 6 {
                    Err(Error::MemberCapacity)
                } else {
                    Ok(())
                }
            );
            assert_eq!(
                case.owner.validate_read_capacity(count),
                case.owner.baseline_validate_read_capacity_v1(count)
            );
            assert_eq!(case.owner.guard_accesses_for_test_v1(), 0);
            assert_eq!(snapshot(&case.owner), before);
        }
    }
    case.owner.reservations.truncate(4);
    case.owner.free = alloc::vec![3, 2, 1];
    case.owner.next_incarnation = 2;
    assert_eq!(case.owner.remaining_read_slots(), 2);
    assert_eq!(
        case.owner.remaining_read_slots(),
        case.owner.baseline_remaining_read_slots_v1()
    );
    for count in [2, 3] {
        let expected = if count == 2 {
            Ok(())
        } else {
            Err(Error::MemberCapacity)
        };
        assert_eq!(case.owner.validate_producer_read_capacity(count), expected);
        assert_eq!(
            case.owner
                .baseline_validate_producer_read_capacity_v1(count),
            expected
        );
    }
}

#[test]
fn shared_admission_prefix_errors_precede_malformed_budget_and_missing_counts() {
    for fault in 0..7 {
        let mut case = setup(2, 4, false, "none");
        case.owner.free.push(0);
        case.owner.free.push(0);
        case.owner.counts.clear();
        let error = match fault {
            0 => {
                case.consumer.context_generation += 1;
                Error::ForeignContext
            }
            1 => {
                case.consumer.local = 0;
                Error::InvalidWriterId
            }
            2 => {
                case.consumer.kind = ContextWriterKindV1::Synchronous;
                Error::InvalidWriterId
            }
            3 => {
                case.requests.clear();
                Error::RosterCapacity
            }
            4 => {
                case.output.pop();
                Error::RosterCapacity
            }
            5 => {
                case.output[1] = Some(case.retained);
                Error::InvalidState
            }
            6 => {
                case.consumer.local = u64::MAX;
                Error::InvalidWriterId
            }
            _ => unreachable!(),
        };
        let before = snapshot(&case.owner);
        let output = case.output.clone();
        for candidate in [false, true] {
            assert_eq!(run(&mut case, candidate), Err(error));
            assert_eq!(case.owner.guard_accesses_for_test_v1(), 0);
            assert_eq!(snapshot(&case.owner), before);
            assert_eq!(case.output, output);
        }
    }
    for allocation_error in [false, true] {
        let mut case = setup(2, 4, false, "none");
        case.owner.counts.clear();
        let (error, accesses) = if allocation_error {
            case.requests[0].read.allocation.slot = usize::MAX;
            (Error::InvalidAllocationReference, 1)
        } else {
            case.requests[0].read.device.local += 1;
            (Error::AllocationDeviceMismatch, 2)
        };
        let before = snapshot(&case.owner);
        for candidate in [false, true] {
            assert_eq!(run(&mut case, candidate), Err(error));
            assert_eq!(case.owner.guard_accesses_for_test_v1(), accesses);
            assert_eq!(snapshot(&case.owner), before);
            assert!(case.output.iter().all(Option::is_none));
        }
    }
}

#[test]
fn shared_admission_status_and_consumer_order_are_exact() {
    for state in 0..5 {
        let mut case = setup(2, 4, false, "none");
        let producer = case.requests[0].producer;
        let (error, accesses) = match state {
            0 => {
                case.owner.mark_unknown(producer).unwrap();
                (Error::AllocationBusy, 3)
            }
            1 => {
                case.owner.stable.query_replace_writer_for_test_v1(
                    producer,
                    Some(ContextWriterStateV1::Reserved),
                );
                (Error::InvalidState, 3)
            }
            2 => {
                case.owner
                    .settle_success(
                        producer,
                        &ContextWriterSuccessEvidenceV1 { writer: producer },
                    )
                    .unwrap();
                (Error::AllocationBusy, 1)
            }
            3 => {
                case.owner
                    .settle_no_effect(
                        producer,
                        &ContextWriterNoEffectEvidenceV1 { writer: producer },
                    )
                    .unwrap();
                (Error::AllocationBusy, 1)
            }
            4 => {
                case.consumer = producer.key;
                (Error::InvalidWriterId, 3)
            }
            _ => unreachable!(),
        };
        let before = snapshot(&case.owner);
        let storage = case.owner.guard_owner_storage_v1();
        for candidate in [false, true] {
            assert_eq!(run(&mut case, candidate), Err(error));
            assert_eq!(case.owner.guard_accesses_for_test_v1(), accesses);
            assert_eq!(snapshot(&case.owner), before);
            assert!(case.output.iter().all(Option::is_none));
            assert_eq!(case.owner.guard_owner_storage_v1(), storage);
        }
    }
}
