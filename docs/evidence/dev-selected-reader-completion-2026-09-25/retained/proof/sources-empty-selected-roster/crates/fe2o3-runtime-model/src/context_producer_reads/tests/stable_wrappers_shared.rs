use super::*;
use crate::context_read_leases::{StableReadFaultV1 as Fault, StableReadResetV1};

mod performance;

struct Case {
    owner: ContextProducerReadJournalV1,
    requests: Vec<ContextAllocationReadV1>,
    output: Vec<Option<ContextReadLeaseReferenceV1>>,
    references: Vec<ContextReadLeaseReferenceV1>,
    consumer: ContextWriterKeyV1,
    evidence: ContextReadQuiescenceEvidenceV1,
    retained: ContextProducerReadReferenceV1,
    stable: ContextReadLeaseReferenceV1,
    release: bool,
}

fn setup(
    k: usize,
    capacity: usize,
    distinct: bool,
    synchronous: bool,
    release: bool,
    phase: &str,
    fault: &str,
) -> Case {
    assert!(k > 0 && capacity >= k + 2);
    let count = if distinct { k } else { 1 };
    let mut owner = ContextProducerReadJournalV1::new(7, count + 2, 2, capacity).unwrap();
    let device = ContextJournalDeviceKeyV1 {
        context_generation: 7,
        local: 1,
    };
    let extent = 2 * k as u64 + 64;
    let allocations: Vec<_> = (0..count + 2)
        .map(|i| {
            owner
                .enroll_allocation(
                    ContextAllocationKeyV1 {
                        context_generation: 7,
                        local: i as u64 + 1,
                    },
                    device,
                    extent,
                )
                .unwrap()
        })
        .collect();
    let read = |index, byte_offset| ContextAllocationReadV1 {
        allocation: allocations[index],
        device,
        byte_extent: extent,
        byte_offset,
        byte_len: 3,
        attempt_epoch: 0,
        content_lineage: 0,
    };
    let producer = owner.register_writer(key(10)).unwrap();
    owner
        .begin_write(
            producer,
            &[ContextAllocationWriteV1 {
                allocation: allocations[count],
                device,
                byte_extent: extent,
            }],
        )
        .unwrap();
    let pending = ContextAllocationReadV1 {
        attempt_epoch: 1,
        ..read(count, 0)
    };
    let mut retained = [None];
    owner
        .acquire_producer_reads(
            key(20),
            &[ContextProducerReadV1 {
                producer,
                read: pending,
            }],
            &mut retained,
        )
        .unwrap();
    let mut stable = [None];
    owner
        .acquire_reads(key(20), &[read(count + 1, 0)], &mut stable)
        .unwrap();
    let retained = retained[0].unwrap();
    let stable = stable[0].unwrap();
    assert_eq!(
        (retained.slot, retained.incarnation, retained.consumer),
        (stable.slot, stable.incarnation, stable.consumer)
    );
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
    let mut requests: Vec<_> = (0..k)
        .map(|i| read(if distinct { i } else { 0 }, 2 * i as u64))
        .collect();
    let consumer = if synchronous {
        ContextWriterKeyV1 {
            local: 5,
            kind: ContextWriterKindV1::Synchronous,
            ..key(5)
        }
    } else {
        key(30)
    };
    let mut output = alloc::vec![None; k];
    let mut references = Vec::new();
    if release {
        owner
            .acquire_reads(consumer, &requests, &mut output)
            .unwrap();
        references.extend(output.iter().map(|value| value.unwrap()));
    }
    let mut evidence = ContextReadQuiescenceEvidenceV1 { consumer };
    match (release, fault) {
        (_, "none") => {}
        (false, "budget") => owner.free.truncate(k),
        (false, "output") => output[k - 1] = Some(stable),
        (false, "extent") => requests[k - 1].byte_len = 0,
        (false, "slot") => owner
            .stable
            .fault_reads_for_test_v1(Fault::FreeSlotFromEnd {
                distance: k,
                slot: usize::MAX,
            }),
        (false, "alias") => owner
            .stable
            .fault_reads_for_test_v1(Fault::FreeSlotFromEnd {
                distance: k,
                slot: 1,
            }),
        (false, "epoch") => owner
            .stable
            .fault_reads_for_test_v1(Fault::NextIncarnation(0)),
        (false, "pending") => requests[k - 1] = pending,
        (false, "count") => owner.stable.fault_reads_for_test_v1(Fault::ReaderCount {
            allocation_slot: requests[k - 1].allocation.slot,
            value: if distinct { capacity } else { capacity - k + 1 },
        }),
        (true, "count") => owner.stable.fault_reads_for_test_v1(Fault::ReaderCount {
            allocation_slot: requests[k - 1].allocation.slot,
            value: if distinct { 0 } else { k - 1 },
        }),
        (true, "reference") => references[k - 1].incarnation += 1,
        (true, "consumer") => references[k - 1].consumer.local += 1,
        (true, "evidence") => evidence.consumer.local += 1,
        (true, "capacity") => owner
            .stable
            .fault_reads_for_test_v1(Fault::TightFreeCapacity),
        (false, "duplicate") => requests[1] = requests[0],
        (true, "duplicate") => references[1] = references[0],
        (false, "order") => requests.swap(0, 1),
        (true, "order") => references.swap(0, 1),
        _ => unreachable!(),
    }
    owner.reset_access_count_for_test_v1();
    Case {
        owner,
        requests,
        output,
        references,
        consumer,
        evidence,
        retained,
        stable,
        release,
    }
}

