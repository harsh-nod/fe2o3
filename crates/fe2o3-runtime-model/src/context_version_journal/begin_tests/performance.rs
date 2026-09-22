use super::*;

fn fixture(count: usize, capacity: usize, pattern: &str) -> (Journal, Reference, Vec<Write>) {
    let mut journal = Journal::new(7, capacity, 8).unwrap();
    let roster: Vec<_> = (0..count)
        .map(|i| {
            let slot = if pattern == "permuted" {
                i * 37 % capacity
            } else {
                i
            };
            let allocation = AllocationReference {
                slot,
                key: allocation_key(i as u64 + 1),
            };
            journal.allocations[slot] = Some(AllocationEntryV1 {
                key: allocation.key,
                device: device(),
                byte_extent: 64,
                attempt_epoch: 17,
                content_lineage: 9,
                pending_member: None,
            });
            Write {
                allocation,
                device: device(),
                byte_extent: 64,
            }
        })
        .collect();
    journal
        .allocation_free
        .retain(|slot| journal.allocations[*slot].is_none());
    if pattern == "permuted" {
        journal.member_free.reverse();
    }
    if pattern == "alias" {
        journal.member_free[capacity - count..].fill(0);
    }
    let writer = journal.register_writer(key(41)).unwrap();
    match pattern {
        "device_first" | "device_last" => {
            let index = if pattern == "device_first" {
                0
            } else {
                count - 1
            };
            journal.allocations[roster[index].allocation.slot]
                .as_mut()
                .unwrap()
                .device
                .local = 12;
        }
        "member_last" => journal.member_free[capacity - count] = usize::MAX,
        "scratch_last" | "dirty_tail" => {
            let index = if pattern == "scratch_last" {
                count - 1
            } else {
                count
            };
            journal.scratch[index] = Some(BeginMemberPlanV1 {
                member_slot: usize::MAX,
                allocation: AllocationReference {
                    slot: usize::MAX,
                    key: allocation_key(0),
                },
                prior_lineage: u64::MAX,
                attempt_epoch: 0,
            });
        }
        _ => {}
    }
    (journal, writer, roster)
}

// Reset only possible Begin writes; full snapshots qualify this reset before timing.
fn reset_touched(journal: &mut Journal, before: &Snapshot, writer: Reference, roster: &[Write]) {
    journal.writers[writer.slot] = before.writers[writer.slot];
    journal.reserved_count = before.reserved_count;
    for destination in roster {
        journal.allocations[destination.allocation.slot] =
            before.allocations[destination.allocation.slot];
    }
    for &slot in before.member_free.iter().rev().take(roster.len()) {
        if let Some(entry) = journal.members.get_mut(slot) {
            *entry = before.members[slot];
        }
    }
    journal.scratch[..roster.len()].copy_from_slice(&before.scratch[..roster.len()]);
    journal
        .member_free
        .extend_from_slice(&before.member_free[journal.member_free.len()..]);
    journal.reset_access_count_for_test_v1();
}

fn cases() -> Vec<(usize, usize, &'static str)> {
    let mut result = vec![(0, 1024, "normal"), (0, 1024, "dirty_tail")];
    for count in [1, 8, 64, 512, 4096] {
        let capacity = (count * 2).max(1024);
        for pattern in [
            "normal",
            "permuted",
            "alias",
            "dirty_tail",
            "device_first",
            "device_last",
            "member_last",
            "scratch_last",
        ] {
            result.push((count, capacity, pattern));
        }
    }
    result.extend([(8, 65536, "normal"), (8, 65536, "permuted")]);
    result
}

fn qualify(
    journal: &mut Journal,
    writer: Reference,
    roster: &[Write],
) -> (Snapshot, Result<(), Error>, usize) {
    let before = snapshot(journal);
    let (expected, accesses) = compare_frozen(journal, writer, roster);
    reset_touched(journal, &before, writer, roster);
    assert_eq!(snapshot(journal), before);
    (before, expected, accesses)
}

#[test]
fn shared_begin_benchmark_fixtures_match_frozen_state_storage_and_reset() {
    for (count, capacity, pattern) in cases() {
        let (mut journal, writer, roster) = fixture(count, capacity, pattern);
        let (before, expected, accesses) = qualify(&mut journal, writer, &roster);
        for baseline in [true, false] {
            let result = if baseline {
                journal.baseline_begin_write_v1(writer, &roster)
            } else {
                journal.begin_write(writer, &roster)
            };
            assert_eq!(result, expected);
            assert_eq!(journal.indexed_accesses.get(), accesses);
            reset_touched(&mut journal, &before, writer, &roster);
            assert_eq!(snapshot(&journal), before);
        }
    }
}

#[test]
#[ignore = "manual release-mode shared Begin comparison"]
fn shared_begin_performance() {
    use std::hint::black_box;
    use std::time::Instant;
    for (count, capacity, pattern) in cases() {
        let (mut journal, writer, roster) = fixture(count, capacity, pattern);
        let (before, expected, accesses) = qualify(&mut journal, writer, &roster);
        let iterations = (1_048_576 / count.max(1)).clamp(256, 65536);
        for round in 0..7 {
            for turn in 0..2 {
                let candidate = (round + turn) % 2 == 0;
                let mut elapsed = 0u128;
                for _ in 0..iterations {
                    reset_touched(&mut journal, &before, writer, &roster);
                    let start = Instant::now();
                    let result = if candidate {
                        black_box(&mut journal).begin_write(black_box(writer), black_box(&roster))
                    } else {
                        black_box(&mut journal)
                            .baseline_begin_write_v1(black_box(writer), black_box(&roster))
                    };
                    let result = black_box(result);
                    elapsed += start.elapsed().as_nanos();
                    assert_eq!(result, expected);
                    assert_eq!(journal.indexed_accesses.get(), accesses);
                }
                reset_touched(&mut journal, &before, writer, &roster);
                assert_eq!(snapshot(&journal), before);
                std::println!(
                    "begin_execution,{count},{capacity},{pattern},{round},{},{iterations},{elapsed},{accesses}",
                    if candidate { "shared" } else { "baseline" }
                );
            }
        }
    }
}
