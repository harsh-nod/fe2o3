//! CPU tests of the actual private round body and transition helpers. Host
//! atomics/barriers do not establish AMDGPU lowering or device visibility.
use super::*;
use core::cell::UnsafeCell;
use std::{sync::Barrier, thread as host_thread, vec::Vec};

const SENTINEL: u32 = 0x7fc0_5a5a;

fn fresh() -> [AtomicU32; MULTIWAVE_STATE_WORDS] {
    [1, 3, 0, 0, 0, 0, 0, 0, 0].map(AtomicU32::new)
}

fn snapshot(state: &[AtomicU32; MULTIWAVE_STATE_WORDS]) -> [u32; MULTIWAVE_STATE_WORDS] {
    core::array::from_fn(|i| state[i].load(Ordering::Acquire))
}

fn terminal(words: [u32; MULTIWAVE_STATE_WORDS]) -> bool {
    words[0] == 1
        && words[1] == 0
        && words[2] == 7
        && words[3] == 7
        && words[4] & !63 == 0
        && (0..3).all(|task| matches!((words[4] >> (2 * task)) & 3, 1 | 2))
        && words[5] == 0
        && words[6..] == [128, 128, 128]
}

struct Payload(UnsafeCell<[f32; PAYLOAD_ELEMENTS]>);

// SAFETY: test participants use unique task/lane cells and the production
// publication helpers. No host payload read occurs until all workers join.
unsafe impl Sync for Payload {}

impl Payload {
    fn new() -> Self {
        Self(UnsafeCell::new(
            [f32::from_bits(SENTINEL); PAYLOAD_ELEMENTS],
        ))
    }
    fn bits(&self) -> [u32; PAYLOAD_ELEMENTS] {
        // SAFETY: callers invoke only before worker creation or after all joins.
        unsafe { (*self.0.get()).map(f32::to_bits) }
    }
}

fn storage<'a>(
    input: &'a [f32; INPUT_ELEMENTS],
    payload: &'a Payload,
    state: &'a [AtomicU32; MULTIWAVE_STATE_WORDS],
) -> MultiwaveJoinStorageV1<'a> {
    // SAFETY: this private fixture retains the disjoint roots and invokes the
    // real closed per-lane body exactly once per simulated physical lane.
    unsafe { MultiwaveJoinStorageV1::from_raw_parts(input, payload.0.get(), state).unwrap() }
}

struct HostGroup {
    slots: [[AtomicU32; WORKGROUP_LANES]; 2],
    barrier: Barrier,
}

impl HostGroup {
    fn new() -> Self {
        Self {
            slots: core::array::from_fn(|_| core::array::from_fn(|_| AtomicU32::new(0))),
            barrier: Barrier::new(WORKGROUP_LANES),
        }
    }
}

struct HostExchange<'a> {
    group: &'a HostGroup,
    injection: Option<(usize, usize)>,
    trace: Vec<(usize, bool)>,
}

impl Exchange128 for HostExchange<'_> {
    fn exchange(&mut self, phase: usize, lane: usize, mut value: u32, leader_only: bool) -> u32 {
        self.trace.push((phase, leader_only));
        if self.injection == Some((phase, lane)) {
            value |= INVALID;
        }
        let bank = &self.group.slots[phase % 2];
        bank[lane].store(value, Ordering::Relaxed);
        self.group.barrier.wait();
        let mut result = bank[0].load(Ordering::Relaxed);
        if !leader_only {
            for item in &bank[1..] {
                result |= item.load(Ordering::Relaxed);
            }
        }
        result
    }
}

fn execute_groups(
    input: &[f32; INPUT_ELEMENTS],
    payload: &Payload,
    state: &[AtomicU32; MULTIWAVE_STATE_WORDS],
    workers: &[u32],
    injection: Option<(usize, usize)>,
) -> Vec<(u32, usize, FiniteJoinWorkerResult, Vec<(usize, bool)>)> {
    let groups: Vec<_> = workers.iter().map(|_| HostGroup::new()).collect();
    host_thread::scope(|scope| {
        let mut handles = Vec::new();
        for (index, &worker) in workers.iter().enumerate() {
            for lane in 0..WORKGROUP_LANES {
                let group = &groups[index];
                handles.push(
                    host_thread::Builder::new()
                        .stack_size(256 * 1024)
                        .spawn_scoped(scope, move || {
                            let bound = storage(input, payload, state);
                            let mut exchange = HostExchange {
                                group,
                                injection,
                                trace: Vec::new(),
                            };
                            let observation = run_lane(&bound, lane, worker, &mut exchange);
                            (worker, lane, observation, exchange.trace)
                        })
                        .unwrap(),
                );
            }
        }
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect()
    })
}

