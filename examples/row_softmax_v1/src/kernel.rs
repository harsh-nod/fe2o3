#![allow(non_upper_case_globals)]

use fe2o3_device::{DeviceMath, DisjointSlice, GridExclusive, kernel, thread};

const ROW_ELEMENTS: usize = 64;

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(64, 64, 64))
)]
pub fn row_softmax_v1(input: &[f32], mut output: DisjointSlice<f32, GridExclusive>) {
    if let Some(leader) = thread::grid_leader() {
        let mut maximum = f32::NEG_INFINITY;
        let mut index = 0_usize;
        while index < ROW_ELEMENTS {
            let value = input[index];
            if value > maximum {
                maximum = value;
            }
            index += 1;
        }

        let math = DeviceMath::current();
        let mut denominator = 0.0_f32;
        index = 0;
        while index < ROW_ELEMENTS {
            denominator += math.exp_f32(input[index] - maximum);
            index += 1;
        }

        index = 0;
        while index < ROW_ELEMENTS {
            let probability = math.exp_f32(input[index] - maximum) / denominator;
            if let Some(slot) = output.get_mut_exclusive(&leader, index) {
                *slot = probability;
            }
            index += 1;
        }
    }
}
