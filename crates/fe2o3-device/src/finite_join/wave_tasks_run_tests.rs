//! Shared production state transitions with sequential CPU atomics. Collective
//! outcomes are explicit test inputs, not execution of LDS, the public run
//! method, numerical callbacks, or a proof of GPU publication/convergence.
use super::*;
use std::vec::Vec;

fn run_state(words: [u32; WAVE_STATE_WORDS]) -> [AtomicU32; WAVE_STATE_WORDS] {
    words.map(AtomicU32::new)
}

fn run_snapshot(state: &[AtomicU32; WAVE_STATE_WORDS]) -> [u32; WAVE_STATE_WORDS] {
    core::array::from_fn(|index| state[index].load(Ordering::SeqCst))
}

#[test]
fn wave_run_claim_helper_preserves_follower_retired_and_completed_paths() {
    for (lane, mut retired, words, expected_retired) in [
        (1, false, [1, 1, 0, 0, 0, 0, 0, 0, 0], false),
        (0, true, [1, 1, 0, 0, 0, 0, 0, 0, 0], true),
        (0, false, [1, 0, 7, 7, 21, 0, 64, 64, 64], true),
    ] {
        let state = run_state(words);
        let mut result = FiniteJoinWorkerResult::default();
        assert_eq!(
            begin_round_claim(&state, lane, 0, &mut retired, &mut result),
            0
        );
        assert_eq!(retired, expected_retired);
        assert_eq!(result.rounds, 1);
        assert_eq!(result.empty_probes, 0);
        assert_eq!(result.executed_tasks, 0);
        assert_eq!(result.error, 0);
        assert!(complete_coverage(0, lane, 0, true));
        finish_round_completion(&state, 0, true, &mut retired, &mut result);
        assert_eq!(run_snapshot(&state), words);
        assert_eq!(result.rounds, 1);
        assert_eq!(result.executed_tasks, 0);
    }
}

#[test]
fn wave_run_empty_retirement_does_not_shorten_the_round_count() {
    // The other worker owns an unfinished norm task; this worker has no claim.
    let words = [1, 0, 0, 1, 2, 0, 0, 0, 0];
    let state = run_state(words);
    let mut result = FiniteJoinWorkerResult::default();
    let mut retired = false;
    for round in 0..MAX_ROUNDS {
        assert_eq!(
            begin_round_claim(&state, 0, 0, &mut retired, &mut result),
            0
        );
        assert_eq!(result.rounds, round + 1);
        assert_eq!(result.empty_probes, (round + 1).min(MAX_EMPTY_PROBES));
        assert_eq!(retired, round + 1 >= MAX_EMPTY_PROBES);
        finish_round_completion(&state, 0, true, &mut retired, &mut result);
    }
    assert_eq!(result.rounds, 8);
    assert_eq!(result.empty_probes, 4);
    assert_eq!(result.executed_tasks, 0);
    assert_eq!(result.error, 0);
    assert_eq!(run_snapshot(&state), words);
}

#[test]
fn wave_run_shared_completion_publishes_only_after_all_arrivals() {
    let state = run_state([1, 1, 0, 0, 0, 0, 0, 0, 0]);
    for (task, worker) in [(0, 0), (1, 1), (2, 1)] {
        let token = task + 1;
        for lane in 0..WAVE_LANES {
            let mut retired = false;
            let mut result = FiniteJoinWorkerResult::default();
            let local_token = begin_round_claim(&state, lane, worker, &mut retired, &mut result);
            assert_eq!(local_token, if lane == 0 { token } else { 0 });
            // Model only the collective's broadcast, then execute the actual
            // admission, coverage and post-collective completion helpers.
            assert!(lane_admitted(&state, token, worker));
            let written = if task == 0 {
                WAVE_LANES
            } else if lane == 0 {
                KEY_CHUNK
            } else {
                0
            };
            assert!(complete_coverage(token, lane, written, true));
            let done_before = state[DONE].load(Ordering::Acquire);
            let ready_before = state[READY].load(Ordering::Acquire);
            finish_round_completion(&state, token, true, &mut retired, &mut result);
            assert!(!retired);
            assert_eq!(result.rounds, 1);
            assert_eq!(result.executed_tasks, 1);
            assert_eq!(result.error, 0);
            assert_eq!(
                state[ARRIVALS + task as usize].load(Ordering::Acquire),
                lane as u32 + 1
            );
            if lane + 1 < WAVE_LANES {
                assert_eq!(state[DONE].load(Ordering::Acquire), done_before);
                assert_eq!(state[READY].load(Ordering::Acquire), ready_before);
            } else {
                assert_eq!(
                    state[DONE].load(Ordering::Acquire),
                    done_before | (1 << task)
                );
                assert_eq!(
                    state[READY].load(Ordering::Acquire),
                    if task == 0 { 6 } else { ready_before }
                );
            }
        }
    }
    assert_eq!(run_snapshot(&state), [1, 0, 7, 7, 41, 0, 64, 64, 64]);
}

