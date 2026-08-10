use fe2o3_host::{
    RecoveredWorkerV2SynchronousHsaHandoffV1, ReviewedHsaImplicitKernargAdapterV1,
};

fn claim_stream<K, A>(authority: &RecoveredWorkerV2SynchronousHsaHandoffV1<K, A>)
where
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    let _ = authority.stream_identity();
}

fn main() {}
