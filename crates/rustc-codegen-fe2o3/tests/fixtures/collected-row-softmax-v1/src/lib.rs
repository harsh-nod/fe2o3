#![allow(non_upper_case_globals)]

use fe2o3_device::{DeviceMath, DisjointSlice, kernel, thread};

const ROW_ELEMENTS: usize = 64;
const FRONTEND_CONTRACT: &[u8] = &[
    70, 69, 50, 79, 51, 75, 70, 0, 1, 0, 1, 0, 52, 0, 0, 0, 0, 0, 0, 0, 3, 0, 0, 0, 64, 0, 0,
    0, 1, 0, 0, 0, 1, 0, 0, 0, 64, 0, 0, 0, 1, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0,
];

#[kernel(
    typed,
    namespace = "b9c43562d541f2f0489f311058c425d85a7ea6c328a3991bb6da17bdf85f766c",
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

#[used]
static __fe2o3_kernel_frontend_contract_v1_row_softmax_v1: (
    u64,
    u16,
    u16,
    &'static str,
    &'static [u8],
    fn(&[f32], DisjointSlice<f32>),
) = (
    0x4146_4b33_4f32_4546,
    1,
    1,
    "row_softmax_v1",
    FRONTEND_CONTRACT,
    <__fe2o3_kernel_marker_row_softmax_v1 as fe2o3_device::KernelMarkerV1>::FUNCTION,
);
