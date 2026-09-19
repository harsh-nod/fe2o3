use super::*;
use std::cell::Cell;
use std::hint::black_box;
use std::time::Instant as StdInstant;

const REQUEST_BASE: u64 = 1_000_000;

struct Fixture {
    ids: Vec<u64>,
    active: HashMap<u64, XgmiRuntimeSubmissionV1>,
    completed: HashMap<u64, SubmissionRecordV1>,
    waiters: HashMap<u64, Vec<u64>>,
    retain_counts: HashMap<u64, usize>,
}

#[derive(Debug, Eq, PartialEq)]
struct Snapshot {
    ids: Vec<u64>,
    active: Vec<(u64, u64, Vec<u64>)>,
    completed: Vec<(u64, u64, BackendPollV1, bool)>,
    waiters: Vec<(u64, Vec<u64>)>,
    retain_counts: Vec<(u64, usize)>,
}

fn snapshot(fixture: &Fixture) -> Snapshot {
    let mut active: Vec<_> = fixture
        .active
        .iter()
        .map(|(key, record)| (*key, record.id, record.dependencies.clone()))
        .collect();
    active.sort_unstable_by_key(|entry| entry.0);
    let mut completed: Vec<_> = fixture
        .completed
        .iter()
        .map(|(key, record)| {
            (
                *key,
                record.stream,
                record.status,
                record.profile_dispatch_published,
            )
        })
        .collect();
    completed.sort_unstable_by_key(|entry| entry.0);
    let mut waiters: Vec<_> = fixture
        .waiters
        .iter()
        .map(|(key, value)| (*key, value.clone()))
        .collect();
    waiters.sort_unstable_by_key(|entry| entry.0);
    let mut retain_counts: Vec<_> = fixture
        .retain_counts
        .iter()
        .map(|(key, value)| (*key, *value))
        .collect();
    retain_counts.sort_unstable_by_key(|entry| entry.0);
    Snapshot {
        ids: fixture.ids.clone(),
        active,
        completed,
        waiters,
        retain_counts,
    }
}

fn succeeded(id: u64) -> SubmissionRecordV1 {
    SubmissionRecordV1 {
        stream: id,
        status: BackendPollV1::Succeeded,
        profile_dispatch_published: false,
    }
}

fn fixture(total: usize, requested: usize, dependencies_per_requested: usize) -> Fixture {
    assert!(requested > 0 && requested <= total);
    assert!(dependencies_per_requested <= MAX_RUNTIME_DEPENDENCIES_V1);
    let ids: Vec<_> = (0..requested)
        .map(|index| REQUEST_BASE + index as u64)
        .collect();
    let mut active = HashMap::with_capacity(total);
    let mut completed = HashMap::with_capacity(dependencies_per_requested);
    let mut waiters: HashMap<u64, Vec<u64>> = HashMap::new();
    let mut retain_counts = HashMap::new();
    for dependency in 1..=dependencies_per_requested as u64 {
        completed.insert(dependency, succeeded(dependency));
        retain_counts.insert(dependency, requested);
    }
    for id in ids.iter().copied() {
        let mut value = record(id, 0);
        value.dependencies = (1..=dependencies_per_requested as u64).collect();
        assert!(active.insert(id, value).is_none());
    }
    for offset in 0..total - requested {
        let id = REQUEST_BASE + requested as u64 + offset as u64;
        let dependency = ids[offset % requested];
        let mut value = record(id, offset & 1);
        value.dependencies.push(dependency);
        assert!(active.insert(id, value).is_none());
        waiters.entry(dependency).or_default().push(id);
        *retain_counts.entry(dependency).or_insert(0) += 1;
    }
    Fixture {
        ids,
        active,
        completed,
        waiters,
        retain_counts,
    }
}

