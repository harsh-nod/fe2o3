use super::*;

fn cases() -> Vec<(usize, usize, &'static str, &'static str)> {
    let mut cases = Vec::new();
    for count in [1, 8, 64, 512, 4096] {
        for shape in ["grouped", "distinct"] {
            for fault in ["none", "late_extent", "late_output", "late_slot"] {
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
    Vec<ContextAllocationReadV1>,
    Vec<Option<ContextReadLeaseReferenceV1>>,
) {
    let mut owner = Journal::new(7, capacity, 4, capacity).unwrap();
    let device = ContextJournalDeviceKeyV1 {
        context_generation: 7,
        local: 1,
    };
    let extent = (count as u64 + 1) * 8;
    let entries: Vec<_> = (0..if shape == "grouped" { 1 } else { count })
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
    let mut requests: Vec<_> = (0..count)
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
    owner
        .acquire_reads(consumer(19), &requests[..1], &mut [None])
        .unwrap();
    let mut output = alloc::vec![None; count];
    match fault {
        "late_extent" => requests[count - 1].byte_len = extent + 1,
        "late_output" => output[count - 1] = owner.leases[0].map(|lease| lease.reference),
        "late_slot" => {
            let index = owner.free_reads.len() - count;
            owner.free_reads[index] = usize::MAX;
        }
        "none" => {}
        _ => unreachable!(),
    }
    (owner, requests, output)
}

struct Reset {
    leases: Vec<(usize, Option<ReadLeaseV1>)>,
    readers: Vec<(usize, usize)>,
    free: Vec<usize>,
    next: u64,
    output: Vec<Option<ContextReadLeaseReferenceV1>>,
}

impl Reset {
    fn capture(
        owner: &Journal,
        requests: &[ContextAllocationReadV1],
        output: &[Option<ContextReadLeaseReferenceV1>],
    ) -> Self {
        let selected: alloc::collections::BTreeSet<_> = owner
            .free_reads
            .iter()
            .rev()
            .take(requests.len())
            .copied()
            .filter(|&slot| slot < owner.leases.len())
            .collect();
        let allocations: alloc::collections::BTreeSet<_> = requests
            .iter()
            .map(|request| request.allocation.slot)
            .collect();
        Self {
            leases: selected
                .into_iter()
                .map(|slot| (slot, owner.leases[slot]))
                .collect(),
            readers: allocations
                .into_iter()
                .map(|slot| (slot, owner.readers[slot]))
                .collect(),
            free: owner.free_reads.clone(),
            next: owner.next_incarnation,
            output: output.to_vec(),
        }
    }

    fn restore(&self, owner: &mut Journal, output: &mut [Option<ContextReadLeaseReferenceV1>]) {
        for &(slot, lease) in &self.leases {
            owner.leases[slot] = lease;
        }
        for &(slot, count) in &self.readers {
            owner.readers[slot] = count;
        }
        assert!(owner.free_reads.len() <= self.free.len());
        owner
            .free_reads
            .extend_from_slice(&self.free[owner.free_reads.len()..]);
        owner.next_incarnation = self.next;
        output.copy_from_slice(&self.output);
        owner.reset_access_count_for_test_v1();
    }
}

fn run(
    owner: &mut Journal,
    requests: &[ContextAllocationReadV1],
    output: &mut [Option<ContextReadLeaseReferenceV1>],
    candidate: bool,
) -> Result<(), Error> {
    if candidate {
        owner.acquire_reads(consumer(20), requests, output)
    } else {
        owner.baseline_acquire_reads_v1(consumer(20), requests, output)
    }
}

struct Qualification {
    reset: Reset,
    before: String,
    storage: Vec<(usize, usize)>,
    output_pointer: usize,
    expected: Result<(), Error>,
    lookups: usize,
}

fn qualify(
    owner: &mut Journal,
    requests: &[ContextAllocationReadV1],
    output: &mut [Option<ContextReadLeaseReferenceV1>],
    fault: &str,
) -> Qualification {
    let reset = Reset::capture(owner, requests, output);
    let before = snapshot(owner);
    let storage = owner.guard_owner_storage_v1();
    let output_pointer = output.as_ptr() as usize;
    let expected = match fault {
        "none" => Ok(()),
        "late_extent" => Err(Error::InvalidExtent),
        "late_output" | "late_slot" => Err(Error::InvalidState),
        _ => unreachable!(),
    };
    let lookups = if fault == "late_output" {
        0
    } else {
        requests.len()
    };
    let mut previous = None;
    for candidate in [false, true] {
        owner.reset_access_count_for_test_v1();
        assert_eq!(run(owner, requests, output, candidate), expected);
        assert_eq!(owner.guard_accesses_for_test_v1(), lookups);
        let state = (snapshot(owner), output.to_vec());
        if let Some(prior) = &previous {
            assert_eq!(&state, prior);
        }
        if expected.is_err() {
            assert_eq!(state.0, before);
            assert_eq!(state.1, reset.output);
        } else {
            assert_reader_invariant(owner);
        }
        previous = Some(state);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
        assert_eq!(output.as_ptr() as usize, output_pointer);
        reset.restore(owner, output);
        assert_eq!(snapshot(owner), before);
        assert_eq!(output, reset.output);
        assert_eq!(owner.guard_owner_storage_v1(), storage);
    }
    Qualification {
        reset,
        before,
        storage,
        output_pointer,
        expected,
        lookups,
    }
}

#[test]
fn shared_acquire_benchmark_fixtures_match_frozen_state_storage_and_reset() {
    for (count, capacity, shape, fault) in cases() {
        let (mut owner, requests, mut output) = setup(count, capacity, shape, fault);
        let _ = qualify(&mut owner, &requests, &mut output, fault);
    }
}

#[test]
#[ignore = "manual release-mode matched stable acquisition comparison"]
fn shared_acquire_performance() {
    use std::hint::black_box;
    use std::time::Instant;
    for (count, capacity, shape, fault) in cases() {
        let (mut owner, requests, mut output) = setup(count, capacity, shape, fault);
        let Qualification {
            reset,
            before,
            storage,
            output_pointer,
            expected,
            lookups,
        } = qualify(&mut owner, &requests, &mut output, fault);
        let iterations = (131072 / count).clamp(64, 8192);
        for round in 0..7 {
            for turn in 0..2 {
                let candidate = (round + turn) % 2 == 0;
                let mut elapsed = 0u128;
                for _ in 0..iterations {
                    reset.restore(&mut owner, &mut output);
                    let start = Instant::now();
                    let result = black_box(run(
                        black_box(&mut owner),
                        black_box(&requests),
                        black_box(&mut output),
                        black_box(candidate),
                    ));
                    elapsed += start.elapsed().as_nanos();
                    assert_eq!(result, expected);
                    assert_eq!(owner.guard_accesses_for_test_v1(), lookups);
                }
                reset.restore(&mut owner, &mut output);
                assert_eq!(snapshot(&owner), before);
                assert_eq!(output, reset.output);
                assert_eq!(owner.guard_owner_storage_v1(), storage);
                assert_eq!(output.as_ptr() as usize, output_pointer);
                std::println!(
                    "stable_acquire,{count},{capacity},{shape},{fault},{round},{},{iterations},{elapsed},{lookups}",
                    if candidate { "shared" } else { "baseline" }
                );
            }
        }
    }
}
