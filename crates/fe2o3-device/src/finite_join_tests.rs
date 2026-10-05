use super::*;
use core::cell::UnsafeCell;
use std::{array, sync::Barrier, thread as host_thread};

const GUARD: f32 = -12345.5;
const POISON: f32 = f32::from_bits(0x7fc0_abcd);

struct Payload(UnsafeCell<[f32; PAYLOAD_ELEMENTS + 2]>);

// Tests access this storage only through unique task claims and after thread
// joins. UnsafeCell permits disjoint ordinary writes without aliasing &mut.
unsafe impl Sync for Payload {}

impl Payload {
    fn new() -> Self {
        let mut data = [POISON; PAYLOAD_ELEMENTS + 2];
        data[0] = GUARD;
        data[PAYLOAD_ELEMENTS + 1] = GUARD;
        Self(UnsafeCell::new(data))
    }

    fn pointer(&self) -> *mut f32 {
        // SAFETY: the first element is the leading guard, not a task payload.
        unsafe { self.0.get().cast::<f32>().add(1) }
    }

    fn check(&self, input: &[f32; INPUT_ELEMENTS]) {
        // SAFETY: every test calls check after all workers have quiesced.
        let actual = unsafe { &*self.0.get() };
        assert_eq!(actual[0], GUARD);
        assert_eq!(actual[PAYLOAD_ELEMENTS + 1], GUARD);
        for cell in 0..INPUT_ELEMENTS {
            assert_eq!(actual[1 + cell].to_bits(), input[cell].to_bits());
        }
        for cell in 0..TILE_ELEMENTS {
            let expected = input[cell] + input[TILE_ELEMENTS + cell];
            assert_eq!(
                actual[1 + INPUT_ELEMENTS + cell].to_bits(),
                expected.to_bits()
            );
        }
    }

    fn check_untouched(&self) {
        // SAFETY: rejection tests have no active workers or ordinary writes.
        let actual = unsafe { &*self.0.get() };
        assert_eq!(actual[0], GUARD);
        assert_eq!(actual[PAYLOAD_ELEMENTS + 1], GUARD);
        assert!(
            actual[1..=PAYLOAD_ELEMENTS]
                .iter()
                .all(|x| x.to_bits() == POISON.to_bits())
        );
    }
}

fn state(epoch: u32) -> [AtomicU32; STATE_WORDS] {
    [epoch, 3, 0, 0, 0, 0].map(AtomicU32::new)
}

fn input() -> [f32; INPUT_ELEMENTS] {
    array::from_fn(|i| {
        if i < TILE_ELEMENTS {
            i as f32 * 0.25
        } else {
            -(i as f32) * 0.125
        }
    })
}

fn check_done(state: &[AtomicU32]) {
    assert_eq!(state[READY].load(Ordering::Relaxed), 0);
    assert_eq!(state[DONE].load(Ordering::Relaxed), 7);
    assert_eq!(state[CLAIMED].load(Ordering::Relaxed), 7);
    assert_eq!(state[ERRORS].load(Ordering::Relaxed), 0);
}

#[test]
fn either_single_resident_worker_finishes_without_a_peer() {
    for worker in 0..2 {
        let input = input();
        let payload = Payload::new();
        let state = state(9);
        // SAFETY: fresh, disjoint storage and only one active leader.
        let result = unsafe { run_leader(input.as_ptr(), payload.pointer(), &state, 9, worker) };
        assert_eq!(result.error, 0);
        assert_eq!(result.executed_tasks, 3);
        assert_eq!(result.rounds, 3);
        assert_eq!(state[OWNERS].load(Ordering::Relaxed), (worker + 1) * 21);
        check_done(&state);
        payload.check(&input);
    }
}

