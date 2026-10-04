use super::super::super::{PendingDispatch, raw_timestamps};
use super::*;
use std::time::Instant;

fn terminal() -> [u32; 284] {
    let mut words = [0; 284];
    words[0] = 1;
    words[3] = 31;
    words[4..9].copy_from_slice(&[1, 48, 1, 16, 64]);
    words[9..14].copy_from_slice(&[1, 48, 1, 16, 64]);
    words[14..18].fill(u32::MAX);
    words[18] = 3;
    words[19..23].fill(u32::MAX);
    words[23] = 3;
    words[24..154].fill(1);
    words[154..284].fill(64);
    words
}

fn round() -> Gfx950EngineeringPeerWaveQkvAttentionOutputTilesRoundV6 {
    Gfx950EngineeringPeerWaveQkvAttentionOutputTilesRoundV6 {
        final_states: [terminal(); 2],
        dispatch_elapsed_ns: [20, 30],
    }
}

fn observation(
    group: u64,
    rank: usize,
    uid: u64,
    host: u64,
) -> Gfx950EngineeringRawTimestampObservationV1 {
    let now = Instant::now();
    let mut pending = PendingDispatch {
        unique_id: uid,
        queue_epoch: 0,
        next: 1,
        started: now,
        deadline: now,
        next_currentness: now,
        wait_started: None,
        profiled: false,
        completed: true,
        raw_timestamps: Some([11, 19]),
    };
    raw_timestamps::completed_observation(group, rank, [uid, 0, 1], &mut pending, host).unwrap()
}

fn pair() -> Vec<Gfx950EngineeringRawTimestampObservationV1> {
    vec![observation(1, 0, 101, 20), observation(1, 1, 102, 30)]
}

#[test]
fn timestamp_prefix_join_preserves_real_state_host_and_raw_observations() {
    let (result, raw) = finish_timestamp_round(round(), pair()).unwrap();
    assert_eq!(result.final_states, [terminal(); 2]);
    assert_eq!(result.dispatch_elapsed_ns, [20, 30]);
    assert_eq!(raw.map(|x| (x.start_tick(), x.end_tick())), [(11, 19); 2]);
}

#[test]
fn timestamp_prefix_join_rejects_missing_or_extra_observations() {
    for raw in [
        Vec::new(),
        vec![pair()[0]],
        vec![pair()[0], pair()[1], pair()[0]],
    ] {
        assert!(finish_timestamp_round(round(), raw).is_err());
    }
}

#[test]
fn timestamp_prefix_join_rejects_wrong_or_duplicate_rank_order() {
    for raw in [
        vec![pair()[1], pair()[0]],
        vec![pair()[0], pair()[0]],
        vec![observation(1, 2, 101, 20), pair()[1]],
    ] {
        assert!(finish_timestamp_round(round(), raw).is_err());
    }
}

#[test]
fn timestamp_prefix_join_rejects_host_timer_substitution() {
    for rank in 0..2 {
        let mut result = round();
        result.dispatch_elapsed_ns[rank] += 1;
        assert!(finish_timestamp_round(result, pair()).is_err());
    }
}

#[test]
fn timestamp_prefix_join_rejects_foreign_group_or_duplicate_device() {
    for raw in [
        vec![pair()[0], observation(2, 1, 102, 30)],
        vec![pair()[0], observation(1, 1, 101, 30)],
    ] {
        assert!(finish_timestamp_round(round(), raw).is_err());
    }
}

#[test]
fn timestamp_prefix_join_checks_every_terminal_word_on_both_ranks() {
    for rank in 0..2 {
        for index in 0..284 {
            let mut result = round();
            result.final_states[rank][index] ^= u32::MAX;
            assert!(
                finish_timestamp_round(result, pair()).is_err(),
                "{rank}:{index}"
            );
        }
    }
}

#[test]
fn timestamp_prefix_join_failure_enters_existing_group_poison_path() {
    let mut group = Gfx950EngineeringPeerGroupV1 {
        incarnation: 1,
        contexts: Vec::new(),
        buffers: BTreeMap::new(),
        next_buffer: 1,
        poisoned: false,
        closed: false,
        shared_full_currentness: false,
    };
    assert!(
        group
            .finish(finish_timestamp_round(round(), Vec::new()))
            .is_err()
    );
    assert!(group.poisoned);
    assert_eq!(
        group.require_active().unwrap_err(),
        "peer group is closed or quarantined"
    );
    assert_eq!(
        group.close().unwrap_err(),
        "peer group is closed or quarantined"
    );
}
