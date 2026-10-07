use super::*;

fn cases() -> Vec<(usize, usize, &'static str, &'static str)> {
    let mut cases = Vec::new();
    for count in [1, 8, 64, 512, 4096] {
        for shape in ["grouped", "distinct"] {
            for fault in [
                "none",
                "late_reference",
                "late_count",
                "evidence",
                "capacity",
            ] {
                cases.push((count, (count + 2).max(1024), shape, fault));
            }
        }
    }
    cases.extend([
        (8, 65536, "grouped", "none"),
        (8, 65536, "distinct", "none"),
    ]);
    cases
}

fn setup(
    count: usize,
    capacity: usize,
    shape: &str,
    fault: &str,
) -> (
    Journal,
    Vec<ContextReadLeaseReferenceV1>,
    ContextReadQuiescenceEvidenceV1,
) {
    let mut owner = Journal::new(7, capacity, 4, capacity).unwrap();
    let device = ContextJournalDeviceKeyV1 {
        context_generation: 7,
        local: 1,
    };
    let extent = (count as u64 + 2) * 8;
    let entries: Vec<_> = (0..if shape == "grouped" { 1 } else { count + 1 })
        .map(|index| ContextAllocationEnrollmentV1 {
            key: ContextAllocationKeyV1 {
                context_generation: 7,
                local: index as u64 + 1,
            },
            device,
            byte_extent: extent,
        })
        .collect();
    let mut allocations = alloc::vec![None; entries.len()];
    owner
        .enroll_allocations(&entries, &mut allocations)
        .unwrap();
    let requests: Vec<_> = (0..count + 1)
        .map(|index| ContextAllocationReadV1 {
            allocation: allocations[if shape == "grouped" { 0 } else { index }].unwrap(),
            device,
            byte_extent: extent,
            byte_offset: index as u64 * 8,
            byte_len: 8,
            attempt_epoch: 0,
            content_lineage: 0,
        })
        .collect();
    let mut output = alloc::vec![None; requests.len()];
    owner
        .acquire_reads(consumer(20), &requests, &mut output)
        .unwrap();
    let mut references: Vec<_> = output[..count].iter().map(|value| value.unwrap()).collect();
    let mut evidence = ContextReadQuiescenceEvidenceV1 {
        consumer: consumer(20),
    };
    match fault {
        "late_reference" => references[count - 1].incarnation += 1,
        "late_count" => {
            owner.readers[requests[count - 1].allocation.slot] =
                if shape == "grouped" { count - 1 } else { 0 }
        }
        "evidence" => evidence.consumer.local += 1,
        "capacity" => {
            owner.free_reads = owner.free_reads.into_boxed_slice().into_vec();
            assert_eq!(owner.free_reads.capacity(), owner.free_reads.len());
        }
        "none" => {}
        _ => unreachable!(),
    }
    (owner, references, evidence)
}

struct Reset {
    leases: Vec<(usize, Option<ReadLeaseV1>)>,
    readers: Vec<(usize, usize)>,
    free_length: usize,
    next: u64,
}

impl Reset {
    fn capture(owner: &Journal, references: &[ContextReadLeaseReferenceV1]) -> Self {
        let slots: alloc::collections::BTreeSet<_> =
            references.iter().map(|reference| reference.slot).collect();
        let allocations: alloc::collections::BTreeSet<_> = slots
            .iter()
            .map(|&slot| owner.leases[slot].unwrap().request.allocation.slot)
            .collect();
        Self {
            leases: slots
                .into_iter()
                .map(|slot| (slot, owner.leases[slot]))
                .collect(),
            readers: allocations
                .into_iter()
                .map(|slot| (slot, owner.readers[slot]))
                .collect(),
            free_length: owner.free_reads.len(),
            next: owner.next_incarnation,
        }
    }