#[test]
fn both_producer_completion_orders_publish_join_exactly_once() {
    for order in [[0, 1], [1, 0]] {
        let input = input();
        let payload = Payload::new();
        let state = state(9);
        assert_eq!(claim_task(&state, 9, 0), Claim::Task(0));
        assert_eq!(claim_task(&state, 9, 1), Claim::Task(1));
        for (number, task) in order.into_iter().enumerate() {
            // SAFETY: the corresponding unique producer claim is held above.
            unsafe { execute_task(input.as_ptr(), payload.pointer(), task) };
            assert_eq!(complete_task(&state, task), Ok(()));
            assert_eq!(
                state[READY].load(Ordering::Relaxed),
                if number == 0 { 0 } else { 4 }
            );
        }
        assert_eq!(claim_task(&state, 9, 1), Claim::Task(2));
        // SAFETY: the acquired join claim follows both producer completions.
        unsafe { execute_task(input.as_ptr(), payload.pointer(), 2) };
        assert_eq!(complete_task(&state, 2), Ok(()));
        check_done(&state);
        payload.check(&input);
        assert_eq!(state[OWNERS].load(Ordering::Relaxed), 41);
    }
}

#[test]
fn real_concurrent_leaders_preserve_payloads_and_guards() {
    for epoch in 1..=64 {
        let input = input();
        let original = input.map(f32::to_bits);
        let payload = Payload::new();
        let state = state(epoch);
        let start = Barrier::new(2);
        let results = host_thread::scope(|scope| {
            let input = &input;
            let payload = &payload;
            let state = &state;
            let start = &start;
            let spawn = |worker| {
                scope.spawn(move || {
                    start.wait();
                    // SAFETY: each scoped thread is one distinct leader; input is
                    // immutable and the shared state is fresh for this invocation.
                    unsafe { run_leader(input.as_ptr(), payload.pointer(), state, epoch, worker) }
                })
            };
            let first = spawn(0);
            let second = spawn(1);
            [first.join().unwrap(), second.join().unwrap()]
        });
        assert!(
            results
                .iter()
                .all(|r| r.error == 0 && r.rounds <= MAX_ROUNDS)
        );
        assert_eq!(results.iter().map(|r| r.executed_tasks).sum::<u32>(), 3);
        assert_eq!(input.map(f32::to_bits), original);
        check_done(&state);
        payload.check(&input);
    }
}

#[test]
fn distinct_producer_threads_publish_to_the_join_without_a_host_read_barrier() {
    let input = input();
    let payload = Payload::new();
    let state = state(19);
    let claimed = Barrier::new(2);
    host_thread::scope(|scope| {
        let input = &input;
        let payload = &payload;
        let state = &state;
        let claimed = &claimed;
        let spawn = |worker| {
            scope.spawn(move || {
                let mut claim = claim_task(state, 19, worker);
                if claim == Claim::Contended {
                    claim = claim_task(state, 19, worker);
                }
                let Claim::Task(task @ 0..=1) = claim else {
                    panic!("producer claim: {claim:?}")
                };
                // Both roots have different owners before either payload is written.
                // There is deliberately no host synchronization after these writes.
                claimed.wait();
                // SAFETY: each thread owns one producer; only task atomics publish
                // its ordinary writes to whichever thread later claims the join.
                unsafe { execute_task(input.as_ptr(), payload.pointer(), task) };
                assert_eq!(complete_task(state, task), Ok(()));
                // SAFETY: continue this same leader's dispatch with the live epoch.
                unsafe { run_leader(input.as_ptr(), payload.pointer(), state, 19, worker) }
            })
        };
        let first = spawn(0);
        let second = spawn(1);
        let results = [first.join().unwrap(), second.join().unwrap()];
        assert!(results.iter().all(|r| r.error == 0));
        assert_eq!(results.iter().map(|r| r.executed_tasks).sum::<u32>(), 1);
    });
    let owners = state[OWNERS].load(Ordering::Relaxed);
    assert_ne!(owners & 3, (owners >> 2) & 3);
    assert!(matches!(owners & 3, 1 | 2));
    assert!(matches!((owners >> 2) & 3, 1 | 2));
    check_done(&state);
    payload.check(&input);
}