// Frozen verbatim from d29afddf7dc971ac58ccbad43585291f31ec29e8.
fn reference_valid_dependency_indexes(
    ids: &[u64],
    active: &HashMap<u64, XgmiRuntimeSubmissionV1>,
    completed: &HashMap<u64, SubmissionRecordV1>,
    dependency_waiters: &HashMap<u64, Vec<u64>>,
    retain_counts: &HashMap<u64, usize>,
) -> bool {
    for id in ids {
        let record = &active[id];
        let waiters = dependency_waiters.get(id).map_or(&[][..], Vec::as_slice);
        let expected_waiters = active
            .values()
            .filter(|record| record.dependencies.contains(id))
            .count();
        if waiters.len() != expected_waiters
            || retain_counts.get(id).copied().unwrap_or(0) != expected_waiters
            || waiters.iter().enumerate().any(|(index, waiter)| {
                index > 0 && waiters[index - 1] >= *waiter
                    || active
                        .get(waiter)
                        .is_none_or(|record| !record.dependencies.contains(id))
            })
        {
            return false;
        }
        for (index, dependency) in record.dependencies.iter().enumerate() {
            if *dependency >= *id
                || record.dependencies[..index].contains(dependency)
                || completed
                    .get(dependency)
                    .is_none_or(|record| record.status != BackendPollV1::Succeeded)
                || dependency_waiters.contains_key(dependency)
                || retain_counts.get(dependency).is_none_or(|count| {
                    *count
                        != active
                            .values()
                            .filter(|record| record.dependencies.contains(dependency))
                            .count()
                })
            {
                return false;
            }
        }
    }
    true
}

fn reference(fixture: &Fixture) -> bool {
    reference_valid_dependency_indexes(
        &fixture.ids,
        &fixture.active,
        &fixture.completed,
        &fixture.waiters,
        &fixture.retain_counts,
    )
}

fn candidate(fixture: &Fixture) -> Result<bool, DependencyScratchCapacity> {
    valid_dependency_indexes(
        &fixture.ids,
        &fixture.active,
        &fixture.completed,
        &fixture.waiters,
        &fixture.retain_counts,
    )
}

fn injected(fixture: &Fixture, fail: bool) -> (Result<bool, DependencyScratchCapacity>, usize) {
    let mut calls = 0;
    let result = valid_dependency_indexes_with_reservation(
        &fixture.ids,
        &fixture.active,
        &fixture.completed,
        &fixture.waiters,
        &fixture.retain_counts,
        |storage, additional| {
            calls += 1;
            if fail {
                Vec::<DependencyUseCount>::new().try_reserve_exact(usize::MAX)
            } else {
                storage.try_reserve_exact(additional)
            }
        },
    );
    (result, calls)
}

fn assert_matches_reference(fixture: &Fixture) {
    assert_eq!(candidate(fixture), Ok(reference(fixture)));
}

#[test]
fn one_active_scan_counts_each_source_record_once_for_relevant_duplicates() {
    let target = REQUEST_BASE;
    let mut first = record(target + 1, 0);
    first.dependencies = vec![target, target, 777, 777, 999, 999];
    let mut second = record(target + 2, 1);
    second.dependencies = vec![target, target];
    let records = HashMap::from([(first.id, first), (second.id, second)]);
    let mut relevant = vec![
        DependencyUseCount {
            id: 999,
            records: 0,
            last_record: usize::MAX,
        },
        DependencyUseCount {
            id: target,
            records: 0,
            last_record: usize::MAX,
        },
    ];
    let visits = Cell::new(0);
    assert!(count_dependency_uses(
        &mut relevant,
        records.values().inspect(|_| visits.set(visits.get() + 1))
    ));
    assert_eq!(visits.get(), records.len());
    assert_eq!(
        relevant
            .iter()
            .map(|entry| (entry.id, entry.records))
            .collect::<Vec<_>>(),
        [(999, 1), (target, 2)]
    );
}

#[test]
fn selected_and_unselected_duplicate_dependencies_preserve_reference_semantics() {
    let mut unselected = fixture(1, 1, 0);
    let target = unselected.ids[0];
    let duplicate_id = target + 1;
    let mut duplicate = record(duplicate_id, 1);
    duplicate.dependencies = vec![target, target];
    unselected.active.insert(duplicate_id, duplicate);
    unselected.waiters.insert(target, vec![duplicate_id]);
    unselected.retain_counts.insert(target, 1);
    let unrelated_id = target + 2;
    let mut unrelated = record(unrelated_id, 0);
    unrelated.dependencies = vec![777, 777];
    unselected.active.insert(unrelated_id, unrelated);
    unselected.waiters.insert(888, vec![u64::MAX]);
    unselected.retain_counts.insert(888, usize::MAX);
    assert!(reference(&unselected));
    assert_matches_reference(&unselected);

    let mut selected = fixture(1, 1, 0);
    selected
        .active
        .get_mut(&selected.ids[0])
        .unwrap()
        .dependencies = vec![1, 1];
    selected.completed.insert(1, succeeded(1));
    selected.retain_counts.insert(1, 1);
    assert!(!reference(&selected));
    assert_matches_reference(&selected);
}

