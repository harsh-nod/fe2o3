use fe2o3_host::{
    AdmittedFinalizedWorkerV2BundleV1 as Admitted,
    RecoveredWorkerV2PinnedDescriptorV1 as Recovered,
};

fn acquire(recovered: &Recovered) {
    let _ = Admitted::acquire_currentness(recovered);
}

fn main() {}