fn check_trace(rows: &[(u32, usize, FiniteJoinWorkerResult, Vec<(usize, bool)>)]) {
    let expected: Vec<_> = (0..MAX_ROUNDS as usize * 3)
        .map(|phase| (phase, phase % 3 == 0))
        .collect();
    for (_, _, result, trace) in rows {
        assert_eq!(result.rounds, MAX_ROUNDS);
        assert_eq!(trace, &expected);
    }
}

fn input_case(case: usize) -> [f32; INPUT_ELEMENTS] {
    core::array::from_fn(|index| match case {
        0 => 0.0,
        1 => index as f32 - 117.0,
        2 => {
            if index < TILE_ELEMENTS {
                index as f32 + 0.25
            } else {
                -((index - TILE_ELEMENTS) as f32 + 0.25)
            }
        }
        3 => match index % 8 {
            0 => f32::from_bits(0x8000_0000),
            1 => 0.0,
            2 => 1.0,
            3 => f32::from_bits(0x3380_0000),
            4 => f32::from_bits(0x3f80_0001),
            5 => -1.0,
            6 => f32::from_bits(0x0080_0000),
            _ => f32::from_bits(0x8080_0000),
        },
        _ => {
            let bits = (index as u32)
                .wrapping_mul(1_664_525)
                .wrapping_add(1_013_904_223);
            (bits as i32 % 65536) as f32 / 128.0
        }
    })
}

fn reference(input: &[f32; INPUT_ELEMENTS]) -> [u32; PAYLOAD_ELEMENTS] {
    core::array::from_fn(|index| {
        if index < INPUT_ELEMENTS {
            input[index].to_bits()
        } else {
            (input[index - INPUT_ELEMENTS] + input[index - TILE_ELEMENTS]).to_bits()
        }
    })
}

