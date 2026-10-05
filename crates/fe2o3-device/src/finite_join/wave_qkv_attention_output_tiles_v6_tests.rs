//! Real atomic arbitration and typed payload helpers; no GPU LDS/subgroup
//! terminals are executed here. Modeled exchanges are not convergence evidence.
use super::*;
use std::{thread as host_thread, vec, vec::Vec};

fn state() -> [AtomicU32; WAVE_STATE_WORDS] {
    initial_state_words().map(AtomicU32::new)
}
fn snapshot(s: &[AtomicU32; WAVE_STATE_WORDS]) -> [u32; WAVE_STATE_WORDS] {
    core::array::from_fn(|i| s[i].load(Ordering::Acquire))
}
fn claim(s: &[AtomicU32; WAVE_STATE_WORDS], worker: u32) -> u32 {
    match claim_fanout(s, worker) {
        Claim::Task(tile) => tile,
        _ => panic!("expected ready tile"),
    }
}
fn complete(s: &[AtomicU32; WAVE_STATE_WORDS], tile: u32) {
    for _ in 0..WAVE_LANES {
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
fn finished_state() -> [u32; WAVE_STATE_WORDS] {
    let s = state();
    for tile in 0..TASK_COUNT {
        assert_eq!(claim(&s, tile % WORKGROUPS), tile);
        complete(&s, tile);
    }
    snapshot(&s)
}

#[test]
fn exact_layout_and_all_284_terminal_words_are_generation_specific() {
    assert_eq!(
        (
            WORKGROUPS,
            TILE_ROWS,
            TASK_COUNT,
            WAVE_STATE_WORDS,
            MAX_ROUNDS
        ),
        (64, 64, 130, 284, 256)
    );
    assert_eq!(STAGE_COUNTS, [1, 48, 1, 16, 64]);
    assert_eq!(STAGE_STARTS, [0, 1, 49, 50, 66]);
    assert_eq!(ARRIVALS + TASK_COUNT as usize, WAVE_STATE_WORDS);
    assert_eq!(
        core::mem::size_of::<WaveQkvAttentionOutputTileStorageV6<'_>>(),
        120
    );
    assert_eq!(
        core::mem::align_of::<WaveQkvAttentionOutputTileStorageV6<'_>>(),
        8
    );
    assert_eq!(
        core::mem::offset_of!(WaveQkvAttentionOutputTileStorageV6<'_>, state),
        112
    );
    assert_eq!(
        super::super::wave_qkv_attention_output_tasks_v5::WAVE_STATE_WORDS,
        22
    );
    assert!(!terminal_snapshot(&initial_state_words()));
    let good = finished_state();
    assert!(terminal_snapshot(&good));
    assert_eq!((good[CLAIMED + 4], good[DONE_TILES + 4]), (3, 3));
    for i in 0..WAVE_STATE_WORDS {
        let mut bad = good;
        bad[i] = if (OWNERS..ARRIVALS).contains(&i) {
            65
        } else {
            bad[i] ^ 4
        };
        assert!(!terminal_snapshot(&bad), "word {i}");
    }
    let mut old = [0; WAVE_STATE_WORDS];
    old[..6].copy_from_slice(&[1, 0, 65535, 65535, 0x55555555, 0]);
    old[6..22].fill(64);
    assert!(!terminal_snapshot(&old));
}

#[test]
fn each_stage_waits_for_every_tile_and_the_last_arriving_lane() {
    let s = state();
    for stage in 0..5 {
        let first = STAGE_STARTS[stage];
        let end = first + STAGE_COUNTS[stage];
        for tile in first..end {
            assert_eq!(claim(&s, tile % 64), tile);
        }
        assert!(matches!(claim_fanout(&s, 0), Claim::Empty));
        for tile in (first + 1..end).rev() {
            complete(&s, tile);
        }
        for _ in 0..63 {
            complete_lane(&s, first).unwrap();
        }
        assert_eq!(s[DONE].load(Ordering::Acquire), (1 << stage) - 1);
        assert_eq!(s[READY].load(Ordering::Acquire), 0);
        assert!(matches!(claim_fanout(&s, 0), Claim::Empty));
        complete_lane(&s, first).unwrap();
        assert_eq!(s[DONE].load(Ordering::Acquire), (1 << (stage + 1)) - 1);
        assert_eq!(
            s[READY].load(Ordering::Acquire),
            if stage == 4 { 0 } else { 1 << (stage + 1) }
        );
    }
    assert!(terminal_snapshot(&snapshot(&s)));
}

#[test]
fn reversed_scheduler_with_one_through_64_workers_covers_every_tile_once() {
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
                    _ => panic!("valid schedule rejected"),
                }
            }
            assert!(!active.is_empty());
            for (tile, worker) in active.into_iter().rev() {
                assert!(lane_admitted(&s, tile + 1, worker));
                complete(&s, tile);
            }
            rounds += 1;
        }
        assert!(visits.iter().all(|&v| v == 1));
        assert!(terminal_snapshot(&snapshot(&s)));
    }
}

