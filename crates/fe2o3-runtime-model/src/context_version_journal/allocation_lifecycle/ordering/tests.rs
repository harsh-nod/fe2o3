use super::*;

mod benchmark_baseline;

fn reference(slot: usize, payload: usize) -> Option<ContextAllocationReferenceV1> {
    Some(ContextAllocationReferenceV1 {
        slot,
        key: ContextAllocationKeyV1 {
            context_generation: payload as u64,
            local: u64::MAX - payload as u64,
        },
    })
}

fn payloads(values: &[Option<ContextAllocationReferenceV1>]) -> Vec<(usize, u64, u64)> {
    let mut result: Vec<_> = values
        .iter()
        .map(|value| {
            let value = value.unwrap();
            (value.slot, value.key.context_generation, value.key.local)
        })
        .collect();
    result.sort_unstable();
    result
}

fn check_sort(mut values: Vec<Option<ContextAllocationReferenceV1>>) {
    let original = values.clone();
    let expected = payloads(&values);
    let storage = (values.as_ptr(), values.capacity());
    let sorted = values
        .windows(2)
        .all(|pair| pair[0].unwrap().slot <= pair[1].unwrap().slot);
    assert_eq!(enrollment_sorted(&values), sorted);
    sort_slots(&mut values);
    assert_eq!(payloads(&values), expected);
    assert!(enrollment_sorted(&values));
    assert_eq!((values.as_ptr(), values.capacity()), storage);
    if sorted {
        assert_eq!(values, original);
    }
    for slot in [0, 1, 2, 3, 7, 15, 31, usize::MAX - 1, usize::MAX] {
        assert_eq!(
            contains_slot(&values, slot),
            values.iter().any(|value| value.unwrap().slot == slot)
        );
    }
}

#[test]
fn ordering_exhaustive_duplicate_slots_preserves_complete_references() {
    for len in 0..=7u32 {
        for code in 0..3usize.pow(len) {
            let mut remaining = code;
            let values = (0..len as usize)
                .map(|index| {
                    let value = reference(remaining % 3, index);
                    remaining /= 3;
                    value
                })
                .collect();
            check_sort(values);
        }
    }
}

#[test]
fn ordering_long_runs_and_extreme_slots() {
    for len in [0, 1, 2, 3, 31, 32, 33, 255, 256, 257, 4096] {
        check_sort((0..len).map(|i| reference(i, i)).collect());
        check_sort((0..len).map(|i| reference(len - i, i)).collect());
        check_sort((0..len).map(|i| reference(i % 11, i)).collect());
        check_sort((0..len).map(|i| reference(usize::MAX - i, i)).collect());
    }
}

#[test]
fn key_search_includes_duplicate_and_extreme_coordinates() {
    let mut entries = Vec::new();
    let coordinates = [0, 1, 2, 7, u64::MAX - 1, u64::MAX];
    for context_generation in coordinates {
        for local in coordinates {
            let key = ContextAllocationKeyV1 {
                context_generation,
                local,
            };
            entries.push(ContextAllocationEnrollmentV1 {
                key,
                device: ContextJournalDeviceKeyV1 {
                    context_generation: 0,
                    local: 0,
                },
                byte_extent: 0,
            });
            entries.push(*entries.last().unwrap());
        }
    }
    for left in &entries {
        for right in &entries {
            assert_eq!(enrollment_less(left.key, right.key), left.key < right.key);
        }
    }
    for length in 0..=entries.len() {
        let values = &entries[..length];
        for context_generation in [0, 1, 3, 7, 8, u64::MAX - 1, u64::MAX] {
            for local in [0, 1, 3, 7, 8, u64::MAX - 1, u64::MAX] {
                let key = ContextAllocationKeyV1 {
                    context_generation,
                    local,
                };
                assert_eq!(
                    contains_key(values, key),
                    values.iter().any(|v| v.key == key)
                );
            }
        }
    }
}

#[test]
fn swap_self_and_range_frame() {
    let mut values: Vec<_> = (0..31).map(|i| reference(i, i)).collect();
    let before = values.clone();
    enrollment_swap(&mut values, 7, 7);
    assert_eq!(values, before);
    enrollment_swap(&mut values, 0, 30);
    assert_eq!(values[0], before[30]);
    assert_eq!(values[30], before[0]);
    assert_eq!(values[1..30], before[1..30]);
}

