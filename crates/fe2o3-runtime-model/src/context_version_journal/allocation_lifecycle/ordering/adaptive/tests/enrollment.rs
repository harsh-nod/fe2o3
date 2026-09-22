use super::*;

fn fixture(
    batch: usize,
    pattern: &str,
) -> (ContextVersionJournalV1, Vec<ContextAllocationEnrollmentV1>) {
    let capacity = (batch * 2).max(1024);
    let mut journal = ContextVersionJournalV1::new(7, capacity, 1).unwrap();
    match pattern {
        "descending" => journal.allocation_free.reverse(),
        "shuffled" => {
            let mut seed = 0x9e3779b97f4a7c15u64;
            for i in (1..capacity).rev() {
                seed ^= seed << 13;
                seed ^= seed >> 7;
                seed ^= seed << 17;
                journal.allocation_free.swap(i, seed as usize % (i + 1));
            }
        }
        "duplicates" => {
            journal.allocation_free[capacity - 1] = journal.allocation_free[capacity - 2]
        }
        "prefix_alias" => journal.allocation_free[0] = journal.allocation_free[capacity - 1],
        _ => {}
    }
    let entries = (0..batch)
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
    (journal, entries)
}

#[test]
fn adaptive_full_enrollment_matches_current_standard_sort() {
    for batch in [8, 16, 17, 64, 255, 256, 257, 512, 4096] {
        for pattern in [
            "ascending",
            "descending",
            "shuffled",
            "duplicates",
            "prefix_alias",
        ] {
            let (mut journal, entries) = fixture(batch, pattern);
            let allocations = journal.allocations.clone();
            let free = journal.allocation_free.clone();
            let mut output = std::vec![None; batch];
            let storage = (output.as_ptr(), output.capacity());
            let result = journal.enroll_allocations(&entries, &mut output);
            let expected = (
                result,
                journal.allocations.clone(),
                journal.allocation_free.clone(),
                output.clone(),
            );
            for candidate in [false, true] {
                journal.allocations.copy_from_slice(&allocations);
                journal.allocation_free.clone_from(&free);
                output.fill(None);
                let result = benchmark_baseline::enroll_allocations(
                    &mut journal,
                    candidate,
                    &entries,
                    &mut output,
                );
                assert_eq!(
                    (
                        result,
                        journal.allocations.clone(),
                        journal.allocation_free.clone(),
                        output.clone()
                    ),
                    expected
                );
                assert_eq!((output.as_ptr(), output.capacity()), storage);
            }
        }
    }
}

#[test]
#[ignore = "manual release-mode sorting-only enrollment comparison"]
fn sort_enrollment_performance() {
    use std::hint::black_box;
    use std::time::Instant;
    for batch in [8usize, 16, 17, 64, 255, 256, 257, 512, 4096] {
        for pattern in [
            "ascending",
            "descending",
            "shuffled",
            "duplicates",
            "prefix_alias",
        ] {
            let (mut journal, entries) = fixture(batch, pattern);
            let allocations = journal.allocations.clone();
            let free = journal.allocation_free.clone();
            let mut output = std::vec![None; batch];
            let iterations = (16_384 / batch).clamp(8, 256);
            let mut expected = None;
            for candidate in [false, true] {
                journal.allocations.copy_from_slice(&allocations);
                journal.allocation_free.clone_from(&free);
                output.fill(None);
                let result = benchmark_baseline::enroll_allocations(
                    &mut journal,
                    candidate,
                    &entries,
                    &mut output,
                );
                let actual = (
                    result,
                    journal.allocations.clone(),
                    journal.allocation_free.clone(),
                    output.clone(),
                );
                if let Some(expected) = &expected {
                    assert_eq!(&actual, expected);
                } else {
                    expected = Some(actual);
                }
            }
            for round in 0..7 {
                for candidate in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let mut elapsed = 0u128;
                    for _ in 0..iterations {
                        journal.allocations.copy_from_slice(&allocations);
                        journal.allocation_free.clone_from(&free);
                        output.fill(None);
                        let start = Instant::now();
                        let result = benchmark_baseline::enroll_allocations(
                            black_box(&mut journal),
                            candidate,
                            black_box(&entries),
                            black_box(&mut output),
                        );
                        let _ = black_box(result);
                        elapsed += start.elapsed().as_nanos();
                        black_box(&output);
                    }
                    std::println!(
                        "sort_enrollment,{batch},{pattern},{round},{candidate},{iterations},{elapsed}"
                    );
                }
            }
        }
    }
}
