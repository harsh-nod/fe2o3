//! These tests use actual storage/view/arbitration/arrival functions. They do
//! not execute host-unavailable subgroup/LDS terminals or claim a GPU proof.
use super::*;
use std::{vec, vec::Vec};

struct Fixture {
    input: Vec<u16>,
    norm_weight: Vec<u16>,
    key_weight: Vec<u16>,
    normalized: Vec<u16>,
    key_output: Vec<u16>,
    state: [AtomicU32; 9],
}

impl Fixture {
    fn new() -> Self {
        Self {
            input: vec![0x3f80; NORM_ELEMENTS],
            norm_weight: vec![0x3f80; NORM_ELEMENTS],
            key_weight: vec![0; KEY_ELEMENTS],
            normalized: vec![0xa55a; NORM_ELEMENTS],
            key_output: vec![0xa55a; KEY_COLUMNS],
            state: core::array::from_fn(|index| AtomicU32::new(u32::from(index < 2))),
        }
    }

    fn storage(&mut self) -> WaveTaskStorageV1<'_> {
        // SAFETY: exact independent Vec allocations and initialized state are
        // borrowed exclusively for this sequential CPU protocol test. No device
        // constructors or workgroup/subgroup terminals are called by these tests.
        unsafe {
            WaveTaskStorageV1::from_raw_parts(
                self.input.as_ptr().cast(),
                self.norm_weight.as_ptr().cast(),
                self.key_weight.as_ptr().cast(),
                self.normalized.as_mut_ptr().cast(),
                self.key_output.as_mut_ptr().cast(),
                &self.state,
            )
            .unwrap()
        }
    }
}

fn write_norm(storage: &WaveTaskStorageV1<'_>) {
    assert_eq!(claim_fanout(storage.state(), 0), Claim::Task(0));
    for lane in 0..64 {
        assert!(lane_admitted(storage.state(), 1, 0));
        let mut written = 0;
        let mut valid = true;
        {
            let mut task = NormTaskV1 {
                storage,
                lane,
                written: &mut written,
                valid: &mut valid,
            };
            for component in 0..64 {
                assert!(task.write_component(component, (lane + component * 64) as u16));
            }
        }
        assert!(complete_coverage(1, lane, written, valid));
        complete_lane(storage.state(), 0).unwrap();
        if lane != 63 {
            assert_eq!(storage.state()[DONE].load(Ordering::Acquire), 0);
            assert_eq!(storage.state()[READY].load(Ordering::Acquire), 0);
        }
    }
}

#[test]
fn wave_task_actual_three_claims_cover_disjoint_payloads() {
    let mut fixture = Fixture::new();
    {
        let storage = fixture.storage();
        write_norm(&storage);
        assert_eq!(storage.state()[READY].load(Ordering::Acquire), 6);
        for task in 1..3 {
            assert_eq!(claim_fanout(storage.state(), 1), Claim::Task(task));
            for lane in 0..64 {
                assert!(lane_admitted(storage.state(), task + 1, 1));
                let mut written = 0;
                let mut valid = true;
                {
                    let mut view = KeyTaskV1 {
                        storage: &storage,
                        lane,
                        base: (task as usize - 1) * 256,
                        written: &mut written,
                        valid: &mut valid,
                    };
                    for component in 0..64 {
                        let index = lane + component * 64;
                        assert_eq!(view.input(index), Some(index as u16));
                    }
                    if lane == 0 {
                        for column in 0..256 {
                            assert!(
                                view.write_column(
                                    column,
                                    ((task - 1) * 256 + column as u32) as u16
                                )
                            );
                        }
                    }
                }
                assert!(complete_coverage(task + 1, lane, written, valid));
                complete_lane(storage.state(), task).unwrap();
            }
        }
        assert_eq!(storage.state()[DONE].load(Ordering::Acquire), 7);
        assert_eq!(storage.state()[CLAIMED].load(Ordering::Acquire), 7);
        assert_eq!(storage.state()[ERRORS].load(Ordering::Acquire), 0);
        assert_eq!(claim_fanout(storage.state(), 0), Claim::Empty);
        assert_eq!(
            (0..3)
                .map(|i| storage.state()[ARRIVALS + i].load(Ordering::Acquire))
                .collect::<Vec<_>>(),
            vec![64; 3]
        );
    }
    assert_eq!(
        fixture.normalized,
        (0..4096).map(|i| i as u16).collect::<Vec<_>>()
    );
    assert_eq!(
        fixture.key_output,
        (0..512).map(|i| i as u16).collect::<Vec<_>>()
    );
}

