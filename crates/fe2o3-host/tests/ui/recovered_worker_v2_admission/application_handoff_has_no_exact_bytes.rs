use fe2o3_host::{
    RecoveredWorkerV2ApplicationHandoffV1, ReviewedHsaImplicitKernargAdapterV1,
};

fn extract<K, A>(authority: &RecoveredWorkerV2ApplicationHandoffV1<'_, K, A>)
where
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    let _ = authority.exact_hsaco_bytes();
}

fn main() {}