#[test]
fn concurrent_cursors_have_unique_owners_without_assuming_host_fairness() {
    let s = state();
    for stage in 0..5 {
        let visits: [AtomicU32; 130] = core::array::from_fn(|_| AtomicU32::new(0));
        host_thread::scope(|scope| {
            for worker in 0..64 {
                let (s, visits) = (&s, &visits);
                scope.spawn(move || {
                    for _ in 0..256 {
                        match claim_fanout(s, worker) {
                            Claim::Task(tile) => {
                                visits[tile as usize].fetch_add(1, Ordering::Relaxed);
                            }
                            Claim::Empty => break,
                            Claim::Contended => {}
                            _ => panic!("valid concurrent claim rejected"),
                        }
                    }
                });
            }
        });
        // A bounded contender can finish while the last winner is descheduled.
        // After joining, drain only remaining claims, never presume fairness.
        while let Claim::Task(tile) = claim_fanout(&s, 0) {
            visits[tile as usize].fetch_add(1, Ordering::Relaxed);
        }
        for tile in STAGE_STARTS[stage]..STAGE_STARTS[stage] + STAGE_COUNTS[stage] {
            assert_eq!(visits[tile as usize].load(Ordering::Acquire), 1);
            assert!((1..=64).contains(&s[OWNERS + tile as usize].load(Ordering::Acquire)));
            complete(&s, tile);
        }
        assert_eq!(s[NEXT + stage].load(Ordering::Acquire), STAGE_COUNTS[stage]);
    }
    assert!(terminal_snapshot(&snapshot(&s)));
}

#[test]
fn exhausted_cursor_before_delayed_ready_clear_never_overshoots() {
    let s = state();
    complete(&s, claim(&s, 0));
    for tile in 1..48 {
        assert_eq!(claim(&s, 0), tile);
        complete(&s, tile);
    }
    assert_eq!(
        s[NEXT + 1].compare_exchange(47, 48, Ordering::AcqRel, Ordering::Acquire),
        Ok(47)
    );
    let before = snapshot(&s);
    for _ in 0..1024 {
        assert!(matches!(claim_fanout(&s, 63), Claim::Contended));
    }
    assert_eq!(snapshot(&s), before);
}

