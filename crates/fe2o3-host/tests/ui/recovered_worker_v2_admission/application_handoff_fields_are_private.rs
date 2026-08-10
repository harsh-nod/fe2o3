use fe2o3_host::{
    RecoveredWorkerV2SynchronousHsaHandoffV1, ReviewedHsaImplicitKernargAdapterV1,
};

fn extract<K, A>(authority: RecoveredWorkerV2SynchronousHsaHandoffV1<K, A>)
where
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    let RecoveredWorkerV2SynchronousHsaHandoffV1 {
        loaded,
        observed,
    } = authority;
    let _ = (loaded, observed);
}

fn main() {}
