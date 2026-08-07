use fe2o3_host::AdmittedFinalizedWorkerV2BundleV1;

fn duplicate(
    token: AdmittedFinalizedWorkerV2BundleV1,
) -> (
    AdmittedFinalizedWorkerV2BundleV1,
    AdmittedFinalizedWorkerV2BundleV1,
) {
    (token.clone(), token)
}

fn main() {}