fn expected(k: usize, fault: &str) -> (Result<(), Error>, usize) {
    match fault {
        "none" | "alias" => (Ok(()), k),
        "budget" => (Err(Error::MemberCapacity), 0),
        "output" | "capacity" => (Err(Error::InvalidState), 0),
        "epoch" => (Err(Error::EpochExhausted), 0),
        "extent" => (Err(Error::InvalidExtent), k),
        "slot" | "count" => (Err(Error::InvalidState), k),
        "pending" => (Err(Error::AllocationBusy), k + 1),
        "reference" | "consumer" => (Err(Error::InvalidReference), k - 1),
        "evidence" => (Err(Error::SettlementEvidenceMismatch), 0),
        "duplicate" | "order" => (Err(Error::NonCanonicalRoster), 2),
        _ => unreachable!(),
    }
}

struct Reset {
    stable: StableReadResetV1,
    output: Vec<Option<ContextReadLeaseReferenceV1>>,
}

impl Reset {
    fn new(case: &Case) -> Self {
        let stable = if case.release {
            case.owner
                .stable
                .capture_release_reads_for_test_v1(&case.references)
        } else {
            case.owner
                .stable
                .capture_acquire_reads_for_test_v1(&case.requests)
        };
        Self {
            stable,
            output: case.output.clone(),
        }
    }

    fn restore(&self, case: &mut Case) {
        self.stable.restore(&mut case.owner.stable);
        case.output.copy_from_slice(&self.output);
    }
}

fn run(case: &mut Case, candidate: bool) -> Result<(), Error> {
    match (case.release, candidate) {
        (false, true) => case
            .owner
            .acquire_reads(case.consumer, &case.requests, &mut case.output),
        (false, false) => {
            case.owner
                .baseline_acquire_reads_v1(case.consumer, &case.requests, &mut case.output)
        }
        (true, true) => case
            .owner
            .release_reads(case.consumer, &case.references, &case.evidence),
        (true, false) => {
            case.owner
                .baseline_release_reads_v1(case.consumer, &case.references, &case.evidence)
        }
    }
}

