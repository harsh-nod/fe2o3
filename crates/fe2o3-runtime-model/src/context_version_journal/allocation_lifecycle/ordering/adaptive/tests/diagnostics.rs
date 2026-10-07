use super::*;

// Frozen simple partition from 5fa588909; only the macro parameters are expanded.
fn simple_lomuto(
    values: &mut [Option<ContextAllocationReferenceV1>],
    pivot: usize,
) -> (usize, usize) {
    let mut less = 0;
    let mut scan = 0;
    while scan < values.len() {
        let below = if values[scan].unwrap().slot < pivot {
            1
        } else {
            0
        };
        enrollment_swap(values, less, scan);
        less += below;
        scan += 1;
    }
    (less, less)
}

fn fixture(len: usize, mut seed: u64) -> Vec<Option<ContextAllocationReferenceV1>> {
    let mut slots: Vec<_> = (0..len).collect();
    for i in (1..len).rev() {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        slots.swap(i, seed as usize % (i + 1));
    }
    input(&slots)
}

#[derive(Default)]
struct Work {
    partitioned: usize,
    partitions: usize,
    heap: usize,
    max_depth: u32,
}

// Untimed algorithm diagnostic, not a proof of machine instruction counts.
fn profile(
    values: &mut [Option<ContextAllocationReferenceV1>],
    depth: u32,
    level: u32,
    cyclic: bool,
    ninther: bool,
    work: &mut Work,
) {
    work.max_depth = work.max_depth.max(level);
    if values.len() <= 20 {
        enrollment_insertion(values);
        return;
    }
    if depth == 0 {
        work.heap += 1;
        enrollment_heapsort(values);
        return;
    }
    let first = values[0].unwrap().slot;
    let middle = values[values.len() / 2].unwrap().slot;
    let last = values[values.len() - 1].unwrap().slot;
    let pivot = if ninther {
        enrollment_pivot(values)
    } else {
        enrollment_median_of_three(first, middle, last)
    };
    work.partitioned += values.len();
    work.partitions += 1;
    let (less, greater) = if (first == pivot && (middle == pivot || last == pivot))
        || (middle == pivot && last == pivot)
    {
        enrollment_partition(values, pivot)
    } else if values.len() >= 256 {
        if cyclic {
            enrollment_lomuto(values, pivot)
        } else {
            simple_lomuto(values, pivot)
        }
    } else {
        enrollment_binary_partition(values, pivot)
    };
    let (left, tail) = values.split_at_mut(less);
    let (_, right) = tail.split_at_mut(greater - less);
    profile(left, depth - 1, level + 1, cyclic, ninther, work);
    profile(right, depth - 1, level + 1, cyclic, ninther, work);
}

#[test]
#[ignore = "manual partition timing and algorithm-work diagnostic"]
fn adaptive_partition_diagnostics() {
    use std::hint::black_box;
    use std::time::Instant;

    for len in [4096, 65536] {
        for seed in [1, 0x9e3779b97f4a7c15u64, 0xdeadbeef] {
            let original = fixture(len, seed);
            let expected = contents(&original);
            for cyclic in [false, true] {
                for ninther in [false, true] {
                    let mut values = original.clone();
                    let mut work = Work::default();
                    profile(
                        &mut values,
                        enrollment_depth(len),
                        0,
                        cyclic,
                        ninther,
                        &mut work,
                    );
                    assert_eq!(contents(&values), expected);
                    assert!(
                        values
                            .windows(2)
                            .all(|v| v[0].unwrap().slot <= v[1].unwrap().slot)
                    );
                    std::println!(
                        "partition_work,{len},{seed},{cyclic},{ninther},{},{},{},{}",
                        work.partitioned,
                        work.partitions,
                        work.heap,
                        work.max_depth
                    );
                }
            }
        }
    }
    for len in [256, 4096, 65536] {
        let original = fixture(len, 0x9e3779b97f4a7c15);
        let mut values = original.clone();
        let expected = contents(&original);
        let pivot = len / 2;
        for cyclic in [false, true] {
            values.copy_from_slice(&original);
            let cut = if cyclic {
                enrollment_lomuto(&mut values, pivot)
            } else {
                simple_lomuto(&mut values, pivot)
            };
            assert_eq!(cut, (pivot, pivot));
            assert_eq!(contents(&values), expected);
            assert!(values[..pivot].iter().all(|v| v.unwrap().slot < pivot));
            assert!(values[pivot..].iter().all(|v| v.unwrap().slot >= pivot));
        }
        let iterations = (1_000_000 / len).max(16);
        for round in 0..7 {
            for cyclic in if round % 2 == 0 {
                [false, true]
            } else {
                [true, false]
            } {
                let mut elapsed = 0;
                for _ in 0..iterations {
                    values.copy_from_slice(&original);
                    let start = Instant::now();
                    let cut = if cyclic {
                        enrollment_lomuto(black_box(&mut values), pivot)
                    } else {
                        simple_lomuto(black_box(&mut values), pivot)
                    };
                    black_box(cut);
                    black_box(&values);
                    elapsed += start.elapsed().as_nanos();
                }
                std::println!("partition,{len},{round},{cyclic},{iterations},{elapsed}");
            }
        }
    }
}
