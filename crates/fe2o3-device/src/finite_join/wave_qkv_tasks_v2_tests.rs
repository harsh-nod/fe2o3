//! Actual storage/arbitration/arrival tests. The explicit collective outcomes
//! below do not execute GPU-only LDS/subgroup terminals or prove GPU progress.
use super::*;
use std::{thread as host_thread, vec, vec::Vec};

fn state() -> [AtomicU32; WAVE_STATE_WORDS] {
    core::array::from_fn(|index| AtomicU32::new(u32::from(index < 2)))
}

fn observation() -> FiniteJoinWorkerResult {
    FiniteJoinWorkerResult {
        error: 0,
        executed_tasks: 0,
        rounds: 0,
        empty_probes: 0,
    }
}

struct Fixture {
    input: Vec<u16>,
    norm_weight: Vec<u16>,
    qkv_weight: Vec<u16>,
    normalized: Vec<u16>,
    output: Vec<u16>,
    state: [AtomicU32; WAVE_STATE_WORDS],
}

impl Fixture {
    fn new() -> Self {
        Self {
            input: vec![0x3f80; NORM_ELEMENTS],
            norm_weight: vec![0x4000; NORM_ELEMENTS],
            qkv_weight: vec![0; QKV_ELEMENTS],
            normalized: vec![0xa55a; NORM_ELEMENTS],
            output: vec![0xa55a; QKV_COLUMNS],
            state: state(),
        }
    }

    fn storage(&mut self) -> WaveQkvTaskStorageV2<'_> {
        // SAFETY: distinct initialized allocations live under this exclusive
        // fixture borrow; CPU tests issue views sequentially, without GPU calls.
        unsafe {
            WaveQkvTaskStorageV2::from_raw_parts(
                self.input.as_ptr().cast(),
                self.norm_weight.as_ptr().cast(),
                self.qkv_weight.as_ptr().cast(),
                self.normalized.as_mut_ptr().cast(),
                self.output.as_mut_ptr().cast(),
                &self.state,
            )
            .unwrap()
        }
    }
}

fn arrive_all(state: &[AtomicU32; WAVE_STATE_WORDS], task: u32) {
    for lane in 0..WAVE_LANES {
        complete_lane(state, task).unwrap();
        assert_eq!(
            state[DONE].load(Ordering::Acquire) & (1 << task) != 0,
            lane == 63
        );
    }
}