#[test]
fn wave_task_array_places_read_exact_root_boundaries() {
    let mut fixture = Fixture::new();
    fixture.input[0] = 0x1201;
    fixture.input[NORM_ELEMENTS - 1] = 0x1202;
    fixture.norm_weight[0] = 0x2301;
    fixture.norm_weight[NORM_ELEMENTS - 1] = 0x2302;
    let half = KEY_CHUNK * NORM_ELEMENTS;
    for (index, value) in [
        (0, 0x3401),
        (half - 1, 0x3402),
        (half, 0x3403),
        (KEY_ELEMENTS - 1, 0x3404),
    ] {
        fixture.key_weight[index] = value;
    }
    let storage = fixture.storage();
    let (mut written, mut valid) = (0, true);
    {
        let view = NormTaskV1 {
            storage: &storage,
            lane: 0,
            written: &mut written,
            valid: &mut valid,
        };
        assert_eq!(view.input(0), Some(0x1201));
        assert_eq!(view.input(NORM_ELEMENTS - 1), Some(0x1202));
        assert_eq!(view.weight(0), Some(0x2301));
        assert_eq!(view.weight(NORM_ELEMENTS - 1), Some(0x2302));
    }
    write_norm(&storage);
    for (base, first, last) in [(0, 0x3401, 0x3402), (KEY_CHUNK, 0x3403, 0x3404)] {
        let view = KeyTaskV1 {
            storage: &storage,
            lane: 0,
            base,
            written: &mut written,
            valid: &mut valid,
        };
        assert_eq!(view.input(0), Some(0));
        assert_eq!(
            view.input(NORM_ELEMENTS - 1),
            Some((NORM_ELEMENTS - 1) as u16)
        );
        assert_eq!(view.weight(0, 0), Some(first));
        assert_eq!(view.weight(KEY_CHUNK - 1, NORM_ELEMENTS - 1), Some(last));
    }
}

#[test]
fn wave_task_array_places_reject_out_of_range_reads() {
    let mut fixture = Fixture::new();
    let storage = fixture.storage();
    let (mut written, mut valid) = (0, true);
    {
        let view = NormTaskV1 {
            storage: &storage,
            lane: 0,
            written: &mut written,
            valid: &mut valid,
        };
        for index in [NORM_ELEMENTS, usize::MAX] {
            assert_eq!(view.input(index), None);
            assert_eq!(view.weight(index), None);
        }
    }
    for base in [0, KEY_CHUNK] {
        let view = KeyTaskV1 {
            storage: &storage,
            lane: 0,
            base,
            written: &mut written,
            valid: &mut valid,
        };
        for index in [NORM_ELEMENTS, usize::MAX] {
            assert_eq!(view.input(index), None);
            assert_eq!(view.weight(0, index), None);
        }
        for column in [KEY_CHUNK, usize::MAX] {
            assert_eq!(view.weight(column, 0), None);
        }
    }
    valid = false;
    let view = KeyTaskV1 {
        storage: &storage,
        lane: 0,
        base: 0,
        written: &mut written,
        valid: &mut valid,
    };
    assert_eq!(view.input(0), None);
}