#[test]
fn missing_predecessor_stale_epoch_wrong_owner_and_duplicate_refuse() {
    for stage in 1..5 {
        let s = state();
        s[READY].store(1 << stage, Ordering::Relaxed);
        assert!(matches!(
            claim_fanout(&s, 0),
            Claim::Rejected(MISSING_PREDECESSOR)
        ));
        assert_eq!(
            s[OWNERS + STAGE_STARTS[stage] as usize].load(Ordering::Acquire),
            0
        );
    }
    let s = state();
    s[EPOCH].store(0, Ordering::Relaxed);
    assert!(matches!(claim_fanout(&s, 0), Claim::Rejected(STALE_EPOCH)));
    let s = state();
    assert!(matches!(claim_fanout(&s, 64), Claim::Rejected(INVALID)));
    let s = state();
    let tile = claim(&s, 63);
    assert!(lane_admitted(&s, tile + 1, 63));
    assert!(!lane_admitted(&s, tile + 1, 0));
    assert!(!lane_admitted(&s, STOP + 1, 63));
    complete(&s, tile);
    assert_eq!(complete_lane(&s, tile), Err(DUPLICATE));
    assert_ne!(s[ERRORS].load(Ordering::Acquire), 0);
    assert!(!terminal_snapshot(&snapshot(&s)));
}

#[test]
fn every_lane_requires_positive_acquire_of_complete_transitive_predecessors() {
    for stage in 1..5 {
        let s = state();
        for tile in 0..STAGE_STARTS[stage] {
            complete(&s, claim(&s, tile % 64));
        }
        let tile = claim(&s, 3);
        assert_eq!(tile, STAGE_STARTS[stage]);
        for bit in 0..stage {
            s[DONE].fetch_and(!(1 << bit), Ordering::Relaxed);
            assert!(!lane_admitted(&s, tile + 1, 3));
            s[DONE].fetch_or(1 << bit, Ordering::Relaxed);
            assert!(lane_admitted(&s, tile + 1, 3));
        }
        s[OWNERS + tile as usize].store(0, Ordering::Relaxed);
        assert!(!lane_admitted(&s, tile + 1, 3));
    }
}

fn write_claim(task: WaveQkvAttentionOutputTileTaskV6<'_, '_>) {
    match task {
        WaveQkvAttentionOutputTileTaskV6::Norm(mut t) => {
            for c in 0..64 {
                assert!(t.write_component(c, (t.lane() + 64 * c) as u16));
            }
        }
        WaveQkvAttentionOutputTileTaskV6::Projection(mut t) => {
            if t.lane() == 0 {
                for c in 0..64 {
                    assert!(t.write_column(c, (t.base + c) as u16));
                }
            }
        }
        WaveQkvAttentionOutputTileTaskV6::Post(mut t) => {
            for h in 0..16 {
                assert!(t.write_query_head(h, 11, 12));
            }
            for h in 0..4 {
                assert!(t.write_key_value_head(h, 21, 22, 31, 32));
            }
        }
        WaveQkvAttentionOutputTileTaskV6::Attention(mut t) => {
            assert!(t.write_head(t.head(), t.head() as u16, t.head() as u16 + 16));
        }
        WaveQkvAttentionOutputTileTaskV6::OutputProjection(mut t) => {
            if t.lane() == 0 {
                for c in 0..64 {
                    assert!(t.write_output(c, (t.base + c) as f32));
                }
            }
        }
    }
}

