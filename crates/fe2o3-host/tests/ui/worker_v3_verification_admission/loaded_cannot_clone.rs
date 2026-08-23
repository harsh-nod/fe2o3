use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, LoadedWorkerV3HsaExecutableV1,
    ReviewedHsaExecutableLifecycleAdapterV1,
};

fn duplicate<K, A>(loaded: LoadedWorkerV3HsaExecutableV1<K, A>)
where
    K: CompilerGeneratedKernelExpectationV1,
    A: ReviewedHsaExecutableLifecycleAdapterV1,
{
    let _duplicate = loaded.clone();
}

fn main() {}
