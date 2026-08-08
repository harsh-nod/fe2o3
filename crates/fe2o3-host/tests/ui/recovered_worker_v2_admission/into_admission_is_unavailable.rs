use fe2o3_host::{
    AdmittedFinalizedWorkerV2BundleV1 as Admitted,
    RecoveredWorkerV2PinnedDescriptorV1 as Recovered,
};

fn extract(recovered: Recovered) -> Admitted {
    recovered.into_admission()
}

fn main() {}
