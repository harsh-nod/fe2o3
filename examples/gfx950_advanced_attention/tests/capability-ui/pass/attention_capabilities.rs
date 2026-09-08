use fe2o3_device::{
    DisjointWrite, Global, Index1D, KernelContext, KernelMarkerV1, ReadOnly, StrictIeee,
    SubgroupWidth64, WriteOnlyDisjointSlice, kernel,
};

fn load_or_zero<Brand>(input: &Global<'_, f32, ReadOnly, Brand>, index: usize) -> f32 {
    input.load(index).unwrap_or(0.0)
}

#[kernel(typed, launch(required = [64, 1, 1], max = [64, 1, 1]))]
pub fn attention_capabilities(
    mut context: KernelContext<'_>,
    input: Global<'_, f32, ReadOnly>,
    mut output: Global<'_, f32, DisjointWrite<Index1D>>,
) {
    let index = context.invocation().index_1d();
    let value = load_or_zero(&input, index.get());
    let _private = context.private_memory::<f32, 4>();
    let policy = context.numerical_policy::<StrictIeee>();
    let device_math = context.math();
    let math = device_math.with_numerical_policy(&policy);
    context.with_workgroup(|workgroup| {
        let subgroup = workgroup.subgroup::<SubgroupWidth64>();
        subgroup.with_matrix(workgroup.epoch(), |matrix, lane| {
            let policy_matrix = matrix.with_numerical_policy(&policy);
            let _ = (policy_matrix, lane);
        });
    });
    assert!(output.store(index.into_disjoint(), math.exp_f32(value)));
}

type PhysicalAttentionKernel = fn(&[f32], WriteOnlyDisjointSlice<f32, Index1D>);

const _: PhysicalAttentionKernel =
    <__fe2o3_kernel_marker_attention_capabilities as KernelMarkerV1>::FUNCTION;