fn qualify(case: &mut Case, wanted: (Result<(), Error>, usize)) {
    let reset = Reset::new(case);
    let before = snapshot(&case.owner);
    let storage = case.owner.guard_owner_storage_v1();
    let inputs = (case.requests.clone(), case.references.clone());
    let buffer_storage = (
        case.requests.as_ptr(),
        case.requests.capacity(),
        case.references.as_ptr(),
        case.references.capacity(),
        case.output.as_ptr(),
        case.output.capacity(),
    );
    let outer = (
        case.owner.reservations.clone(),
        case.owner.free.clone(),
        case.owner.counts.clone(),
        case.owner.next_incarnation,
    );
    let mut frozen = None;
    for candidate in [false, true] {
        case.owner.reset_access_count_for_test_v1();
        assert_eq!(run(case, candidate), wanted.0);
        assert_eq!(case.owner.guard_accesses_for_test_v1(), wanted.1);
        let after = snapshot(&case.owner);
        if wanted.0.is_err() {
            assert_eq!(after, before);
            assert_eq!(case.output, reset.output);
        }
        if let Some((prior, output)) = &frozen {
            assert_eq!(&after, prior);
            assert_eq!(&case.output, output);
        }
        frozen = Some((after, case.output.clone()));
        assert_eq!(case.owner.reservations, outer.0);
        assert_eq!(case.owner.free, outer.1);
        assert_eq!(case.owner.counts, outer.2);
        assert_eq!(case.owner.next_incarnation, outer.3);
        assert_eq!(case.requests, inputs.0);
        assert_eq!(case.references, inputs.1);
        assert_eq!(
            (
                case.requests.as_ptr(),
                case.requests.capacity(),
                case.references.as_ptr(),
                case.references.capacity(),
                case.output.as_ptr(),
                case.output.capacity()
            ),
            buffer_storage
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
fn stable_wrappers_match_frozen_state_storage_consumers_and_producer_statuses() {
    for k in [2, 8, 64] {
        for distinct in [false, true] {
            for synchronous in [false, true] {
                for phase in ["pending", "unknown", "success", "no_effect"] {
                    for release in [false, true] {
                        let faults: &[&str] = if release {
                            &[
                                "none",
                                "reference",
                                "consumer",
                                "count",
                                "evidence",
                                "capacity",
                                "duplicate",
                                "order",
                            ]
                        } else {
                            &[
                                "none",
                                "budget",
                                "output",
                                "extent",
                                "slot",
                                "count",
                                "epoch",
                                "duplicate",
                                "order",
                            ]
                        };
                        for fault in faults {
                            qualify(
                                &mut setup(k, k + 2, distinct, synchronous, release, phase, fault),
                                expected(k, fault),
                            );
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn stable_wrappers_share_resolved_allocation_with_retained_producer_custody() {
    for phase in ["success", "no_effect"] {
        for release in [false, true] {
            let mut case = setup(1, 3, false, true, false, phase, "none");
            let mut request = case.owner.lookup_producer_read(case.retained).unwrap().read;
            request.content_lineage = case
                .owner
                .lookup_allocation(request.allocation)
                .unwrap()
                .content_lineage;
            case.requests[0] = request;
            if release {
                case.owner
                    .acquire_reads(case.consumer, &case.requests, &mut case.output)
                    .unwrap();
                case.references.push(case.output[0].unwrap());
                case.release = true;
            }
            qualify(&mut case, expected(1, "none"));
        }
    }
}

#[test]
fn stable_wrappers_preserve_raw_alias_and_late_pending_rejection() {
    for phase in ["pending", "unknown"] {
        qualify(
            &mut setup(8, 10, true, true, false, phase, "pending"),
            expected(8, "pending"),
        );
    }
    qualify(
        &mut setup(2, 4, false, true, false, "pending", "alias"),
        expected(2, "alias"),
    );
}

#[test]
fn stable_wrapper_prefix_precedes_malformed_budget_and_reader_storage() {
    for fault in ["foreign", "zero", "max", "empty", "mismatch", "output"] {
        let mut case = setup(2, 4, false, true, false, "pending", "none");
        case.owner.free.extend([0, 0]);
        case.owner.counts.clear();
        case.owner
            .stable
            .fault_reads_for_test_v1(Fault::TruncateReaders(0));
        let error = match fault {
            "foreign" => {
                case.consumer.context_generation = 0;
                Error::ForeignContext
            }
            "zero" => {
                case.consumer.local = 0;
                Error::InvalidWriterId
            }
            "max" => {
                case.consumer.local = u64::MAX;
                Error::InvalidWriterId
            }
            "empty" => {
                case.requests.clear();
                case.output.clear();
                Error::RosterCapacity
            }
            "mismatch" => {
                case.output.pop();
                Error::RosterCapacity
            }
            "output" => {
                case.output[1] = Some(case.stable);
                Error::InvalidState
            }
            _ => unreachable!(),
        };
        qualify(&mut case, (Err(error), 0));
    }
}

#[test]
fn stable_wrappers_ignore_producer_epoch_counts_and_release_budget() {
    for release in [false, true] {
        for next in [0, u64::MAX] {
            let mut case = setup(2, 4, false, true, release, "no_effect", "none");
            case.owner.next_incarnation = next;
            case.owner.counts.clear();
            if release {
                case.owner.free.extend([0, 0, 0]);
            }
            qualify(&mut case, expected(2, "none"));
        }
    }
}

#[test]
fn stable_shared_budget_rejection_precedes_stable_epoch_and_has_no_leaf_equivalent() {
    for exhaust_epoch in [false, true] {
        let mut case = setup(3, 5, false, true, false, "pending", "none");
        let mut extra = [None];
        case.owner
            .acquire_reads(key(31), &case.requests[..1], &mut extra)
            .unwrap();
        assert_eq!(case.owner.remaining_read_slots(), 2);
        assert_eq!(case.owner.stable.remaining_read_slots(), 3);
        if exhaust_epoch {
            case.owner
                .stable
                .fault_reads_for_test_v1(Fault::NextIncarnation(u64::MAX));
        }
        qualify(&mut case, (Err(Error::MemberCapacity), 0));
        assert_eq!(
            case.owner
                .stable
                .acquire_reads(case.consumer, &case.requests, &mut case.output),
            if exhaust_epoch {
                Err(Error::EpochExhausted)
            } else {
                Ok(())
            }
        );
    }
}
