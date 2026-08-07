use fe2o3_host::AdmittedFinalizedWorkerV2BundleV1;

fn inspect(token: AdmittedFinalizedWorkerV2BundleV1) {
    let AdmittedFinalizedWorkerV2BundleV1 { current_lease, .. } = token;
    let _ = current_lease;
}

fn main() {}
