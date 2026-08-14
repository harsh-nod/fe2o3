use fe2o3_host::{
    RecoveredWorkerV2SynchronousHsaScalarGemmV1PreparedInvocationV1,
    ReviewedHsaImplicitKernargAdapterV1,
};

fn duplicate<'loaded, 'allocation, Root, Selected, Adapter, Arguments>(
    prepared: RecoveredWorkerV2SynchronousHsaScalarGemmV1PreparedInvocationV1<
        'loaded,
        'allocation,
        Root,
        Selected,
        Adapter,
        Arguments,
    >,
) where
    Adapter: ReviewedHsaImplicitKernargAdapterV1,
{
    let _ = prepared.clone();
}

fn main() {}
