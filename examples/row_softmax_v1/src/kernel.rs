//! Fixed 64-element row-softmax teaching baseline.
//!
//! This intentionally simple profile assigns the whole row to the grid leader.
//! It demonstrates stable max-subtracted softmax and exclusive store ownership;
//! use `row_softmax_general_v1` for wave-parallel dynamic rows.

#![allow(non_upper_case_globals)]

use fe2o3_device::{DeviceMath, DisjointSlice, GridExclusive, kernel, thread};

const ROW_ELEMENTS: usize = 64;

#[kernel(
    typed,
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(64, 64, 64))
)]
/// Computes one stable 64-element softmax row on the unique grid leader.
pub fn row_softmax_v1(input: &[f32], mut output: DisjointSlice<f32, GridExclusive>) {
    // GridLeader is both the execution guard and the capability required for stores.
    if let Some(leader) = thread::grid_leader() {
        // Pass 1 establishes the subtraction point used by every exponential.
        let mut maximum = f32::NEG_INFINITY;
        let mut index = 0_usize;
        while index < ROW_ELEMENTS {
            let value = input[index];
            if value > maximum {
                maximum = value;
            }
            index += 1;
        }

        // Pass 2 forms the FP32 normalizer after max subtraction.
        let math = DeviceMath::current();
        let mut denominator = 0.0_f32;
        // Pass 3 normalizes and commits through the exclusive output capability.
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
