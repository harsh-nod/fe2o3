use fe2o3_host::{
    CompilerGeneratedKernelExpectationV1, GeneratedWorkerV3PreparedInvocationV1,
    ReviewedHsaImplicitKernargAdapterV1,
};

fn dispatch_twice<K, A, Arguments>(
    prepared: GeneratedWorkerV3PreparedInvocationV1<'_, '_, K, A, Arguments>,
)
where
    K: CompilerGeneratedKernelExpectationV1,
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    let _first = prepared.dispatch();
    let _second = prepared.dispatch();
}

fn main() {}
