use super::*;
use std::{sync::atomic::AtomicUsize, thread as host_thread, vec, vec::Vec};

fn state() -> [AtomicU32; WAVE_STATE_WORDS] {
    initial_state_words().map(AtomicU32::new)
}
fn snapshot(s: &[AtomicU32; WAVE_STATE_WORDS]) -> [u32; WAVE_STATE_WORDS] {
    core::array::from_fn(|i| s[i].load(Ordering::Acquire))
}
fn claim(s: &[AtomicU32; WAVE_STATE_WORDS], worker: u32) -> u32 {
    match claim_fanout(s, worker) {
        Claim::Task(tile) => tile,
        _ => panic!("expected a ready tile"),
    }
}
fn complete(s: &[AtomicU32; WAVE_STATE_WORDS], tile: u32) {
    for _ in 0..64 {
        complete_lane(s, tile).unwrap();
    }
}
fn observation() -> FiniteJoinWorkerResult {
    FiniteJoinWorkerResult {
        error: 0,
        executed_tasks: 0,
        rounds: 0,
        empty_probes: 0,
    }
}

#[test]
fn fixed_layout_and_terminal_census_are_distinct_from_v1() {
    assert_eq!(core::mem::size_of::<WaveMlpTileStorageV2<'_>>(), 88);
    assert_eq!(core::mem::align_of::<WaveMlpTileStorageV2<'_>>(), 8);
    assert_eq!(
        (WORKGROUPS, TILE_ROWS, TASK_COUNT, WAVE_STATE_WORDS),
        (64, 64, 258, 548)
    );
    assert_eq!(STAGE_COUNTS.iter().sum::<u32>(), TASK_COUNT);
    assert_eq!(ARRIVALS + TASK_COUNT as usize, WAVE_STATE_WORDS);
    assert_eq!(super::super::wave_mlp_tasks_v1::WAVE_STATE_WORDS, 11);
    let s = state();
    assert!(!terminal_snapshot(&snapshot(&s)));
    for tile in 0..TASK_COUNT {
        assert_eq!(claim(&s, tile % WORKGROUPS), tile);
        complete(&s, tile);
    }
    let good = snapshot(&s);
    assert!(terminal_snapshot(&good));
    assert_eq!((good[CLAIMED + 8], good[DONE_TILES + 8]), (3, 3));
    for index in 0..WAVE_STATE_WORDS {
        let mut bad = good;
        bad[index] = if (OWNERS..ARRIVALS).contains(&index) {
            65
        } else {
            bad[index] ^ 4
        };
        assert!(!terminal_snapshot(&bad), "word {index}");
    }
}

#[test]
fn one_active_group_completes_all_tiles_then_uniform_stop_within_bound() {
    let s = state();
    let mut result = observation();
    let mut retired = false;
    for tile in 0..TASK_COUNT {
        let token = begin_round_claim(&s, 0, 63, &mut retired, &mut result);
        assert_eq!(token, tile + 1);
        for lane in 0..64 {
            assert!(lane_admitted(&s, token, 63));
            let mut local = observation();
            let mut local_retired = false;
            finish_round_completion(&s, token, true, &mut local_retired, &mut local);
            assert_eq!(local.executed_tasks, 1, "lane {lane}");
        }
    }
    let token = begin_round_claim(&s, 0, 63, &mut retired, &mut result);
    assert_eq!(token, STOP);
    assert_eq!(result.rounds, 259);
    assert!(result.rounds < MAX_ROUNDS);
    let before = snapshot(&s);
    finish_round_completion(&s, STOP, true, &mut retired, &mut result);
    assert_eq!(snapshot(&s), before);
    assert!(terminal_snapshot(&before));
    // Every group may be delayed except this one; this is not a scheduling proof.
}

#[test]
fn batch_scheduler_reversed_completions_preserve_unique_coverage() {
    for workers in [1, 2, 7, 64] {
        let s = state();
        let mut visits = [0u8; TASK_COUNT as usize];
        let mut rounds = 0;
        while s[DONE].load(Ordering::Acquire) != ALL_STAGES {
            assert!(rounds < MAX_ROUNDS);
            let mut active = Vec::new();
            for worker in 0..workers {
                match claim_fanout(&s, worker) {
                    Claim::Task(tile) => {
                        visits[tile as usize] += 1;
                        active.push((tile, worker));
                    }
                    Claim::Empty | Claim::Contended => {}
                    Claim::Rejected(_) => panic!("valid scheduler rejected"),
                }
            }
            assert!(!active.is_empty());
            for (tile, worker) in active.into_iter().rev() {
                assert!(lane_admitted(&s, tile + 1, worker));
                complete(&s, tile);
            }
            rounds += 1;
        }
        assert!(visits.iter().all(|v| *v == 1));
        assert!(terminal_snapshot(&snapshot(&s)));
    }
}

#[test]
fn fan_in_waits_for_last_lane_and_last_tile_of_both_projections() {
    for finish_gate_last in [false, true] {
        let s = state();
        assert_eq!(claim(&s, 0), 0);
        for _ in 0..63 {
            complete_lane(&s, 0).unwrap();
        }
        assert!(matches!(claim_fanout(&s, 1), Claim::Empty));
        assert_eq!(s[DONE].load(Ordering::Acquire), 0);
        complete_lane(&s, 0).unwrap();
        for tile in 1..193 {
            assert_eq!(claim(&s, tile % 64), tile);
        }
        for tile in 1..193 {
            if tile != 96 && tile != 192 {
                complete(&s, tile);
            }
        }
        let (first, last) = if finish_gate_last {
            (192, 96)
        } else {
            (96, 192)
        };
        complete(&s, first);
        for _ in 0..63 {
            complete_lane(&s, last).unwrap();
        }
        assert!(matches!(claim_fanout(&s, 0), Claim::Empty));
        assert_eq!(s[READY].load(Ordering::Acquire), 0);
        complete_lane(&s, last).unwrap();
        assert_eq!(claim(&s, 0), 193);
        assert_eq!(s[DONE].load(Ordering::Acquire), 7);
        for _ in 0..63 {
            complete_lane(&s, 193).unwrap();
        }
        assert!(matches!(claim_fanout(&s, 0), Claim::Empty));
        complete_lane(&s, 193).unwrap();
        assert_eq!(claim(&s, 1), 194);
    }
}

#[test]
fn saturated_cursor_never_overshoots_even_before_delayed_ready_clear() {
    let s = state();
    complete(&s, claim(&s, 0));
    for tile in 1..96 {
        assert_eq!(claim(&s, 1), tile);
        complete(&s, tile);
    }
    // Model a real final claimant paused immediately after its successful CAS.
    assert_eq!(
        s[NEXT + 1].compare_exchange(95, 96, Ordering::AcqRel, Ordering::Acquire),
        Ok(95)
    );
    for _ in 0..1024 {
        assert!(matches!(claim_fanout(&s, 2), Claim::Contended));
        assert_eq!(s[NEXT + 1].load(Ordering::Acquire), 96);
        assert_eq!(s[NEXT + 2].load(Ordering::Acquire), 0);
    }
    assert_eq!(s[DONE].load(Ordering::Acquire), 1);
    // Resume exactly the remaining private claim operations, not a new claim.
    s[READY].fetch_and(!2, Ordering::AcqRel);
    assert_eq!(s[CLAIMED + 3].fetch_or(1, Ordering::AcqRel) & 1, 0);
    assert_eq!(
        s[OWNERS + 96].compare_exchange(0, 2, Ordering::Release, Ordering::Relaxed),
        Ok(0)
    );
    complete(&s, 96);
    assert_eq!(claim(&s, 2), 97);
    assert_eq!(s[ERRORS].load(Ordering::Acquire), 0);
}

#[test]
fn concurrent_leaders_issue_each_cursor_once_and_do_not_overshoot() {
    let s = state();
    let winners = AtomicUsize::new(0);
    host_thread::scope(|scope| {
        for worker in 0..64 {
            let s = &s;
            let winners = &winners;
            scope.spawn(move || match claim_fanout(s, worker) {
                Claim::Task(0) => {
                    winners.fetch_add(1, Ordering::Relaxed);
                }
                Claim::Empty | Claim::Contended => {}
                _ => panic!("duplicate norm"),
            });
        }
    });
    assert_eq!(winners.load(Ordering::Relaxed), 1);
    complete(&s, 0);
    let visits: [AtomicU32; 192] = core::array::from_fn(|_| AtomicU32::new(0));
    host_thread::scope(|scope| {
        for worker in 0..64 {
            let s = &s;
            let visits = &visits;
            scope.spawn(move || {
                for _ in 0..1024 {
                    match claim_fanout(s, worker) {
                        Claim::Task(tile) => {
                            visits[tile as usize - 1].fetch_add(1, Ordering::Relaxed);
                        }
                        Claim::Empty => break,
                        Claim::Contended => {}
                        _ => panic!("valid cursor rejected"),
                    }
                }
            });
        }
    });
    // Bounded contenders may all exhaust while the final cursor winner is
    // descheduled before clearing READY. Once joined, drain with one active
    // owner rather than assuming host scheduler fairness in this CPU test.
    while let Claim::Task(tile) = claim_fanout(&s, 0) {
        visits[tile as usize - 1].fetch_add(1, Ordering::Relaxed);
    }
    assert!(visits.iter().all(|n| n.load(Ordering::Relaxed) == 1));
    assert_eq!(
        (
            s[NEXT + 1].load(Ordering::Acquire),
            s[NEXT + 2].load(Ordering::Acquire)
        ),
        (96, 96)
    );
    assert_eq!(s[ERRORS].load(Ordering::Acquire), 0);
}

#[test]
fn lane_and_tile_atomic_fan_in_publishes_all_relaxed_payload_words() {
    let s = state();
    complete(&s, claim(&s, 0));
    for tile in 1..193 {
        assert_eq!(claim(&s, tile % 64), tile);
    }
    let payload: Vec<AtomicU32> = (0..192 * 64).map(|_| AtomicU32::new(0)).collect();
    host_thread::scope(|scope| {
        for lane in 0..64 {
            let s = &s;
            let payload = &payload;
            scope.spawn(move || {
                for tile in (1..193).rev() {
                    let at = (tile - 1) * 64 + lane;
                    payload[at].store(at as u32 + 1, Ordering::Relaxed);
                    complete_lane(s, tile as u32).unwrap();
                }
            });
        }
        // Deliberately observe before joining the writers: READY/claim, not the
        // host join, must provide visibility. This tests CPU atomics only.
        let mut found = false;
        for _ in 0..1_000_000 {
            match claim_fanout(&s, 0) {
                Claim::Task(193) => {
                    found = true;
                    break;
                }
                Claim::Empty | Claim::Contended => host_thread::yield_now(),
                _ => panic!("unexpected successor"),
            }
        }
        assert!(found);
        for (at, word) in payload.iter().enumerate() {
            assert_eq!(word.load(Ordering::Relaxed), at as u32 + 1);
        }
    });
}

#[test]
fn malformed_claims_duplicate_completion_and_wrong_owners_fail_closed() {
    for (index, value, error) in [
        (EPOCH, 2, STALE_EPOCH),
        (ERRORS, 128, 128),
        (READY, 32, INVALID),
        (NEXT, 2, INVALID),
        (CLAIMED, 1, DUPLICATE),
        (OWNERS, 1, DUPLICATE),
    ] {
        let s = state();
        s[index].store(value, Ordering::Relaxed);
        assert!(matches!(claim_fanout(&s, 0), Claim::Rejected(e) if e == error));
        assert_ne!(s[ERRORS].load(Ordering::Acquire), 0);
        assert!(!terminal_snapshot(&snapshot(&s)));
    }
    let s = state();
    assert!(matches!(claim_fanout(&s, 64), Claim::Rejected(INVALID)));
    let s = state();
    s[READY].store(2, Ordering::Relaxed);
    assert!(matches!(
        claim_fanout(&s, 0),
        Claim::Rejected(MISSING_PREDECESSOR)
    ));
    let s = state();
    let tile = claim(&s, 63);
    assert!(lane_admitted(&s, tile + 1, 63));
    assert!(!lane_admitted(&s, tile + 1, 0));
    complete(&s, tile);
    assert_eq!(complete_lane(&s, tile), Err(DUPLICATE));
    assert!(matches!(claim_fanout(&s, 1), Claim::Rejected(_)));
    let s = state();
    assert_eq!(complete_lane(&s, 0), Err(MISSING_PREDECESSOR));
    let s = state();
    assert_eq!(complete_lane(&s, TASK_COUNT), Err(INVALID));
}

#[test]
fn coverage_counts_every_lane_and_stop_without_fabricated_arrivals() {
    for token in 0..=STOP {
        for lane in 0..64 {
            let expected = if token == 1 {
                64
            } else if token == 194 {
                96
            } else if (2..=TASK_COUNT).contains(&token) && lane == 0 {
                64
            } else {
                0
            };
            assert!(complete_coverage(token, lane, expected, true));
            assert!(!complete_coverage(token, lane, expected + 1, true));
            assert!(!complete_coverage(token, lane, expected, false));
            if expected != 0 {
                assert!(!complete_coverage(token, lane, expected - 1, true));
            }
        }
    }
    assert!(!complete_coverage(STOP + 1, 0, 0, true));
    assert!(!complete_coverage(0, 64, 0, true));
    let s = state();
    let token = claim(&s, 0) + 1;
    let mut result = observation();
    let mut retired = false;
    finish_round_completion(&s, token, false, &mut retired, &mut result);
    assert!(retired);
    assert_eq!(s[ARRIVALS].load(Ordering::Acquire), 0);
    assert_eq!(s[DONE].load(Ordering::Acquire), 0);
    assert_eq!(begin_round_claim(&s, 0, 0, &mut retired, &mut result), STOP);
    assert!(lane_admitted(&s, STOP, 0));
    // The broadcast token takes every lane through the final coverage exchange.
    assert!(complete_coverage(STOP, 63, 0, true));
}

#[test]
fn idle_round_bound_does_not_claim_graph_completion() {
    let s = state();
    s[READY].store(0, Ordering::Relaxed);
    let mut result = observation();
    let mut retired = false;
    for _ in 0..MAX_ROUNDS {
        let token = begin_round_claim(&s, 0, 0, &mut retired, &mut result);
        assert_eq!(token, 0);
        finish_round_completion(&s, token, true, &mut retired, &mut result);
    }
    assert_eq!(result.rounds, MAX_ROUNDS);
    assert_eq!(result.executed_tasks, 0);
    assert!(!terminal_snapshot(&snapshot(&s)));
}

struct Fixture {
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
        let mut f = Self {
            input: vec![0x3f80; 4096],
            norm: vec![0x4000; 4096],
            gate_weight: vec![0; WEIGHT_ELEMENTS],
            up_weight: vec![0; WEIGHT_ELEMENTS],
            down_weight: vec![0; WEIGHT_ELEMENTS],
            normalized: vec![0xa55a; 4096],
            gate: vec![0xa55a; 6144],
            up: vec![0xa55a; 6144],
            activation: vec![0xa55a; 6144],
            down: vec![f32::from_bits(0x7fc0_1234); 4096],
            state: state(),
        };
        for row in 0..6144 {
            f.gate_weight[row * 4096 + 4095] = row as u16;
            f.up_weight[row * 4096 + 4095] = row as u16 + 8192;
        }
        for row in 0..4096 {
            f.down_weight[row * 6144 + 6143] = row as u16 + 16384;
        }
        f
    }
    fn storage(&mut self) -> WaveMlpTileStorageV2<'_> {
        // Exact disjoint heap arrays, retained for this scalar CPU view test.
        unsafe {
            WaveMlpTileStorageV2::from_raw_parts(
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

fn assert_retired_padding_is_inert(
    f: &mut Fixture,
    mut results: [FiniteJoinWorkerResult; WAVE_LANES],
    mut retired: [bool; WAVE_LANES],
    first_stop_round: u32,
    expected_error: u32,
) {
    let before = snapshot(&f.state);
    let before_results = results;
    let start = results[0].rounds;
    assert_eq!(start + 1, first_stop_round);
    assert!(results.iter().all(|result| result.rounds == start));
    let mut callbacks = 0usize;
    {
        let storage = f.storage();
        let state = storage.state();
        for round in start..MAX_ROUNDS {
            let token = begin_round_claim(state, 0, 0, &mut retired[0], &mut results[0]);
            assert_eq!(token, STOP);
            assert_eq!(results[0].rounds, round + 1);
            assert_eq!(results[0].error, expected_error);
            // Model the leader-token and two all-lane OR exchanges explicitly;
            // these scalar tests do not execute hardware LDS collectives.
            for lane in 1..WAVE_LANES {
                assert_eq!(
                    begin_round_claim(state, lane, 0, &mut retired[lane], &mut results[lane]),
                    0
                );
            }
            let admitted = (0..WAVE_LANES).all(|_| lane_admitted(state, token, 0));
            assert!(admitted);
            let mut all_covered = true;
            for lane in 0..WAVE_LANES {
                let (mut written, mut valid) = (0, admitted);
                execute_task_at_v2(&storage, token, lane, &mut written, &mut valid, &mut |_| {
                    callbacks += 1
                });
                assert_eq!(written, 0);
                assert!(valid);
                all_covered &= complete_coverage(token, lane, written, valid);
            }
            assert!(all_covered);
            for lane in 0..WAVE_LANES {
                finish_round_completion(
                    state,
                    token,
                    all_covered,
                    &mut retired[lane],
                    &mut results[lane],
                );
            }
            assert_eq!(snapshot(state), before);
        }
    }
    assert!(retired[0]);
    assert!(retired[1..].iter().all(|value| !value));
    assert_eq!(callbacks, 0);
    assert_eq!(results[0].error, expected_error);
    for (result, before) in results.iter().zip(before_results) {
        assert_eq!(result.rounds, MAX_ROUNDS);
        assert_eq!(result.executed_tasks, before.executed_tasks);
        assert_eq!(result.empty_probes, before.empty_probes);
    }
    assert!(results[1..].iter().all(|result| result.error == 0));
    assert_eq!(snapshot(&f.state), before);
    assert!(f.normalized.iter().all(|&word| word == 0xa55a));
    assert!(f.gate.iter().all(|&word| word == 0xa55a));
    assert!(f.up.iter().all(|&word| word == 0xa55a));
    assert!(f.activation.iter().all(|&word| word == 0xa55a));
    assert!(f.down.iter().all(|value| value.to_bits() == 0x7fc0_1234));
}

#[test]
fn fixed_round_completed_retirement_pads_to_512_without_state_or_payload_writes() {
    let mut f = Fixture::new();
    let mut results = [observation(); WAVE_LANES];
    let mut retired = [false; WAVE_LANES];
    // Exercise the actual claim/completion helpers for the active prefix;
    // payload arithmetic is covered separately, not fabricated by this test.
    for tile in 0..TASK_COUNT {
        let token = begin_round_claim(&f.state, 0, 0, &mut retired[0], &mut results[0]);
        assert_eq!(token, tile + 1);
        for lane in 0..WAVE_LANES {
            if lane != 0 {
                assert_eq!(
                    begin_round_claim(&f.state, lane, 0, &mut retired[lane], &mut results[lane]),
                    0
                );
            }
            assert!(lane_admitted(&f.state, token, 0));
            finish_round_completion(
                &f.state,
                token,
                true,
                &mut retired[lane],
                &mut results[lane],
            );
        }
    }
    assert!(terminal_snapshot(&snapshot(&f.state)));
    assert!(
        results
            .iter()
            .all(|result| result.rounds == 258 && result.executed_tasks == 258)
    );
    assert_retired_padding_is_inert(&mut f, results, retired, 259, 0);
    assert!(terminal_snapshot(&snapshot(&f.state)));
}

#[test]
fn fixed_round_error_retirement_pads_to_512_without_state_or_payload_writes() {
    let mut f = Fixture::new();
    f.state[ERRORS].store(INCOMPLETE_WRITES, Ordering::Release);
    assert_retired_padding_is_inert(
        &mut f,
        [observation(); WAVE_LANES],
        [false; WAVE_LANES],
        1,
        INCOMPLETE_WRITES,
    );
    assert!(!terminal_snapshot(&snapshot(&f.state)));
}

#[test]
fn private_views_map_every_row_once_and_reject_adjacent_tile_writes() {
    let mut f = Fixture::new();
    let mut gate_seen = [0u8; 6144];
    let mut up_seen = [0u8; 6144];
    let mut down_seen = [0u8; 4096];
    for token in 1..=TASK_COUNT {
        for lane in 0..64 {
            let (mut written, mut valid) = (0, true);
            let storage = f.storage();
            execute_task_at_v2(
                &storage,
                token,
                lane,
                &mut written,
                &mut valid,
                &mut |task| match task {
                    WaveMlpTileTaskV2::Norm(mut task) => {
                        assert_eq!(task.input(4095), Some(0x3f80));
                        assert_eq!(task.weight(4095), Some(0x4000));
                        for component in 0..64 {
                            assert!(task.write_component(component, 0x3f80));
                        }
                    }
                    WaveMlpTileTaskV2::Gate(mut task) => {
                        assert_eq!(task.input(4095), Some(0x3f80));
                        assert_eq!(task.weight(64, 0), None);
                        assert_eq!(task.weight(0, 4096), None);
                        for row in 0..64 {
                            let global = (token as usize - 2) * 64 + row;
                            assert_eq!(task.weight(row, 4095), Some(global as u16));
                            if lane == 0 {
                                gate_seen[global] += 1;
                                assert!(task.write_column(row, global as u16));
                            }
                        }
                    }
                    WaveMlpTileTaskV2::Up(mut task) => {
                        assert_eq!(task.weight(64, 0), None);
                        for row in 0..64 {
                            let global = (token as usize - 98) * 64 + row;
                            assert_eq!(task.weight(row, 4095), Some(global as u16 + 8192));
                            if lane == 0 {
                                up_seen[global] += 1;
                                assert!(task.write_column(row, global as u16 + 8192));
                            }
                        }
                    }
                    WaveMlpTileTaskV2::SwiGlu(mut task) => {
                        assert_eq!(task.gate(6143), Some(6143));
                        assert_eq!(task.up(6143), Some(14335));
                        for component in 0..96 {
                            assert!(task.write_component(component, 0x4080));
                        }
                    }
                    WaveMlpTileTaskV2::Down(mut task) => {
                        assert_eq!(task.input(6143), Some(0x4080));
                        assert_eq!(task.weight(64, 0), None);
                        assert_eq!(task.weight(0, 6144), None);
                        for row in 0..64 {
                            let global = (token as usize - 195) * 64 + row;
                            assert_eq!(task.weight(row, 6143), Some(global as u16 + 16384));
                            if lane == 0 {
                                down_seen[global] += 1;
                                assert!(task.write_output(
                                    row,
                                    f32::from_bits(0x3f80_0000 + global as u32)
                                ));
                            }
                        }
                    }
                },
            );
            assert!(complete_coverage(token, lane, written, valid));
        }
    }
    assert!(
        gate_seen
            .iter()
            .chain(&up_seen)
            .chain(&down_seen)
            .all(|v| *v == 1)
    );
    for row in 0..6144 {
        assert_eq!(f.gate[row], row as u16);
        assert_eq!(f.up[row], row as u16 + 8192);
    }
    for row in 0..4096 {
        assert_eq!(f.down[row].to_bits(), 0x3f80_0000 + row as u32);
    }
    for token in [2, 97, 98, 193, 195, 258] {
        for (lane, row) in [(0, 64), (0, 1), (63, 0)] {
            let (mut written, mut valid) = (0, true);
            let storage = f.storage();
            execute_task_at_v2(
                &storage,
                token,
                lane,
                &mut written,
                &mut valid,
                &mut |task| match task {
                    WaveMlpTileTaskV2::Gate(mut task) => assert!(!task.write_column(row, 0)),
                    WaveMlpTileTaskV2::Up(mut task) => assert!(!task.write_column(row, 0)),
                    WaveMlpTileTaskV2::Down(mut task) => assert!(!task.write_output(row, 0.0)),
                    _ => unreachable!(),
                },
            );
            assert!(!valid);
            assert_eq!(written, 0);
        }
    }
    for row in 0..6144 {
        assert_eq!(f.gate[row], row as u16);
        assert_eq!(f.up[row], row as u16 + 8192);
    }
    for row in 0..4096 {
        assert_eq!(f.down[row].to_bits(), 0x3f80_0000 + row as u32);
    }
    assert_eq!(snapshot(&f.state), initial_state_words());
}

#[test]
fn region_validator_checks_all_eleven_exact_disjoint_roots() {
    let good = core::array::from_fn(|index| (0x1000 + index * 0x1000, 0x900, 4));
    assert!(disjoint_regions(good));
    for index in 0..11 {
        for replacement in [
            (0, 0x900, 4),
            (good[index].0 + 1, 0x900, 4),
            (good[index].0, 0, 4),
            (usize::MAX - 3, 8, 4),
            good[(index + 1) % 11],
        ] {
            let mut bad = good;
            bad[index] = replacement;
            assert!(!disjoint_regions(bad));
        }
    }
}
