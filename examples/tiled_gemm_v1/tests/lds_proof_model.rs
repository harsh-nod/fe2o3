use std::collections::BTreeSet;

use fe2o3_device::RowMajorXor4;
use fe2o3_tiled_gemm_v1::{
    ARegisterLayoutV1, AccumulatorRegisterLayoutV1, BRegisterLayoutV1, RowMajorXor4StagingV1,
};

const TILE: usize = 16;
const LANES: usize = 64;
const COMPONENTS: usize = 4;
const EPOCH: u32 = 7;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct Initialized<T> {
    epoch: u32,
    value: T,
}

fn read_in_epoch<T: Copy>(slot: Option<Initialized<T>>, epoch: u32) -> Option<T> {
    slot.filter(|initialized| initialized.epoch == epoch)
        .map(|initialized| initialized.value)
}

#[test]
fn exhaustive_slice1_event_model_stages_and_multiplies_the_fixed_tile() {
    let a = std::array::from_fn::<_, 256, _>(|index| (index as i64 % 19) - 9);
    let b = std::array::from_fn::<_, 256, _>(|index| (index as i64 % 23) - 11);
    let mut a_lds = [None; 256];
    let mut b_lds = [None; 256];
    let mut a_writes = BTreeSet::new();
    let mut b_writes = BTreeSet::new();
    let mut writes_per_lane = [0_usize; LANES];

    for (lane, writes_for_lane) in writes_per_lane.iter_mut().enumerate() {
        for component in 0..COMPONENTS {
            let a_register = ARegisterLayoutV1::coordinate(lane, component).unwrap();
            let b_register = BRegisterLayoutV1::coordinate(lane, component).unwrap();
            let a_stage = RowMajorXor4StagingV1::a_coordinate(lane, component).unwrap();
            let b_stage = RowMajorXor4StagingV1::b_transposed_coordinate(lane, component).unwrap();

            assert!(a_writes.insert(a_stage.physical_index));
            assert!(b_writes.insert(b_stage.physical_index));
            a_lds[a_stage.physical_index] = Some(Initialized {
                epoch: EPOCH,
                value: a[a_register.row * TILE + a_register.depth],
            });
            b_lds[b_stage.physical_index] = Some(Initialized {
                epoch: EPOCH,
                value: b[b_register.depth * TILE + b_register.column],
            });
            *writes_for_lane += 2;
        }
    }

    assert_eq!(a_writes, (0..256).collect());
    assert_eq!(b_writes, (0..256).collect());
    assert_eq!(writes_per_lane, [8; LANES]);
    let arrived = writes_per_lane.map(|writes| writes == 8);
    assert!(arrived.into_iter().all(|lane_arrived| lane_arrived));

    let mut staged_product = [0_i64; 256];
    let mut global_product = [0_i64; 256];
    for row in 0..TILE {
        for column in 0..TILE {
            let mut staged = 0_i64;
            let mut global = 0_i64;
            for depth in 0..TILE {
                let a_physical = RowMajorXor4::physical_index(row, depth).unwrap();
                let b_physical = RowMajorXor4::physical_index(column, depth).unwrap();
                let a_value = read_in_epoch(a_lds[a_physical], EPOCH).unwrap();
                let b_value = read_in_epoch(b_lds[b_physical], EPOCH).unwrap();
                assert_eq!(read_in_epoch(a_lds[a_physical], EPOCH + 1), None);
                assert_eq!(read_in_epoch(b_lds[b_physical], EPOCH + 1), None);
                assert_eq!(a_value, a[row * TILE + depth]);
                assert_eq!(b_value, b[depth * TILE + column]);
                staged += a_value * b_value;
                global += a[row * TILE + depth] * b[depth * TILE + column];
            }
            staged_product[row * TILE + column] = staged;
            global_product[row * TILE + column] = global;
        }
    }
    assert_eq!(staged_product, global_product);

    let mut c_stores = BTreeSet::new();
    for lane in 0..LANES {
        for component in 0..COMPONENTS {
            let output = AccumulatorRegisterLayoutV1::coordinate(lane, component).unwrap();
            assert!(c_stores.insert(output.row * TILE + output.column));
        }
    }
    assert_eq!(c_stores, (0..256).collect());
}
