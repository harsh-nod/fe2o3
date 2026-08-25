use fe2o3_core::DeviceBuffer;
use fe2o3_host::{
    CompilerGeneratedKernelContractV1, GeneratedWorkerV2VecAddExecutorV1,
    ReviewedHsaImplicitKernargAdapterV1,
};
use std::error::Error;

fn prepare_canary_regions<K, A>(
    executor: &mut GeneratedWorkerV2VecAddExecutorV1<K, A>,
    a: &DeviceBuffer<f32>,
    b: &DeviceBuffer<f32>,
    c: &mut DeviceBuffer<f32>,
) -> Result<(), Box<dyn Error>>
where
    K: CompilerGeneratedKernelContractV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
    A::Error: Error + 'static,
{
    let a = a.view(1..257)?;
    let b = b.view(1..257)?;
    let c = c.view_mut(1..257)?;
    let prepared = executor.prepare_views(a, b, c)?;
    drop(prepared);
    Ok(())
}

fn main() {}
