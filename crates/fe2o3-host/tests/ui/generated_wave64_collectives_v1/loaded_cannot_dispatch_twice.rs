use fe2o3_host::{LoadedWave64CollectivesV1, ReviewedWave64CollectivesRuntimeAdapterV1};

fn replay<A: ReviewedWave64CollectivesRuntimeAdapterV1>(
    value: LoadedWave64CollectivesV1<'_, '_, '_, '_, A>,
) {
    let _ = value.dispatch_and_wait();
    let _ = value.dispatch_and_wait();
}

fn main() {}