#[test]
fn wave_run_late_peer_publication_allows_claim_after_an_empty_probe() {
    let state = run_state([1, 1, 0, 0, 0, 0, 0, 0, 0]);
    let mut peer_retired = false;
    let mut peer_leader = FiniteJoinWorkerResult::default();
    assert_eq!(
        begin_round_claim(&state, 0, 1, &mut peer_retired, &mut peer_leader),
        1
    );
    let mut waiting_retired = false;
    let mut waiting = FiniteJoinWorkerResult::default();
    assert_eq!(
        begin_round_claim(&state, 0, 0, &mut waiting_retired, &mut waiting),
        0
    );
    finish_round_completion(&state, 0, true, &mut waiting_retired, &mut waiting);
    assert_eq!(waiting.rounds, 1);
    assert_eq!(waiting.empty_probes, 1);
    assert!(!waiting_retired);
    for lane in 0..WAVE_LANES {
        let mut result = FiniteJoinWorkerResult::default();
        let mut retired = false;
        if lane == 0 {
            result = peer_leader;
        } else {
            assert_eq!(
                begin_round_claim(&state, lane, 1, &mut retired, &mut result),
                0
            );
        }
        assert!(lane_admitted(&state, 1, 1));
        assert!(complete_coverage(1, lane, WAVE_LANES, true));
        finish_round_completion(&state, 1, true, &mut retired, &mut result);
        assert_eq!(result.executed_tasks, 1);
        assert_eq!(result.error, 0);
        assert!(!retired);
        if lane + 1 < WAVE_LANES {
            assert_eq!(state[READY].load(Ordering::Acquire), 0);
        }
    }
    assert_eq!(state[DONE].load(Ordering::Acquire), 1);
    assert_eq!(state[READY].load(Ordering::Acquire), 6);
    assert_eq!(state[ARRIVALS].load(Ordering::Acquire), 64);
    assert_eq!(
        begin_round_claim(&state, 0, 0, &mut waiting_retired, &mut waiting),
        2
    );
    assert!(lane_admitted(&state, 2, 0));
    assert_eq!(waiting.rounds, 2);
    assert_eq!(waiting.empty_probes, 1);
    assert_eq!(waiting.executed_tasks, 0);
    assert_eq!(waiting.error, 0);
    assert!(!waiting_retired);
    assert_eq!(state[CLAIMED].load(Ordering::Acquire), 3);
    assert_eq!(state[READY].load(Ordering::Acquire), 4);
}

#[test]
fn wave_run_shared_helpers_keep_rejection_and_duplicate_errors() {
    for (words, expected_error) in [
        ([0, 1, 0, 0, 0, 0, 0, 0, 0], super::super::STALE_EPOCH),
        ([1, 2, 0, 0, 0, 0, 0, 0, 0], MISSING_PREDECESSOR),
    ] {
        let state = run_state(words);
        let mut retired = false;
        let mut result = FiniteJoinWorkerResult::default();
        assert_eq!(
            begin_round_claim(&state, 0, 0, &mut retired, &mut result),
            0
        );
        assert!(retired);
        assert_eq!(result.rounds, 1);
        assert_eq!(result.error, expected_error);
        assert_eq!(state[ERRORS].load(Ordering::Acquire), expected_error);
        finish_round_completion(&state, 0, true, &mut retired, &mut result);
        assert_eq!(result.executed_tasks, 0);
        assert_eq!(state[DONE].load(Ordering::Acquire), 0);
        assert_eq!(state[ARRIVALS].load(Ordering::Acquire), 0);
    }
    let state = run_state([1, 6, 1, 1, 1, 0, 64, 0, 0]);
    let mut retired = false;
    let mut result = FiniteJoinWorkerResult::default();
    finish_round_completion(&state, 1, true, &mut retired, &mut result);
    assert!(retired);
    assert_eq!(result.error, DUPLICATE);
    assert_eq!(result.executed_tasks, 0);
    assert_eq!(state[ERRORS].load(Ordering::Acquire), DUPLICATE);
    assert_eq!(state[ARRIVALS].load(Ordering::Acquire), 65);
    assert_eq!(state[DONE].load(Ordering::Acquire), 1);
}

