use fe2o3_host::{
    ReviewedHsaImplicitKernargAdapterV1,
    __generated::GeneratedAlphaZetaCov6PreparedInvocationV1,
};

fn inspect<P, K, A: ReviewedHsaImplicitKernargAdapterV1, Arguments>(
    prepared: GeneratedAlphaZetaCov6PreparedInvocationV1<'_, '_, P, K, A, Arguments>,
) {
    let GeneratedAlphaZetaCov6PreparedInvocationV1 { resolved, geometry, .. } = prepared;
    let _ = (resolved, geometry);
}

fn main() {}
