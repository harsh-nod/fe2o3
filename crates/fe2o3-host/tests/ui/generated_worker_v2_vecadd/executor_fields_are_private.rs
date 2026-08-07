use fe2o3_host::{
    CompilerGeneratedKernelContractV1, GeneratedWorkerV2VecAddExecutorV1,
    ReviewedHsaImplicitKernargAdapterV1,
};

fn extract_loaded<K, A>(executor: GeneratedWorkerV2VecAddExecutorV1<K, A>)
where
    K: CompilerGeneratedKernelContractV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    let _loaded = executor.loaded;
}

fn main() {}
