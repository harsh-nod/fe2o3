use super::*;
use std::hint::black_box;
use std::time::Instant as StdInstant;

struct Fixture {
    requested: Vec<u64>,
    active: HashMap<u64, XgmiRuntimeSubmissionV1>,
    ready: [VecDeque<u64>; 2],
    in_flight: [Vec<u64>; 2],
    completed: HashMap<u64, SubmissionRecordV1>,
}

#[derive(Debug, Eq, PartialEq)]
struct RecordSnapshot {
    key: u64,
    id: u64,
    stream: u64,
    direction: usize,
    source: u64,
    destination: u64,
    source_offset: u64,
    destination_offset: u64,
    byte_len: u32,
    dependencies: Vec<u64>,
    dependency_cursor: usize,
    published: bool,
}

#[derive(Debug, Eq, PartialEq)]
struct FixtureSnapshot {
    requested: Vec<u64>,
    active: Vec<RecordSnapshot>,
    ready: [Vec<u64>; 2],
    in_flight: [Vec<u64>; 2],
    completed_ids: Vec<u64>,
}

fn snapshot(fixture: &Fixture) -> FixtureSnapshot {
    let mut active: Vec<_> = fixture
        .active
        .iter()
        .map(|(key, record)| RecordSnapshot {
            key: *key,
            id: record.id,
            stream: record.stream,
            direction: record.direction,
            source: record.source,
            destination: record.destination,
            source_offset: record.source_offset,
            destination_offset: record.destination_offset,
            byte_len: record.byte_len,
            dependencies: record.dependencies.clone(),
            dependency_cursor: record.dependency_cursor,
            published: record.ticket.is_some(),
        })
        .collect();
    active.sort_unstable_by_key(|record| record.key);
    let mut completed_ids: Vec<_> = fixture.completed.keys().copied().collect();
    completed_ids.sort_unstable();
    FixtureSnapshot {
        requested: fixture.requested.clone(),
        active,
        ready: [
            fixture.ready[0].iter().copied().collect(),
            fixture.ready[1].iter().copied().collect(),
        ],
        in_flight: fixture.in_flight.clone(),
        completed_ids,
    }
}

fn ready_fixture(total: usize, requested: usize, opposite_ready: usize) -> Fixture {
    assert!(requested > 0);
    assert!(requested <= MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1);
    assert!(requested + opposite_ready <= total);
    let requested_ids: Vec<_> = (1..=requested as u64).collect();
    let opposite_ids: Vec<_> =
        ((requested + 1) as u64..=(requested + opposite_ready) as u64).collect();
    let mut active = HashMap::with_capacity(total);
    for id in &requested_ids {
        assert!(active.insert(*id, record(*id, 0)).is_none());
    }
    for id in &opposite_ids {
        assert!(active.insert(*id, record(*id, 1)).is_none());
    }
    for id in (requested + opposite_ready + 1) as u64..=total as u64 {
        let mut blocked = record(id, id as usize & 1);
        blocked.dependencies.push(requested_ids[0]);
        blocked.ready_indexed = false;
        assert!(active.insert(id, blocked).is_none());
    }
    Fixture {
        requested: requested_ids.clone(),
        active,
        ready: [
            requested_ids.iter().rev().copied().collect(),
            opposite_ids.iter().rev().copied().collect(),
        ],
        in_flight: [Vec::new(), Vec::new()],
        completed: HashMap::new(),
    }
}

fn candidate(fixture: &Fixture, requested: &[u64]) -> Result<Admission, AdmissionError> {
    admit(
        requested,
        &fixture.active,
        &fixture.ready,
        &fixture.in_flight,
        &fixture.completed,
    )
}

