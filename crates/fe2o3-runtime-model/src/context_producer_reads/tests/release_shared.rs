use super::*;

mod performance;

struct Case {
    owner: ContextProducerReadJournalV1,
    references: Vec<ContextProducerReadReferenceV1>,
    consumer: ContextWriterKeyV1,
    evidence: ContextReadQuiescenceEvidenceV1,
    retained: ContextProducerReadReferenceV1,
    stable: ContextReadLeaseReferenceV1,
}

fn settle(
    owner: &mut ContextProducerReadJournalV1,
    producer: ContextWriterReferenceV1,
    phase: &str,
) {
    match phase {
        "pending" => {}
        "unknown" => owner.mark_unknown(producer).unwrap(),
        "success" => owner
            .settle_success(
                producer,
                &ContextWriterSuccessEvidenceV1 { writer: producer },
            )
            .unwrap(),
        "no_effect" => owner
            .settle_no_effect(
                producer,
                &ContextWriterNoEffectEvidenceV1 { writer: producer },
            )
            .unwrap(),
        _ => unreachable!(),
    }
}

fn setup(k: usize, capacity: usize, distinct: bool, phase: &str, fault: &str) -> Case {
    assert!(k > 0 && capacity >= k + 2);
    let count = if distinct { k } else { 1 };
    let mut owner = ContextProducerReadJournalV1::new(7, count + 1, 2, capacity).unwrap();
    let device = ContextJournalDeviceKeyV1 {
        context_generation: 7,
        local: 1,
    };
    let extent = 2 * k as u64 + 64;
    let allocations: Vec<_> = (0..count + 1)
        .map(|index| {
            owner
                .enroll_allocation(
                    ContextAllocationKeyV1 {
                        context_generation: 7,
                        local: index as u64 + 1,
                    },
                    device,
                    extent,
                )
                .unwrap()
        })
        .collect();
    let producer = owner.register_writer(key(10)).unwrap();
    let writes: Vec<_> = allocations[..count]
        .iter()
        .map(|&allocation| ContextAllocationWriteV1 {
            allocation,
            device,
            byte_extent: extent,
        })
        .collect();
    owner.begin_write(producer, &writes).unwrap();
    let requests: Vec<_> = (0..k)
        .map(|i| ContextProducerReadV1 {
            producer,
            read: ContextAllocationReadV1 {
                allocation: allocations[if distinct { i } else { 0 }],
                device,
                byte_extent: extent,
                byte_offset: 2 * i as u64,
                byte_len: 3,
                attempt_epoch: 1,
                content_lineage: 0,
            },
        })
        .collect();
    let mut output = alloc::vec![None; k];
    owner
        .acquire_producer_reads(key(20), &requests, &mut output)
        .unwrap();
    let references = output.into_iter().map(Option::unwrap).collect();
    let mut retained = [None];
    owner
        .acquire_producer_reads(key(21), &requests[..1], &mut retained)
        .unwrap();
    let mut stable = [None];
    owner
        .acquire_reads(
            key(22),
            &[ContextAllocationReadV1 {
                allocation: allocations[count],
                device,
                byte_extent: extent,
                byte_offset: 0,
                byte_len: 8,
                attempt_epoch: 0,
                content_lineage: 0,
            }],
            &mut stable,
        )
        .unwrap();
    settle(&mut owner, producer, phase);
    let mut case = Case {
        owner,
        references,
        consumer: key(20),
        evidence: ContextReadQuiescenceEvidenceV1 { consumer: key(20) },
        retained: retained[0].unwrap(),
        stable: stable[0].unwrap(),
    };
    match fault {
        "none" => {}
        "reference" => case.references[k - 1].incarnation += 1,
        "consumer" => case.references[k - 1].consumer.local += 1,
        "count" => {
            case.owner.counts[requests[k - 1].read.allocation.slot] =
                if distinct { 0 } else { k - 1 }
        }
        "evidence" => case.evidence.consumer.local += 1,
        "capacity" => {
            case.owner.free = case.owner.free.into_boxed_slice().into_vec();
            assert_eq!(case.owner.free.len(), case.owner.free.capacity());
        }
        "logical" => {
            case.owner.free.push(usize::MAX);
            case.owner.free.push(usize::MAX);
            case.owner.free.push(usize::MAX);
        }
        "duplicate" => {
            assert!(k >= 2);
            case.references[1] = case.references[0];
        }
        "order" => {
            assert!(k >= 2);
            case.references.swap(0, 1);
        }
        _ => unreachable!(),
    }
    case.owner.reset_access_count_for_test_v1();
    case
}

