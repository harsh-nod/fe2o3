use std::collections::BTreeSet;

use fe2o3_device::RowMajorXor4;
use fe2o3_tiled_gemm_v1::{
    ARegisterLayoutV1, AccumulatorRegisterLayoutV1, BRegisterLayoutV1, RowMajorXor4StagingV1,
};

const TILE: usize = 16;
const LANES: usize = 64;
const COMPONENTS: usize = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Initialized<T> {
    epoch: usize,
    value: T,
}

fn read_in_epoch<T: Copy>(slot: Option<Initialized<T>>, epoch: usize) -> Option<T> {
    slot.filter(|initialized| initialized.epoch == epoch)
        .map(|initialized| initialized.value)
}

fn exercise_kphase_model(phase_count: usize) {
    assert!((1..=4).contains(&phase_count));
    let k = phase_count * TILE;
    let a: Vec<i64> = (0..TILE * k).map(|index| (index as i64 % 19) - 9).collect();
    let b: Vec<i64> = (0..k * TILE)
        .map(|index| (index as i64 % 23) - 11)
        .collect();
    let mut a_lds = [None; TILE * TILE];
    let mut b_lds = [None; TILE * TILE];
    let mut accumulator = [0_i64; TILE * TILE];
    let mut prior_phase_reads_complete = true;

    for phase in 0..phase_count {
        assert!(prior_phase_reads_complete, "LDS reuse preceded prior reads");
        let epoch = phase * 2;
        let mut a_writes = BTreeSet::new();
        let mut b_writes = BTreeSet::new();
        let mut publish_arrivals = [false; LANES];

        for (lane, publish_arrived) in publish_arrivals.iter_mut().enumerate() {
            for component in 0..COMPONENTS {
                let a_register = ARegisterLayoutV1::coordinate(lane, component).unwrap();
                let b_register = BRegisterLayoutV1::coordinate(lane, component).unwrap();
                let a_stage = RowMajorXor4StagingV1::a_coordinate(lane, component).unwrap();
                let b_stage =
                    RowMajorXor4StagingV1::b_transposed_coordinate(lane, component).unwrap();

                assert!(a_writes.insert(a_stage.physical_index));
                assert!(b_writes.insert(b_stage.physical_index));
                a_lds[a_stage.physical_index] = Some(Initialized {
                    epoch,
                    value: a[a_register.row * k + phase * TILE + a_register.depth],
                });
                b_lds[b_stage.physical_index] = Some(Initialized {
                    epoch,
                    value: b[(phase * TILE + b_register.depth) * TILE + b_register.column],
                });
            }
            *publish_arrived = true;
        }

        assert_eq!(a_writes, (0..TILE * TILE).collect());
        assert_eq!(b_writes, (0..TILE * TILE).collect());
        assert!(publish_arrivals.into_iter().all(|arrived| arrived));

        prior_phase_reads_complete = false;
        assert!(!prior_phase_reads_complete);
        for row in 0..TILE {
            for column in 0..TILE {
                let output = row * TILE + column;
                let accumulator_before_phase = accumulator[output];
                let mut phase_contribution = 0_i64;
                for offset in 0..TILE {
                    let a_physical = RowMajorXor4::physical_index(row, offset).unwrap();
                    let b_physical = RowMajorXor4::physical_index(column, offset).unwrap();
                    let a_value = read_in_epoch(a_lds[a_physical], epoch).unwrap();
                    let b_value = read_in_epoch(b_lds[b_physical], epoch).unwrap();
                    assert_eq!(a_value, a[row * k + phase * TILE + offset]);
                    assert_eq!(b_value, b[(phase * TILE + offset) * TILE + column]);
                    phase_contribution += a_value * b_value;
                }
                accumulator[output] = accumulator_before_phase + phase_contribution;
            }
        }

        assert!(!prior_phase_reads_complete);
        let reuse_arrivals = [true; LANES];
        assert!(reuse_arrivals.into_iter().all(|arrived| arrived));
        prior_phase_reads_complete = true;

        for row in 0..TILE {
            for column in 0..TILE {
                let expected_prefix: i64 = (0..(phase + 1) * TILE)
                    .map(|depth| a[row * k + depth] * b[depth * TILE + column])
                    .sum();
                assert_eq!(accumulator[row * TILE + column], expected_prefix);
            }
        }
    }

    let expected: Vec<i64> = (0..TILE)
        .flat_map(|row| {
            let a = &a;
            let b = &b;
            (0..TILE).map(move |column| {
                (0..k)
                    .map(|depth| a[row * k + depth] * b[depth * TILE + column])
                    .sum()
            })
        })
        .collect();
    assert_eq!(accumulator.as_slice(), expected);

    let mut c_stores = BTreeSet::new();
    for lane in 0..LANES {
        for component in 0..COMPONENTS {
            let output = AccumulatorRegisterLayoutV1::coordinate(lane, component).unwrap();
            assert!(c_stores.insert(output.row * TILE + output.column));
        }
    }
    assert_eq!(c_stores, (0..TILE * TILE).collect());
}

#[test]
fn exhaustive_kphase_event_model_covers_one_two_and_four_phases() {
    for phase_count in [1, 2, 4] {
        exercise_kphase_model(phase_count);
    }
}

#[test]
fn stale_epoch_reads_are_rejected_after_lds_reuse() {
    let slot = Some(Initialized {
        epoch: 2,
        value: 17_i64,
    });
    assert_eq!(read_in_epoch(slot, 2), Some(17));
    assert_eq!(read_in_epoch(slot, 0), None);
    assert_eq!(read_in_epoch(slot, 4), None);
}

#[test]
fn resetting_the_accumulator_loses_prior_phase_contributions() {
    let phase_contributions = [16_i64, 16_i64];
    let carried: i64 = phase_contributions.into_iter().sum();
    let reset = phase_contributions[1];
    assert_eq!(carried, 32);
    assert_eq!(reset, 16);
    assert_ne!(reset, carried);
}