#[test]
fn qkv_profile_has_distinct_extents_and_fixed_pointer_layout() {
    assert_eq!(
        (QKV_ELEMENTS, QKV_COLUMNS, WAVE_STATE_WORDS),
        (12582912, 3072, 19)
    );
    assert_eq!(core::mem::size_of::<WaveQkvTaskStorageV2<'_>>(), 48);
    assert_eq!(core::mem::align_of::<WaveQkvTaskStorageV2<'_>>(), 8);
    assert_eq!((ALL_TASKS, PROJECTION_TASKS), (8191, 8190));
    // The old public profile is not widened by the sibling module.
    assert_eq!(super::super::wave_tasks::KEY_COLUMNS, 512);
    assert_eq!(super::super::wave_tasks::WAVE_STATE_WORDS, 9);
}

#[test]
fn thirteen_claims_cover_every_norm_and_packed_output_once() {
    for worker in 0..2 {
        let mut fixture = Fixture::new();
        {
            let storage = fixture.storage();
            assert_eq!(claim_fanout(storage.state(), worker), Claim::Task(0));
            for lane in 0..WAVE_LANES {
                assert!(lane_admitted(storage.state(), 1, worker));
                let (mut written, mut valid) = (0, true);
                {
                    let mut task = QkvNormTaskV2 {
                        storage: &storage,
                        lane,
                        written: &mut written,
                        valid: &mut valid,
                    };
                    for component in 0..64 {
                        let column = lane + component * 64;
                        assert_eq!(task.input(column), Some(0x3f80));
                        assert_eq!(task.weight(column), Some(0x4000));
                        assert!(task.write_component(component, column as u16));
                    }
                }
                assert!(complete_coverage(1, lane, written, valid));
                complete_lane(storage.state(), 0).unwrap();
                assert_eq!(
                    storage.state()[READY].load(Ordering::Acquire),
                    if lane == 63 { PROJECTION_TASKS } else { 0 }
                );
            }
            for task in 1..TASK_COUNT {
                assert_eq!(claim_fanout(storage.state(), worker), Claim::Task(task));
                for lane in 0..WAVE_LANES {
                    assert!(lane_admitted(storage.state(), task + 1, worker));
                    let (mut written, mut valid) = (0, true);
                    {
                        let mut view = QkvProjectionTaskV2 {
                            storage: &storage,
                            lane,
                            base: (task as usize - 1) * QKV_CHUNK,
                            written: &mut written,
                            valid: &mut valid,
                        };
                        for component in 0..64 {
                            let inner = lane + 64 * component;
                            assert_eq!(view.input(inner), Some(inner as u16));
                        }
                        if lane == 0 {
                            for column in 0..QKV_CHUNK {
                                assert!(view.write_column(
                                    column,
                                    ((task as usize - 1) * QKV_CHUNK + column) as u16
                                ));
                            }
                        }
                    }
                    assert!(complete_coverage(task + 1, lane, written, valid));
                    complete_lane(storage.state(), task).unwrap();
                }
            }
            assert_eq!(storage.state()[READY].load(Ordering::Acquire), 0);
            assert_eq!(storage.state()[DONE].load(Ordering::Acquire), ALL_TASKS);
            assert_eq!(storage.state()[CLAIMED].load(Ordering::Acquire), ALL_TASKS);
            assert_eq!(storage.state()[ERRORS].load(Ordering::Acquire), 0);
            for task in 0..TASK_COUNT {
                assert_eq!(
                    storage.state()[ARRIVALS + task as usize].load(Ordering::Acquire),
                    64
                );
                assert_eq!(
                    (storage.state()[OWNERS].load(Ordering::Relaxed) >> (2 * task)) & 3,
                    worker + 1
                );
            }
        }
        assert_eq!(fixture.normalized, (0..4096).collect::<Vec<u16>>());
        assert_eq!(fixture.output, (0..3072).collect::<Vec<u16>>());
    }
}

#[test]
fn each_tile_reads_exact_weight_boundaries() {
    let mut fixture = Fixture::new();
    for tile in 0..12 {
        fixture.qkv_weight[tile * QKV_CHUNK * NORM_ELEMENTS] = (100 + tile) as u16;
        fixture.qkv_weight[(tile + 1) * QKV_CHUNK * NORM_ELEMENTS - 1] = (200 + tile) as u16;
    }
    let storage = fixture.storage();
    assert_eq!(claim_fanout(storage.state(), 0), Claim::Task(0));
    arrive_all(storage.state(), 0);
    for tile in 0..12 {
        assert_eq!(
            claim_fanout(storage.state(), 1),
            Claim::Task(tile as u32 + 1)
        );
        let (mut written, mut valid) = (0, true);
        let view = QkvProjectionTaskV2 {
            storage: &storage,
            lane: 0,
            base: tile * QKV_CHUNK,
            written: &mut written,
            valid: &mut valid,
        };
        assert_eq!(view.weight(0, 0), Some((100 + tile) as u16));
        assert_eq!(view.weight(255, 4095), Some((200 + tile) as u16));
        assert_eq!(view.weight(256, 0), None);
        assert_eq!(view.weight(0, 4096), None);
        assert_eq!(view.input(4096), None);
    }
}

#[test]
fn projection_writes_require_leader_sequential_cursor_and_valid_claim() {
    for (lane, column, initial_valid) in [(1, 0, true), (0, 1, true), (0, 256, true), (0, 0, false)]
    {
        let mut fixture = Fixture::new();
        {
            let storage = fixture.storage();
            let (mut written, mut valid) = (0, initial_valid);
            let mut view = QkvProjectionTaskV2 {
                storage: &storage,
                lane,
                base: 2816,
                written: &mut written,
                valid: &mut valid,
            };
            assert!(!view.write_column(column, 42));
            assert!(!valid);
            assert_eq!(written, 0);
        }
        assert!(fixture.output.iter().all(|&word| word == 0xa55a));
    }
}

#[test]
fn coverage_rejects_missing_extra_invalid_and_out_of_range_writes() {
    for token in 0..=TASK_COUNT {
        for lane in 0..64 {
            let expected = if token == 1 {
                64
            } else if token >= 2 && lane == 0 {
                256
            } else {
                0
            };
            assert!(complete_coverage(token, lane, expected, true));
            assert!(!complete_coverage(token, lane, expected + 1, true));
            assert!(!complete_coverage(token, lane, expected, false));
            if expected > 0 {
                assert!(!complete_coverage(token, lane, expected - 1, true));
            }
        }
    }
    assert!(!complete_coverage(14, 0, 256, true));
    assert!(!complete_coverage(1, 64, 64, true));
}

#[test]
fn invalid_task_token_worker_epoch_ready_and_duplicate_claim_are_rejected() {
    let state = state();
    assert_eq!(complete_lane(&state, 13), Err(INVALID));
    assert!(!lane_admitted(&state, 14, 0));
    assert!(!lane_admitted(&state, 1, 2));
    assert_eq!(claim_fanout(&state, 2), Claim::Rejected(INVALID));
    assert_eq!(state[CLAIMED].load(Ordering::Acquire), 0);
    state[EPOCH].store(2, Ordering::Release);
    assert_eq!(claim_fanout(&state, 0), Claim::Rejected(STALE_EPOCH));
    state[EPOCH].store(1, Ordering::Release);
    state[READY].store(1 << 13, Ordering::Release);
    assert_eq!(claim_fanout(&state, 0), Claim::Rejected(INVALID));
    state[READY].store(1, Ordering::Release);
    assert_eq!(claim_fanout(&state, 0), Claim::Task(0));
    state[READY].store(1, Ordering::Release);
    assert_eq!(claim_fanout(&state, 1), Claim::Rejected(DUPLICATE));
    assert!(!lane_admitted(&state, 1, 1));
}

#[test]
fn missing_predecessor_and_arrival_never_release_projection() {
    let state = state();
    assert_eq!(claim_fanout(&state, 0), Claim::Task(0));
    for _ in 0..63 {
        complete_lane(&state, 0).unwrap();
    }
    assert_eq!(state[READY].load(Ordering::Acquire), 0);
    assert!(!lane_admitted(&state, 2, 1));
    state[READY].store(2, Ordering::Release);
    assert_eq!(
        claim_fanout(&state, 1),
        Claim::Rejected(MISSING_PREDECESSOR)
    );
    assert_eq!(state[DONE].load(Ordering::Acquire), 0);
}

#[test]
fn duplicate_arrival_sets_error_without_double_publication() {
    let state = state();
    assert_eq!(claim_fanout(&state, 0), Claim::Task(0));
    arrive_all(&state, 0);
    state[READY].store(0, Ordering::Release);
    assert_eq!(complete_lane(&state, 0), Err(DUPLICATE));
    assert_eq!(state[READY].load(Ordering::Acquire), 0);
    assert_eq!(state[ERRORS].load(Ordering::Acquire), DUPLICATE);
}

#[test]
fn concurrent_claims_are_unique_and_cover_all_twelve_projections() {
    for _ in 0..32 {
        let state = state();
        assert_eq!(claim_fanout(&state, 0), Claim::Task(0));
        arrive_all(&state, 0);
        let claimed = host_thread::scope(|scope| {
            let a = scope.spawn(|| claim_remaining(&state, 0));
            let b = scope.spawn(|| claim_remaining(&state, 1));
            let mut all = a.join().unwrap();
            all.extend(b.join().unwrap());
            all
        });
        let mut tasks = claimed.iter().map(|&(task, _)| task).collect::<Vec<_>>();
        tasks.sort_unstable();
        assert_eq!(tasks, (1..13).collect::<Vec<_>>());
        for (task, worker) in claimed {
            assert!(lane_admitted(&state, task + 1, worker));
            assert!(!lane_admitted(&state, task + 1, 1 - worker));
        }
        assert_eq!(state[CLAIMED].load(Ordering::Acquire), ALL_TASKS);
        assert_eq!(state[ERRORS].load(Ordering::Acquire), 0);
    }
}

fn claim_remaining(state: &[AtomicU32; WAVE_STATE_WORDS], worker: u32) -> Vec<(u32, u32)> {
    let mut tasks = Vec::new();
    loop {
        match claim_fanout(state, worker) {
            Claim::Task(task) => tasks.push((task, worker)),
            Claim::Empty => return tasks,
            Claim::Contended => {}
            other => panic!("unexpected claim: {other:?}"),
        }
    }
}

#[test]
fn late_publication_after_four_empty_rounds_is_not_lost() {
    let state = state();
    assert_eq!(claim_fanout(&state, 0), Claim::Task(0));
    let (mut retired, mut result) = (false, observation());
    for _ in 0..5 {
        assert_eq!(
            begin_round_claim(&state, 0, 1, &mut retired, &mut result),
            0
        );
        assert!(!retired);
    }
    assert_eq!(result.empty_probes, 5 * POLLS_PER_ROUND);
    arrive_all(&state, 0);
    assert_eq!(
        begin_round_claim(&state, 0, 1, &mut retired, &mut result),
        2
    );
    assert!(!retired);
}

#[test]
fn stalled_peer_has_bounded_polling_and_no_false_completion() {
    let state = state();
    assert_eq!(claim_fanout(&state, 0), Claim::Task(0));
    let (mut retired, mut result) = (false, observation());
    for round in 0..MAX_ROUNDS {
        let token = begin_round_claim(&state, 0, 1, &mut retired, &mut result);
        assert_eq!(token, 0);
        assert_eq!(result.rounds, round + 1);
        finish_round_completion(&state, token, true, &mut retired, &mut result);
    }
    assert_eq!(result.rounds, 32);
    assert_eq!(result.empty_probes, 8192);
    assert_eq!(result.executed_tasks, 0);
    assert!(!retired);
    assert_eq!(state[DONE].load(Ordering::Acquire), 0);
}

#[test]
fn either_worker_can_complete_all_tasks_after_peer_exhausts_budget() {
    for worker in 0..2 {
        let state = state();
        let (mut retired, mut result) = (false, observation());
        assert_eq!(
            begin_round_claim(&state, 0, worker, &mut retired, &mut result),
            1
        );
        let (mut peer_retired, mut peer) = (false, observation());
        for _ in 0..MAX_ROUNDS {
            assert_eq!(
                begin_round_claim(&state, 0, 1 - worker, &mut peer_retired, &mut peer),
                0
            );
        }
        arrive_all(&state, 0);
        for expected in 2..=TASK_COUNT {
            let token = begin_round_claim(&state, 0, worker, &mut retired, &mut result);
            assert_eq!(token, expected);
            arrive_all(&state, token - 1);
        }
        for _ in TASK_COUNT..MAX_ROUNDS {
            assert_eq!(
                begin_round_claim(&state, 0, worker, &mut retired, &mut result),
                0
            );
        }
        assert!(retired);
        assert_eq!(result.rounds, MAX_ROUNDS);
        assert_eq!(result.error, 0);
        assert_eq!(state[DONE].load(Ordering::Acquire), ALL_TASKS);
    }
}

#[test]
fn incomplete_handler_never_publishes_and_peer_observes_error() {
    let state = state();
    assert_eq!(claim_fanout(&state, 0), Claim::Task(0));
    let (mut retired, mut result) = (false, observation());
    finish_round_completion(&state, 1, false, &mut retired, &mut result);
    assert!(retired);
    assert_eq!(result.error, INCOMPLETE_WRITES);
    assert_eq!(state[ARRIVALS].load(Ordering::Acquire), 0);
    assert_eq!(state[DONE].load(Ordering::Acquire), 0);
    assert_eq!(state[READY].load(Ordering::Acquire), 0);
    let (mut peer_retired, mut peer) = (false, observation());
    assert_eq!(
        begin_round_claim(&state, 0, 1, &mut peer_retired, &mut peer),
        0
    );
    assert!(peer_retired);
    assert_eq!(peer.error, INCOMPLETE_WRITES);
}

#[test]
fn peer_error_takes_precedence_over_ready_and_global_completion() {
    for done in [0, ALL_TASKS] {
        let state = state();
        state[DONE].store(done, Ordering::Release);
        state[ERRORS].store(INCOMPLETE_WRITES, Ordering::Release);
        let (mut retired, mut result) = (false, observation());
        for _ in 0..MAX_ROUNDS {
            assert_eq!(
                begin_round_claim(&state, 0, 0, &mut retired, &mut result),
                0
            );
        }
        assert!(retired);
        assert_eq!(result.error, INCOMPLETE_WRITES);
        assert_eq!(result.rounds, MAX_ROUNDS);
        assert_eq!(state[CLAIMED].load(Ordering::Acquire), 0);
    }
}

#[test]
fn follower_and_retired_leader_do_not_claim() {
    let state = state();
    let before: Vec<_> = state
        .iter()
        .map(|word| word.load(Ordering::Relaxed))
        .collect();
    let (mut retired, mut result) = (false, observation());
    assert_eq!(
        begin_round_claim(&state, 63, 0, &mut retired, &mut result),
        0
    );
    retired = true;
    assert_eq!(
        begin_round_claim(&state, 0, 0, &mut retired, &mut result),
        0
    );
    assert_eq!(result.rounds, 2);
    assert_eq!(
        (result.empty_probes, result.error, result.executed_tasks),
        (0, 0, 0)
    );
    assert_eq!(
        state
            .iter()
            .map(|word| word.load(Ordering::Relaxed))
            .collect::<Vec<_>>(),
        before
    );
    assert_eq!(state[CLAIMED].load(Ordering::Acquire), 0);
}

#[test]
fn every_ready_mask_claims_only_its_lowest_bit() {
    for worker in 0..2 {
        for mask in 1..=ALL_TASKS {
            let state = state();
            state[READY].store(mask, Ordering::Release);
            let done = u32::from(mask & 1 == 0);
            state[DONE].store(done, Ordering::Release);
            let task = mask.trailing_zeros();
            let bit = 1 << task;
            assert_eq!(claim_fanout(&state, worker), Claim::Task(task));
            assert_eq!(state[READY].load(Ordering::Acquire), mask & !bit);
            assert_eq!(state[CLAIMED].load(Ordering::Acquire), bit);
            assert_eq!(
                state[OWNERS].load(Ordering::Relaxed),
                (worker + 1) << (2 * task)
            );
            assert_eq!(state[DONE].load(Ordering::Acquire), done);
            assert_eq!(state[ERRORS].load(Ordering::Acquire), 0);
            assert_eq!(state[EPOCH].load(Ordering::Acquire), 1);
            assert!(
                state[ARRIVALS..]
                    .iter()
                    .all(|word| word.load(Ordering::Acquire) == 0)
            );
        }
    }
}

#[test]
fn empty_and_invalid_masks_do_not_change_scheduler_state() {
    for worker in 0..2 {
        for mask in [
            0,
            1 << 13,
            1 << 31,
            (1 << 12) | (1 << 13),
            ALL_TASKS | (1 << 31),
            u32::MAX,
        ] {
            let state = state();
            state[READY].store(mask, Ordering::Release);
            let mut expected: Vec<_> = state
                .iter()
                .map(|word| word.load(Ordering::Acquire))
                .collect();
            let outcome = if mask == 0 {
                Claim::Empty
            } else {
                expected[ERRORS] |= INVALID;
                Claim::Rejected(INVALID)
            };
            assert_eq!(claim_fanout(&state, worker), outcome);
            assert_eq!(
                state
                    .iter()
                    .map(|word| word.load(Ordering::Acquire))
                    .collect::<Vec<_>>(),
                expected,
                "worker={worker}, mask={mask:#x}"
            );
        }
    }
}

#[test]
fn remaining_poll_slots_do_not_claim_after_success() {
    for worker in 0..2 {
        let state = state();
        state[READY].store(PROJECTION_TASKS, Ordering::Release);
        state[DONE].store(1, Ordering::Release);
        let (mut retired, mut result) = (false, observation());
        assert_eq!(
            begin_round_claim(&state, 0, worker, &mut retired, &mut result),
            2
        );
        assert_eq!(state[CLAIMED].load(Ordering::Acquire), 2);
        assert_eq!(state[READY].load(Ordering::Acquire), PROJECTION_TASKS & !2);
        assert_eq!(state[OWNERS].load(Ordering::Relaxed), (worker + 1) << 2);
        assert_eq!(
            (result.rounds, result.empty_probes, result.error),
            (1, 0, 0)
        );
        assert!(!retired);
    }
}

#[test]
fn root_ranges_reject_overlap_null_misalignment_and_overflow() {
    let roots = [
        (0x1000, 8192, 2),
        (0x4000, 8192, 2),
        (0x10000, 25165824, 2),
        (0x2000000, 8192, 2),
        (0x2004000, 6144, 2),
        (0x2008000, 76, 4),
    ];
    assert!(disjoint_regions(roots));
    for bad in [
        (0, 8192, 2),
        (0x4001, 8192, 2),
        (0x1ffe, 8192, 2),
        (usize::MAX - 1, 8192, 2),
    ] {
        let mut altered = roots;
        altered[1] = bad;
        assert!(!disjoint_regions(altered));
    }
}

#[test]
fn constructor_uses_full_qkv_weight_and_output_extents() {
    let mut fixture = Fixture::new();
    // Invalid roots are only passed to the address-range validator, which must
    // return before constructing a usable binding or dereferencing any pointer.
    unsafe {
        let build = |input, norm, weights, normalized, output, state| {
            WaveQkvTaskStorageV2::from_raw_parts(input, norm, weights, normalized, output, state)
                .err()
        };
        let input = fixture.input.as_ptr().cast();
        let norm = fixture.norm_weight.as_ptr().cast();
        let weights = fixture.qkv_weight.as_ptr().cast();
        let normalized = fixture.normalized.as_mut_ptr().cast();
        let output = fixture.output.as_mut_ptr().cast();
        let state = &fixture.state as *const _;
        assert_eq!(
            build(core::ptr::null(), norm, weights, normalized, output, state),
            Some(INVALID)
        );
        assert_eq!(
            build(input, norm, core::ptr::null(), normalized, output, state),
            Some(INVALID)
        );
        assert_eq!(
            build(
                input,
                norm,
                weights,
                normalized,
                core::ptr::null_mut(),
                state
            ),
            Some(INVALID)
        );
        assert_eq!(
            build(
                input,
                norm,
                weights,
                normalized,
                output,
                (state as usize + 1) as *const _
            ),
            Some(INVALID)
        );
        assert_eq!(
            build(
                input,
                norm,
                (usize::MAX - 1) as *const _,
                normalized,
                output,
                state
            ),
            Some(INVALID)
        );
        // An input alias near the end would escape a stale K-only weight extent.
        let tail = fixture
            .qkv_weight
            .as_ptr()
            .add(QKV_ELEMENTS - NORM_ELEMENTS)
            .cast();
        assert_eq!(
            build(tail, norm, weights, normalized, output, state),
            Some(INVALID)
        );
        // This state alias is beyond the old K512 output but inside QKV3072.
        let inside_output = fixture.output.as_ptr().add(1024).cast();
        assert_eq!(
            build(input, norm, weights, normalized, output, inside_output),
            Some(INVALID)
        );
    }
}

#[test]
fn rejected_norm_view_cannot_write_or_skip_components() {
    for (lane, component, initial_valid) in
        [(64, 0, true), (0, 64, true), (0, 1, true), (0, 0, false)]
    {
        let mut fixture = Fixture::new();
        {
            let storage = fixture.storage();
            let (mut written, mut valid) = (0, initial_valid);
            let mut view = QkvNormTaskV2 {
                storage: &storage,
                lane,
                written: &mut written,
                valid: &mut valid,
            };
            assert_eq!(view.input(4096), None);
            assert_eq!(view.weight(4096), None);
            assert!(!view.write_component(component, 42));
            assert!(!valid);
            assert_eq!(written, 0);
        }
        assert!(fixture.normalized.iter().all(|&word| word == 0xa55a));
    }
}