#[test]
fn asymmetric_dependency_predicates_and_requested_order_match_the_reference() {
    let mut missing_selected_retain = fixture(3, 1, 0);
    missing_selected_retain
        .retain_counts
        .remove(&missing_selected_retain.ids[0]);
    assert!(!reference(&missing_selected_retain));
    assert_matches_reference(&missing_selected_retain);

    for present_zero in [false, true] {
        let mut no_selected_waiters = fixture(1, 1, 0);
        if present_zero {
            no_selected_waiters
                .retain_counts
                .insert(no_selected_waiters.ids[0], 0);
        }
        assert!(reference(&no_selected_waiters));
        assert_matches_reference(&no_selected_waiters);
    }

    for mutation in 0..6 {
        let mut value = fixture(4, 2, 2);
        let selected = value.ids[1];
        let completed = value.active[&selected].dependencies[1];
        match mutation {
            0 => {
                value.retain_counts.remove(&completed);
            }
            1 => {
                value.completed.get_mut(&completed).unwrap().status =
                    BackendPollV1::Failed { code: -1 };
            }
            2 => {
                value.completed.remove(&completed);
            }
            3 => {
                value.waiters.insert(completed, Vec::new());
            }
            4 => {
                value.active.get_mut(&selected).unwrap().dependencies[1] = selected;
            }
            _ => {
                value.active.get_mut(&selected).unwrap().dependencies[1] = selected + 1;
            }
        }
        assert!(!reference(&value), "mutation={mutation}");
        assert_matches_reference(&value);
    }

    for permutation in [[0, 1, 2, 3], [3, 2, 1, 0], [1, 3, 0, 2]] {
        let mut value = fixture(8, 4, 2);
        let ordered = value.ids.clone();
        value.ids = permutation.map(|index| ordered[index]).to_vec();
        assert!(value.ids.iter().all(|id| value.active.contains_key(id)));
        assert!(reference(&value));
        assert_matches_reference(&value);
    }
}

#[test]
fn dependency_use_count_overflow_is_reported_without_wrapping() {
    let target = REQUEST_BASE;
    let mut value = record(target + 1, 0);
    value.dependencies.push(target);
    let records = HashMap::from([(value.id, value)]);
    let mut relevant = [DependencyUseCount {
        id: target,
        records: usize::MAX,
        last_record: usize::MAX,
    }];
    assert!(!count_dependency_uses(&mut relevant, records.values()));
    assert_eq!(relevant[0].records, usize::MAX);
}

#[test]
fn late_waiter_and_count_corruptions_match_the_reference() {
    for mutation in 0..5 {
        let mut value = fixture(4_096, 4, 2);
        let target = value.ids[0];
        let waiters = value.waiters.get_mut(&target).unwrap();
        match mutation {
            0 => {
                waiters.pop().unwrap();
            }
            1 => {
                let duplicate = *waiters.last().unwrap();
                waiters.push(duplicate);
            }
            2 => {
                *waiters.last_mut().unwrap() = u64::MAX;
            }
            3 => {
                let last = waiters.len() - 1;
                waiters.swap(0, last);
            }
            _ => {
                *value.retain_counts.get_mut(&target).unwrap() += 1;
            }
        }
        assert!(!reference(&value), "mutation={mutation}");
        assert_matches_reference(&value);
    }
}

#[test]
fn empty_selection_is_valid_without_scratch_reservation() {
    let value = fixture(8, 1, 1);
    let mut calls = 0;
    let result = valid_dependency_indexes_with_reservation(
        &[],
        &value.active,
        &value.completed,
        &value.waiters,
        &value.retain_counts,
        |_storage, _additional| {
            calls += 1;
            Vec::<DependencyUseCount>::new().try_reserve_exact(usize::MAX)
        },
    );
    assert_eq!(result, Ok(true));
    assert_eq!(calls, 0);
}