    fn restore(&self, owner: &mut Journal) {
        for &(slot, lease) in &self.leases {
            owner.leases[slot] = lease;
        }
        for &(slot, count) in &self.readers {
            owner.readers[slot] = count;
        }
        assert!(owner.free_reads.len() >= self.free_length);
        owner.free_reads.truncate(self.free_length);
        assert_eq!(owner.next_incarnation, self.next);
        owner.reset_access_count_for_test_v1();
    }
}

fn run(
    owner: &mut Journal,
    references: &[ContextReadLeaseReferenceV1],
    evidence: &ContextReadQuiescenceEvidenceV1,
    candidate: bool,
) -> Result<(), Error> {
    if candidate {
        owner.release_reads(consumer(20), references, evidence)
    } else {
        owner.baseline_release_reads_v1(consumer(20), references, evidence)
    }
}

struct Qualification {
    reset: Reset,
    before: String,
    storage: Vec<(usize, usize)>,
    expected: Result<(), Error>,
    lookups: usize,
}

fn qualify(
    owner: &mut Journal,
    references: &[ContextReadLeaseReferenceV1],
    evidence: &ContextReadQuiescenceEvidenceV1,
    fault: &str,
) -> Qualification {
    let reset = Reset::capture(owner, references);
    let before = snapshot(owner);
    let storage = owner.guard_owner_storage_v1();
    let (expected, lookups) = match fault {
        "none" => (Ok(()), references.len()),
        "late_reference" => (Err(Error::InvalidReference), references.len() - 1),
        "late_count" => (Err(Error::InvalidState), references.len()),
        "evidence" => (Err(Error::SettlementEvidenceMismatch), 0),
        "capacity" => (Err(Error::InvalidState), 0),
        _ => unreachable!(),
    };
    let mut previous = None;
    for candidate in [false, true] {
        owner.reset_access_count_for_test_v1();
        assert_eq!(run(owner, references, evidence, candidate), expected);
        assert_eq!(owner.guard_accesses_for_test_v1(), lookups);
        let state = snapshot(owner);
        if let Some(prior) = &previous {
            assert_eq!(&state, prior);
        }
        if expected.is_err() {
            assert_eq!(state, before);
        } else {
            assert_reader_invariant(owner);
        }
        previous = Some(state);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
        reset.restore(owner);
        assert_eq!(snapshot(owner), before);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
    }
    Qualification {
        reset,
        before,
        storage,
        expected,
        lookups,
    }
}

#[test]
fn shared_release_benchmark_fixtures_match_frozen_state_storage_and_reset() {
    for (count, capacity, shape, fault) in cases() {
        let (mut owner, references, evidence) = setup(count, capacity, shape, fault);
        let _ = qualify(&mut owner, &references, &evidence, fault);
    }
}

#[test]
#[ignore = "manual release-mode matched stable release comparison"]
fn shared_release_performance() {
    use std::hint::black_box;
    use std::time::Instant;
    for (count, capacity, shape, fault) in cases() {
        let (mut owner, references, evidence) = setup(count, capacity, shape, fault);
        let Qualification {
            reset,
            before,
            storage,
            expected,
            lookups,
        } = qualify(&mut owner, &references, &evidence, fault);
        let iterations = (131072 / count).clamp(64, 8192);
        for round in 0..7 {
            for turn in 0..2 {
                let candidate = (round + turn) % 2 == 0;
                let mut elapsed = 0u128;
                for _ in 0..iterations {
                    reset.restore(&mut owner);
                    let start = Instant::now();
                    let result = black_box(run(
                        black_box(&mut owner),
                        black_box(&references),
                        black_box(&evidence),
                        black_box(candidate),
                    ));
                    elapsed += start.elapsed().as_nanos();
                    assert_eq!(result, expected);
                    assert_eq!(owner.guard_accesses_for_test_v1(), lookups);
                }
                reset.restore(&mut owner);
                assert_eq!(snapshot(&owner), before);
                assert_eq!(owner.guard_owner_storage_v1(), storage);
                std::println!(
                    "stable_release,{count},{capacity},{shape},{fault},{round},{},{iterations},{elapsed},{lookups}",
                    if candidate { "shared" } else { "baseline" }
                );
            }
        }
    }
}
