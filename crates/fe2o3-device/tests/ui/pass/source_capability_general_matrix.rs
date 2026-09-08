use fe2o3_device::{
    CurrentTarget, ExclusiveReadWrite, Global, KernelCapabilityBrand, KernelContext, ReadOnly,
    RegisteredLaunch, StrictIeee, SubgroupWidth64,
};

enum Kernel {}

type Brand<'kernel> = KernelCapabilityBrand<'kernel, Kernel, CurrentTarget, RegisteredLaunch>;

fn general_matrix<'kernel>(
    mut context: KernelContext<'kernel, Kernel>,
    a_bits: Global<'kernel, u16, ReadOnly, Brand<'kernel>>,
    b_bits: Global<'kernel, u16, ReadOnly, Brand<'kernel>>,
    mut output: Global<'kernel, f32, ExclusiveReadWrite, Brand<'kernel>>,
) {
    let policy = context.numerical_policy::<StrictIeee>();
    let math = context.math();
    let _policy_math = math.with_numerical_policy(&policy);

    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let matrix = matrix.with_numerical_policy(&policy);
            let a = matrix
                .bf16_a_global_row_major(&a_bits, 0, 19, 23, 29)
                .unwrap();
            let b = matrix
                .bf16_b_global_row_major(&b_bits, 0, 23, 21, 27)
                .unwrap();
            let lhs = a.load_m16k16(lane, 16, 16);
            let rhs = b.load_k16n16(lane, 16, 16);
            let accumulator = matrix.bf16_zero_accumulator(lane);
            let accumulator = matrix.multiply_accumulate(lhs, rhs, accumulator);
            let mut c = matrix
                .f32_accumulator_global_row_major(&mut output, 0, 19, 21, 25)
                .unwrap();
            let prior = c.load_lane_values(lane, 16, 16);
            let mut values = accumulator.into_values();
            for component in 0..values.len() {
                let product = 0.5 * prior[component];
                values[component] += product;
            }
            let _stored = c.store_lane_values(lane, 16, 16, values);
        });
    });
}

fn main() {
    let _ = general_matrix;
}