#[test]
fn scratch_capacity_precedes_predicate_mismatch_without_mutating_inputs() {
    for corrupt in [false, true] {
        let mut value = fixture(64, 4, 2);
        if corrupt {
            let target = value.ids[0];
            value.waiters.get_mut(&target).unwrap().pop();
        }
        let before = snapshot(&value);
        let (result, calls) = injected(&value, true);
        assert_eq!(result, Err(DependencyScratchCapacity));
        assert_eq!(calls, 1);
        assert_eq!(snapshot(&value), before);
    }
}

#[test]
fn successful_scratch_reservation_is_single_and_nonmutating() {
    let value = fixture(1_024, 63, 4);
    let before = snapshot(&value);
    let (result, calls) = injected(&value, false);
    assert_eq!(result, Ok(reference(&value)));
    assert_eq!(calls, 1);
    assert_eq!(snapshot(&value), before);
}

#[test]
fn maximum_healthy_requested_dependency_breadth_is_accepted() {
    let ids: Vec<_> = (0..63).map(|index| REQUEST_BASE + index as u64).collect();
    let mut active = HashMap::with_capacity(ids.len());
    let mut completed = HashMap::with_capacity(63 * MAX_RUNTIME_DEPENDENCIES_V1);
    let mut retain_counts = HashMap::with_capacity(63 * MAX_RUNTIME_DEPENDENCIES_V1);
    let mut next_dependency = 1_u64;
    for id in ids.iter().copied() {
        let mut value = record(id, 0);
        value.dependencies =
            (next_dependency..next_dependency + MAX_RUNTIME_DEPENDENCIES_V1 as u64).collect();
        for dependency in &value.dependencies {
            completed.insert(*dependency, succeeded(*dependency));
            retain_counts.insert(*dependency, 1);
        }
        next_dependency += MAX_RUNTIME_DEPENDENCIES_V1 as u64;
        active.insert(id, value);
    }
    assert_eq!(
        ids.len()
            + active
                .values()
                .map(|record| record.dependencies.len())
                .sum::<usize>(),
        16_191
    );
    let value = Fixture {
        ids,
        active,
        completed,
        waiters: HashMap::new(),
        retain_counts,
    };
    assert_eq!(candidate(&value), Ok(true));
}

#[test]
fn dependency_scaling_profile_rows() {
    for (case, total, requested, dependencies_per_requested) in [
        ("one-empty-64", 64, 1, 0),
        ("one-four-4096", 4_096, 1, 4),
        ("sixty-three-empty-4096", 4_096, 63, 0),
        ("sixty-three-four-16384", 16_384, 63, 4),
    ] {
        let value = fixture(total, requested, dependencies_per_requested);
        let before = snapshot(&value);
        let started = StdInstant::now();
        let expected = black_box(reference(black_box(&value)));
        let reference_ns = started.elapsed().as_nanos();
        let started = StdInstant::now();
        let actual = black_box(candidate(black_box(&value)));
        let candidate_ns = started.elapsed().as_nanos();
        assert_eq!(actual, Ok(expected));
        assert!(expected);
        assert_eq!(snapshot(&value), before);
        println!(
            "schema=fe2o3.xgmi-dependency-index-scaling.v1 case={case} active={total} requested={requested} requested_dependency_edges={} blocked_waiters={} reference_ns={reference_ns} candidate_ns={candidate_ns} result=valid",
            requested * dependencies_per_requested,
            total - requested
        );
    }

    let total = 65_536;
    let requested = 63;
    let dependencies_per_requested = 4;
    let value = fixture(total, requested, dependencies_per_requested);
    let before = snapshot(&value);
    let started = StdInstant::now();
    assert_eq!(black_box(candidate(black_box(&value))), Ok(true));
    let candidate_ns = started.elapsed().as_nanos();
    assert_eq!(snapshot(&value), before);
    println!(
        "schema=fe2o3.xgmi-dependency-index-scaling.v1 case=sixty-three-four-65536 active=65536 requested=63 requested_dependency_edges=252 blocked_waiters=65473 reference_ns=not-run candidate_ns={candidate_ns} result=valid"
    );
}
