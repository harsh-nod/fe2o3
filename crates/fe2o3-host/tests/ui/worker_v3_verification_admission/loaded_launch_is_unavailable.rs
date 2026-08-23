use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, LoadedWorkerV3HsaExecutableV1,
    ReviewedHsaExecutableLifecycleAdapterV1,
};

fn launch_is_not_public<K, A>(loaded: LoadedWorkerV3HsaExecutableV1<K, A>)
where
    K: CompilerGeneratedKernelExpectationV1,
    A: ReviewedHsaExecutableLifecycleAdapterV1,
{
    loaded.launch();
}

fn main() {}
