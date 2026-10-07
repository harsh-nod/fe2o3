use super::*;
use crate::{R67ResourceKindV1 as K, R70ResourceBatchErrorV1, r70_resource_batch_reserve_v1};
use alloc::vec;

const KINDS: [K; 19] = [
    K::LogicalPayloadBytes,
    K::RequestedAllocationBytes,
    K::ResidentHostAllocationBytes,
    K::ResidentDeviceAllocationBytes,
    K::ExecutableHostImageBytes,
    K::ExecutableDeviceBytes,
    K::ControlResidentBytes,
    K::QueueResidentBytes,
    K::SignalResidentBytes,
    K::KernargResidentBytes,
    K::QueueSlots,
    K::SignalSlots,
    K::KernargSlots,
    K::OperationSlots,
    K::ReplyBytes,
    K::ReplyCells,
    K::TerminalRecordBytes,
    K::QuarantineBookkeepingBytes,
    K::AllocationRecords,
];

fn vector(value: u64) -> R67ResourceVectorV1 {
    KINDS
        .iter()
        .fold(R67ResourceVectorV1::ZERO, |v, &kind| v.with(kind, value))
}

// The original adapter loop, including checked counters before the R70 call.
fn reference(
    facts: &[R75ResourceDomainFactsV1; 4],
    depth: usize,
    profile: usize,
    charges: &[R67ResourceVectorV1],
    owner: u64,
) -> Result<R75ResourceDomainPlanV1, R75ResourceDomainErrorV1> {
    use R75ResourceDomainErrorV1 as E;
    if ![3, 4].contains(&profile) || depth == 0 || depth > profile || owner == 0 {
        return Err(E::Invariant);
    }
    let mut result = R75ResourceDomainPlanV1 {
        next_used: [R67ResourceVectorV1::ZERO; 4],
        next_reserved: [0; 4],
        next_owner: owner,
    };
    for (i, fact) in facts[..depth].iter().enumerate() {
        let occupied = fact
            .counts
            .iter()
            .try_fold(0usize, |sum, &n| sum.checked_add(n))
            .filter(|&n| n <= fact.record_limit)
            .ok_or(E::Invariant)?;
        let plan = r70_resource_batch_reserve_v1(
            fact.used,
            charges,
            fact.capacity,
            fact.record_limit - occupied,
            owner,
        )
        .map_err(|error| match error {
            R70ResourceBatchErrorV1::InvalidMemberCount => E::InvalidMemberCount,
            R70ResourceBatchErrorV1::Capacity => E::Capacity,
            R70ResourceBatchErrorV1::RecordCapacity => E::RecordCapacity,
            R70ResourceBatchErrorV1::GenerationExhausted => E::GenerationExhausted,
        })?;
        result.next_used[i] = plan.next_used;
        result.next_reserved[i] = fact.counts[0] + charges.len();
        result.next_owner = plan.next_owner;
    }
    Ok(result)
}

fn compare(
    facts: [R75ResourceDomainFactsV1; 4],
    depth: usize,
    profile: usize,
    charges: &[R67ResourceVectorV1],
    owner: u64,
) {
    assert_eq!(
        r75_resource_domain_reserve_v1(&facts, depth, profile, charges, owner),
        reference(&facts, depth, profile, charges, owner),
        "depth={depth} profile={profile} count={} owner={owner} facts={facts:?}",
        charges.len()
    );
}

fn fact() -> R75ResourceDomainFactsV1 {
    R75ResourceDomainFactsV1 {
        used: vector(1),
        capacity: vector(u64::MAX),
        counts: [1, 2, 3],
        record_limit: 65_542,
    }
}

