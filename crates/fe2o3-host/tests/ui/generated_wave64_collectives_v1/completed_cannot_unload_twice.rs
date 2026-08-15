use fe2o3_host::{CompletedWave64CollectivesV1, ReviewedWave64CollectivesRuntimeAdapterV1};

fn replay<A: ReviewedWave64CollectivesRuntimeAdapterV1>(
    value: CompletedWave64CollectivesV1<A>,
) {
    let _ = value.unload();
    let _ = value.unload();
}

fn main() {}