// Frozen verbatim from ca7a30d7d1a9705a6299ea9f5c8410a336856da5 so the
// optimized implementation remains behaviorally comparable to its predecessor.
fn reference_admit(
    requested: &[u64],
    active: &HashMap<u64, XgmiRuntimeSubmissionV1>,
    ready: &[VecDeque<u64>; 2],
    in_flight: &[Vec<u64>; 2],
    completed: &HashMap<u64, SubmissionRecordV1>,
) -> Result<Admission, AdmissionError> {
    if requested.is_empty() || requested.len() > MAX_RUNTIME_PEER_COPY_BATCH_SUBMISSIONS_V1 {
        return Err(AdmissionError::Invalid);
    }
    for (index, id) in requested.iter().enumerate() {
        if requested[..index].contains(id) {
            return Err(AdmissionError::Invalid);
        }
    }
    // Check index integrity before deciding whether a healthy caller supplied
    // a subset. Ready order is FIFO; only the in-flight index is ID-sorted.
    for d in 0..2 {
        for (index, id) in ready[d].iter().enumerate() {
            let record = active.get(id).ok_or(AdmissionError::Corrupt)?;
            if record.id != *id
                || ready[d].iter().take(index).any(|other| other == id)
                || !xgmi_submission_is_ready_v1(record, completed, d)
                || completed.contains_key(id)
            {
                return Err(AdmissionError::Corrupt);
            }
        }
        for (index, id) in in_flight[d].iter().enumerate() {
            let record = active.get(id).ok_or(AdmissionError::Corrupt)?;
            if record.id != *id
                || record.direction != d
                || record.ticket.is_none()
                || index > 0 && in_flight[d][index - 1] >= *id
                || completed.contains_key(id)
                || record.dependencies.iter().any(|dependency| {
                    completed
                        .get(dependency)
                        .is_none_or(|r| r.status != BackendPollV1::Succeeded)
                })
            {
                return Err(AdmissionError::Corrupt);
            }
        }
    }
    for (id, record) in active {
        let d = record.direction;
        if d > 1
            || record.id != *id
            || record.source == record.destination
            || completed.contains_key(id)
            || ready[1 - d].contains(id)
            || in_flight[1 - d].contains(id)
            || in_flight[d].contains(id) != record.ticket.is_some()
            || ready[d].contains(id) != xgmi_submission_is_ready_v1(record, completed, d)
        {
            return Err(AdmissionError::Corrupt);
        }
    }
    if requested.iter().any(|id| !active.contains_key(id)) {
        return Err(AdmissionError::Invalid);
    }
    let first = &active[&requested[0]];
    let admission = Admission {
        direction: first.direction,
        published: first.ticket.is_some(),
    };
    let direction = admission.direction;
    if admission.published {
        if in_flight[direction].len() != requested.len()
            || requested
                .iter()
                .any(|id| !in_flight[direction].contains(id))
        {
            return Err(AdmissionError::Busy);
        }
    } else if !in_flight[direction].is_empty()
        || ready[direction].len() != requested.len()
        || requested.iter().any(|id| !ready[direction].contains(id))
    {
        return Err(AdmissionError::Busy);
    }
    for (index, id) in requested.iter().enumerate() {
        let record = &active[id];
        if record.direction != direction || record.ticket.is_some() != admission.published {
            return Err(AdmissionError::Corrupt);
        }
        if admission.published {
            if ready[direction].contains(id) {
                return Err(AdmissionError::Corrupt);
            }
        } else if !xgmi_submission_is_ready_v1(record, completed, direction) {
            return Err(AdmissionError::Corrupt);
        }
        for earlier in &requested[..index] {
            let previous = active.get(earlier).ok_or(AdmissionError::Corrupt)?;
            if previous.stream == record.stream
                || [previous.source, previous.destination]
                    .iter()
                    .any(|id| *id == record.source || *id == record.destination)
            {
                return Err(AdmissionError::Corrupt);
            }
        }
    }
    Ok(admission)
}

fn reference(fixture: &Fixture, requested: &[u64]) -> Result<Admission, AdmissionError> {
    reference_admit(
        requested,
        &fixture.active,
        &fixture.ready,
        &fixture.in_flight,
        &fixture.completed,
    )
}

fn injected_reservation(
    fixture: &Fixture,
    requested: &[u64],
    fail_at: usize,
) -> (Result<Admission, AdmissionError>, usize) {
    let mut calls = 0;
    let result = admit_with_reservation(
        requested,
        &fixture.active,
        &fixture.ready,
        &fixture.in_flight,
        &fixture.completed,
        |storage, additional| {
            calls += 1;
            if calls == fail_at {
                Vec::<u64>::new().try_reserve_exact(usize::MAX)
            } else {
                storage.try_reserve_exact(additional)
            }
        },
    );
    (result, calls)
}

