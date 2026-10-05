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

#[test]
fn production_cps_dispatch_preserves_callback_gates_and_validated_post_slot() {
    // Exercise the shared helper in its explicitly scalar metadata mode.
    // run uses WAVE=true; these tests do not simulate its GPU collectives.
    let mut fixture = Fixture::new();
    let original_last_page = fixture.metadata[PAGE_COUNT];
    let initial_state: [u32; WAVE_STATE_WORDS] =
        core::array::from_fn(|index| fixture.state[index].load(Ordering::Relaxed));
    for metadata_case in 0..4 {
        fixture.metadata[0] = if metadata_case == 1 { 2304 } else { 2048 };
        fixture.metadata[PAGE_COUNT] = match metadata_case {
            2 => 144,
            3 => fixture.metadata[1],
            _ => original_last_page,
        };
        let metadata_valid = metadata_case == 0;
        for token in 0..=17_u32 {
            for lane in [0, 63] {
                for valid_initial in [false, true] {
                    let (mut written, mut valid) = (7, valid_initial);
                    let mut calls = 0;
                    let mut variant = None;
                    {
                        let storage = fixture.storage();
                        execute_task_at_v5::<false>(
                            &storage,
                            token,
                            lane,
                            &mut written,
                            &mut valid,
                            &mut |task| {
                                calls += 1;
                                match task {
                                    WaveQkvAttentionOutputTaskV5::Norm(task) => {
                                        assert_eq!(task.lane(), lane);
                                        variant = Some(0);
                                    }
                                    WaveQkvAttentionOutputTaskV5::Projection(task) => {
                                        assert_eq!(task.lane(), lane);
                                        assert_eq!(task.base, (token as usize - 2) * 256);
                                        variant = Some(1);
                                    }
                                    WaveQkvAttentionOutputTaskV5::Post(task) => {
                                        assert_eq!(task.lane(), lane);
                                        assert_eq!(task.slot, 1136);
                                        variant = Some(2);
                                    }
                                    WaveQkvAttentionOutputTaskV5::Attention(task) => {
                                        assert_eq!(task.lane(), lane);
                                        assert_eq!(task.position(), 2048);
                                        variant = Some(3);
                                    }
                                    WaveQkvAttentionOutputTaskV5::OutputProjection(task) => {
                                        assert_eq!(task.lane(), lane);
                                        variant = Some(4);
                                    }
                                }
                            },
                        );
                    }
                    let expected = valid_initial
                        && ((1..=13).contains(&token)
                            || ((14..=15).contains(&token) && metadata_valid)
                            || token == 16);
                    assert_eq!(
                        calls,
                        usize::from(expected),
                        "metadata={metadata_case}, token={token}, valid={valid_initial}"
                    );
                    assert_eq!(
                        variant,
                        if !expected {
                            None
                        } else if token == 1 {
                            Some(0)
                        } else if token == 14 {
                            Some(2)
                        } else if token == 15 {
                            Some(3)
                        } else if token == 16 {
                            Some(4)
                        } else {
                            Some(1)
                        }
                    );
                    assert_eq!(
                        valid,
                        valid_initial && !((14..=15).contains(&token) && !metadata_valid)
                    );
                    assert_eq!(written, 7);
                }
            }
        }
    }
    for (index, expected) in initial_state.into_iter().enumerate() {
        assert_eq!(fixture.state[index].load(Ordering::Relaxed), expected);
    }
    assert!(
        fixture
            .normalized
            .iter()
            .chain(&fixture.output)
            .chain(&fixture.query)
            .chain(&fixture.key_cache)
            .chain(&fixture.value_cache)
            .chain(&fixture.attention)
            .all(|&value| value == 0xa55a)
    );
}

#[test]
fn production_cps_dispatch_keeps_callback_cursor_and_rejection_borrowed() {
    for token in [1, 2, 14, 15, 16] {
        let mut fixture = Fixture::new();
        let (mut written, mut valid) = (0, true);
        let mut calls = 0;
        {
            let storage = fixture.storage();
            execute_task_at_v5::<false>(
                &storage,
                token,
                0,
                &mut written,
                &mut valid,
                &mut |task| {
                    calls += 1;
                    match task {
                        WaveQkvAttentionOutputTaskV5::Norm(mut task) => {
                            assert!(task.write_component(0, 1));
                            task.reject();
                        }
                        WaveQkvAttentionOutputTaskV5::Projection(mut task) => {
                            assert!(task.write_column(0, 2));
                            task.reject();
                        }
                        WaveQkvAttentionOutputTaskV5::Post(mut task) => {
                            assert!(task.write_query_head(0, 3, 4));
                            task.reject();
                        }
                        WaveQkvAttentionOutputTaskV5::Attention(mut task) => {
                            assert!(task.write_head(0, 5, 6));
                            task.reject();
                        }
                        WaveQkvAttentionOutputTaskV5::OutputProjection(mut task) => {
                            assert!(task.write_output(0, 7.25));
                            task.reject();
                        }
                    }
                },
            );
        }
        assert_eq!(calls, 1);
        assert_eq!(written, if (14..=15).contains(&token) { 2 } else { 1 });
        assert!(!valid);
        assert!(!complete_coverage(token, 0, written, valid));
        match token {
            1 => assert_eq!(fixture.normalized[0], 1),
            2 => assert_eq!(fixture.output[0], 2),
            14 => {
                assert_eq!(fixture.query[0], 3);
                assert_eq!(fixture.query[64], 4);
            }
            15 => {
                assert_eq!(fixture.attention[0], 5);
                assert_eq!(fixture.attention[64], 6);
            }
            16 => assert_eq!(fixture.partial[0], 7.25),
            _ => unreachable!(),
        }
        assert!(
            fixture
                .key_cache
                .iter()
                .chain(&fixture.value_cache)
                .all(|&value| value == 0xa55a)
        );
        assert_eq!(fixture.state[DONE].load(Ordering::Acquire), 0);
        assert_eq!(fixture.state[ERRORS].load(Ordering::Acquire), 0);
    }
}