fn expected(k: usize, phase: &str, fault: &str) -> (Result<(), Error>, usize) {
    let weight = if matches!(phase, "pending" | "unknown") {
        3
    } else {
        1
    };
    match fault {
        "none" => (Ok(()), weight * k),
        "reference" | "consumer" => (Err(Error::InvalidReference), weight * (k - 1)),
        "count" => (Err(Error::InvalidState), weight * k),
        "evidence" => (Err(Error::SettlementEvidenceMismatch), 0),
        "capacity" | "logical" => (Err(Error::InvalidState), 0),
        "duplicate" | "order" => (Err(Error::NonCanonicalRoster), 2 * weight),
        _ => unreachable!(),
    }
}

struct Reset {
    entries: Vec<(usize, Option<ReservationV1>)>,
    counts: Vec<(usize, usize)>,
    free_length: usize,
    next: u64,
}

impl Reset {
    fn new(case: &Case) -> Self {
        let entries: Vec<_> = case
            .references
            .iter()
            .map(|r| (r.slot, case.owner.reservations[r.slot]))
            .collect();
        let counts = entries
            .iter()
            .map(|(_, entry)| {
                let slot = entry.unwrap().request.read.allocation.slot;
                (slot, case.owner.counts[slot])
            })
            .collect();
        Self {
            entries,
            counts,
            free_length: case.owner.free.len(),
            next: case.owner.next_incarnation,
        }
    }

    fn restore(&self, case: &mut Case) {
        for &(slot, entry) in &self.entries {
            case.owner.reservations[slot] = entry;
        }
        for &(slot, count) in &self.counts {
            case.owner.counts[slot] = count;
        }
        assert!(case.owner.free.len() >= self.free_length);
        case.owner.free.truncate(self.free_length);
        assert_eq!(case.owner.next_incarnation, self.next);
        case.owner.reset_access_count_for_test_v1();
    }
}

fn run(case: &mut Case, candidate: bool) -> Result<(), Error> {
    if candidate {
        case.owner
            .release_producer_reads(case.consumer, &case.references, &case.evidence)
    } else {
        case.owner.baseline_release_producer_reads_v1(
            case.consumer,
            &case.references,
            &case.evidence,
        )
    }
}

fn qualify(case: &mut Case, phase: &str, fault: &str) {
    qualify_expected(case, expected(case.references.len(), phase, fault));
}

fn qualify_expected(case: &mut Case, wanted: (Result<(), Error>, usize)) {
    let reset = Reset::new(case);
    let before = snapshot(&case.owner);
    let storage = case.owner.guard_owner_storage_v1();
    let references = case.references.clone();
    let reference_storage = (case.references.as_ptr(), case.references.capacity());
    let mut frozen = None;
    for candidate in [false, true] {
        case.owner.reset_access_count_for_test_v1();
        assert_eq!(run(case, candidate), wanted.0);
        assert_eq!(case.owner.guard_accesses_for_test_v1(), wanted.1);
        let after = snapshot(&case.owner);
        if wanted.0.is_err() {
            assert_eq!(after, before);
        }
        if let Some(prior) = &frozen {
            assert_eq!(&after, prior);
        }
        frozen = Some(after);
        assert_eq!(case.references, references);
        assert_eq!(
            (case.references.as_ptr(), case.references.capacity()),
            reference_storage
        );
        assert_eq!(case.owner.guard_owner_storage_v1(), storage);
        assert!(case.owner.lookup_producer_read(case.retained).is_ok());
        assert!(case.owner.lookup_read(case.stable).is_ok());
        reset.restore(case);
        assert_eq!(snapshot(&case.owner), before);
        assert_eq!(case.owner.guard_owner_storage_v1(), storage);
    }
}