#[test]
fn actual_callbacks_complete_then_first_stop_131_pads_inertly_through_256() {
    let mut f = Fixture::new();
    let mut retired = [false; 64];
    let mut results: [FiniteJoinWorkerResult; 64] = core::array::from_fn(|_| observation());
    for tile in 0..TASK_COUNT {
        let token = begin_round_claim(&f.state, 0, 63, &mut retired[0], &mut results[0]);
        assert_eq!(token, tile + 1);
        for lane in 0..64 {
            if lane != 0 {
                assert_eq!(
                    begin_round_claim(&f.state, lane, 63, &mut retired[lane], &mut results[lane]),
                    0
                );
            }
            assert!(lane_admitted(&f.state, token, 63));
            let (mut written, mut valid) = (0, true);
            execute_task_at_v6::<false>(
                &f.storage(),
                token,
                lane,
                &mut written,
                &mut valid,
                &mut write_claim,
            );
            assert!(complete_coverage(token, lane, written, valid));
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
    let before = snapshot(&f.state);
    let payload = f.payload();
    let token = begin_round_claim(&f.state, 0, 63, &mut retired[0], &mut results[0]);
    assert_eq!((token, results[0].rounds), (STOP, 131));
    assert!(retired[0]);
    assert!(retired[1..].iter().all(|x| !*x));
    for round in 131..=MAX_ROUNDS {
        if round != 131 {
            assert_eq!(
                begin_round_claim(&f.state, 0, 63, &mut retired[0], &mut results[0]),
                STOP
            );
        }
        for lane in 0..64 {
            if lane != 0 {
                assert_eq!(
                    begin_round_claim(&f.state, lane, 63, &mut retired[lane], &mut results[lane]),
                    0
                );
            }
            let (mut written, mut valid) = (0, true);
            execute_task_at_v6::<false>(
                &f.storage(),
                STOP,
                lane,
                &mut written,
                &mut valid,
                &mut |_| panic!("retired callback"),
            );
            assert!(lane_admitted(&f.state, STOP, 63));
            assert!(complete_coverage(STOP, lane, written, valid));
            finish_round_completion(&f.state, STOP, true, &mut retired[lane], &mut results[lane]);
        }
        assert_eq!(snapshot(&f.state), before);
    }
    assert_eq!(f.payload(), payload);
    assert!(results.iter().all(|r| r.rounds == 256
        && r.executed_tasks == 130
        && r.empty_probes == 0
        && r.error == 0));
    assert!(retired[1..].iter().all(|x| !*x));
    assert_eq!(MAX_ROUNDS * 3, 768); // Modeled phase count only, not executed LDS.
}

#[test]
fn failed_completion_retires_without_later_callbacks_writes_or_terminal_claim() {
    let mut f = Fixture::new();
    let mut retired = false;
    let mut result = observation();
    let token = begin_round_claim(&f.state, 0, 0, &mut retired, &mut result);
    assert_eq!(token, 1);
    finish_round_completion(&f.state, token, false, &mut retired, &mut result);
    assert!(retired);
    let before = snapshot(&f.state);
    let payload = f.payload();
    for _ in 1..MAX_ROUNDS {
        assert_eq!(
            begin_round_claim(&f.state, 0, 0, &mut retired, &mut result),
            STOP
        );
        for lane in 0..64 {
            let (mut w, mut v) = (0, true);
            execute_task_at_v6::<false>(&f.storage(), STOP, lane, &mut w, &mut v, &mut |_| {
                panic!("error callback")
            });
            assert!(complete_coverage(STOP, lane, w, v));
        }
        finish_round_completion(&f.state, STOP, true, &mut retired, &mut result);
    }
    assert_eq!(snapshot(&f.state), before);
    assert_eq!(f.payload(), payload);
    assert!(!terminal_snapshot(&before));
    assert_eq!(result.executed_tasks, 0);
    assert_eq!(result.error, INCOMPLETE_WRITES);
    assert_eq!(result.rounds, MAX_ROUNDS);
}

#[test]
fn exhausted_round_budget_with_unclaimed_work_is_not_completion() {
    let s = state();
    s[READY].store(0, Ordering::Relaxed);
    let (mut r, mut o) = (false, observation());
    for _ in 0..MAX_ROUNDS {
        assert_eq!(begin_round_claim(&s, 0, 0, &mut r, &mut o), 0);
        finish_round_completion(&s, 0, true, &mut r, &mut o);
    }
    assert_eq!(o.error, 0);
    assert_eq!(o.empty_probes, MAX_ROUNDS * 256);
    assert!(!r);
    assert!(!terminal_snapshot(&snapshot(&s)));
}

#[test]
fn qkv_and_output_tiles_cover_disjoint_rows_and_keep_exact_dot_operand_order() {
    let mut f = Fixture::new();
    // Distinct finite BF16 patterns at exact row/column coordinates.
    for (i, w) in f.qkv_weight.iter_mut().enumerate() {
        *w = 0x3f00 + (i % 127) as u16;
    }
    for (i, w) in f.output_weight.iter_mut().enumerate() {
        *w = 0x3e00 + (i % 251) as u16;
    }
    f.normalized.fill(0x3f80);
    f.attention.fill(0x4000);
    let qkv = f.qkv_weight.clone();
    let output = f.output_weight.clone();
    let mut qvis = [0u8; QKV_COLUMNS];
    let mut ovis = [0u8; OUTPUT_ELEMENTS];
    for token in (2..=49).chain(67..=130) {
        let (mut w, mut v) = (0, true);
        let storage = f.storage();
        execute_task_at_v6::<false>(&storage, token, 0, &mut w, &mut v, &mut |task| match task {
            WaveQkvAttentionOutputTileTaskV6::Projection(mut t) => {
                for row in 0..64 {
                    let global = t.base + row;
                    qvis[global] += 1;
                    for lane in 0..64 {
                        for step in 0..64 {
                            let k = step * 64 + lane;
                            assert_eq!(t.input(k), Some(0x3f80));
                            assert_eq!(t.weight(row, k), Some(qkv[global * 4096 + k]));
                        }
                    }
                    assert!(t.write_column(row, global as u16));
                }
                assert_eq!(t.weight(64, 0), None);
                assert_eq!(t.weight(0, 4096), None);
            }
            WaveQkvAttentionOutputTileTaskV6::OutputProjection(mut t) => {
                for row in 0..64 {
                    let global = t.base + row;
                    ovis[global] += 1;
                    for lane in 0..64 {
                        for step in 0..32 {
                            let k = step * 64 + lane;
                            assert_eq!(t.input(k), Some(0x4000));
                            assert_eq!(t.weight(row, k), Some(output[global * 2048 + k]));
                        }
                    }
                    assert!(t.write_output(row, f32::from_bits(0x3f00_0000 + global as u32)));
                }
                assert_eq!(t.weight(64, 0), None);
                assert_eq!(t.weight(0, 2048), None);
            }
            _ => panic!("wrong tile"),
        });
        assert!(complete_coverage(token, 0, w, v));
    }
    assert!(qvis.iter().all(|&x| x == 1));
    assert!(ovis.iter().all(|&x| x == 1));
    assert_eq!(
        f.output,
        (0..QKV_COLUMNS).map(|i| i as u16).collect::<Vec<_>>()
    );
    assert_eq!(
        f.partial.iter().map(|x| x.to_bits()).collect::<Vec<_>>(),
        (0..OUTPUT_ELEMENTS)
            .map(|i| 0x3f00_0000 + i as u32)
            .collect::<Vec<_>>()
    );
    assert_eq!(f.qkv_weight, qkv);
    assert_eq!(f.output_weight, output);
    assert!(f.normalized.iter().all(|&x| x == 0x3f80));
    assert!(f.attention.iter().all(|&x| x == 0x4000));
}

#[test]
fn all_16_attention_head_claims_have_disjoint_writes_and_exact_causal_kv_mapping() {
    let mut f = Fixture::new();
    for (i, v) in f.query.iter_mut().enumerate() {
        *v = i as u16;
    }
    for (i, v) in f.key_cache.iter_mut().enumerate() {
        *v = (i % 65521) as u16;
    }
    for (i, v) in f.value_cache.iter_mut().enumerate() {
        *v = (i % 65519) as u16;
    }
    let (query, keys, values, metadata) = (
        f.query.clone(),
        f.key_cache.clone(),
        f.value_cache.clone(),
        f.metadata.clone(),
    );
    let mut visits = [0u8; ATTENTION_ELEMENTS];
    for head in (0..16).rev() {
        for lane in 0..64 {
            let (mut w, mut v) = (0, true);
            execute_task_at_v6::<false>(
                &f.storage(),
                51 + head as u32,
                lane,
                &mut w,
                &mut v,
                &mut |task| {
                    let WaveQkvAttentionOutputTileTaskV6::Attention(mut t) = task else {
                        panic!("head task")
                    };
                    assert_eq!((t.head(), t.position()), (head, 2048));
                    for half in 0..2 {
                        let at = head * 128 + lane + half * 64;
                        assert_eq!(t.query(head, half), Some(query[at]));
                        visits[at] += 1;
                        for token in [0, 15, 16, 2047, 2048] {
                            let page = metadata[1 + token / 16];
                            assert_eq!(t.page(token), Some(page));
                            let index = (page as usize * 16 + token % 16) * 512
                                + (head / 4) * 128
                                + lane
                                + half * 64;
                            assert_eq!(t.key(head, token, page, half), Some(keys[index]));
                            assert_eq!(t.value(head, token, page, half), Some(values[index]));
                        }
                    }
                    assert_eq!(t.page(2049), None);
                    assert_eq!(t.query((head + 1) % 16, 0), None);
                    assert_eq!(t.key((head + 1) % 16, 0, metadata[1], 0), None);
                    assert_eq!(t.value(head, 0, (metadata[1] + 1) % 144, 0), None);
                    assert_eq!(t.query(head, 2), None);
                    assert!(t.write_head(
                        head,
                        (head * 128 + lane) as u16,
                        (head * 128 + lane + 64) as u16
                    ));
                },
            );
            assert!(complete_coverage(51 + head as u32, lane, w, v));
        }
    }
    assert!(visits.iter().all(|&v| v == 1));
    assert_eq!(
        f.attention,
        (0..ATTENTION_ELEMENTS)
            .map(|i| i as u16)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        (f.query, f.key_cache, f.value_cache, f.metadata),
        (query, keys, values, metadata)
    );
}

#[test]
fn bad_metadata_blocks_post_and_each_attention_head_before_any_callback_or_cache_write() {
    let mut f = Fixture::new();
    let original = f.metadata.clone();
    let payload = f.payload();
    for case in 0..5 {
        f.metadata = original.clone();
        match case {
            0 => f.metadata[0] = 2304,
            1 => f.metadata[0] = u32::MAX,
            2 => f.metadata[144] = 144,
            3 => f.metadata[144] = f.metadata[1],
            _ => f.metadata[1] = 0x7fc0_0001,
        }
        for token in 50..=66 {
            for lane in [0, 63] {
                let (mut w, mut v) = (0, true);
                execute_task_at_v6::<false>(&f.storage(), token, lane, &mut w, &mut v, &mut |_| {
                    panic!("invalid metadata callback")
                });
                assert!(!v);
                assert_eq!(w, 0);
                assert!(!complete_coverage(token, lane, w, v));
            }
        }
        assert_eq!(f.payload(), payload);
        assert_eq!(snapshot(&f.state), initial_state_words());
    }
}

#[test]
fn post_appends_only_selected_physical_row_after_full_permutation_validation() {
    let mut f = Fixture::new();
    let raw = f.output.clone();
    let metadata = f.metadata.clone();
    let physical = metadata[1 + 2048 / 16] as usize * 16;
    for lane in 0..64 {
        let (mut w, mut v) = (0, true);
        execute_task_at_v6::<false>(&f.storage(), 50, lane, &mut w, &mut v, &mut |task| {
            let WaveQkvAttentionOutputTileTaskV6::Post(mut t) = task else {
                panic!("post")
            };
            assert_eq!(t.slot, physical);
            for h in 0..16 {
                assert!(t.write_query_head(
                    h,
                    (h * 128 + lane) as u16,
                    (h * 128 + lane + 64) as u16
                ));
            }
            for h in 0..4 {
                assert!(t.write_key_value_head(h, 1, 2, 3, 4));
            }
        });
        assert!(complete_coverage(50, lane, w, v));
    }
    for i in 0..CACHE_ELEMENTS {
        let in_row = (physical * 512..(physical + 1) * 512).contains(&i);
        let half = (i % 128) / 64;
        assert_eq!(
            f.key_cache[i],
            if in_row { 1 + half as u16 } else { 0xa55a }
        );
        assert_eq!(
            f.value_cache[i],
            if in_row { 3 + half as u16 } else { 0xa55a }
        );
    }
    assert_eq!(
        f.query,
        (0..QUERY_ELEMENTS).map(|i| i as u16).collect::<Vec<_>>()
    );
    assert_eq!(f.output, raw);
    assert_eq!(f.metadata, metadata);
}

#[test]
fn head_and_row_cursor_rejection_never_partially_overwrites_a_different_owner() {
    let mut f = Fixture::new();
    let before = f.payload();
    for (token, lane, wrong) in [
        (2, 1, 0),
        (2, 0, 1),
        (49, 0, 64),
        (67, 63, 0),
        (130, 0, 64),
        (51, 0, 1),
        (66, 63, 14),
    ] {
        let (mut w, mut v) = (0, true);
        execute_task_at_v6::<false>(&f.storage(), token, lane, &mut w, &mut v, &mut |task| {
            match task {
                WaveQkvAttentionOutputTileTaskV6::Projection(mut t) => {
                    assert!(!t.write_column(wrong, 7))
                }
                WaveQkvAttentionOutputTileTaskV6::OutputProjection(mut t) => {
                    assert!(!t.write_output(wrong, -0.0))
                }
                WaveQkvAttentionOutputTileTaskV6::Attention(mut t) => {
                    assert!(!t.write_head(wrong, 7, 8))
                }
                _ => panic!("unexpected"),
            }
        });
        assert!(!v);
        assert_eq!(w, 0);
        assert_eq!(f.payload(), before);
    }
    for token in [2, 67, 51] {
        let (mut w, mut v) = (0, true);
        execute_task_at_v6::<false>(
            &f.storage(),
            token,
            0,
            &mut w,
            &mut v,
            &mut |task| match task {
                WaveQkvAttentionOutputTileTaskV6::Projection(mut t) => {
                    assert!(t.write_column(0, 11));
                    assert!(!t.write_column(0, 12));
                }
                WaveQkvAttentionOutputTileTaskV6::OutputProjection(mut t) => {
                    assert!(t.write_output(0, -0.0));
                    assert!(!t.write_output(0, 0.0));
                }
                WaveQkvAttentionOutputTileTaskV6::Attention(mut t) => {
                    assert!(t.write_head(0, 13, 14));
                    assert!(!t.write_head(0, 15, 16));
                }
                _ => panic!("unexpected"),
            },
        );
        assert!(!v);
    }
    assert_eq!(f.output[0], 11);
    assert_eq!(f.partial[0].to_bits(), (-0.0f32).to_bits());
    assert_eq!((f.attention[0], f.attention[64]), (13, 14));
}

#[test]
fn exact_claim_coverage_rejects_incomplete_extra_invalid_and_forged_tokens() {
    for token in 0..=STOP {
        for lane in 0..64 {
            let expected = match token {
                1 => 64,
                50 => 48,
                51..=66 => 2,
                2..=49 | 67..=130 if lane == 0 => 64,
                _ => 0,
            };
            assert!(complete_coverage(token, lane, expected, true));
            assert!(!complete_coverage(token, lane, expected + 1, true));
            if expected != 0 {
                assert!(!complete_coverage(token, lane, expected - 1, true));
            }
            assert!(!complete_coverage(token, lane, expected, false));
        }
    }
    assert!(!complete_coverage(STOP + 1, 0, 0, true));
    assert!(!complete_coverage(0, 64, 0, true));
    let mut f = Fixture::new();
    for token in [0, STOP, STOP + 1, u32::MAX] {
        let (mut w, mut v) = (0, true);
        execute_task_at_v6::<false>(&f.storage(), token, 0, &mut w, &mut v, &mut |_| {
            panic!("non-task callback")
        });
        assert_eq!(w, 0);
    }
}

#[test]
fn all_fifteen_raw_root_ranges_require_alignment_nonzero_extent_and_pairwise_disjointness() {
    let lengths = [
        8192,
        8192,
        25165824,
        512,
        512,
        580,
        16777216,
        8192,
        6144,
        4096,
        4718592 / 2,
        4718592 / 2,
        4096,
        16384,
        1136,
    ];
    let mut cursor = 0x1000usize;
    let good: [(usize, usize, usize); 15] = core::array::from_fn(|i| {
        let alignment = if matches!(i, 4 | 5 | 13 | 14) { 4 } else { 2 };
        let base = cursor;
        cursor += ((lengths[i] + 4095) / 4096) * 4096;
        (base, lengths[i], alignment)
    });
    assert!(disjoint_regions(good));
    for i in 0..15 {
        for case in 0..4 {
            let mut bad = good;
            match case {
                0 => bad[i].0 = 0,
                1 => bad[i].0 += 1,
                2 => bad[i].1 = 0,
                _ => bad[i].0 = usize::MAX - (good[i].2 - 1),
            }
            assert!(!disjoint_regions(bad));
        }
        for j in 0..i {
            let mut bad = good;
            bad[i].0 = bad[j].0;
            assert!(!disjoint_regions(bad));
        }
    }
    let mut f = Fixture::new();
    let storage = f.storage();
    // Constructor overlap rejection occurs before any pointer dereference.
    let bad = unsafe {
        WaveQkvAttentionOutputTileStorageV6::from_raw_parts(
            storage.input,
            storage.norm_weight,
            storage.qkv_weight,
            storage.head_norm_weight,
            storage.rotary,
            storage.cache_metadata,
            storage.output_weight,
            storage.normalized,
            storage.qkv_output,
            storage.query,
            storage.key_cache,
            storage.value_cache,
            storage.attention_output,
            storage.attention_output.cast(),
            storage.state,
        )
    };
    assert!(matches!(bad, Err(INVALID)));
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

    fn storage(&mut self) -> WaveQkvAttentionOutputTileStorageV6<'_> {
        // SAFETY: distinct initialized allocations live under this exclusive
        // fixture borrow; CPU tests issue views sequentially, without GPU calls.
        unsafe {
            WaveQkvAttentionOutputTileStorageV6::from_raw_parts(
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

impl Fixture {
    fn payload(&self) -> (Vec<Vec<u16>>, Vec<u32>) {
        (
            vec![
                self.normalized.clone(),
                self.output.clone(),
                self.query.clone(),
                self.key_cache.clone(),
                self.value_cache.clone(),
                self.attention.clone(),
            ],
            self.partial.iter().map(|v| v.to_bits()).collect(),
        )
    }
}

#[test]
fn observed_other_worker_error_retires_before_any_new_claim() {
    let s = state();
    s[ERRORS].store(INVALID, Ordering::Release);
    let before = snapshot(&s);
    let (mut retired, mut o) = (false, observation());
    assert_eq!(begin_round_claim(&s, 0, 17, &mut retired, &mut o), STOP);
    assert!(retired);
    assert_eq!(o.error, INVALID);
    assert_eq!(o.executed_tasks, 0);
    assert_eq!(snapshot(&s), before);
}

#[test]
fn invalid_admission_never_calls_a_view_or_mutates_any_payload() {
    let mut f = Fixture::new();
    let before = f.payload();
    let words = snapshot(&f.state);
    for token in 0..=STOP {
        for lane in [0, 63] {
            let (mut w, mut v) = (0, false);
            execute_task_at_v6::<false>(&f.storage(), token, lane, &mut w, &mut v, &mut |_| {
                panic!("invalid admitted callback")
            });
            assert_eq!(w, 0);
            assert!(!v);
        }
    }
    assert_eq!(f.payload(), before);
    assert_eq!(snapshot(&f.state), words);
}