#[test]
fn pending_producers_allow_bounded_empty_exit_without_reading_poison() {
    let input = input();
    let payload = Payload::new();
    let state = state(9);
    assert_eq!(claim_task(&state, 9, 0), Claim::Task(0));
    assert_eq!(claim_task(&state, 9, 0), Claim::Task(1));
    // SAFETY: producer claims are held but not executed; no successor is ready.
    let result = unsafe { run_leader(input.as_ptr(), payload.pointer(), &state, 9, 1) };
    assert_eq!(result.empty_probes, MAX_EMPTY_PROBES);
    assert_eq!(result.rounds, MAX_EMPTY_PROBES);
    assert_eq!(result.executed_tasks, 0);
    payload.check_untouched();
}

#[test]
fn stale_or_zero_epoch_never_claims_or_accesses_payloads() {
    for expected in [0, 8, 10] {
        let input = input();
        let payload = Payload::new();
        let state = state(9);
        // SAFETY: all allocations are valid; epoch rejection precedes access.
        let result = unsafe { run_leader(input.as_ptr(), payload.pointer(), &state, expected, 0) };
        assert_eq!(result.error, STALE_EPOCH);
        assert_eq!(state[READY].load(Ordering::Relaxed), 3);
        assert_eq!(state[CLAIMED].load(Ordering::Relaxed), 0);
        payload.check_untouched();
    }
}

#[test]
fn malformed_ready_and_early_join_are_rejected_before_a_payload_claim() {
    for (ready, error) in [(8, INVALID), (u32::MAX, INVALID), (4, MISSING_PREDECESSOR)] {
        let state = state(9);
        state[READY].store(ready, Ordering::Relaxed);
        assert_eq!(claim_task(&state, 9, 0), Claim::Rejected(error));
        assert_eq!(state[OWNERS].load(Ordering::Relaxed), 0);
    }
}

#[test]
fn duplicate_claim_and_completion_do_not_republish_the_join() {
    let state = state(9);
    assert_eq!(claim_task(&state, 9, 0), Claim::Task(0));
    state[READY].fetch_or(1, Ordering::Relaxed);
    assert_eq!(claim_task(&state, 9, 1), Claim::Rejected(DUPLICATE));
    state[DONE].store(3, Ordering::Relaxed);
    state[READY].store(0, Ordering::Relaxed);
    assert_eq!(complete_task(&state, 0), Err(DUPLICATE));
    assert_eq!(complete_task(&state, 1), Err(DUPLICATE));
    assert_eq!(state[READY].load(Ordering::Relaxed), 0);
}

#[test]
fn invalid_shape_returns_before_device_index_queries_or_memory_effects() {
    let mut input = input();
    let mut payload = [POISON; PAYLOAD_ELEMENTS];
    let state = state(9);
    // SAFETY: all local allocations are live and disjoint; the intentionally
    // short input is rejected before querying device execution state.
    let result = unsafe {
        finite_join_128(
            DisjointSlice::from_raw_parts(input.as_mut_ptr(), INPUT_ELEMENTS - 1),
            DisjointSlice::from_raw_parts(payload.as_mut_ptr(), PAYLOAD_ELEMENTS),
            &state,
            9,
        )
    };
    assert_eq!(result.error, INVALID);
    assert!(payload.iter().all(|x| x.to_bits() == POISON.to_bits()));
    assert_eq!(state[READY].load(Ordering::Relaxed), 3);
}

fn scalar_claim<const N: usize>(state: &[AtomicU32; N], expected_epoch: u32) -> Claim {
    claim_ready_task(
        &state[EPOCH],
        &state[READY],
        &state[CLAIMED],
        &state[ERRORS],
        expected_epoch,
    )
}

