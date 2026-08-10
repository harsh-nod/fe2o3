use fe2o3_host::{
    RecoveredWorkerV2ApplicationHandoffV1, ReviewedHsaImplicitKernargAdapterV1,
};

fn escape<K, A>(
    authority: RecoveredWorkerV2ApplicationHandoffV1<'_, K, A>,
) -> RecoveredWorkerV2ApplicationHandoffV1<'static, K, A>
where
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    authority
}

fn main() {}