#[test]
fn wave_task_array_places_write_last_elements_without_neighbor_changes() {
    let mut fixture = Fixture::new();
    {
        let storage = fixture.storage();
        let (mut written, mut valid) = (WAVE_LANES - 1, true);
        {
            let mut view = NormTaskV1 {
                storage: &storage,
                lane: WAVE_LANES - 1,
                written: &mut written,
                valid: &mut valid,
            };
            assert!(view.write_component(WAVE_LANES - 1, 0x4567));
            assert!(!view.write_component(usize::MAX, 0xabcd));
        }
        assert_eq!(written, WAVE_LANES);
        assert!(!valid);
        for base in [0, KEY_CHUNK] {
            let (mut written, mut valid) = (KEY_CHUNK - 1, true);
            {
                let mut view = KeyTaskV1 {
                    storage: &storage,
                    lane: 0,
                    base,
                    written: &mut written,
                    valid: &mut valid,
                };
                assert!(view.write_column(KEY_CHUNK - 1, 0x5678));
                assert!(!view.write_column(usize::MAX, 0xabcd));
            }
            assert_eq!(written, KEY_CHUNK);
            assert!(!valid);
        }
    }
    assert!(
        fixture.normalized[..NORM_ELEMENTS - 1]
            .iter()
            .all(|&v| v == 0xa55a)
    );
    assert_eq!(fixture.normalized[NORM_ELEMENTS - 1], 0x4567);
    for (index, value) in fixture.key_output.iter().enumerate() {
        assert_eq!(
            *value,
            if index == KEY_CHUNK - 1 || index == KEY_COLUMNS - 1 {
                0x5678
            } else {
                0xa55a
            }
        );
    }
}

#[test]
fn wave_task_missing_arrival_cannot_release_successors() {
    let mut fixture = Fixture::new();
    let storage = fixture.storage();
    assert_eq!(claim_fanout(storage.state(), 0), Claim::Task(0));
    for _ in 0..63 {
        complete_lane(storage.state(), 0).unwrap();
    }
    assert_eq!(storage.state()[READY].load(Ordering::Acquire), 0);
    assert_eq!(storage.state()[DONE].load(Ordering::Acquire), 0);
    assert!(!lane_admitted(storage.state(), 2, 0));
    complete_lane(storage.state(), 0).unwrap();
    assert_eq!(storage.state()[READY].load(Ordering::Acquire), 6);
    assert_eq!(complete_lane(storage.state(), 0), Err(DUPLICATE));
    assert_eq!(storage.state()[ERRORS].load(Ordering::Acquire), DUPLICATE);
}

#[test]
fn wave_task_lane_and_component_misuse_do_not_write() {
    let mut fixture = Fixture::new();
    {
        let storage = fixture.storage();
        assert_eq!(claim_fanout(storage.state(), 0), Claim::Task(0));
        for (lane, component) in [(64, 0), (0, 1), (0, 64)] {
            let (mut written, mut valid) = (0, true);
            let mut view = NormTaskV1 {
                storage: &storage,
                lane,
                written: &mut written,
                valid: &mut valid,
            };
            assert!(!view.write_component(component, 17));
            assert_eq!(written, 0);
            assert!(!valid);
        }
        let (mut written, mut valid) = (0, true);
        let mut view = NormTaskV1 {
            storage: &storage,
            lane: 0,
            written: &mut written,
            valid: &mut valid,
        };
        assert!(view.write_component(0, 99));
        assert!(!view.write_component(0, 100));
    }
    assert_eq!(fixture.normalized[0], 99);
    assert!(fixture.normalized[1..].iter().all(|&x| x == 0xa55a));
}

