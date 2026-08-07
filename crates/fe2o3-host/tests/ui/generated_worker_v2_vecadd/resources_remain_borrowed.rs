use fe2o3_core::DeviceBuffer;
use fe2o3_host::{
    CompilerGeneratedKernelContractV1, GeneratedWorkerV2VecAddExecutorV1,
    ReviewedHsaImplicitKernargAdapterV1,
};

fn release_output_while_prepared<K, A>(
    executor: &mut GeneratedWorkerV2VecAddExecutorV1<K, A>,
    a: DeviceBuffer<f32>,
    b: DeviceBuffer<f32>,
    mut c: DeviceBuffer<f32>,
) where
    K: CompilerGeneratedKernelContractV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    let prepared = executor.prepare(&a, &b, &mut c).unwrap();
    drop(c);
    let _still_live = prepared;
}

fn main() {}
