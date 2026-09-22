use super::*;

mod benchmark_baseline;
mod enrollment;

fn input(slots: &[usize]) -> Vec<Option<ContextAllocationReferenceV1>> {
    slots
        .iter()
        .enumerate()
        .map(|(i, &slot)| {
            Some(ContextAllocationReferenceV1 {
                slot,
                key: ContextAllocationKeyV1 {
                    context_generation: i as u64,
                    local: u64::MAX - i as u64,
                },
            })
        })
        .collect()
}

fn contents(values: &[Option<ContextAllocationReferenceV1>]) -> Vec<(usize, u64, u64)> {
    let mut result: Vec<_> = values
        .iter()
        .map(|v| {
            let v = v.unwrap();
            (v.slot, v.key.context_generation, v.key.local)
        })
        .collect();
    result.sort_unstable();
    result
}

fn check(slots: &[usize]) {
    let mut values = input(slots);
    let before = values.clone();
    let expected = contents(&before);
    let storage = (values.as_ptr(), values.capacity());
    adaptive_sort_slots(&mut values);
    assert_eq!(contents(&values), expected);
    assert!(
        values
            .windows(2)
            .all(|p| p[0].unwrap().slot <= p[1].unwrap().slot)
    );
    assert_eq!((values.as_ptr(), values.capacity()), storage);
    if slots.windows(2).all(|p| p[0] <= p[1]) {
        assert_eq!(values, before);
    }
    for depth in [0, 1, 2, 8] {
        values.copy_from_slice(&before);
        enrollment_introsort(&mut values, depth);
        assert_eq!(contents(&values), expected);
        assert!(
            values
                .windows(2)
                .all(|p| p[0].unwrap().slot <= p[1].unwrap().slot)
        );
    }
}

#[test]
fn adaptive_exhaustive_small_sort_and_partition() {
    for len in 0..=7u32 {
        for code in 0..3usize.pow(len) {
            let mut code = code;
            let slots: Vec<_> = (0..len)
                .map(|_| {
                    let slot = code % 3;
                    code /= 3;
                    slot
                })
                .collect();
            check(&slots);
            for pivot in [0, 1, 2, 3, usize::MAX] {
                let mut values = input(&slots);
                let expected = contents(&values);
                let (less, greater) = enrollment_partition(&mut values, pivot);
                assert!(less <= greater && greater <= values.len());
                assert_eq!(contents(&values), expected);
                assert!(values[..less].iter().all(|v| v.unwrap().slot < pivot));
                assert!(
                    values[less..greater]
                        .iter()
                        .all(|v| v.unwrap().slot == pivot)
                );
                assert!(values[greater..].iter().all(|v| v.unwrap().slot > pivot));
                for partition in [enrollment_binary_partition, enrollment_lomuto] {
                    let mut values = input(&slots);
                    let (less, greater) = partition(&mut values, pivot);
                    assert_eq!(less, greater);
                    assert!(greater <= values.len());
                    assert_eq!(contents(&values), expected);
                    assert!(values[..less].iter().all(|v| v.unwrap().slot <= pivot));
                    assert!(values[greater..].iter().all(|v| v.unwrap().slot >= pivot));
                }
            }
        }
    }
}

#[test]
fn adaptive_patterns_thresholds_and_forced_fallback() {
    for len in [
        0, 1, 2, 8, 15, 16, 17, 20, 31, 32, 33, 64, 255, 256, 257, 4096,
    ] {
        check(&(0..len).collect::<Vec<_>>());
        check(&(0..len).rev().collect::<Vec<_>>());
        check(&(0..len).map(|i| (len - i) / 4).collect::<Vec<_>>());
        check(&(0..len).map(|i| i.min(len - i)).collect::<Vec<_>>());
        check(&(0..len).map(|i| i % 7).collect::<Vec<_>>());
        check(&(0..len).map(|_| usize::MAX).collect::<Vec<_>>());
        check(
            &(0..len)
                .map(|i| [usize::MAX, 0, usize::MAX - 1][i % 3])
                .collect::<Vec<_>>(),
        );
        let mut shuffled: Vec<_> = (0..len).collect();
        let mut seed = 0x9e3779b97f4a7c15u64;
        for i in (1..len).rev() {
            seed ^= seed << 13;
            seed ^= seed >> 7;
            seed ^= seed << 17;
            shuffled.swap(i, seed as usize % (i + 1));
        }
        check(&shuffled);
    }
}

#[test]
fn adaptive_depth_budget_matches_bit_length() {
    for len in [0usize, 1, 2, 3, 15, 16, 17, 4096, usize::MAX] {
        assert_eq!(
            enrollment_depth(len),
            if len == 0 { 0 } else { len.ilog2() * 2 }
        );
    }
}

#[test]
#[ignore = "manual release-mode adaptive candidate comparison"]
fn adaptive_performance() {
    use std::hint::black_box;
    use std::time::Instant;
    for len in [8usize, 16, 17, 64, 255, 256, 257, 512, 4096] {
        for pattern in ["ascending", "descending", "shuffled", "duplicates"] {
            let mut slots: Vec<_> = (0..len).collect();
            match pattern {
                "descending" => slots.reverse(),
                "duplicates" => slots.iter_mut().for_each(|v| *v %= 7),
                "shuffled" => {
                    let mut seed = 0x9e3779b97f4a7c15u64;
                    for i in (1..len).rev() {
                        seed ^= seed << 13;
                        seed ^= seed >> 7;
                        seed ^= seed << 17;
                        slots.swap(i, seed as usize % (i + 1));
                    }
                }
                _ => {}
            }
            check(&slots);
            let fixture = input(&slots);
            let mut work = fixture.clone();
            let iterations = (1_000_000 / len).max(100);
            for round in 0..7 {
                for candidate in if round % 2 == 0 {
                    [false, true]
                } else {
                    [true, false]
                } {
                    let mut elapsed = 0u128;
                    for _ in 0..iterations {
                        work.copy_from_slice(&fixture);
                        let start = Instant::now();
                        if candidate {
                            adaptive_sort_slots(black_box(&mut work));
                        } else {
                            black_box(&mut work).sort_unstable_by_key(|v| v.unwrap().slot);
                        }
                        black_box(&work);
                        elapsed += start.elapsed().as_nanos();
                    }
                    std::println!(
                        "adaptive,{len},{pattern},{round},{candidate},{iterations},{elapsed}"
                    );
                }
            }
        }
    }
}