fn check_scalar_claim(
    before: [u32; STATE_WORDS],
    expected_epoch: u32,
    expected_claim: Claim,
    after: [u32; STATE_WORDS],
) {
    let six = before.map(AtomicU32::new);
    let nine: [AtomicU32; wave_tasks::WAVE_STATE_WORDS] = array::from_fn(|index| {
        AtomicU32::new(if index < STATE_WORDS {
            before[index]
        } else {
            100 + index as u32
        })
    });
    assert_eq!(scalar_claim(&six, expected_epoch), expected_claim);
    assert_eq!(scalar_claim(&nine, expected_epoch), expected_claim);
    assert_eq!(
        six.each_ref().map(|word| word.load(Ordering::Relaxed)),
        after
    );
    for index in 0..STATE_WORDS {
        assert_eq!(nine[index].load(Ordering::Relaxed), after[index]);
    }
    for (index, word) in nine.iter().enumerate().skip(STATE_WORDS) {
        assert_eq!(word.load(Ordering::Relaxed), 100 + index as u32);
    }
}

#[test]
fn scalar_arbitration_selects_the_same_task_for_six_and_nine_word_states() {
    for (ready, claim, remaining, claimed) in [
        (0, Claim::Empty, 0, 0),
        (1, Claim::Task(0), 0, 1),
        (2, Claim::Task(1), 0, 2),
        (3, Claim::Task(0), 2, 1),
        (4, Claim::Task(2), 0, 4),
        (5, Claim::Task(0), 4, 1),
        (6, Claim::Task(1), 4, 2),
        (7, Claim::Task(0), 6, 1),
    ] {
        check_scalar_claim(
            [9, ready, 3, 0, 21, 16],
            9,
            claim,
            [9, remaining, 3, claimed, 21, 16],
        );
    }
}

#[test]
fn scalar_arbitration_rejects_identically_without_touching_arrivals() {
    for expected_epoch in [0, 8, 10] {
        check_scalar_claim(
            [9, 3, 0, 0, 21, 16],
            expected_epoch,
            Claim::Rejected(STALE_EPOCH),
            [9, 3, 0, 0, 21, 16 | STALE_EPOCH],
        );
    }
    for ready in [8, 9, u32::MAX] {
        check_scalar_claim(
            [9, ready, 0, 0, 21, 16],
            9,
            Claim::Rejected(INVALID),
            [9, ready, 0, 0, 21, 16 | INVALID],
        );
    }
    for bit in [1, 2, 4] {
        check_scalar_claim(
            [9, bit, 3, bit, 21, 16],
            9,
            Claim::Rejected(DUPLICATE),
            [9, 0, 3, bit, 21, 16 | DUPLICATE],
        );
    }
}

fn check_scalar_claim_race<const N: usize>() {
    for _ in 0..64 {
        let state: [AtomicU32; N] = array::from_fn(|index| {
            AtomicU32::new(match index {
                EPOCH => 9,
                READY => 1,
                6.. => 100 + index as u32,
                _ => 0,
            })
        });
        let start = Barrier::new(2);
        let outcomes = host_thread::scope(|scope| {
            let state = &state;
            let start = &start;
            let first = scope.spawn(move || {
                start.wait();
                scalar_claim(state, 9)
            });
            let second = scope.spawn(move || {
                start.wait();
                scalar_claim(state, 9)
            });
            [first.join().unwrap(), second.join().unwrap()]
        });
        assert_eq!(outcomes.iter().filter(|&&c| c == Claim::Task(0)).count(), 1);
        // The loser can read READY before or after the winner's RMW. Neither
        // schedule is forced by the host barrier, and neither is an error.
        assert_eq!(
            outcomes
                .iter()
                .filter(|&&c| matches!(c, Claim::Empty | Claim::Contended))
                .count(),
            1
        );
        for (index, expected) in [9, 0, 0, 1, 0, 0].into_iter().enumerate() {
            assert_eq!(state[index].load(Ordering::Relaxed), expected);
        }
        for (index, word) in state.iter().enumerate().skip(STATE_WORDS) {
            assert_eq!(word.load(Ordering::Relaxed), 100 + index as u32);
        }
    }
}

#[test]
fn scalar_arbitration_has_one_winner_with_empty_or_contended_loser() {
    check_scalar_claim_race::<STATE_WORDS>();
    check_scalar_claim_race::<{ wave_tasks::WAVE_STATE_WORDS }>();
}