struct Fixture {
    input: Vec<u16>,
    norm_weight: Vec<u16>,
    qkv_weight: Vec<u16>,
    head_norm_weight: Vec<u16>,
    rotary: Vec<f32>,
    metadata: Vec<u32>,
    output_weight: Vec<u16>,
    query: Vec<u16>,
    key_cache: Vec<u16>,
    value_cache: Vec<u16>,
    attention: Vec<u16>,
    partial: Vec<f32>,
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
            head_norm_weight: vec![0x3f80; HEAD_WEIGHT_ELEMENTS],
            rotary: vec![0.5; ROTARY_ELEMENTS],
            metadata: core::iter::once(2048)
                .chain((0..PAGE_COUNT as u32).map(|p| (5 * p + 7) % 144))
                .collect(),
            output_weight: vec![0; OUTPUT_WEIGHT_ELEMENTS],
            query: vec![0xa55a; QUERY_ELEMENTS],
            key_cache: vec![0xa55a; CACHE_ELEMENTS],
            value_cache: vec![0xa55a; CACHE_ELEMENTS],
            attention: vec![0xa55a; ATTENTION_ELEMENTS],
            partial: vec![f32::from_bits(0x7fc0_a55a); OUTPUT_ELEMENTS],
            normalized: vec![0xa55a; NORM_ELEMENTS],
            output: vec![0xa55a; QKV_COLUMNS],
            state: state(),
        }
    }

    fn storage(&mut self) -> WaveQkvAttentionOutputTaskStorageV5<'_> {
        // SAFETY: distinct initialized allocations live under this exclusive
        // fixture borrow; CPU tests issue views sequentially, without GPU calls.
        unsafe {
            WaveQkvAttentionOutputTaskStorageV5::from_raw_parts(
                self.input.as_ptr().cast(),
                self.norm_weight.as_ptr().cast(),
                self.qkv_weight.as_ptr().cast(),
                self.head_norm_weight.as_ptr().cast(),
                self.rotary.as_ptr().cast(),
                self.metadata.as_ptr().cast(),
                self.output_weight.as_ptr().cast(),
                self.normalized.as_mut_ptr().cast(),
                self.output.as_mut_ptr().cast(),
                self.query.as_mut_ptr().cast(),
                self.key_cache.as_mut_ptr().cast(),
                self.value_cache.as_mut_ptr().cast(),
                self.attention.as_mut_ptr().cast(),
                self.partial.as_mut_ptr().cast(),
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
        (12582912, 3072, 22)
    );
    assert_eq!(
        core::mem::size_of::<WaveQkvAttentionOutputTaskStorageV5<'_>>(),
        120
    );
    assert_eq!(
        core::mem::align_of::<WaveQkvAttentionOutputTaskStorageV5<'_>>(),
        8
    );
    assert_eq!((ALL_TASKS, PROJECTION_TASKS), (65535, 8190));
    assert_eq!((OUTPUT_ELEMENTS, OUTPUT_WEIGHT_ELEMENTS), (4096, 8388608));
    assert_eq!(
        core::mem::offset_of!(WaveQkvAttentionOutputTaskStorageV5<'_>, output_weight),
        48
    );
    assert_eq!(
        core::mem::offset_of!(WaveQkvAttentionOutputTaskStorageV5<'_>, attention_output),
        96
    );
    assert_eq!(
        core::mem::offset_of!(WaveQkvAttentionOutputTaskStorageV5<'_>, state),
        112
    );
    assert_eq!(
        core::mem::offset_of!(WaveQkvAttentionOutputTaskStorageV5<'_>, output_partial),
        104
    );
    assert_eq!(super::super::wave_qkv_attention_tasks_v4::TASK_COUNT, 15);
    assert_eq!(
        super::super::wave_qkv_attention_tasks_v4::WAVE_STATE_WORDS,
        21
    );
    assert_eq!(
        core::mem::size_of::<
            super::super::wave_qkv_attention_tasks_v4::WaveQkvAttentionTaskStorageV4<'_>,
        >(),
        104
    );
    assert_eq!(super::super::wave_qkv_post_tasks_v3::TASK_COUNT, 14);
    assert_eq!(super::super::wave_qkv_post_tasks_v3::WAVE_STATE_WORDS, 20);
    assert_eq!(
        core::mem::size_of::<super::super::wave_qkv_post_tasks_v3::WaveQkvPostTaskStorageV3<'_>>(),
        96
    );
    // The old public profile is not widened by the sibling module.
    assert_eq!(super::super::wave_tasks::KEY_COLUMNS, 512);
    assert_eq!(super::super::wave_tasks::WAVE_STATE_WORDS, 9);
}

#[test]
fn sixteen_claims_cover_every_output_once() {
    for worker in 0..2 {
        let mut fixture = Fixture::new();
        {
            let storage = fixture.storage();
            assert_eq!(claim_fanout(storage.state(), worker), Claim::Task(0));
            for lane in 0..WAVE_LANES {
                assert!(lane_admitted(storage.state(), 1, worker));
                let (mut written, mut valid) = (0, true);
                {
                    let mut task = QkvAttentionOutputNormTaskV5 {
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
            for task in 1..POST_TASK {
                assert_eq!(claim_fanout(storage.state(), worker), Claim::Task(task));
                for lane in 0..WAVE_LANES {
                    assert!(lane_admitted(storage.state(), task + 1, worker));
                    let (mut written, mut valid) = (0, true);
                    {
                        let mut view = QkvAttentionOutputProjectionTaskV5 {
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
            assert_eq!(storage.state()[READY].load(Ordering::Acquire), POST_BIT);
            assert_eq!(
                claim_fanout(storage.state(), worker),
                Claim::Task(POST_TASK)
            );
            for lane in 0..WAVE_LANES {
                assert!(lane_admitted(storage.state(), 14, worker));
                let (mut written, mut valid) = (0, true);
                let mut task = AttentionReadyTaskV5 {
                    storage: &storage,
                    lane,
                    slot: storage.cache_slot().unwrap(),
                    written: &mut written,
                    valid: &mut valid,
                };
                for head in 0..16 {
                    assert!(task.write_query_head(
                        head,
                        (head * 128 + lane) as u16,
                        (head * 128 + lane + 64) as u16
                    ));
                }
                for head in 0..4 {
                    assert!(task.write_key_value_head(head, 42, 43, 44, 45));
                }
                assert!(complete_coverage(14, lane, written, valid));
                complete_lane(storage.state(), POST_TASK).unwrap();
            }
            assert_eq!(
                storage.state()[READY].load(Ordering::Acquire),
                ATTENTION_BIT
            );
            assert_eq!(
                claim_fanout(storage.state(), worker),
                Claim::Task(ATTENTION_TASK)
            );
            for lane in 0..WAVE_LANES {
                assert!(lane_admitted(storage.state(), 15, worker));
                let (mut written, mut valid) = (0, true);
                let mut task = AttentionTaskV5 {
                    storage: &storage,
                    lane,
                    position: 2048,
                    written: &mut written,
                    valid: &mut valid,
                };
                for head in 0..16 {
                    assert_eq!(task.query(head, 0), Some((head * 128 + lane) as u16));
                    assert_eq!(task.query(head, 1), Some((head * 128 + lane + 64) as u16));
                    assert!(task.write_head(
                        head,
                        (head * 128 + lane) as u16,
                        (head * 128 + lane + 64) as u16
                    ));
                }
                assert!(complete_coverage(15, lane, written, valid));
                complete_lane(storage.state(), ATTENTION_TASK).unwrap();
            }
            assert_eq!(storage.state()[READY].load(Ordering::Acquire), OUTPUT_BIT);
            assert_eq!(
                claim_fanout(storage.state(), worker),
                Claim::Task(OUTPUT_TASK)
            );
            for lane in 0..WAVE_LANES {
                assert!(lane_admitted(storage.state(), 16, worker));
                let (mut written, mut valid) = (0, true);
                let mut task = OutputProjectionTaskV5 {
                    storage: &storage,
                    lane,
                    written: &mut written,
                    valid: &mut valid,
                };
                for inner in [0, 1023, 2047] {
                    assert_eq!(task.input(inner), Some(inner as u16));
                }
                if lane == 0 {
                    for column in 0..OUTPUT_ELEMENTS {
                        assert!(task.write_output(column, column as f32 + 0.25));
                    }
                }
                assert!(complete_coverage(16, lane, written, valid));
                complete_lane(storage.state(), OUTPUT_TASK).unwrap();
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
        assert_eq!(fixture.query, (0..2048).collect::<Vec<u16>>());
        assert_eq!(fixture.attention, (0..2048).collect::<Vec<u16>>());
        assert_eq!(
            fixture.partial,
            (0..4096).map(|i| i as f32 + 0.25).collect::<Vec<_>>()
        );
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
        let view = QkvAttentionOutputProjectionTaskV5 {
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
            let mut view = QkvAttentionOutputProjectionTaskV5 {
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
            let expected = if token == 14 {
                48
            } else if token == 15 {
                32
            } else if token == 16 {
                if lane == 0 { 4096 } else { 0 }
            } else if token == 1 {
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
    assert!(!complete_coverage(17, 0, 4096, true));
    assert!(!complete_coverage(1, 64, 64, true));
}

#[test]
fn invalid_task_token_worker_epoch_ready_and_duplicate_claim_are_rejected() {
    let state = state();
    assert_eq!(complete_lane(&state, 16), Err(INVALID));
    assert!(!lane_admitted(&state, 17, 0));
    assert!(!lane_admitted(&state, 1, 2));
    assert_eq!(claim_fanout(&state, 2), Claim::Rejected(INVALID));
    assert_eq!(state[CLAIMED].load(Ordering::Acquire), 0);
    state[EPOCH].store(2, Ordering::Release);
    assert_eq!(claim_fanout(&state, 0), Claim::Rejected(STALE_EPOCH));
    state[EPOCH].store(1, Ordering::Release);
    state[READY].store(1 << 16, Ordering::Release);
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
        assert_eq!(state[CLAIMED].load(Ordering::Acquire), QKV_PREDECESSORS);
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
            let task = mask.trailing_zeros();
            let done = predecessor_mask(task);
            state[DONE].store(done, Ordering::Release);
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
            1 << 16,
            1 << 31,
            (1 << 12) | (1 << 16),
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
fn all_fifteen_root_ranges_reject_overlap_null_misalignment_and_overflow() {
    let lengths = [
        8192, 8192, 25165824, 512, 512, 580, 16777216, 8192, 6144, 4096, 2359296, 2359296, 4096,
        16384, 88,
    ];
    let aligns = [2, 2, 2, 2, 4, 4, 2, 2, 2, 2, 2, 2, 2, 4, 4];
    let roots = core::array::from_fn(|i| ((i + 1) * 0x2000000, lengths[i], aligns[i]));
    assert!(disjoint_regions(roots));
    for i in 0..15 {
        for bad in [0, roots[i].0 + 1, usize::MAX - 1] {
            let mut changed = roots;
            changed[i].0 = bad;
            assert!(!disjoint_regions(changed), "role {i}");
        }
        for j in 0..15 {
            if i == j {
                continue;
            }
            let mut changed = roots;
            changed[i].0 = roots[j].0 + roots[j].1 - aligns[i];
            assert!(!disjoint_regions(changed), "roles {i}, {j}");
        }
    }
}

#[test]
fn post_requires_every_predecessor_for_leader_and_followers() {
    for missing in 0..13 {
        let state = state();
        state[DONE].store(QKV_PREDECESSORS & !(1 << missing), Ordering::Release);
        state[READY].store(POST_BIT, Ordering::Release);
        assert_eq!(
            claim_fanout(&state, 0),
            Claim::Rejected(MISSING_PREDECESSOR)
        );
        state[OWNERS].store(1 << 26, Ordering::Relaxed);
        assert!(!lane_admitted(&state, 14, 0));
        assert_eq!(state[DONE].load(Ordering::Acquire) & POST_BIT, 0);
    }
}

#[test]
fn post_packed_weights_and_rotary_halves_keep_exact_offsets() {
    let mut fixture = Fixture::new();
    fixture.head_norm_weight = (0..256).map(|i| 1000 + i).collect();
    fixture.rotary = (0..128).map(|i| i as f32 + 0.25).collect();
    let storage = fixture.storage();
    for lane in 0..64 {
        let (mut written, mut valid) = (0, true);
        let task = AttentionReadyTaskV5 {
            storage: &storage,
            lane,
            slot: 0,
            written: &mut written,
            valid: &mut valid,
        };
        for head in [0, 15, 16, 19] {
            for column in [0, 63, 64, 127] {
                assert_eq!(
                    task.head_weight(head, column),
                    Some((1000 + column + if head < 16 { 0 } else { 128 }) as u16)
                );
            }
        }
        assert_eq!(task.rotary(0), Some(lane as f32 + 0.25));
        assert_eq!(task.rotary(1), Some((lane + 64) as f32 + 0.25));
    }
}

#[test]
fn two_distinct_final_projections_publish_post_only_after_both() {
    for first in 1..12 {
        let state = state();
        state[DONE].store(
            QKV_PREDECESSORS & !(1 << first) & !(1 << 12),
            Ordering::Release,
        );
        state[READY].store(0, Ordering::Release);
        state[ARRIVALS + first].store(63, Ordering::Release);
        state[ARRIVALS + 12].store(63, Ordering::Release);
        host_thread::scope(|scope| {
            let a = scope.spawn(|| complete_lane(&state, first as u32));
            let b = scope.spawn(|| complete_lane(&state, 12));
            a.join().unwrap().unwrap();
            b.join().unwrap().unwrap();
        });
        assert_eq!(state[DONE].load(Ordering::Acquire), QKV_PREDECESSORS);
        assert_eq!(state[READY].load(Ordering::Acquire), POST_BIT);
        assert_eq!(claim_fanout(&state, 0), Claim::Task(POST_TASK));
        assert_eq!(claim_fanout(&state, 1), Claim::Empty);
    }
}

#[test]
fn final_projection_arrivals_publish_exactly_one_post_task() {
    for last in 1..13 {
        let state = state();
        state[DONE].store(QKV_PREDECESSORS & !(1 << last), Ordering::Release);
        state[READY].store(0, Ordering::Release);
        state[ARRIVALS + last].store(62, Ordering::Release);
        host_thread::scope(|scope| {
            let a = scope.spawn(|| complete_lane(&state, last as u32));
            let b = scope.spawn(|| complete_lane(&state, last as u32));
            a.join().unwrap().unwrap();
            b.join().unwrap().unwrap();
        });
        assert_eq!(state[READY].load(Ordering::Acquire), POST_BIT);
        assert_eq!(claim_fanout(&state, 0), Claim::Task(POST_TASK));
        assert_eq!(claim_fanout(&state, 1), Claim::Empty);
        assert_eq!(complete_lane(&state, last as u32), Err(DUPLICATE));
        assert_eq!(state[READY].load(Ordering::Acquire), 0);
    }
}

#[test]
fn scalar_metadata_reads_preserve_raw_u32_bits_without_collectives() {
    let mut fixture = Fixture::new();
    for word in [0, 143, 144, 0x7f80_0001, 0x7fc0_0000, 0x8000_0000, u32::MAX] {
        for index in [0, 1, PAGE_COUNT] {
            fixture.metadata[index] = word;
            assert_eq!(fixture.storage().metadata_word::<false>(index), word);
        }
    }
}

#[test]
fn scalar_cps_rejects_nan_shaped_and_high_metadata_before_callback() {
    let mut fixture = Fixture::new();
    let original = fixture.metadata.clone();
    let initial_state: [u32; WAVE_STATE_WORDS] =
        core::array::from_fn(|index| fixture.state[index].load(Ordering::Relaxed));
    // These values include signaling/quiet NaN-shaped and sign-bit payloads.
    // CPU tests validate the scalar policy only; true broadcast bits require
    // checked gfx950 lowering and numerical GPU acceptance.
    for word in [0x7f80_0001, 0x7fc0_0000, 0xffc0_0001, 0x8000_0000, u32::MAX] {
        for index in [0, 1, PAGE_COUNT / 2, PAGE_COUNT] {
            fixture.metadata.copy_from_slice(&original);
            fixture.metadata[index] = word;
            for (token, lane) in [14, 15]
                .into_iter()
                .flat_map(|token| [0, WAVE_LANES - 1].map(|lane| (token, lane)))
            {
                let (mut written, mut valid, mut calls) = (7, true, 0);
                {
                    let storage = fixture.storage();
                    assert_eq!(storage.cache_slot(), None);
                    assert_eq!(storage.cache_slot_impl::<false>(), None);
                    execute_task_at_v5::<false>(
                        &storage,
                        token,
                        lane,
                        &mut written,
                        &mut valid,
                        &mut |_| {
                            calls += 1;
                        },
                    );
                }
                assert_eq!(calls, 0, "index={index}, word={word:#x}, lane={lane}");
                assert_eq!(written, 7);
                assert!(!valid);
            }
        }
    }
    for (index, expected) in initial_state.into_iter().enumerate() {
        assert_eq!(fixture.state[index].load(Ordering::Relaxed), expected);
    }
    assert!(
        fixture
            .normalized
            .iter()
            .chain(&fixture.output)
            .chain(&fixture.query)
            .chain(&fixture.key_cache)
            .chain(&fixture.value_cache)
            .all(|&value| value == 0xa55a)
    );
}

#[test]
fn post_page_permutation_and_position_are_checked_before_writes() {
    let mut fixture = Fixture::new();
    for position in [0, 15, 16, 2047, 2048, 2303] {
        fixture.metadata[0] = position;
        let expected = ((5 * (position / 16) + 7) % 144) * 16 + position % 16;
        assert_eq!(fixture.storage().cache_slot(), Some(expected as usize));
    }
    fixture.metadata[0] = 2304;
    assert_eq!(fixture.storage().cache_slot(), None);
    fixture.metadata[0] = 2048;
    for entry in 1..=PAGE_COUNT {
        let original = fixture.metadata[entry];
        fixture.metadata[entry] = 144;
        assert_eq!(fixture.storage().cache_slot(), None);
        fixture.metadata[entry] = fixture.metadata[1 + entry % PAGE_COUNT];
        assert_eq!(fixture.storage().cache_slot(), None);
        fixture.metadata[entry] = original;
    }
    assert!(
        fixture
            .query
            .iter()
            .chain(&fixture.key_cache)
            .chain(&fixture.value_cache)
            .all(|&v| v == 0xa55a)
    );
}

#[test]
fn post_pair_writes_are_injective_and_preserve_raw_qkv_and_unselected_cache() {
    let mut fixture = Fixture::new();
    fixture.output = (0..3072).collect();
    let raw = fixture.output.clone();
    for position in [0, 15, 16, 2047, 2048, 2303] {
        fixture.metadata[0] = position;
        fixture.query.fill(0xa55a);
        fixture.key_cache.fill(0xa55a);
        fixture.value_cache.fill(0xa55a);
        let slot;
        {
            let storage = fixture.storage();
            slot = storage.cache_slot().unwrap();
            for lane in 0..64 {
                let (mut written, mut valid) = (0, true);
                let mut task = AttentionReadyTaskV5 {
                    storage: &storage,
                    lane,
                    slot,
                    written: &mut written,
                    valid: &mut valid,
                };
                for head in 0..16 {
                    assert_eq!(
                        task.head_input(head, lane),
                        Some((head * 128 + lane) as u16)
                    );
                    assert!(task.write_query_head(
                        head,
                        (head * 128 + lane) as u16,
                        (head * 128 + lane + 64) as u16
                    ));
                }
                for head in 0..4 {
                    let low = task.value(head, 0).unwrap();
                    let high = task.value(head, 1).unwrap();
                    assert!(task.write_key_value_head(
                        head,
                        (head * 128 + lane) as u16,
                        (head * 128 + lane + 64) as u16,
                        low,
                        high
                    ));
                }
                assert!(complete_coverage(14, lane, written, valid));
            }
        }
        assert_eq!(fixture.output, raw);
        assert_eq!(fixture.query, (0..2048).collect::<Vec<u16>>());
        for index in 0..CACHE_ELEMENTS {
            let selected = (slot * 512..(slot + 1) * 512).contains(&index);
            assert_eq!(
                fixture.key_cache[index],
                if selected {
                    (index % 512) as u16
                } else {
                    0xa55a
                }
            );
            assert_eq!(
                fixture.value_cache[index],
                if selected {
                    (2560 + index % 512) as u16
                } else {
                    0xa55a
                }
            );
        }
    }
}

#[test]
fn post_view_bounds_and_cursor_failures_reject_without_partial_pair_stores() {
    let mut fixture = Fixture::new();
    for (lane, cursor, head, key_value, valid_initial, slot) in [
        (64, 0, 0, false, true, 0),
        (0, 0, 1, false, true, 0),
        (0, 0, 16, false, true, 0),
        (0, 0, 0, false, false, 0),
        (0, 0, 0, true, true, 0),
        (0, 32, 1, true, true, 0),
        (0, 32, 4, true, true, 0),
        (0, 32, 0, true, true, 2304),
    ] {
        let storage = fixture.storage();
        let (mut written, mut valid) = (cursor, valid_initial);
        let mut task = AttentionReadyTaskV5 {
            storage: &storage,
            lane,
            slot,
            written: &mut written,
            valid: &mut valid,
        };
        let accepted = if key_value {
            task.write_key_value_head(head, 1, 2, 3, 4)
        } else {
            task.write_query_head(head, 1, 2)
        };
        assert!(!accepted);
        assert_eq!(written, cursor);
        assert!(!valid);
    }
    assert!(
        fixture
            .query
            .iter()
            .chain(&fixture.key_cache)
            .chain(&fixture.value_cache)
            .all(|&v| v == 0xa55a)
    );
    let storage = fixture.storage();
    let (mut written, mut valid) = (0, true);
    let task = AttentionReadyTaskV5 {
        storage: &storage,
        lane: 0,
        slot: 0,
        written: &mut written,
        valid: &mut valid,
    };
    assert_eq!(task.head_input(20, 0), None);
    assert_eq!(task.head_input(0, 128), None);
    assert_eq!(task.head_weight(20, 0), None);
    assert_eq!(task.head_weight(0, 128), None);
    assert_eq!(task.rotary(2), None);
    assert_eq!(task.value(4, 0), None);
    assert_eq!(task.value(0, 2), None);
}

#[test]
fn attention_requires_post_and_every_transitive_predecessor() {
    for missing in 0..14 {
        let state = state();
        state[DONE].store(ATTENTION_PREDECESSORS & !(1 << missing), Ordering::Release);
        state[READY].store(ATTENTION_BIT, Ordering::Release);
        assert_eq!(
            claim_fanout(&state, 0),
            Claim::Rejected(MISSING_PREDECESSOR)
        );
        state[OWNERS].store(1 << 28, Ordering::Relaxed);
        assert!(!lane_admitted(&state, 15, 0));
        assert_eq!(state[DONE].load(Ordering::Acquire) & ATTENTION_BIT, 0);
    }
    for worker in 0..2 {
        let state = state();
        state[DONE].store(ATTENTION_PREDECESSORS, Ordering::Release);
        state[READY].store(ATTENTION_BIT, Ordering::Release);
        assert_eq!(claim_fanout(&state, worker), Claim::Task(ATTENTION_TASK));
        assert!(lane_admitted(&state, 15, worker));
        assert!(!lane_admitted(&state, 15, 1 - worker));
    }
}

#[test]
fn final_arrivals_release_attention_then_output_once_without_an_output_successor() {
    let state = state();
    state[DONE].store(QKV_PREDECESSORS, Ordering::Release);
    state[READY].store(POST_BIT, Ordering::Release);
    assert_eq!(claim_fanout(&state, 1), Claim::Task(POST_TASK));
    for arrival in 0..64 {
        complete_lane(&state, POST_TASK).unwrap();
        assert_eq!(
            state[READY].load(Ordering::Acquire),
            if arrival == 63 { ATTENTION_BIT } else { 0 }
        );
    }
    assert_eq!(claim_fanout(&state, 0), Claim::Task(ATTENTION_TASK));
    assert_eq!(complete_lane(&state, POST_TASK), Err(DUPLICATE));
    assert_eq!(state[READY].load(Ordering::Acquire), 0);
    arrive_all(&state, ATTENTION_TASK);
    assert_eq!(state[DONE].load(Ordering::Acquire), OUTPUT_PREDECESSORS);
    assert_eq!(state[READY].load(Ordering::Acquire), OUTPUT_BIT);
    assert_eq!(claim_fanout(&state, 1), Claim::Task(OUTPUT_TASK));
    assert_eq!(complete_lane(&state, ATTENTION_TASK), Err(DUPLICATE));
    assert_eq!(state[READY].load(Ordering::Acquire), 0);
    arrive_all(&state, OUTPUT_TASK);
    assert_eq!(state[DONE].load(Ordering::Acquire), ALL_TASKS);
    assert_eq!(state[READY].load(Ordering::Acquire), 0);
    assert_eq!(
        state[ARRIVALS + ATTENTION_TASK as usize].load(Ordering::Acquire),
        65 // The deliberate duplicate above increments before it is rejected.
    );
}

#[test]
fn finite_round_budget_can_leave_error_zero_attention_ready_unclaimed() {
    let state = state();
    state[READY].store(0, Ordering::Release);
    state[DONE].store(QKV_PREDECESSORS, Ordering::Release);
    state[CLAIMED].store(QKV_PREDECESSORS, Ordering::Release);
    let (mut a_retired, mut b_retired) = (false, false);
    let (mut a, mut b) = (observation(), observation());
    for _ in 0..MAX_ROUNDS - 1 {
        assert_eq!(begin_round_claim(&state, 0, 0, &mut a_retired, &mut a), 0);
        assert_eq!(begin_round_claim(&state, 0, 1, &mut b_retired, &mut b), 0);
    }
    state[READY].store(POST_BIT, Ordering::Release);
    assert_eq!(begin_round_claim(&state, 0, 0, &mut a_retired, &mut a), 14);
    assert_eq!(begin_round_claim(&state, 0, 1, &mut b_retired, &mut b), 0);
    assert_eq!((a.rounds, b.rounds), (MAX_ROUNDS, MAX_ROUNDS));
    arrive_all(&state, POST_TASK);
    assert_eq!(state[ERRORS].load(Ordering::Acquire), 0);
    assert_eq!(state[READY].load(Ordering::Acquire), ATTENTION_BIT);
    assert_eq!(state[DONE].load(Ordering::Acquire), ATTENTION_PREDECESSORS);
    assert_eq!(
        state[CLAIMED].load(Ordering::Acquire),
        ATTENTION_PREDECESSORS
    );
    assert_eq!(
        state[ARRIVALS + ATTENTION_TASK as usize].load(Ordering::Acquire),
        0
    );
    assert!(!lane_admitted(&state, 15, 0));
    // No33rd round is executed: a successful dispatch alone cannot prove completion.
}

#[test]
fn attention_reads_causal_nonidentity_pages_all_heads_and_both_halves() {
    let mut fixture = Fixture::new();
    for position in [0, 15, 16, 2047, 2048, 2303] {
        fixture.metadata[0] = position as u32;
        fixture.query = (0..QUERY_ELEMENTS).map(|index| index as u16).collect();
        let tokens = [0, position.min(15), position.min(16), position];
        let mut expected = Vec::new();
        for token in tokens {
            let page = ((token / 16) * 5 + 7) % 144;
            for head in 0..16 {
                for lane in [0, 17, 63] {
                    for half in 0..2 {
                        let dim = lane + half * 64;
                        let index = (page * 16 + token % 16) * 512 + (head / 4) * 128 + dim;
                        let key = (token as u16).wrapping_mul(13) ^ ((head / 4) * 128 + dim) as u16;
                        let value = key ^ 0x9235;
                        fixture.key_cache[index] = key;
                        fixture.value_cache[index] = value;
                        expected.push((token, page as u32, head, lane, half, key, value));
                    }
                }
            }
        }
        {
            let storage = fixture.storage();
            for (token, page, head, lane, half, key, value) in expected {
                let (mut written, mut valid) = (0, true);
                execute_task_at_v5::<false>(
                    &storage,
                    15,
                    lane,
                    &mut written,
                    &mut valid,
                    &mut |task| {
                        let WaveQkvAttentionOutputTaskV5::Attention(task) = task else {
                            panic!("attention variant")
                        };
                        assert_eq!(task.position(), position);
                        assert_eq!(task.page(token), Some(page));
                        assert_eq!(
                            task.query(head, half),
                            Some((head * 128 + lane + half * 64) as u16)
                        );
                        assert_eq!(task.key(head, token, page, half), Some(key));
                        assert_eq!(task.value(head, token, page, half), Some(value));
                        assert_eq!(task.key(head, token, (page + 1) % 144, half), None);
                        assert_eq!(task.value(head, token, (page + 1) % 144, half), None);
                        assert_eq!(task.page(position + 1), None);
                        assert_eq!(task.key(head, position + 1, page, half), None);
                        assert_eq!(task.value(head, position + 1, page, half), None);
                    },
                );
                assert!(valid);
                assert_eq!(written, 0);
            }
        }
        assert!(fixture.attention.iter().all(|&word| word == 0xa55a));
    }
}

#[test]
fn attention_scalar_bounds_and_page_substitution_never_expose_cache_words() {
    let mut fixture = Fixture::new();
    for lane in [0, 64] {
        let storage = fixture.storage();
        let (mut written, mut valid) = (0, true);
        let mut task = AttentionTaskV5 {
            storage: &storage,
            lane,
            position: 2048,
            written: &mut written,
            valid: &mut valid,
        };
        for (head, token, page, half) in [
            (16, 0, 7, 0),
            (0, 0, 7, 2),
            (0, 0, 144, 0),
            (0, 0, u32::MAX, 0),
            (0, 2049, 7, 0),
            (0, 2304, 7, 0),
            (0, usize::MAX, 7, 0),
            (0, 0, 8, 0),
        ] {
            assert_eq!(task.key(head, token, page, half), None);
            assert_eq!(task.value(head, token, page, half), None);
        }
        assert_eq!(task.query(16, 0), None);
        assert_eq!(task.query(0, 2), None);
        assert_eq!(task.page(usize::MAX), None);
        if lane == 64 {
            assert_eq!(task.query(0, 0), None);
            assert_eq!(task.page(0), None);
            assert_eq!(task.key(0, 0, 7, 0), None);
        }
        task.reject();
        assert_eq!(task.query(0, 0), None);
        assert_eq!(task.page(0), None);
        assert_eq!(task.key(0, 0, 7, 0), None);
        assert_eq!(task.value(0, 0, 7, 0), None);
        assert!(!task.write_head(0, 1, 2));
    }
    assert!(fixture.attention.iter().all(|&word| word == 0xa55a));
}

#[test]
fn attention_pair_writes_cover_output_once_and_preserve_all_predecessor_payloads() {
    let mut fixture = Fixture::new();
    {
        let storage = fixture.storage();
        for lane in 0..64 {
            let (mut written, mut valid) = (0, true);
            execute_task_at_v5::<false>(
                &storage,
                15,
                lane,
                &mut written,
                &mut valid,
                &mut |task| {
                    let WaveQkvAttentionOutputTaskV5::Attention(mut task) = task else {
                        panic!("attention variant")
                    };
                    for head in 0..16 {
                        assert!(task.write_head(
                            head,
                            (head * 128 + lane) as u16,
                            (head * 128 + lane + 64) as u16
                        ));
                    }
                },
            );
            assert!(complete_coverage(15, lane, written, valid));
        }
    }
    assert_eq!(fixture.attention, (0..2048).collect::<Vec<u16>>());
    assert!(
        fixture
            .normalized
            .iter()
            .chain(&fixture.output)
            .chain(&fixture.query)
            .chain(&fixture.key_cache)
            .chain(&fixture.value_cache)
            .all(|&word| word == 0xa55a)
    );
}

#[test]
fn attention_pair_cursor_and_bounds_reject_before_either_store() {
    for (lane, head, cursor, initial_valid) in [
        (64, 0, 0, true),
        (0, 16, 32, true),
        (0, 1, 0, true),
        (0, 0, 1, true),
        (0, 0, 0, false),
    ] {
        let mut fixture = Fixture::new();
        let (mut written, mut valid) = (cursor, initial_valid);
        {
            let storage = fixture.storage();
            let mut task = AttentionTaskV5 {
                storage: &storage,
                lane,
                position: 2048,
                written: &mut written,
                valid: &mut valid,
            };
            assert!(!task.write_head(head, 1, 2));
        }
        assert_eq!(written, cursor);
        assert!(!valid);
        assert!(fixture.attention.iter().all(|&word| word == 0xa55a));
    }
}

#[test]
fn output_requires_every_predecessor_even_with_valid_claim_and_owner() {
    for worker in 0..2 {
        for missing in 0..OUTPUT_TASK {
            let state = state();
            let incomplete = OUTPUT_PREDECESSORS & !(1 << missing);
            state[DONE].store(incomplete, Ordering::Release);
            state[READY].store(OUTPUT_BIT, Ordering::Release);
            assert_eq!(
                claim_fanout(&state, worker),
                Claim::Rejected(MISSING_PREDECESSOR)
            );
            assert_eq!(state[ERRORS].load(Ordering::Acquire), MISSING_PREDECESSOR);

            // Prove the follower's acquire gate independently of claim/owner.
            state[CLAIMED].store(OUTPUT_BIT, Ordering::Release);
            state[OWNERS].store((worker + 1) << 30, Ordering::Relaxed);
            assert!(!lane_admitted(&state, 16, worker));
            state[DONE].store(OUTPUT_PREDECESSORS, Ordering::Release);
            assert!(lane_admitted(&state, 16, worker));
            assert!(!lane_admitted(&state, 16, 1 - worker));
        }
    }
}

#[test]
fn output_uses_last_owner_pair_without_changing_previous_owners() {
    for worker in 0..2 {
        let state = state();
        let earlier = 0x2aaa_aaaa;
        state[DONE].store(OUTPUT_PREDECESSORS, Ordering::Release);
        state[READY].store(OUTPUT_BIT, Ordering::Release);
        state[OWNERS].store(earlier, Ordering::Relaxed);
        assert_eq!(claim_fanout(&state, worker), Claim::Task(OUTPUT_TASK));
        assert_eq!(
            state[OWNERS].load(Ordering::Relaxed),
            earlier | ((worker + 1) << 30)
        );
        assert!(lane_admitted(&state, 16, worker));
        for bad in [0, 3] {
            state[OWNERS].store(earlier | (bad << 30), Ordering::Relaxed);
            assert!(!lane_admitted(&state, 16, worker));
        }
    }
}

#[test]
fn only_the_final_attention_arrival_releases_output() {
    let state = state();
    state[DONE].store(ATTENTION_PREDECESSORS, Ordering::Release);
    state[READY].store(ATTENTION_BIT, Ordering::Release);
    assert_eq!(claim_fanout(&state, 0), Claim::Task(ATTENTION_TASK));
    for _ in 0..63 {
        complete_lane(&state, ATTENTION_TASK).unwrap();
        assert_eq!(state[READY].load(Ordering::Acquire), 0);
        assert_eq!(claim_fanout(&state, 1), Claim::Empty);
    }
    complete_lane(&state, ATTENTION_TASK).unwrap();
    assert_eq!(state[READY].load(Ordering::Acquire), OUTPUT_BIT);
    assert_eq!(claim_fanout(&state, 1), Claim::Task(OUTPUT_TASK));
    assert!(lane_admitted(&state, 16, 1));
    assert_eq!(complete_lane(&state, ATTENTION_TASK), Err(DUPLICATE));
    assert_eq!(state[READY].load(Ordering::Acquire), 0);
}

#[test]
fn finite_round_budget_can_leave_error_zero_output_ready_unclaimed() {
    let state = state();
    state[READY].store(0, Ordering::Release);
    state[DONE].store(ATTENTION_PREDECESSORS, Ordering::Release);
    state[CLAIMED].store(ATTENTION_PREDECESSORS, Ordering::Release);
    let (mut a_retired, mut b_retired) = (false, false);
    let (mut a, mut b) = (observation(), observation());
    for _ in 0..MAX_ROUNDS - 1 {
        assert_eq!(begin_round_claim(&state, 0, 0, &mut a_retired, &mut a), 0);
        assert_eq!(begin_round_claim(&state, 0, 1, &mut b_retired, &mut b), 0);
    }
    state[READY].store(ATTENTION_BIT, Ordering::Release);
    assert_eq!(begin_round_claim(&state, 0, 0, &mut a_retired, &mut a), 15);
    assert_eq!(begin_round_claim(&state, 0, 1, &mut b_retired, &mut b), 0);
    arrive_all(&state, ATTENTION_TASK);
    assert_eq!((a.rounds, b.rounds), (MAX_ROUNDS, MAX_ROUNDS));
    assert_eq!(state[ERRORS].load(Ordering::Acquire), 0);
    assert_eq!(state[READY].load(Ordering::Acquire), OUTPUT_BIT);
    assert_eq!(state[DONE].load(Ordering::Acquire), OUTPUT_PREDECESSORS);
    assert_eq!(state[CLAIMED].load(Ordering::Acquire), OUTPUT_PREDECESSORS);
    assert_eq!(
        state[ARRIVALS + OUTPUT_TASK as usize].load(Ordering::Acquire),
        0
    );
    assert!(!lane_admitted(&state, 16, 0));
    // Error zero is not completion: the host must require all16 DONE bits.
}

#[test]
fn output_reads_attention_and_exact_row_major_rank_weight_corners() {
    let mut fixture = Fixture::new();
    fixture.attention[0] = 0x3f80;
    fixture.attention[2047] = 0xc040;
    for (column, inner, value) in [(0, 0, 10), (0, 2047, 11), (1, 0, 12), (4095, 2047, 13)] {
        fixture.output_weight[column * 2048 + inner] = value;
    }
    let storage = fixture.storage();
    let (mut written, mut valid) = (0, true);
    let mut task = OutputProjectionTaskV5 {
        storage: &storage,
        lane: 63,
        written: &mut written,
        valid: &mut valid,
    };
    assert_eq!(task.input(0), Some(0x3f80));
    assert_eq!(task.input(2047), Some(0xc040));
    for (column, inner, value) in [(0, 0, 10), (0, 2047, 11), (1, 0, 12), (4095, 2047, 13)] {
        assert_eq!(task.weight(column, inner), Some(value));
    }
    for (column, inner) in [(4096, 0), (0, 2048), (usize::MAX, 0), (0, usize::MAX)] {
        assert_eq!(task.weight(column, inner), None);
    }
    assert_eq!(task.input(2048), None);
    assert_eq!(task.input(usize::MAX), None);
    task.reject();
    assert_eq!(task.input(0), None);
    assert_eq!(task.weight(0, 0), None);
}

#[test]
fn output_preserves_fp32_bits_and_all_predecessor_payloads() {
    let mut fixture = Fixture::new();
    {
        let storage = fixture.storage();
        for lane in 0..64 {
            let (mut written, mut valid) = (0, true);
            execute_task_at_v5::<false>(
                &storage,
                16,
                lane,
                &mut written,
                &mut valid,
                &mut |task| {
                    let WaveQkvAttentionOutputTaskV5::OutputProjection(mut task) = task else {
                        panic!("output variant")
                    };
                    if lane == 0 {
                        for column in 0..4096 {
                            // Low mantissa bits deliberately cannot survive BF16 narrowing.
                            let bits = if column == 0 {
                                0x8000_0000
                            } else {
                                0x3f80_0000 | column as u32
                            };
                            assert!(task.write_output(column, f32::from_bits(bits)));
                        }
                    }
                },
            );
            assert!(complete_coverage(16, lane, written, valid));
        }
    }
    for column in 0..4096 {
        assert_eq!(
            fixture.partial[column].to_bits(),
            if column == 0 {
                0x8000_0000
            } else {
                0x3f80_0000 | column as u32
            }
        );
    }
    assert!(fixture.input.iter().all(|&word| word == 0x3f80));
    assert!(fixture.norm_weight.iter().all(|&word| word == 0x4000));
    assert!(
        fixture
            .qkv_weight
            .iter()
            .chain(&fixture.output_weight)
            .all(|&word| word == 0)
    );
    assert!(fixture.head_norm_weight.iter().all(|&word| word == 0x3f80));
    assert!(fixture.rotary.iter().all(|&value| value == 0.5));
    assert_eq!(fixture.metadata[0], 2048);
    assert!(
        fixture.metadata[1..]
            .iter()
            .enumerate()
            .all(|(p, &v)| v == ((5 * p + 7) % 144) as u32)
    );
    assert!(
        fixture
            .normalized
            .iter()
            .chain(&fixture.output)
            .chain(&fixture.query)
            .chain(&fixture.key_cache)
            .chain(&fixture.value_cache)
            .chain(&fixture.attention)
            .all(|&word| word == 0xa55a)
    );
}

#[test]
fn output_wrong_lane_cursor_bounds_or_rejection_cannot_store() {
    for (lane, column, cursor, initial_valid) in [
        (1, 0, 0, true),
        (63, 0, 0, true),
        (64, 0, 0, true),
        (0, 1, 0, true),
        (0, 0, 1, true),
        (0, 4096, 4096, true),
        (0, usize::MAX, usize::MAX, true),
        (0, 0, 0, false),
    ] {
        let mut fixture = Fixture::new();
        let (mut written, mut valid) = (cursor, initial_valid);
        {
            let storage = fixture.storage();
            let mut task = OutputProjectionTaskV5 {
                storage: &storage,
                lane,
                written: &mut written,
                valid: &mut valid,
            };
            assert!(!task.write_output(column, 1.0));
        }
        assert!(!valid);
        assert_eq!(written, cursor);
        assert!(
            fixture
                .partial
                .iter()
                .all(|value| value.to_bits() == 0x7fc0_a55a)
        );
    }
}