#[test]
fn multiwave_layout_keeps_three_roots_but_not_the_old_state_or_lds_contract() {
    assert_eq!(core::mem::size_of::<MultiwaveJoinStorageV1<'_>>(), 24);
    assert_eq!(core::mem::align_of::<MultiwaveJoinStorageV1<'_>>(), 8);
    assert_eq!(core::mem::offset_of!(MultiwaveJoinStorageV1<'_>, input), 0);
    assert_eq!(
        core::mem::offset_of!(MultiwaveJoinStorageV1<'_>, payload),
        8
    );
    assert_eq!(core::mem::offset_of!(MultiwaveJoinStorageV1<'_>, state), 16);
    assert_eq!(MULTIWAVE_STATE_WORDS, 9);
    assert_eq!(CONTROL_LDS_BYTES, 1024);
    assert_eq!(WORKGROUP_LANES * WORKGROUPS, 256);
    assert_eq!(super::super::STATE_WORDS, 6);
}

#[test]
fn multiwave_binding_rejects_null_alignment_overflow_and_all_overlap_pairs() {
    let valid = [(0x1000, 1024, 4), (0x2000, 1536, 4), (0x3000, 36, 4)];
    assert!(disjoint_regions(valid));
    for root in 0..3 {
        for base in [0, valid[root].0 + 1, usize::MAX - 3] {
            let mut invalid = valid;
            invalid[root].0 = base;
            assert!(!disjoint_regions(invalid));
        }
        for other in 0..3 {
            if root != other {
                let mut invalid = valid;
                invalid[root].0 = valid[other].0 + 4;
                assert!(!disjoint_regions(invalid));
            }
        }
    }
    // Invalid raw roots are checked without dereferencing them.
    assert!(
        unsafe {
            MultiwaveJoinStorageV1::from_raw_parts(
                core::ptr::null(),
                0x2000 as *mut [f32; PAYLOAD_ELEMENTS],
                0x3000 as *const [AtomicU32; MULTIWAVE_STATE_WORDS],
            )
        }
        .is_err()
    );
}

#[test]
fn multiwave_all_cells_match_legacy_arithmetic_and_independent_formula_bitwise() {
    for case in 0..5 {
        let input = input_case(case);
        let payload = Payload::new();
        let state = fresh();
        let bound = storage(&input, &payload, &state);
        let mut legacy = [f32::from_bits(SENTINEL); PAYLOAD_ELEMENTS];
        for task in 0..3 {
            assert_eq!(claim_task(&state, task % 2), Claim::Task(task));
            for lane in 0..WORKGROUP_LANES {
                assert_eq!(admission_error(&state, task + 1, task % 2), 0);
                // Each cell executes once after admission. Delay arrivals until
                // all admissions; predecessor tasks have already completed.
                execute_cell(&bound, task, lane).unwrap();
            }
            for _ in 0..WORKGROUP_LANES {
                complete_lane(&state, task).unwrap();
            }
            // SAFETY: independent retained leader-only reference, separate output.
            unsafe { super::super::execute_task(input.as_ptr(), legacy.as_mut_ptr(), task) };
        }
        assert_eq!(payload.bits(), reference(&input));
        assert_eq!(payload.bits(), legacy.map(f32::to_bits));
        assert!(terminal(snapshot(&state)));
    }
}

#[test]
fn multiwave_each_task_uses_both_waves_without_adjacent_output_writes() {
    let input = input_case(1);
    let payload = Payload::new();
    let state = fresh();
    let bound = storage(&input, &payload, &state);
    for task in 0..3 {
        assert_eq!(claim_task(&state, 0), Claim::Task(task));
        for lane in (0..WORKGROUP_LANES).rev() {
            let before = payload.bits();
            execute_cell(&bound, task, lane).unwrap();
            let after = payload.bits();
            let cell = task as usize * TILE_ELEMENTS + lane;
            for index in 0..PAYLOAD_ELEMENTS {
                assert_eq!(
                    after[index],
                    if index == cell {
                        reference(&input)[index]
                    } else {
                        before[index]
                    }
                );
            }
        }
        for _ in 0..WORKGROUP_LANES {
            complete_lane(&state, task).unwrap();
        }
    }
    assert_eq!(payload.bits(), reference(&input));
}

#[test]
fn multiwave_second_wave_is_required_to_overwrite_all_output_sentinels() {
    let input = input_case(0);
    let payload = Payload::new();
    let state = fresh();
    let bound = storage(&input, &payload, &state);
    assert_eq!(claim_task(&state, 0), Claim::Task(0));
    for lane in 0..64 {
        execute_cell(&bound, 0, lane).unwrap();
        complete_lane(&state, 0).unwrap();
    }
    assert_eq!(&payload.bits()[..64], &[0; 64]);
    assert_eq!(&payload.bits()[64..128], &[SENTINEL; 64]);
    assert_eq!(state[DONE].load(Ordering::Acquire), 0);
    assert_eq!(state[ARRIVALS].load(Ordering::Acquire), 64);
    assert_ne!(payload.bits(), reference(&input));
}

#[test]
fn multiwave_cell_rejects_invalid_tasks_without_root_or_state_writes() {
    let input = input_case(1);
    let input_before = input.map(f32::to_bits);
    let payload = Payload::new();
    let state = fresh();
    let bound = storage(&input, &payload, &state);
    let payload_before = payload.bits();
    let state_before = snapshot(&state);
    for task in [3, 4, u32::MAX] {
        for lane in [0, WORKGROUP_LANES - 1, usize::MAX] {
            assert_eq!(execute_cell(&bound, task, lane), Err(INVALID));
            assert_eq!(input.map(f32::to_bits), input_before);
            assert_eq!(payload.bits(), payload_before);
            assert_eq!(snapshot(&state), state_before);
        }
    }
}

#[test]
fn multiwave_cell_rejects_invalid_lanes_and_round_keeps_all_exchanges() {
    let input = input_case(1);
    let input_before = input.map(f32::to_bits);
    let payload = Payload::new();
    let state = fresh();
    let bound = storage(&input, &payload, &state);
    let payload_before = payload.bits();
    let state_before = snapshot(&state);
    for task in 0..3 {
        for lane in [WORKGROUP_LANES, WORKGROUP_LANES + 1, usize::MAX] {
            assert_eq!(execute_cell(&bound, task, lane), Err(INVALID));
            assert_eq!(input.map(f32::to_bits), input_before);
            assert_eq!(payload.bits(), payload_before);
            assert_eq!(snapshot(&state), state_before);
        }
    }

    // Fault injection bypasses the real exchange solely to reach the defensive
    // coordinate rejection. It does not claim to simulate an admitted workgroup.
    struct InvalidLaneExchange(Vec<(usize, bool)>);
    impl Exchange128 for InvalidLaneExchange {
        fn exchange(&mut self, phase: usize, _: usize, value: u32, leader_only: bool) -> u32 {
            self.0.push((phase, leader_only));
            match phase % 3 {
                0 => 1,
                1 => 0,
                _ => value,
            }
        }
    }
    assert_eq!(claim_task(&state, 0), Claim::Task(0));
    let claimed = snapshot(&state);
    let mut exchange = InvalidLaneExchange(Vec::new());
    let result = run_lane(&bound, WORKGROUP_LANES, 0, &mut exchange);
    assert_eq!(result.error, INVALID);
    assert_eq!(result.executed_tasks, 0);
    assert_eq!(result.rounds, MAX_ROUNDS);
    let expected: Vec<_> = (0..MAX_ROUNDS as usize * 3)
        .map(|phase| (phase, phase % 3 == 0))
        .collect();
    assert_eq!(exchange.0, expected);
    let mut rejected = claimed;
    rejected[ERRORS] = INVALID;
    assert_eq!(snapshot(&state), rejected);
    assert_eq!(input.map(f32::to_bits), input_before);
    assert_eq!(payload.bits(), payload_before);
}

#[test]
fn multiwave_actual_round_body_finishes_with_either_workgroup_entirely_delayed() {
    for first in [0, 1] {
        let input = input_case(4);
        let payload = Payload::new();
        let state = fresh();
        let active = execute_groups(&input, &payload, &state, &[first], None);
        check_trace(&active);
        assert!(
            active
                .iter()
                .all(|(_, _, result, _)| result.error == 0 && result.executed_tasks == 3)
        );
        assert!(terminal(snapshot(&state)));
        assert_eq!(payload.bits(), reference(&input));
        let delayed = execute_groups(&input, &payload, &state, &[1 - first], None);
        check_trace(&delayed);
        assert!(
            delayed
                .iter()
                .all(|(_, _, result, _)| result.error == 0 && result.executed_tasks == 0)
        );
        assert_eq!(payload.bits(), reference(&input));
    }
}

#[test]
fn multiwave_two_active_workgroups_use_the_same_closed_round_body() {
    let input = input_case(3);
    let payload = Payload::new();
    let state = fresh();
    let rows = execute_groups(&input, &payload, &state, &[0, 1], None);
    check_trace(&rows);
    assert!(rows.iter().all(|(_, _, result, _)| result.error == 0));
    assert_eq!(
        rows.iter()
            .map(|(_, _, result, _)| result.executed_tasks)
            .sum::<u32>(),
        3 * 128
    );
    assert!(terminal(snapshot(&state)));
    assert_eq!(payload.bits(), reference(&input));
}

#[test]
fn multiwave_stale_epoch_keeps_all_twenty_four_exchanges_and_never_writes() {
    let input = input_case(1);
    let payload = Payload::new();
    let state = fresh();
    state[EPOCH].store(2, Ordering::Relaxed);
    let rows = execute_groups(&input, &payload, &state, &[0], None);
    check_trace(&rows);
    assert!(
        rows.iter()
            .all(|(_, _, result, _)| result.error & STALE_EPOCH != 0 && result.executed_tasks == 0)
    );
    assert_eq!(payload.bits(), [SENTINEL; PAYLOAD_ELEMENTS]);
    assert_eq!(snapshot(&state)[6..], [0, 0, 0]);
}

#[test]
fn multiwave_error_from_only_wave_one_prevents_both_waves_payload_and_arrivals() {
    let input = input_case(1);
    let payload = Payload::new();
    let state = fresh();
    let rows = execute_groups(&input, &payload, &state, &[0], Some((1, 64)));
    check_trace(&rows);
    assert!(
        rows.iter()
            .all(|(_, _, result, _)| result.error & INVALID != 0 && result.executed_tasks == 0)
    );
    assert_eq!(payload.bits(), [SENTINEL; PAYLOAD_ELEMENTS]);
    assert_eq!(snapshot(&state)[6..], [0, 0, 0]);
}

#[test]
fn multiwave_empty_workgroup_exhausts_without_waiting_for_other_residency() {
    let input = input_case(1);
    let payload = Payload::new();
    let state = [1, 0, 0, 3, 10, 0, 0, 0, 0].map(AtomicU32::new);
    let rows = execute_groups(&input, &payload, &state, &[0], None);
    check_trace(&rows);
    assert!(rows.iter().all(|(_, _, result, _)| result.error == 0
        && result.executed_tasks == 0
        && result.empty_probes == 8));
    assert!(!terminal(snapshot(&state)));
    assert_eq!(payload.bits(), [SENTINEL; PAYLOAD_ELEMENTS]);
}

#[test]
fn multiwave_last_arrival_and_last_producer_are_the_only_dependency_publishers() {
    let state = fresh();
    assert_eq!(claim_task(&state, 0), Claim::Task(0));
    assert_eq!(claim_task(&state, 1), Claim::Task(1));
    for task in [1, 0] {
        let before = state[DONE].load(Ordering::Acquire);
        for count in 1..128 {
            complete_lane(&state, task).unwrap();
            assert_eq!(
                state[ARRIVALS + task as usize].load(Ordering::Acquire),
                count
            );
            assert_eq!(state[DONE].load(Ordering::Acquire), before);
            assert_eq!(state[READY].load(Ordering::Acquire), 0);
        }
        complete_lane(&state, task).unwrap();
        assert_eq!(state[DONE].load(Ordering::Acquire), before | (1 << task));
    }
    assert_eq!(state[READY].load(Ordering::Acquire), 4);
    assert_eq!(claim_task(&state, 1), Claim::Task(2));
    assert_eq!(admission_error(&state, 3, 1), 0);
    for _ in 0..128 {
        complete_lane(&state, 2).unwrap();
    }
    assert!(terminal(snapshot(&state)));
    assert_eq!(complete_lane(&state, 2), Err(DUPLICATE));
    assert!(!terminal(snapshot(&state)));
}

#[test]
fn multiwave_lane_admission_rejects_owner_predecessor_and_arrival_mismatches() {
    let state = fresh();
    assert_eq!(claim_task(&state, 0), Claim::Task(0));
    assert_eq!(admission_error(&state, 1, 0), 0);
    assert_eq!(admission_error(&state, 1, 1), INVALID);
    state[ARRIVALS].store(1, Ordering::Relaxed);
    assert_eq!(admission_error(&state, 1, 0), DUPLICATE);
    state[ARRIVALS].store(0, Ordering::Relaxed);
    state[CLAIMED].store(5, Ordering::Relaxed);
    state[OWNERS].store(17, Ordering::Relaxed);
    assert_eq!(admission_error(&state, 3, 0), MISSING_PREDECESSOR);
    for token in [ERROR_TOKEN, 7, u32::MAX] {
        assert_eq!(admission_error(&state, token, 0), INVALID);
    }
    assert_eq!(claim_task(&state, 2), Claim::Rejected(INVALID));
}

#[test]
fn multiwave_admission_explicit_token_interval_preserves_all_status_values() {
    for owner in 0..2 {
        for done in [0, 3] {
            let state = fresh();
            state[CLAIMED].store(7, Ordering::Relaxed);
            state[OWNERS].store((owner + 1) * 21, Ordering::Relaxed);
            state[DONE].store(done, Ordering::Relaxed);
            let before = snapshot(&state);
            for worker in [0, 1, 2, u32::MAX] {
                for token in (0..=255).chain([u32::MAX - 1, u32::MAX]) {
                    let expected = if worker >= 2 || token > 5 {
                        INVALID
                    } else {
                        match token {
                            0 | 4 | 5 => 0,
                            1..=3 if worker != owner => INVALID,
                            1 | 2 if done == 3 => DUPLICATE,
                            3 if done == 0 => MISSING_PREDECESSOR,
                            1..=3 => 0,
                            _ => unreachable!(),
                        }
                    };
                    assert_eq!(admission_error(&state, token, worker), expected);
                    assert_eq!(snapshot(&state), before);
                }
            }
        }
    }
}

#[test]
fn multiwave_admission_header_errors_precede_token_and_worker_rejection() {
    for (index, value, expected) in [
        (ERRORS, 1 << 31, 1 << 31),
        (EPOCH, 0, STALE_EPOCH),
        (READY, 8, INVALID),
        (DONE, 8, INVALID),
        (CLAIMED, 8, INVALID),
        (OWNERS, 3, INVALID),
        (OWNERS, 64, INVALID),
    ] {
        let state = fresh();
        state[index].store(value, Ordering::Relaxed);
        let before = snapshot(&state);
        for worker in [0, 1, 2, u32::MAX] {
            for token in [
                0,
                1,
                2,
                3,
                CONTENDED_TOKEN,
                RETIRED_TOKEN,
                ERROR_TOKEN,
                7,
                u32::MAX,
            ] {
                assert_eq!(admission_error(&state, token, worker), expected);
                assert_eq!(snapshot(&state), before);
            }
        }
    }
    let source = include_str!("multiwave_join_v1.rs");
    let admission = source
        .split("fn admission_error(")
        .nth(1)
        .unwrap()
        .split("fn complete_lane(")
        .next()
        .unwrap();
    let invalid = admission
        .find("if token > RETIRED_TOKEN || worker >= WORKGROUPS as u32")
        .unwrap();
    let interval = admission.find("if token < 1 || token > 3").unwrap();
    let subtract = admission.find("let task = token - 1;").unwrap();
    let access = admission.find("state[ARRIVALS + task as usize]").unwrap();
    assert!(invalid < interval && interval < subtract && subtract < access);
}

#[test]
fn multiwave_all_terminal_words_and_unused_owner_bits_are_checked() {
    let valid = [1, 0, 7, 7, 25, 0, 128, 128, 128];
    assert!(terminal(valid));
    for index in 0..MULTIWAVE_STATE_WORDS {
        let mut bad = valid;
        bad[index] ^= if index == OWNERS { 1 << 6 } else { 1 };
        assert!(!terminal(bad), "unchecked terminal word {index}");
    }
    for task in 0..3 {
        for owner in [0, 3] {
            let mut bad = valid;
            bad[OWNERS] = (bad[OWNERS] & !(3 << (2 * task))) | (owner << (2 * task));
            assert!(!terminal(bad));
        }
    }
    for (index, value) in [
        (READY, 8),
        (DONE, 8),
        (CLAIMED, 8),
        (OWNERS, 64),
        (OWNERS, 3),
        (EPOCH, 0),
    ] {
        let state = fresh();
        state[index].store(value, Ordering::Relaxed);
        assert_ne!(header_error(&state), 0);
    }
}

#[test]
fn multiwave_concurrent_arbitration_never_duplicates_a_ready_task() {
    let state = fresh();
    let mut tasks = host_thread::scope(|scope| {
        let handles: Vec<_> = (0..16)
            .map(|index| {
                let state = &state;
                scope.spawn(move || claim_task(state, index % 2))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    // A one-attempt contender can lose task0 even while task1 remains ready.
    // Drain at most the two initial tasks after joining; do not assume fairness.
    for _ in 0..2 {
        tasks.push(claim_task(&state, 0));
    }
    assert_eq!(
        tasks.iter().filter(|task| **task == Claim::Task(0)).count(),
        1
    );
    assert_eq!(
        tasks.iter().filter(|task| **task == Claim::Task(1)).count(),
        1
    );
    assert!(
        tasks
            .iter()
            .all(|task| !matches!(task, Claim::Task(2) | Claim::Rejected(_)))
    );
    assert_eq!(state[CLAIMED].load(Ordering::Acquire), 3);
    assert_eq!(state[READY].load(Ordering::Acquire), 0);
}

#[test]
fn multiwave_acqrel_arrival_chain_exposes_both_waves_before_host_thread_join() {
    let state = fresh();
    assert_eq!(claim_task(&state, 0), Claim::Task(0));
    let payload: [AtomicU32; WORKGROUP_LANES] = core::array::from_fn(|_| AtomicU32::new(0));
    let observers = AtomicU32::new(0);
    host_thread::scope(|scope| {
        for lane in 0..WORKGROUP_LANES {
            let state = &state;
            let payload = &payload;
            let observers = &observers;
            scope.spawn(move || {
                payload[lane].store(lane as u32 + 1, Ordering::Relaxed);
                complete_lane(state, 0).unwrap();
                if state[DONE].load(Ordering::Acquire) & 1 != 0 {
                    // Checked inside the participants, before the host joins.
                    for (index, cell) in payload.iter().enumerate() {
                        assert_eq!(cell.load(Ordering::Relaxed), index as u32 + 1);
                    }
                    observers.fetch_add(1, Ordering::Relaxed);
                }
            });
        }
    });
    assert!(observers.load(Ordering::Relaxed) >= 1);
    assert_eq!(state[ARRIVALS].load(Ordering::Acquire), 128);
}

#[test]
fn multiwave_source_keeps_closed_fixed_rounds_and_true_workgroup_exchange() {
    let source = include_str!("multiwave_join_v1.rs");
    let run = source
        .split("fn run_lane(")
        .nth(1)
        .unwrap()
        .split("fn launch_dimensions_valid")
        .next()
        .unwrap();
    assert_eq!(run.matches("exchange.exchange(").count(), 3);
    assert!(run.contains("while round < MAX_ROUNDS"));
    for forbidden in [
        "break;",
        "continue;",
        "return ",
        "broadcast_f32",
        "Gfx950Subgroup",
    ] {
        assert!(!run.contains(forbidden));
    }
    let native = source
        .split("impl NativeExchange<'_>")
        .nth(1)
        .unwrap()
        .split("impl Exchange128 for NativeExchange")
        .next()
        .unwrap();
    assert!(native.contains("while index < WORKGROUP_LANES"));
    assert_eq!(native.matches(".wait(phase)").count(), 1);
    assert!(!native.contains("Gfx950Subgroup"));
    let body = source
        .split("fn execute_cell(")
        .nth(1)
        .unwrap()
        .split("trait Exchange128")
        .next()
        .unwrap();
    assert!(!body.contains("while "));
    assert!(!body.contains("lane == 0"));
    assert!(body.contains("left + right"));
    assert!(!source.contains("unsafe fn execute_cell("));
    for forbidden in [
        ".cast::<f32>()",
        ".add(",
        ".read()",
        ".write(",
        "&*",
        "&mut *",
    ] {
        assert!(!body.contains(forbidden));
    }
    let (_, guarded) = body
        .split_once("if task >= 3 || lane >= WORKGROUP_LANES")
        .unwrap();
    let (refusal, scalar_places) = guarded.split_once("unsafe {").unwrap();
    assert!(refusal.contains("return Err(INVALID);"));
    assert!(scalar_places.contains("(*storage.payload)[cell] = (*storage.input)[cell];"));
    assert!(scalar_places.contains("(*storage.payload)[2 * TILE_ELEMENTS + lane] = left + right;"));
    assert!(run.contains("match execute_cell(storage, token - 1, lane)"));
    assert!(run.contains("Ok(()) => match complete_lane(state, token - 1)"));
}

#[test]
fn multiwave_native_exchange_keeps_force_inline_off_the_trait_method() {
    let source = include_str!("multiwave_join_v1.rs");
    let inherent = source
        .split("impl NativeExchange<'_>")
        .nth(1)
        .unwrap()
        .split("impl Exchange128 for NativeExchange")
        .next()
        .unwrap();
    assert!(inherent.contains(
        "#[cfg_attr(target_arch = \"amdgpu\", rustc_force_inline)]\n    fn exchange_native("
    ));
    let adapter = source
        .split("impl Exchange128 for NativeExchange")
        .nth(1)
        .unwrap()
        .split("#[cfg_attr")
        .next()
        .unwrap();
    assert!(adapter.contains("#[inline(always)]\n    fn exchange("));
    assert!(!adapter.contains("#[rustc_force_inline"));
    assert!(!adapter.contains("cfg_attr"));
    assert_eq!(adapter.matches("self.exchange_native(").count(), 1);
    assert!(adapter.contains("self.exchange_native(phase, lane, value, leader_only)"));
    assert!(!adapter.contains("self.pipeline"));

    let mut remaining = inherent;
    for operation in [
        "self.pipeline.stage(phase);",
        "self.pipeline.write(phase, lane, value);",
        "self.pipeline.commit(phase);",
        "self.pipeline.wait(phase);",
        "self.pipeline.consume(phase);",
        "self.pipeline.read(phase, 0);",
        "self.pipeline.release(phase);",
    ] {
        let (_, rest) = remaining.split_once(operation).unwrap();
        remaining = rest;
        assert_eq!(inherent.matches(operation).count(), 1);
    }
}
