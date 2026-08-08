use fe2o3_host::{
    AdmittedFinalizedWorkerV2BundleV1 as Admitted,
    RecoveredWorkerV2PinnedDescriptorV1 as Recovered,
};

fn convert(recovered: Recovered) -> Admitted {
    recovered.into()
}

fn main() {}
