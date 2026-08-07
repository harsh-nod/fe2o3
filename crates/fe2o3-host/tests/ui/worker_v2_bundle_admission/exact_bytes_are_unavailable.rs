use fe2o3_host::CurrentFinalizedWorkerV2BundleAdmissionV1;

fn extract<'a>(guard: &'a CurrentFinalizedWorkerV2BundleAdmissionV1<'a>) -> &'a [u8] {
    guard.exact_artifact_bytes()
}

fn main() {}
