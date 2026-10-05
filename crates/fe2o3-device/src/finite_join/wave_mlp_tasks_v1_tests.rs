//! Real atomic/view tests. Explicit round outcomes do not execute GPU LDS or
//! prove GPU collective convergence, device coherence, or scheduling progress.
use super::*;
use std::{
    sync::{Mutex, MutexGuard},
    vec,
    vec::Vec,
};

fn state() -> [AtomicU32; WAVE_STATE_WORDS] {
    core::array::from_fn(|index| AtomicU32::new(u32::from(index < 2)))
}
fn complete(state: &[AtomicU32; WAVE_STATE_WORDS], task: u32) {
    for _ in 0..64 {
        assert_eq!(complete_lane(state, task), Ok(()));
    }
}

#[test]
fn exact_storage_layout_and_five_task_constants() {
    assert_eq!(core::mem::size_of::<WaveMlpTaskStorageV1<'_>>(), 88);
    assert_eq!(core::mem::align_of::<WaveMlpTaskStorageV1<'_>>(), 8);
    assert_eq!(WAVE_STATE_WORDS, 11);
    assert_eq!(WEIGHT_ELEMENTS, 25_165_824);
    assert_eq!(ALL_TASKS, 0x1f);
    assert_eq!([0, 1, 2, 3, 4].map(predecessor_mask), [0, 1, 1, 7, 15]);
}

#[test]
fn both_projection_completion_orders_publish_swiglu_once() {
    for first in [1, 2] {
        let state = state();
        assert_eq!(claim_fanout(&state, 0), Claim::Task(0));
        complete(&state, 0);
        assert_eq!(state[READY].load(Ordering::Acquire), 6);
        assert_eq!(claim_fanout(&state, 0), Claim::Task(1));
        assert_eq!(claim_fanout(&state, 1), Claim::Task(2));
        assert!(lane_admitted(&state, 2, 0));
        assert!(lane_admitted(&state, 3, 1));
        complete(&state, first);
        assert_eq!(state[READY].load(Ordering::Acquire), 0);
        complete(&state, 3 - first);
        assert_eq!(state[READY].load(Ordering::Acquire), 8);
        assert_eq!(claim_fanout(&state, 0), Claim::Task(3));
        complete(&state, 3);
        assert_eq!(claim_fanout(&state, 1), Claim::Task(4));
        assert!(lane_admitted(&state, 5, 1));
        complete(&state, 4);
        assert_eq!(state[DONE].load(Ordering::Acquire), ALL_TASKS);
        assert_eq!(state[CLAIMED].load(Ordering::Acquire), ALL_TASKS);
        assert_eq!(state[READY].load(Ordering::Acquire), 0);
        assert_eq!(state[ERRORS].load(Ordering::Acquire), 0);
        assert_eq!((state[OWNERS].load(Ordering::Relaxed) >> 8) & 3, 2);
        assert_eq!(state[OWNERS].load(Ordering::Relaxed) >> 10, 0);
        for task in 0..5 {
            assert_eq!(state[ARRIVALS + task].load(Ordering::Acquire), 64);
        }
    }
}

#[test]
fn no_successor_before_the_final_lane_arrives() {
    let state = state();
    assert_eq!(claim_fanout(&state, 0), Claim::Task(0));
    for _ in 0..63 {
        assert_eq!(complete_lane(&state, 0), Ok(()));
    }
    assert_eq!(state[READY].load(Ordering::Acquire), 0);
    assert_eq!(state[DONE].load(Ordering::Acquire), 0);
    assert_eq!(complete_lane(&state, 0), Ok(()));
    assert_eq!(state[READY].load(Ordering::Acquire), 6);
    assert_eq!(complete_lane(&state, 0), Err(DUPLICATE));
    assert_eq!(state[ERRORS].load(Ordering::Acquire), DUPLICATE);
}

#[test]
fn malformed_claims_and_dependency_admission_fail_closed() {
    for (epoch, ready, worker, error) in [
        (2, 1, 0, STALE_EPOCH),
        (1, 32, 0, INVALID),
        (1, 1, 2, INVALID),
        (1, 2, 0, MISSING_PREDECESSOR),
        (1, 8, 0, MISSING_PREDECESSOR),
        (1, 16, 0, MISSING_PREDECESSOR),
    ] {
        let state = state();
        state[EPOCH].store(epoch, Ordering::Relaxed);
        state[READY].store(ready, Ordering::Relaxed);
        assert_eq!(claim_fanout(&state, worker), Claim::Rejected(error));
        assert_eq!(state[ERRORS].load(Ordering::Acquire), error);
    }
    let state = state();
    assert_eq!(claim_fanout(&state, 0), Claim::Task(0));
    assert!(!lane_admitted(&state, 1, 1));
    assert!(!lane_admitted(&state, 6, 0));
    assert!(!lane_admitted(&state, 1, 2));
    state[READY].store(1, Ordering::Relaxed);
    assert_eq!(claim_fanout(&state, 0), Claim::Rejected(DUPLICATE));
    assert_eq!(complete_lane(&state, 5), Err(INVALID));
}

#[test]
fn simultaneous_leaders_cannot_both_claim_norm() {
    let state = state();
    let claims = std::thread::scope(|scope| {
        let a = scope.spawn(|| claim_fanout(&state, 0));
        let b = scope.spawn(|| claim_fanout(&state, 1));
        [a.join().unwrap(), b.join().unwrap()]
    });
    assert_eq!(
        claims
            .iter()
            .filter(|claim| **claim == Claim::Task(0))
            .count(),
        1
    );
    assert!(
        claims
            .iter()
            .all(|claim| matches!(claim, Claim::Task(0) | Claim::Empty | Claim::Contended))
    );
    assert_eq!(state[ERRORS].load(Ordering::Acquire), 0);
}

#[test]
fn rejected_and_idle_rounds_keep_the_fixed_schedule_without_publication() {
    let state = state();
    let mut result = FiniteJoinWorkerResult::default();
    let mut retired = false;
    let token = begin_round_claim(&state, 0, 0, &mut retired, &mut result);
    assert_eq!(token, 1);
    finish_round_completion(&state, token, false, &mut retired, &mut result);
    assert!(retired);
    for _ in 1..MAX_ROUNDS {
        assert_eq!(
            begin_round_claim(&state, 0, 0, &mut retired, &mut result),
            0
        );
        finish_round_completion(&state, 0, true, &mut retired, &mut result);
    }
    assert_eq!(result.rounds, MAX_ROUNDS);
    assert_eq!(result.error, INCOMPLETE_WRITES);
    assert_eq!(result.executed_tasks, 0);
    assert_eq!(state[DONE].load(Ordering::Acquire), 0);
    assert_eq!(state[READY].load(Ordering::Acquire), 0);
    assert_eq!(state[ARRIVALS].load(Ordering::Acquire), 0);
}

#[test]
fn bounded_error_zero_is_not_completion_when_projection_finishes_late() {
    let state = state();
    assert_eq!(claim_fanout(&state, 0), Claim::Task(0));
    complete(&state, 0);
    assert_eq!(claim_fanout(&state, 0), Claim::Task(1));
    assert_eq!(claim_fanout(&state, 1), Claim::Task(2));
    complete(&state, 2);
    let (mut retired, mut result) = (false, FiniteJoinWorkerResult::default());
    for _ in 0..MAX_ROUNDS {
        assert_eq!(
            begin_round_claim(&state, 0, 1, &mut retired, &mut result),
            0
        );
        finish_round_completion(&state, 0, true, &mut retired, &mut result);
    }
    complete(&state, 1);
    assert_eq!(result.error, 0);
    assert_eq!(result.empty_probes, MAX_ROUNDS * POLLS_PER_ROUND);
    assert_eq!(state[ERRORS].load(Ordering::Acquire), 0);
    assert_eq!(state[DONE].load(Ordering::Acquire), 7);
    assert_eq!(state[READY].load(Ordering::Acquire), 8);
    assert_ne!(state[DONE].load(Ordering::Acquire), ALL_TASKS);
}

#[test]
fn coverage_is_exact_for_every_token_lane_and_cursor_boundary() {
    for token in 0..=5 {
        for lane in 0..64 {
            let expected = if token == 1 {
                64
            } else if token == 4 {
                96
            } else if (token == 2 || token == 3) && lane == 0 {
                6144
            } else if token == 5 && lane == 0 {
                4096
            } else {
                0
            };
            assert!(complete_coverage(token, lane, expected, true));
            assert!(!complete_coverage(token, lane, expected + 1, true));
            if expected > 0 {
                assert!(!complete_coverage(token, lane, expected - 1, true));
            }
            assert!(!complete_coverage(token, lane, expected, false));
        }
    }
    assert!(!complete_coverage(6, 0, 0, true));
    assert!(!complete_coverage(0, 64, 0, true));
}

#[test]
fn region_validator_rejects_alignment_overlap_zero_and_overflow() {
    let good = core::array::from_fn(|index| (0x1000 + index * 0x100, 0x80, 4));
    assert!(disjoint_regions(good));
    for index in 0..11 {
        for change in 0..5 {
            let mut bad = good;
            bad[index] = match change {
                0 => (0, 0x80, 4),
                1 => (good[index].0 + 1, 0x80, 4),
                2 => (good[index].0, 0, 4),
                3 => (usize::MAX - 3, 8, 4),
                _ => (good[(index + 1) % 11].0, 0x80, 4),
            };
            assert!(!disjoint_regions(bad));
        }
    }
}

// Serialize these150MiB real-root fixtures, rather than multiplying their peak
// memory with cargo's test parallelism. All arrays are allocated on the heap.
static FIXTURE_LOCK: Mutex<()> = Mutex::new(());
struct Fixture {
    _guard: MutexGuard<'static, ()>,
    input: Vec<u16>,
    norm: Vec<u16>,
    gate_weight: Vec<u16>,
    up_weight: Vec<u16>,
    down_weight: Vec<u16>,
    normalized: Vec<u16>,
    gate: Vec<u16>,
    up: Vec<u16>,
    activation: Vec<u16>,
    down: Vec<f32>,
    state: [AtomicU32; WAVE_STATE_WORDS],
}
impl Fixture {
    fn new() -> Self {
        Self {
            _guard: FIXTURE_LOCK.lock().unwrap(),
            input: vec![0x3f80; 4096],
            norm: vec![0x4000; 4096],
            gate_weight: vec![0x3f00; WEIGHT_ELEMENTS],
            up_weight: vec![0x3e80; WEIGHT_ELEMENTS],
            down_weight: vec![0xbf80; WEIGHT_ELEMENTS],
            normalized: vec![0xa55a; 4096],
            gate: vec![0xa55a; 6144],
            up: vec![0xa55a; 6144],
            activation: vec![0xa55a; 6144],
            down: vec![f32::from_bits(0x7fc0_1234); 4096],
            state: state(),
        }
    }
    fn storage(&mut self) -> WaveMlpTaskStorageV1<'_> {
        // Exact heap-backed roots, retained throughout each scalar CPU view.
        unsafe {
            WaveMlpTaskStorageV1::from_raw_parts(
                self.input.as_ptr().cast(),
                self.norm.as_ptr().cast(),
                self.gate_weight.as_ptr().cast(),
                self.up_weight.as_ptr().cast(),
                self.down_weight.as_ptr().cast(),
                self.normalized.as_mut_ptr().cast(),
                self.gate.as_mut_ptr().cast(),
                self.up.as_mut_ptr().cast(),
                self.activation.as_mut_ptr().cast(),
                self.down.as_mut_ptr().cast(),
                &self.state,
            )
            .unwrap()
        }
    }
}

#[test]
fn real_storage_constructor_rejects_root_alias_before_any_access() {
    let mut f = Fixture::new();
    let result = unsafe {
        WaveMlpTaskStorageV1::from_raw_parts(
            f.input.as_ptr().cast(),
            f.input.as_ptr().cast(),
            f.gate_weight.as_ptr().cast(),
            f.up_weight.as_ptr().cast(),
            f.down_weight.as_ptr().cast(),
            f.normalized.as_mut_ptr().cast(),
            f.gate.as_mut_ptr().cast(),
            f.up.as_mut_ptr().cast(),
            f.activation.as_mut_ptr().cast(),
            f.down.as_mut_ptr().cast(),
            &f.state,
        )
    };
    assert!(matches!(result, Err(INVALID)));
}

#[test]
fn cps_helper_admits_only_exact_valid_token_without_state_mutation() {
    let mut f = Fixture::new();
    for token in 0..=6 {
        for initial_valid in [false, true] {
            let (mut written, mut valid, mut calls) = (0, initial_valid, 0);
            let storage = f.storage();
            execute_task_at_v1(&storage, token, 63, &mut written, &mut valid, &mut |task| {
                calls += 1;
                let (which, lane) = match task {
                    WaveMlpTaskV1::Norm(task) => (1, task.lane()),
                    WaveMlpTaskV1::Gate(task) => (2, task.lane()),
                    WaveMlpTaskV1::Up(task) => (3, task.lane()),
                    WaveMlpTaskV1::SwiGlu(task) => (4, task.lane()),
                    WaveMlpTaskV1::Down(task) => (5, task.lane()),
                };
                assert_eq!((which, lane), (token, 63));
            });
            assert_eq!(
                calls,
                usize::from(initial_valid && (1..=5).contains(&token))
            );
            assert_eq!(written, 0);
            assert_eq!(valid, initial_valid);
        }
    }
    assert_eq!(f.state[DONE].load(Ordering::Acquire), 0);
    assert!(
        f.normalized
            .iter()
            .chain(&f.gate)
            .chain(&f.up)
            .chain(&f.activation)
            .all(|word| *word == 0xa55a)
    );
}

#[test]
fn actual_claim_views_cover_each_output_once_and_preserve_fp32_low_bits() {
    let mut f = Fixture::new();
    for token in 1..=5 {
        for lane in 0..64 {
            let (mut written, mut valid) = (0, true);
            {
                let storage = f.storage();
                execute_task_at_v1(
                    &storage,
                    token,
                    lane,
                    &mut written,
                    &mut valid,
                    &mut |task| match task {
                        WaveMlpTaskV1::Norm(mut task) => {
                            assert_eq!(task.input(4095), Some(0x3f80));
                            assert_eq!(task.weight(4095), Some(0x4000));
                            assert_eq!(task.input(4096), None);
                            for component in 0..64 {
                                assert!(task.write_component(component, 0x3f80));
                            }
                        }
                        WaveMlpTaskV1::Gate(mut task) => {
                            assert_eq!(task.input(4095), Some(0x3f80));
                            assert_eq!(task.weight(6143, 4095), Some(0x3f00));
                            assert_eq!(task.weight(6144, 0), None);
                            assert_eq!(task.weight(0, 4096), None);
                            if lane == 0 {
                                for row in 0..6144 {
                                    assert!(task.write_column(row, 0x4000));
                                }
                            }
                        }
                        WaveMlpTaskV1::Up(mut task) => {
                            assert_eq!(task.weight(6143, 4095), Some(0x3e80));
                            if lane == 0 {
                                for row in 0..6144 {
                                    assert!(task.write_column(row, 0x4040));
                                }
                            }
                        }
                        WaveMlpTaskV1::SwiGlu(mut task) => {
                            assert_eq!(task.gate(6143), Some(0x4000));
                            assert_eq!(task.up(6143), Some(0x4040));
                            assert_eq!(task.gate(6144), None);
                            for component in 0..96 {
                                assert!(task.write_component(component, 0x4080));
                            }
                        }
                        WaveMlpTaskV1::Down(mut task) => {
                            assert_eq!(task.input(6143), Some(0x4080));
                            assert_eq!(task.weight(4095, 6143), Some(0xbf80));
                            assert_eq!(task.input(6144), None);
                            assert_eq!(task.weight(4096, 0), None);
                            assert_eq!(task.weight(0, 6144), None);
                            if lane == 0 {
                                for row in 0..4096 {
                                    assert!(task.write_output(row, f32::from_bits(0x3f80_0123)));
                                }
                            }
                        }
                    },
                );
            }
            assert!(complete_coverage(token, lane, written, valid));
        }
    }
    assert!(f.normalized.iter().all(|word| *word == 0x3f80));
    assert!(f.gate.iter().all(|word| *word == 0x4000));
    assert!(f.up.iter().all(|word| *word == 0x4040));
    assert!(f.activation.iter().all(|word| *word == 0x4080));
    assert!(f.down.iter().all(|value| value.to_bits() == 0x3f80_0123));
    assert_eq!(f.state[DONE].load(Ordering::Acquire), 0);
}

#[test]
fn reordered_duplicate_and_nonleader_writes_reject_without_advancing() {
    let mut f = Fixture::new();
    for token in 1..=5 {
        for lane in [0, 63] {
            let (mut written, mut valid) = (0, true);
            let storage = f.storage();
            execute_task_at_v1(
                &storage,
                token,
                lane,
                &mut written,
                &mut valid,
                &mut |task| match task {
                    WaveMlpTaskV1::Norm(mut task) => {
                        assert!(!task.write_component(1, 0));
                    }
                    WaveMlpTaskV1::Gate(mut task) => {
                        assert!(!task.write_column(1, 0));
                    }
                    WaveMlpTaskV1::Up(mut task) => {
                        assert!(!task.write_column(1, 0));
                    }
                    WaveMlpTaskV1::SwiGlu(mut task) => {
                        assert!(!task.write_component(1, 0));
                    }
                    WaveMlpTaskV1::Down(mut task) => {
                        assert!(!task.write_output(1, 0.0));
                    }
                },
            );
            assert!(!valid);
            assert_eq!(written, 0);
        }
    }
    for token in [2, 3, 5] {
        let (mut written, mut valid) = (0, true);
        let storage = f.storage();
        execute_task_at_v1(
            &storage,
            token,
            63,
            &mut written,
            &mut valid,
            &mut |task| match task {
                WaveMlpTaskV1::Gate(mut task) => {
                    assert!(!task.write_column(0, 0));
                }
                WaveMlpTaskV1::Up(mut task) => {
                    assert!(!task.write_column(0, 0));
                }
                WaveMlpTaskV1::Down(mut task) => {
                    assert!(!task.write_output(0, 0.0));
                }
                _ => unreachable!(),
            },
        );
        assert!(!valid);
        assert_eq!(written, 0);
    }
    let (mut written, mut valid) = (0, true);
    let storage = f.storage();
    execute_task_at_v1(&storage, 4, 0, &mut written, &mut valid, &mut |task| {
        let WaveMlpTaskV1::SwiGlu(mut task) = task else {
            unreachable!()
        };
        assert!(task.write_component(0, 1));
        assert!(!task.write_component(0, 2));
        assert_eq!(task.gate(0), None);
        assert_eq!(task.up(0), None);
    });
    assert!(!valid);
    assert_eq!(written, 1);
}
