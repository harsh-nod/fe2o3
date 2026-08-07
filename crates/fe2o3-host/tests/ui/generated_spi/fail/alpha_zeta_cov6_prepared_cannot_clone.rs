use fe2o3_host::{
    ReviewedHsaImplicitKernargAdapterV1,
    __generated::GeneratedAlphaZetaCov6PreparedInvocationV1,
};

fn clone_prepared<P, K, A: ReviewedHsaImplicitKernargAdapterV1, Arguments>(
    prepared: GeneratedAlphaZetaCov6PreparedInvocationV1<'_, '_, P, K, A, Arguments>,
) {
    let _ = prepared.clone();
}

fn main() {}
