use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, GeneratedWorkerV3PreparedInvocationV1,
    ReviewedHsaImplicitKernargAdapterV1,
};

fn duplicate<K, A, Arguments>(
    prepared: GeneratedWorkerV3PreparedInvocationV1<'_, '_, K, A, Arguments>,
)
where
    K: CompilerGeneratedKernelExpectationV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    let _duplicate = prepared.clone();
}

fn main() {}
