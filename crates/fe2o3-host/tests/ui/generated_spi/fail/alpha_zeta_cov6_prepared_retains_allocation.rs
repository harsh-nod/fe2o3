use fe2o3_host::{
    ReviewedHsaImplicitKernargAdapterV1,
    __generated::GeneratedAlphaZetaCov6PreparedInvocationV1,
};

fn escape<'loaded, 'allocation, P, K, A, Arguments>(
    prepared: GeneratedAlphaZetaCov6PreparedInvocationV1<
        'loaded,
        'allocation,
        P,
        K,
        A,
        Arguments,
    >,
) -> GeneratedAlphaZetaCov6PreparedInvocationV1<'loaded, 'static, P, K, A, Arguments>
where
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    prepared
}

fn main() {}