#[test]
fn r75_each_ancestor_coordinate_and_final_member_matches_old_loop() {
    for profile in [3, 4] {
        for depth in 1..=profile {
            for level in 0..depth {
                for kind in KINDS {
                    for limit in [3, 4, u64::MAX] {
                        let mut facts = [fact(); 4];
                        facts[level].capacity = facts[level].capacity.with(kind, limit);
                        compare(
                            facts,
                            depth,
                            profile,
                            &[vector(1), vector(1), vector(1)],
                            17,
                        );
                        facts[level].used = facts[level].used.with(kind, u64::MAX);
                        compare(
                            facts,
                            depth,
                            profile,
                            &[vector(0), vector(0), vector(1)],
                            17,
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn r75_boundary_arithmetic_and_owner_interval_match_old_loop() {
    let values = [0, 1, u64::MAX / 2, u64::MAX - 1, u64::MAX];
    for depth in 1..=4 {
        for initial in values {
            for first in values {
                for second in values {
                    for capacity in values {
                        let mut facts = [fact(); 4];
                        for (i, fact) in facts[..depth].iter_mut().enumerate() {
                            fact.used = vector(initial).with(K::ReplyBytes, i as u64);
                            fact.capacity = vector(capacity);
                        }
                        for owner in [0, 1, u64::MAX - 2, u64::MAX - 1, u64::MAX] {
                            compare(facts, depth, 4, &[vector(first), vector(second)], owner);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn r75_counter_failures_preserve_leaf_first_error_identity() {
    use R75ResourceDomainErrorV1 as E;
    for depth in 1..=4 {
        for bad in 0..depth {
            for counts in [
                [usize::MAX, 1, 0],
                [0, usize::MAX, 1],
                [0, 0, 65_543],
                [65_540, 1, 0],
            ] {
                let mut facts = [fact(); 4];
                facts[bad].counts = counts;
                for owner in [0, 1, u64::MAX] {
                    compare(facts, depth, 4, &[vector(1), vector(1)], owner);
                    let mut masked = facts;
                    masked[0].capacity = vector(0);
                    compare(masked, depth, 4, &[vector(1), vector(1)], owner);
                }
            }
        }
    }
    let mut facts = [fact(); 4];
    facts[0].capacity = vector(0);
    facts[1].counts = [usize::MAX, 1, 0];
    assert_eq!(
        r75_resource_domain_reserve_v1(&facts, 4, 4, &[vector(1)], 1),
        Err(E::Capacity)
    );
    facts[0].record_limit = 6;
    assert_eq!(
        r75_resource_domain_reserve_v1(&facts, 4, 4, &[vector(1)], u64::MAX),
        Err(E::RecordCapacity)
    );
}

#[test]
fn r75_member_profile_bounds_and_inactive_padding_match_old_loop() {
    for count in [0, 1, 2, 3, 65_536, 65_537] {
        let charges = vec![vector(0); count];
        for depth in 0..=5 {
            for profile in [0, 2, 3, 4, 5] {
                compare([fact(); 4], depth, profile, &charges, 1);
            }
        }
    }
    for depth in 1..=4 {
        let mut facts = [fact(); 4];
        for fact in &mut facts[depth..] {
            fact.counts = [usize::MAX; 3];
        }
        let plan = r75_resource_domain_reserve_v1(&facts, depth, 4, &[vector(1)], 9).unwrap();
        for i in depth..4 {
            assert_eq!(plan.next_used[i], R67ResourceVectorV1::ZERO);
            assert_eq!(plan.next_reserved[i], 0);
        }
        compare(facts, depth, 4, &[vector(1)], 9);
    }
}

#[test]
fn r75_deterministic_mixed_vectors_match_old_loop() {
    let mut seed = 0x75baf00du64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for case in 0..4096 {
        let mut facts = [fact(); 4];
        let mut charges = [vector(0); 3];
        for kind in KINDS {
            for fact in &mut facts {
                fact.used = fact.used.with(kind, next() % 16);
                fact.capacity = fact.capacity.with(kind, 8 + next() % 128);
            }
            for charge in &mut charges {
                *charge = charge.with(kind, next() % 8);
            }
        }
        compare(facts, 1 + case % 4, 4, &charges[..1 + case % 3], 1);
    }
}

#[test]
#[ignore = "development CPU planner microbenchmark, not native runtime qualification"]
fn r75_planner_microbenchmark() {
    use std::{hint::black_box, time::Instant};
    std::println!("depth,count,round,old_ns,new_ns");
    for depth in 1..=4 {
        for count in [1, 2, 16, 1024, 65_536] {
            let facts = [fact(); 4];
            let charges = vec![vector(1); count];
            compare(facts, depth, 4, &charges, 1);
            let repeats = (32_768 / count).clamp(2, 32_768);
            let run = |old: bool| {
                let start = Instant::now();
                for _ in 0..repeats {
                    let result = if old {
                        reference(
                            black_box(&facts),
                            black_box(depth),
                            4,
                            black_box(&charges),
                            1,
                        )
                    } else {
                        r75_resource_domain_reserve_v1(
                            black_box(&facts),
                            black_box(depth),
                            4,
                            black_box(&charges),
                            1,
                        )
                    };
                    let _ = black_box(result);
                }
                start.elapsed().as_nanos() as f64 / repeats as f64
            };
            let _ = (run(true), run(false));
            for round in 0..9 {
                let (old, new) = if round % 2 == 0 {
                    (run(true), run(false))
                } else {
                    let new = run(false);
                    (run(true), new)
                };
                std::println!("{depth},{count},{round},{old:.2},{new:.2}");
            }
        }
    }
}

#[test]
fn r75_shared_vector_arithmetic_matches_independent_wide_oracle() {
    let values = [0, 1, u64::MAX / 2, u64::MAX - 1, u64::MAX];
    for kind in KINDS {
        for used in values {
            for charge in values {
                for capacity in values {
                    let u = vector(2).with(kind, used);
                    let c = vector(1).with(kind, charge);
                    let cap = vector(3).with(kind, capacity);
                    let mut expected_add = Some(R67ResourceVectorV1::ZERO);
                    let mut expected_sub = Some(R67ResourceVectorV1::ZERO);
                    for coordinate in KINDS {
                        let a = u128::from(u.get(coordinate));
                        let b = u128::from(c.get(coordinate));
                        expected_add = expected_add.and_then(|v| {
                            (a + b <= u128::from(cap.get(coordinate)))
                                .then(|| v.with(coordinate, (a + b) as u64))
                        });
                        expected_sub = expected_sub
                            .and_then(|v| (a >= b).then(|| v.with(coordinate, (a - b) as u64)));
                    }
                    assert_eq!(r67_resource_reserve_v1(u, c, cap), expected_add);
                    assert_eq!(r67_resource_release_v1(u, c), expected_sub);
                }
            }
        }
    }
}