#[test]
fn shared_release_matches_frozen_results_state_storage_and_all_statuses() {
    for k in [2, 8, 64] {
        for capacity in [k + 2, 1024] {
            for distinct in [false, true] {
                for phase in ["pending", "unknown", "success", "no_effect"] {
                    for fault in [
                        "none",
                        "reference",
                        "consumer",
                        "count",
                        "evidence",
                        "capacity",
                        "logical",
                        "duplicate",
                        "order",
                    ] {
                        qualify(
                            &mut setup(k, capacity, distinct, phase, fault),
                            phase,
                            fault,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn shared_release_keeps_existing_free_aliases_and_ignores_acquisition_epoch_and_budget() {
    for phase in ["pending", "unknown", "success", "no_effect"] {
        let mut case = setup(2, 8, false, phase, "none");
        case.owner.free.fill(case.retained.slot);
        case.owner.next_incarnation = 0;
        qualify(&mut case, phase, "none");
        let mut case = setup(2, 8, false, phase, "none");
        case.owner.free.clear();
        case.owner.next_incarnation = u64::MAX;
        assert!(case.owner.retained_read_count() > case.owner.reservations.len());
        qualify(&mut case, phase, "none");
    }
}

#[test]
fn shared_release_orders_identical_ranges_by_incarnation() {
    for phase in ["pending", "unknown", "success", "no_effect"] {
        let mut case = setup(1, 8, false, "pending", "none");
        let request = case.owner.reservations[case.references[0].slot]
            .unwrap()
            .request;
        let mut next = [None];
        case.owner
            .acquire_producer_reads(case.consumer, &[request], &mut next)
            .unwrap();
        case.references.push(next[0].unwrap());
        assert!(case.references[0].incarnation < case.references[1].incarnation);
        settle(&mut case.owner, request.producer, phase);
        qualify(&mut case, phase, "none");
        case.references.swap(0, 1);
        qualify(&mut case, phase, "order");
        case.references[1] = case.references[0];
        qualify(&mut case, phase, "duplicate");
    }
}

#[test]
fn shared_release_accepts_a_mixed_status_roster() {
    let mut owner = ContextProducerReadJournalV1::new(7, 5, 4, 8).unwrap();
    let device = ContextJournalDeviceKeyV1 {
        context_generation: 7,
        local: 1,
    };
    let consumer = key(100);
    let producers: Vec<_> = (1..=4)
        .map(|i| owner.register_writer(key(i * 10)).unwrap())
        .collect();
    let mut references = Vec::new();
    let mut retained = None;
    for (i, phase) in ["pending", "unknown", "success", "no_effect"]
        .into_iter()
        .enumerate()
    {
        let allocation = owner
            .enroll_allocation(
                ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: i as u64 + 1,
                },
                device,
                64,
            )
            .unwrap();
        let producer = producers[i];
        owner
            .begin_write(
                producer,
                &[ContextAllocationWriteV1 {
                    allocation,
                    device,
                    byte_extent: 64,
                }],
            )
            .unwrap();
        let request = ContextProducerReadV1 {
            producer,
            read: ContextAllocationReadV1 {
                allocation,
                device,
                byte_extent: 64,
                byte_offset: 0,
                byte_len: 8,
                attempt_epoch: 1,
                content_lineage: 0,
            },
        };
        let mut output = [None];
        owner
            .acquire_producer_reads(consumer, &[request], &mut output)
            .unwrap();
        references.push(output[0].unwrap());
        if i == 0 {
            output[0] = None;
            owner
                .acquire_producer_reads(key(101), &[request], &mut output)
                .unwrap();
            retained = output[0];
        }
        settle(&mut owner, producer, phase);
    }
    let allocation = owner
        .enroll_allocation(
            ContextAllocationKeyV1 {
                context_generation: 7,
                local: 5,
            },
            device,
            64,
        )
        .unwrap();
    let mut stable = [None];
    owner
        .acquire_reads(
            key(102),
            &[ContextAllocationReadV1 {
                allocation,
                device,
                byte_extent: 64,
                byte_offset: 0,
                byte_len: 8,
                attempt_epoch: 0,
                content_lineage: 0,
            }],
            &mut stable,
        )
        .unwrap();
    qualify_expected(
        &mut Case {
            owner,
            references,
            consumer,
            evidence: ContextReadQuiescenceEvidenceV1 { consumer },
            retained: retained.unwrap(),
            stable: stable[0].unwrap(),
        },
        (Ok(()), 8),
    );
}

fn qualify_rejection(case: &mut Case, error: Error, accesses: usize) {
    let before = snapshot(&case.owner);
    let storage = case.owner.guard_owner_storage_v1();
    let references = case.references.clone();
    let reference_storage = (case.references.as_ptr(), case.references.capacity());
    for candidate in [false, true] {
        case.owner.reset_access_count_for_test_v1();
        assert_eq!(run(case, candidate), Err(error));
        assert_eq!(case.owner.guard_accesses_for_test_v1(), accesses);
        assert_eq!(snapshot(&case.owner), before);
        assert_eq!(case.owner.guard_owner_storage_v1(), storage);
        assert_eq!(case.references, references);
        assert_eq!(
            (case.references.as_ptr(), case.references.capacity()),
            reference_storage
        );
    }
}

#[test]
fn shared_release_full_identity_precedes_stored_request_validation() {
    for phase in ["pending", "unknown", "success", "no_effect"] {
        let weight = if matches!(phase, "pending" | "unknown") {
            3
        } else {
            1
        };
        let mut case = setup(2, 8, true, phase, "none");
        let reference = case.references[1];
        case.owner.reservations[reference.slot]
            .as_mut()
            .unwrap()
            .request
            .read
            .device
            .local += 1;
        for field in 0..5 {
            let mut bad = reference;
            match field {
                0 => bad.incarnation += 1,
                1 => bad.consumer.local += 1,
                2 => bad.consumer.context_generation += 1,
                3 => bad.consumer.kind = ContextWriterKindV1::Synchronous,
                4 => bad.slot = case.retained.slot,
                _ => unreachable!(),
            }
            case.references[1] = bad;
            qualify_rejection(&mut case, Error::InvalidReference, weight);
        }
        case.references[1] = reference;
        qualify_rejection(
            &mut case,
            Error::AllocationDeviceMismatch,
            weight + if weight == 3 { 2 } else { 1 },
        );
    }
}

#[test]
fn shared_release_observes_physical_capacity_for_identical_logical_contents() {
    let mut ample = setup(2, 8, false, "pending", "none");
    let mut short = setup(2, 8, false, "pending", "capacity");
    assert_eq!(snapshot(&ample.owner), snapshot(&short.owner));
    assert!(ample.owner.free.capacity() > short.owner.free.capacity());
    qualify(&mut ample, "pending", "none");
    qualify(&mut short, "pending", "capacity");
}

#[test]
fn shared_release_early_errors_precede_missing_count_storage() {
    let mut case = setup(2, 8, true, "pending", "none");
    case.owner.counts.clear();
    case.evidence.consumer.local += 1;
    qualify_rejection(&mut case, Error::SettlementEvidenceMismatch, 0);
    case.evidence.consumer = case.consumer;
    case.references.clear();
    qualify_rejection(&mut case, Error::RosterCapacity, 0);
    case.references.push(case.retained);
    case.owner
        .free
        .resize(case.owner.reservations.len(), usize::MAX);
    qualify_rejection(&mut case, Error::InvalidState, 0);

    // Header success lies outside the conservative formal storage domain here.
    for phase in ["pending", "unknown", "success", "no_effect"] {
        let weight = if matches!(phase, "pending" | "unknown") {
            3
        } else {
            1
        };
        let mut case = setup(2, 8, true, phase, "none");
        let slot = case.references[1].slot;
        let allocation_slot = case.owner.reservations[slot]
            .unwrap()
            .request
            .read
            .allocation
            .slot;
        assert!(allocation_slot > 0);
        case.owner.counts.truncate(allocation_slot);
        case.owner.reservations[slot]
            .as_mut()
            .unwrap()
            .request
            .read
            .allocation
            .key
            .context_generation += 1;
        qualify_rejection(&mut case, Error::InvalidAllocationReference, weight + 1);
    }
}
