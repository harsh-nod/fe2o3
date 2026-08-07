use fe2o3_host::{AdmittedFinalizedWorkerV2BundleV1, LoadedKernel};

struct Kernel;

fn forge(admission: AdmittedFinalizedWorkerV2BundleV1) -> LoadedKernel<Kernel> {
    admission.into()
}

fn main() {}
