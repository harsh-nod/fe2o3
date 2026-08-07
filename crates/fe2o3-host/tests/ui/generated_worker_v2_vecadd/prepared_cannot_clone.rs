use fe2o3_host::{GeneratedWorkerV2VecAddPreparedV1, ReviewedHsaImplicitKernargAdapterV1};

fn duplicate<K, A>(prepared: GeneratedWorkerV2VecAddPreparedV1<'_, '_, K, A>)
where
    A: ReviewedHsaImplicitKernargAdapterV1,
{
    let _duplicate = prepared.clone();
}

fn main() {}