fn permutations(values: &[u64]) -> Vec<Vec<u64>> {
    fn extend(prefix: &mut Vec<u64>, remaining: &mut Vec<u64>, output: &mut Vec<Vec<u64>>) {
        if remaining.is_empty() {
            output.push(prefix.clone());
            return;
        }
        for index in 0..remaining.len() {
            let value = remaining.remove(index);
            prefix.push(value);
            extend(prefix, remaining, output);
            prefix.pop();
            remaining.insert(index, value);
        }
    }

    let mut output = Vec::new();
    extend(&mut Vec::new(), &mut values.to_vec(), &mut output);
    output
}

#[test]
fn bounded_ready_permutations_match_the_frozen_reference() {
    // A successful published roster is deliberately not synthesized here:
    // its native ticket is opaque with no public constructor. The parent
    // mutation test retains coverage of inconsistent in-flight metadata.
    for count in 1_usize..=4 {
        let ids: Vec<_> = (1..=count as u64).collect();
        for ready_order in permutations(&ids) {
            for requested in permutations(&ids) {
                let mut fixture = ready_fixture(count, count, 0);
                fixture.ready[0] = ready_order.iter().copied().collect();
                let expected = reference(&fixture, &requested);
                assert_eq!(
                    candidate(&fixture, &requested),
                    expected,
                    "count={count} ready={ready_order:?} requested={requested:?}"
                );
                assert_eq!(
                    expected,
                    Ok(Admission {
                        direction: 0,
                        published: false,
                    })
                );
            }
        }
    }
}

#[test]
fn bounded_metadata_mutations_match_the_frozen_reference() {
    for position in 0..6 {
        for mutation in 0..4 {
            let mut fixture = ready_fixture(9, 3, 6);
            match mutation {
                0 => {
                    let duplicate = fixture.ready[1][(position + 1) % 6];
                    fixture.ready[1][position] = duplicate;
                }
                1 => {
                    fixture.ready[1].remove(position);
                }
                2 => {
                    let id = fixture.ready[1][position];
                    fixture.active.get_mut(&id).unwrap().direction = 0;
                }
                _ => fixture.ready[1][position] = 10_000 + position as u64,
            }
            for requested in [fixture.requested[..1].to_vec(), vec![u64::MAX]] {
                let expected = reference(&fixture, &requested);
                assert_eq!(expected, Err(AdmissionError::Corrupt));
                assert_eq!(
                    candidate(&fixture, &requested),
                    expected,
                    "position={position} mutation={mutation} requested={requested:?}"
                );
            }
        }
    }
}

#[test]
fn malformed_requests_are_invalid_before_any_scratch_reservation() {
    let fixture = ready_fixture(64, 63, 1);
    for requested in [Vec::new(), (1..=64).collect(), vec![1, 1]] {
        let before = snapshot(&fixture);
        let (result, calls) = injected_reservation(&fixture, &requested, usize::MAX);
        assert_eq!(result, Err(AdmissionError::Invalid));
        assert_eq!(calls, 0, "requested={requested:?}");
        assert_eq!(snapshot(&fixture), before);
    }
}

#[test]
fn either_scratch_reservation_failure_precedes_later_classification() {
    for fail_at in 1..=2 {
        for case in 0..4 {
            let mut fixture = ready_fixture(32, 4, 28);
            let requested = match case {
                0 => fixture.requested.clone(),
                1 => fixture.requested[..1].to_vec(),
                2 => vec![u64::MAX],
                _ => {
                    let duplicate = fixture.ready[1][0];
                    fixture.ready[1].push_back(duplicate);
                    fixture.requested.clone()
                }
            };
            let before = snapshot(&fixture);
            let (result, calls) = injected_reservation(&fixture, &requested, fail_at);
            assert_eq!(result, Err(AdmissionError::Capacity), "case={case}");
            assert_eq!(calls, fail_at, "case={case}");
            assert_eq!(snapshot(&fixture), before, "case={case}");
        }
    }
}