#[test]
fn wave_task_only_key_lane_zero_can_write_its_partition() {
    let mut fixture = Fixture::new();
    {
        let storage = fixture.storage();
        write_norm(&storage);
        assert_eq!(claim_fanout(storage.state(), 1), Claim::Task(1));
        for lane in 1..64 {
            let (mut written, mut valid) = (0, true);
            let mut view = KeyTaskV1 {
                storage: &storage,
                lane,
                base: 0,
                written: &mut written,
                valid: &mut valid,
            };
            assert!(!view.write_column(0, 123));
            assert!(!valid);
        }
        let (mut written, mut valid) = (0, true);
        let mut view = KeyTaskV1 {
            storage: &storage,
            lane: 0,
            base: 0,
            written: &mut written,
            valid: &mut valid,
        };
        assert!(view.write_column(0, 12));
        assert!(!view.write_column(256, 13));
        assert_eq!(view.input(4096), None);
        assert_eq!(view.weight(256, 0), None);
    }
    assert_eq!(fixture.key_output[0], 12);
    assert!(fixture.key_output[1..].iter().all(|&x| x == 0xa55a));
}

#[test]
fn wave_task_coverage_rejects_early_finish_and_invalid_rounds() {
    for lane in 0..64 {
        assert!(complete_coverage(0, lane, 0, true));
        assert!(complete_coverage(1, lane, 64, true));
        assert!(!complete_coverage(1, lane, 63, true));
        assert!(!complete_coverage(1, lane, 65, true));
        for token in 2..4 {
            assert!(complete_coverage(
                token,
                lane,
                if lane == 0 { 256 } else { 0 },
                true
            ));
            assert!(!complete_coverage(
                token,
                lane,
                if lane == 0 { 255 } else { 1 },
                true
            ));
        }
        assert!(!complete_coverage(0, lane, 0, false));
        assert!(!complete_coverage(4, lane, 0, true));
    }
    assert!(!complete_coverage(0, 64, 0, true));
}

#[test]
fn wave_task_wrong_epoch_and_unpublished_key_are_rejected() {
    let mut fixture = Fixture::new();
    fixture.state[EPOCH].store(2, Ordering::Relaxed);
    assert_eq!(
        claim_fanout(&fixture.state, 0),
        Claim::Rejected(super::super::STALE_EPOCH)
    );
    assert_eq!(fixture.state[CLAIMED].load(Ordering::Relaxed), 0);
    fixture = Fixture::new();
    fixture.state[READY].store(2, Ordering::Relaxed);
    assert_eq!(
        claim_fanout(&fixture.state, 0),
        Claim::Rejected(MISSING_PREDECESSOR)
    );
    assert!(!lane_admitted(&fixture.state, 2, 0));
    assert!(!lane_admitted(&fixture.state, 4, 0));
    assert!(!lane_admitted(&fixture.state, 1, 2));
}

#[test]
fn wave_task_six_root_byte_ranges_are_checked_exactly() {
    let mut ranges = [
        (0x1000, 0x100, 2),
        (0x1100, 0x100, 2),
        (0x1200, 0x100, 2),
        (0x1300, 0x100, 2),
        (0x1400, 0x100, 2),
        (0x1500, 36, 4),
    ];
    assert!(disjoint_regions(ranges));
    for index in 0..6 {
        let original = ranges[index];
        ranges[index].0 = 0;
        assert!(!disjoint_regions(ranges));
        ranges[index] = original;
        ranges[index].0 += 1;
        assert!(!disjoint_regions(ranges));
        ranges[index] = original;
        ranges[index].1 = usize::MAX;
        assert!(!disjoint_regions(ranges));
        ranges[index] = original;
    }
    for left in 0..6 {
        for right in left + 1..6 {
            let original = ranges[right];
            ranges[right].0 = ranges[left].0;
            assert!(!disjoint_regions(ranges));
            ranges[right] = original;
        }
    }
}

#[test]
fn wave_task_claims_reuse_existing_priority_and_at_most_once_guard() {
    let mut fixture = Fixture::new();
    assert_eq!(claim_fanout(&fixture.state, 1), Claim::Task(0));
    assert_eq!(claim_fanout(&fixture.state, 0), Claim::Empty);
    fixture.state[READY].store(1, Ordering::Relaxed);
    assert_eq!(claim_fanout(&fixture.state, 0), Claim::Rejected(DUPLICATE));
    fixture = Fixture::new();
    fixture.state[READY].store(8, Ordering::Relaxed);
    assert_eq!(claim_fanout(&fixture.state, 0), Claim::Rejected(INVALID));
    assert_eq!((MAX_ROUNDS, MAX_EMPTY_PROBES), (8, 4));
}

