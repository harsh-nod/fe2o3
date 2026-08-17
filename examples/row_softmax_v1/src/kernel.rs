#![allow(non_upper_case_globals)]

use fe2o3_device::{DeviceMath, DisjointSlice, kernel, thread};

const ROW_ELEMENTS: usize = 64;

#[kernel(
    typed,
    namespace = "b9c43562d541f2f0489f311058c425d85a7ea6c328a3991bb6da17bdf85f766c",
    launch(required = [64, 1, 1], max = [64, 1, 1]),
    control_flow(loop_bounds(64, 64, 64))
)]
pub fn row_softmax_v1(input: &[f32], mut output: DisjointSlice<f32>) {
    let lane = thread::index_1d().get();
    if lane == 0 {
        let mut maximum = f32::NEG_INFINITY;
        let mut index = 0_usize;
        while index < ROW_ELEMENTS {
            let value = input[index];
            if value > maximum {
                maximum = value;
            }
            index += 1;
        }

        let math = unsafe { DeviceMath::from_compiler() };
        let mut denominator = 0.0_f32;
        index = 0;
        while index < ROW_ELEMENTS {
            denominator += math.exp_f32(input[index] - maximum);
            index += 1;
        }

        index = 0;
        while index < ROW_ELEMENTS {
            let probability = math.exp_f32(input[index] - maximum) / denominator;
            if let Some(slot) = unsafe { output.get_mut_at(index) } {
                *slot = probability;
            }
            index += 1;
        }
    }
}
