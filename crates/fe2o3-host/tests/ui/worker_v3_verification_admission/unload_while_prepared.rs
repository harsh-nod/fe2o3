use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, CompilerGeneratedWorkerV3ArgumentsV1,
    HsaLaunchGeometryV1, LoadedWorkerV3HsaExecutableV1, ObservedContext,
    ReviewedHsaImplicitKernargAdapterV1,
};

fn unload_while_prepared<'allocation, K, A, Arguments>(
    mut loaded: LoadedWorkerV3HsaExecutableV1<K, A>,
    observed: &ObservedContext,
    arguments: Arguments,
)
where
    K: CompilerGeneratedKernelExpectationV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
    Arguments: CompilerGeneratedWorkerV3ArgumentsV1<'allocation, K>,
{
    let prepared = loaded
        .prepare_generated_worker_v3_v1(
            observed,
            HsaLaunchGeometryV1::new([1, 1, 1], [1, 1, 1], 0),
            arguments,
        )
        .unwrap();
    let _unloaded = loaded.unload();
    drop(prepared);
}

fn main() {}