#[test]
fn wave_run_invalid_or_incomplete_coverage_never_publishes() {
    for (token, lane, written, valid) in [(4, 0, 0, true), (1, 0, 63, true), (1, 0, 64, false)] {
        let state = run_state([1, 0, 0, 1, 1, 0, 0, 0, 0]);
        let mut retired = false;
        let mut result = FiniteJoinWorkerResult::default();
        if token == 4 {
            assert!(!lane_admitted(&state, token, 0));
        }
        let all_valid = complete_coverage(token, lane, written, valid);
        assert!(!all_valid);
        finish_round_completion(&state, token, all_valid, &mut retired, &mut result);
        assert!(retired);
        assert_eq!(result.error, INCOMPLETE_WRITES);
        assert_eq!(result.executed_tasks, 0);
        assert_eq!(
            run_snapshot(&state),
            [1, 0, 0, 1, 1, INCOMPLETE_WRITES, 0, 0, 0]
        );
    }
}

#[derive(Debug, Eq, PartialEq)]
struct RunTrace {
    exchanges: Vec<(usize, u32, bool)>,
    result: FiniteJoinWorkerResult,
    state: [u32; WAVE_STATE_WORDS],
    retired: bool,
}

fn shared_helper_trace(
    explicit_counter: bool,
    words: [u32; WAVE_STATE_WORDS],
    initially_retired: bool,
    reject_first: bool,
) -> RunTrace {
    let state = run_state(words);
    let mut result = FiniteJoinWorkerResult::default();
    let mut retired = initially_retired;
    let mut exchanges = Vec::new();
    let mut local_round = 0_u32;
    while if explicit_counter {
        local_round < MAX_ROUNDS
    } else {
        result.rounds < MAX_ROUNDS
    } {
        let round = if explicit_counter {
            local_round
        } else {
            result.rounds
        };
        let phase = round as usize * 3;
        let token = begin_round_claim(&state, 0, 0, &mut retired, &mut result);
        exchanges.push((phase, token, true));
        let valid = lane_admitted(&state, token, 0);
        exchanges.push((phase + 1, u32::from(!valid), false));
        let written = if token == 1 { WAVE_LANES } else { 0 };
        let all_valid =
            complete_coverage(token, 0, written, valid && !(reject_first && round == 0));
        exchanges.push((phase + 2, u32::from(!all_valid), false));
        finish_round_completion(&state, token, all_valid, &mut retired, &mut result);
        local_round += 1;
    }
    RunTrace {
        exchanges,
        result,
        state: run_snapshot(&state),
        retired,
    }
}

#[test]
fn wave_run_scalar_and_legacy_round_traces_keep_all_twenty_four_phases() {
    // Fixed expectation is independent of either loop's phase arithmetic.
    let expected_phases = [
        0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23,
    ];
    for (words, retired, reject_first) in [
        ([1, 1, 0, 0, 0, 0, 0, 0, 0], false, false),
        ([1, 0, 0, 1, 2, 0, 0, 0, 0], false, false),
        ([1, 1, 0, 0, 0, 0, 0, 0, 0], true, false),
        ([1, 0, 7, 7, 21, 0, 64, 64, 64], false, false),
        ([0, 1, 0, 0, 0, 0, 0, 0, 0], false, false),
        ([1, 1, 0, 0, 0, 0, 0, 0, 0], false, true),
    ] {
        let legacy = shared_helper_trace(false, words, retired, reject_first);
        let explicit = shared_helper_trace(true, words, retired, reject_first);
        assert_eq!(legacy, explicit);
        assert_eq!(explicit.result.rounds, 8);
        assert_eq!(explicit.exchanges.len(), 24);
        assert_eq!(
            explicit
                .exchanges
                .iter()
                .map(|event| event.0)
                .collect::<Vec<_>>(),
            expected_phases
        );
        assert_eq!(explicit.exchanges.iter().filter(|event| event.2).count(), 8);
    }
}

#[test]
fn wave_run_callback_signature_accepts_only_borrow_scoped_task_views() {
    // Compile-check the real entry without invoking host-unavailable terminals.
    fn callback_shape<'group, 'dispatch>(
        storage: WaveTaskStorageV1<'dispatch>,
        scope: &mut WorkgroupLdsScope<'group>,
    ) -> Result<FiniteJoinWorkerResult, u32> {
        WaveWorkerV1::run(storage, scope, |task| match task {
            WaveTaskV1::Norm(mut view) => view.reject(),
            WaveTaskV1::Key(mut view) => view.reject(),
        })
    }
    let _ = callback_shape;
}