#[test]
fn injected_success_reserves_both_indexes_and_matches_the_reference() {
    let fixture = ready_fixture(64, 4, 60);
    let before = snapshot(&fixture);
    let expected = reference(&fixture, &fixture.requested);
    let (result, calls) = injected_reservation(&fixture, &fixture.requested, usize::MAX);
    assert_eq!(result, expected);
    assert_eq!(calls, 2);
    assert_eq!(snapshot(&fixture), before);
}

#[test]
fn late_index_corruption_precedes_subset_or_foreign_caller_errors() {
    for mutation in 0..4 {
        let mut fixture = ready_fixture(4_096, 4, 4_092);
        match mutation {
            0 => {
                let duplicate = fixture.ready[1][0];
                fixture.ready[1].push_back(duplicate);
            }
            1 => {
                fixture.ready[1].pop_back().unwrap();
            }
            2 => {
                let id = *fixture.ready[1].back().unwrap();
                fixture.active.get_mut(&id).unwrap().direction = 0;
            }
            _ => fixture.ready[1].push_back(u64::MAX - 1),
        }
        for requested in [fixture.requested[..1].to_vec(), vec![u64::MAX]] {
            assert_eq!(
                candidate(&fixture, &requested),
                Err(AdmissionError::Corrupt),
                "mutation={mutation} requested={requested:?}"
            );
        }
    }
}

#[test]
fn sixty_five_thousand_active_blocked_records_preserve_rosters() {
    let mut fixture = ready_fixture(65_536, 63, 0);
    fixture.requested.rotate_left(17);
    let requested_before = fixture.requested.clone();
    let ready_before = fixture.ready.clone();
    assert_eq!(
        candidate(&fixture, &fixture.requested),
        Ok(Admission {
            direction: 0,
            published: false,
        })
    );
    assert_eq!(fixture.requested, requested_before);
    assert_eq!(fixture.ready, ready_before);
    assert!(fixture.ready[1].is_empty());
    assert_eq!(
        fixture.ready[0].iter().copied().collect::<Vec<_>>(),
        (1..=63).rev().collect::<Vec<_>>()
    );
}

#[test]
fn admission_scaling_profile_rows() {
    for total in [64, 256, 1_024, 4_096, 16_384] {
        let fixture = ready_fixture(total, 1, total - 1);
        let requested_before = fixture.requested.clone();
        let ready_before = fixture.ready.clone();
        let started = StdInstant::now();
        let expected = black_box(reference(
            black_box(&fixture),
            black_box(&fixture.requested),
        ));
        let reference_ns = started.elapsed().as_nanos();
        let started = StdInstant::now();
        let actual = black_box(candidate(
            black_box(&fixture),
            black_box(&fixture.requested),
        ));
        let candidate_ns = started.elapsed().as_nanos();
        assert_eq!(actual, expected);
        assert_eq!(
            actual,
            Ok(Admission {
                direction: 0,
                published: false,
            })
        );
        assert_eq!(fixture.requested, requested_before);
        assert_eq!(fixture.ready, ready_before);
        println!(
            "schema=fe2o3.xgmi-batch-admission-scaling.v1 active={total} requested=1 opposite_ready={} blocked=0 reference_ns={reference_ns} candidate_ns={candidate_ns} accepted=true",
            total - 1
        );
    }

    let mut fixture = ready_fixture(65_536, 63, 65_536 - 63);
    fixture.requested.rotate_left(17);
    let requested_before = fixture.requested.clone();
    let ready_before = fixture.ready.clone();
    let started = StdInstant::now();
    assert_eq!(
        black_box(candidate(
            black_box(&fixture),
            black_box(&fixture.requested),
        )),
        Ok(Admission {
            direction: 0,
            published: false,
        })
    );
    let candidate_ns = started.elapsed().as_nanos();
    assert_eq!(fixture.requested, requested_before);
    assert_eq!(fixture.ready, ready_before);
    assert_eq!(fixture.ready[1].front(), Some(&65_536));
    assert_eq!(fixture.ready[1].back(), Some(&64));
    println!(
        "schema=fe2o3.xgmi-batch-admission-scaling.v1 active=65536 requested=63 opposite_ready=65473 blocked=0 reference_ns=not-run candidate_ns={candidate_ns} accepted=true"
    );
}
