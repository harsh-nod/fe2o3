use fe2o3_host::{
    RecoveredWorkerV2ApplicationHandoffV1, ReviewedHsaImplicitKernargAdapterV1,
};

fn duplicate<K, A>(authority: RecoveredWorkerV2ApplicationHandoffV1<'_, K, A>)
where
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    let _ = authority.clone();
}

fn main() {}