#[test]
fn wave_task_solo_owner_finishes_after_peer_empty_retirement() {
    // The peer exhausts its empty budget while Norm is paused. Both physical
    // worker choices must still finish the entire DAG without peer residency.
    for owner in 0..2 {
        let fixture = Fixture::new();
        assert_eq!(claim_fanout(&fixture.state, owner), Claim::Task(0));
        let mut owner_rounds = 1;
        for _ in 0..MAX_EMPTY_PROBES {
            assert_eq!(claim_fanout(&fixture.state, 1 - owner), Claim::Empty);
        }
        // Model the next-round leader racing ahead of its final local arrival.
        for _ in 0..63 {
            complete_lane(&fixture.state, 0).unwrap();
        }
        assert_eq!(claim_fanout(&fixture.state, owner), Claim::Empty);
        owner_rounds += 1;
        complete_lane(&fixture.state, 0).unwrap();
        for task in 1..3 {
            assert_eq!(claim_fanout(&fixture.state, owner), Claim::Task(task));
            owner_rounds += 1;
            for _ in 0..64 {
                complete_lane(&fixture.state, task).unwrap();
            }
        }
        assert!(owner_rounds <= MAX_ROUNDS);
        assert_eq!(fixture.state[DONE].load(Ordering::Acquire), 7);
        assert_eq!(fixture.state[ERRORS].load(Ordering::Acquire), 0);
    }
}

#[test]
fn wave_task_key_owner_may_retire_while_other_key_is_paused() {
    for last_owner in 0..2 {
        let fixture = Fixture::new();
        assert_eq!(claim_fanout(&fixture.state, 0), Claim::Task(0));
        for _ in 0..64 {
            complete_lane(&fixture.state, 0).unwrap();
        }
        assert_eq!(claim_fanout(&fixture.state, 0), Claim::Task(1));
        assert_eq!(claim_fanout(&fixture.state, 1), Claim::Task(2));
        let first_task = 2 - last_owner;
        let last_task = 1 + last_owner;
        for _ in 0..64 {
            complete_lane(&fixture.state, first_task).unwrap();
        }
        for _ in 0..MAX_EMPTY_PROBES {
            assert_eq!(claim_fanout(&fixture.state, 1 - last_owner), Claim::Empty);
        }
        assert_ne!(fixture.state[DONE].load(Ordering::Acquire), 7);
        for _ in 0..64 {
            complete_lane(&fixture.state, last_task).unwrap();
        }
        assert_eq!(fixture.state[DONE].load(Ordering::Acquire), 7);
        assert_eq!(fixture.state[ERRORS].load(Ordering::Acquire), 0);
    }
}

#[test]
fn wave_task_eligible_contender_after_ready_removal_gets_no_claim() {
    let fixture = Fixture::new();
    assert_eq!(claim_fanout(&fixture.state, 0), Claim::Task(0));
    // Norm cannot be claimed again while its payload/arrival work is pending.
    for _ in 0..MAX_EMPTY_PROBES {
        assert_eq!(claim_fanout(&fixture.state, 1), Claim::Empty);
    }
    for _ in 0..64 {
        complete_lane(&fixture.state, 0).unwrap();
    }
    assert_eq!(claim_fanout(&fixture.state, 0), Claim::Task(1));
    assert_eq!(claim_fanout(&fixture.state, 1), Claim::Task(2));
    assert_eq!(claim_fanout(&fixture.state, 0), Claim::Empty);
    assert_eq!(claim_fanout(&fixture.state, 1), Claim::Empty);
    assert_eq!(fixture.state[CLAIMED].load(Ordering::Acquire), 7);
}