#[test]
#[ignore = "manual release-mode CPU ordering comparison"]
fn ordering_performance() {
    use std::hint::black_box;
    use std::time::Instant;

    for len in [8usize, 64, 512, 4096] {
        for pattern in ["ascending", "descending", "shuffled", "duplicates"] {
            let mut input: Vec<_> = (0..len).map(|i| reference(i, i)).collect();
            match pattern {
                "descending" => input.reverse(),
                "shuffled" => {
                    let mut seed = 0x9e3779b97f4a7c15u64;
                    for i in (1..len).rev() {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        input.swap(i, seed as usize % (i + 1));
                    }
                }
                "duplicates" => {
                    for (i, cell) in input.iter_mut().enumerate() {
                        *cell = reference(i % 7, i);
                    }
                }
                _ => {}
            }
            check_sort(input.clone());
            let mut work = input.clone();
            let iterations = (1_000_000 / len).max(100);
            for round in 0..7 {
                for candidate in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let mut elapsed = 0u128;
                    for _ in 0..iterations {
                        work.copy_from_slice(&input);
                        let start = Instant::now();
                        if candidate {
                            sort_slots(black_box(&mut work));
                        } else {
                            black_box(&mut work).sort_unstable_by_key(|v| v.unwrap().slot);
                        }
                        black_box(&work);
                        elapsed += start.elapsed().as_nanos();
                    }
                    std::println!(
                        "ordering,{len},{pattern},{round},{candidate},{iterations},{elapsed}"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "manual release-mode CPU search comparison"]
fn search_performance() {
    use std::hint::black_box;
    use std::time::Instant;

    for len in [0usize, 1, 2, 8, 64, 512, 4096] {
        let values: Vec<_> = (0..len).map(|i| reference(i * 2, i)).collect();
        let entries: Vec<_> = (0..len)
            .map(|i| ContextAllocationEnrollmentV1 {
                key: ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: (i * 2) as u64,
                },
                device: ContextJournalDeviceKeyV1 {
                    context_generation: 7,
                    local: 11,
                },
                byte_extent: 64,
            })
            .collect();
        for pattern in ["repeated", "mixed"] {
            let mut seed = 0x9e3779b97f4a7c15u64;
            let queries: Vec<_> = (0..1024)
                .map(|_| {
                    seed ^= seed << 13;
                    seed ^= seed >> 7;
                    seed ^= seed << 17;
                    if pattern == "repeated" {
                        len
                    } else {
                        seed as usize % (len * 2 + 1)
                    }
                })
                .collect();
            for &slot in &queries {
                let key = ContextAllocationKeyV1 {
                    context_generation: 7,
                    local: slot as u64,
                };
                assert_eq!(
                    contains_slot(&values, slot),
                    values
                        .binary_search_by_key(&slot, |v| v.unwrap().slot)
                        .is_ok()
                );
                assert_eq!(
                    contains_key(&entries, key),
                    entries.binary_search_by_key(&key, |v| v.key).is_ok()
                );
            }
            for kind in ["key", "slot"] {
                for round in 0..7 {
                    for candidate in if round % 2 == 0 {
                        [false, true]
                    } else {
                        [true, false]
                    } {
                        let start = Instant::now();
                        for _ in 0..128 {
                            for &query in black_box(&queries) {
                                let found = if kind == "key" {
                                    let key = ContextAllocationKeyV1 {
                                        context_generation: 7,
                                        local: query as u64,
                                    };
                                    if candidate {
                                        contains_key(black_box(&entries), black_box(key))
                                    } else {
                                        black_box(&entries)
                                            .binary_search_by_key(&black_box(key), |v| v.key)
                                            .is_ok()
                                    }
                                } else if candidate {
                                    contains_slot(black_box(&values), black_box(query))
                                } else {
                                    black_box(&values)
                                        .binary_search_by_key(&black_box(query), |v| {
                                            v.unwrap().slot
                                        })
                                        .is_ok()
                                };
                                black_box(found);
                            }
                        }
                        let elapsed = start.elapsed().as_nanos();
                        std::println!(
                            "search,{len},{pattern},{kind},{round},{candidate},131072,{elapsed}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn production_uses_shared_sort_and_searches() {
    let source = include_str!("../../allocation_lifecycle.rs");
    assert!(source.contains("ordering::sort_enrollment_slots(output)"));
    assert!(source.contains("ordering::contains_key(canonical, entry.key)"));
    assert!(source.contains("ordering::contains_slot(output, slot)"));
    assert!(!source.contains("ordering::sort_slots"));
    assert!(!source.contains("sort_unstable"));
    let adapter = include_str!("../ordering.rs");
    assert!(
        adapter.contains("pub(super) use adaptive::adaptive_sort_slots as sort_enrollment_slots;")
    );
    assert!(!adapter.contains("#[cfg(test)]\nmod adaptive;"));
}

#[test]
#[ignore = "manual release-mode complete enrollment comparison"]
fn enrollment_performance() {
    use std::hint::black_box;
    use std::time::Instant;

    for batch in [8usize, 64, 512] {
        for scenario in [
            "fresh",
            "half",
            "replay_first",
            "replay_last",
            "capacity",
            "alias",
        ] {
            let entries: Vec<_> = (0..batch)
                .map(|i| ContextAllocationEnrollmentV1 {
                    key: ContextAllocationKeyV1 {
                        context_generation: 7,
                        local: i as u64 + 1,
                    },
                    device: ContextJournalDeviceKeyV1 {
                        context_generation: 7,
                        local: 11,
                    },
                    byte_extent: 64,
                })
                .collect();
            let mut journal = ContextVersionJournalV1::new(7, 1024, 1).unwrap();
            if scenario == "half" {
                for i in 0..512 {
                    journal.allocations[i] = Some(AllocationEntryV1 {
                        key: ContextAllocationKeyV1 {
                            context_generation: 7,
                            local: 2048 + i as u64,
                        },
                        device: entries[0].device,
                        byte_extent: 64,
                        attempt_epoch: 0,
                        content_lineage: 0,
                        pending_member: None,
                    });
                }
                journal.allocation_free.truncate(512);
            } else if scenario.starts_with("replay") {
                let slot = if scenario == "replay_first" { 0 } else { 1023 };
                journal.allocations[slot] = Some(AllocationEntryV1 {
                    key: entries[0].key,
                    device: entries[0].device,
                    byte_extent: 64,
                    attempt_epoch: 0,
                    content_lineage: 0,
                    pending_member: None,
                });
                journal.allocation_free.retain(|&value| value != slot);
            } else if scenario == "capacity" {
                journal.allocation_free.clear();
            } else if scenario == "alias" {
                journal.allocation_free[0] = 0;
            }
            let allocations = journal.allocations.clone();
            let free = journal.allocation_free.clone();
            let mut output = std::vec![None; batch];
            let mut baseline_result = None;
            for candidate in [false, true] {
                journal.allocations.copy_from_slice(&allocations);
                journal.allocation_free.clone_from(&free);
                output.fill(None);
                let result = if candidate {
                    journal.enroll_allocations(&entries, &mut output)
                } else {
                    benchmark_baseline::enroll_allocations(&mut journal, &entries, &mut output)
                };
                let observed = (
                    result,
                    journal.allocations.clone(),
                    journal.allocation_free.clone(),
                    output.clone(),
                );
                if let Some(expected) = &baseline_result {
                    assert_eq!(&observed, expected);
                } else {
                    baseline_result = Some(observed);
                }
            }
            for round in 0..7 {
                for candidate in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let mut elapsed = 0u128;
                    for _ in 0..1024 {
                        journal.allocations.copy_from_slice(&allocations);
                        journal.allocation_free.clone_from(&free);
                        output.fill(None);
                        let start = Instant::now();
                        let result = if candidate {
                            black_box(&mut journal)
                                .enroll_allocations(black_box(&entries), black_box(&mut output))
                        } else {
                            benchmark_baseline::enroll_allocations(
                                black_box(&mut journal),
                                black_box(&entries),
                                black_box(&mut output),
                            )
                        };
                        let _ = black_box(result);
                        elapsed += start.elapsed().as_nanos();
                        black_box(&output);
                    }
                    std::println!(
                        "enrollment,{batch},{scenario},{round},{candidate},1024,{elapsed}"
                    );
                }
            }
        }
    }
}
