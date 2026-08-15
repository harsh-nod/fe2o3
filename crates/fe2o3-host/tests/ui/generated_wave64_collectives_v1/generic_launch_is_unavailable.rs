use fe2o3_host::{LoadedWave64CollectivesV1, ReviewedWave64CollectivesRuntimeAdapterV1};

fn launch<A: ReviewedWave64CollectivesRuntimeAdapterV1>(
    value: LoadedWave64CollectivesV1<'_, '_, '_, '_, A>,
) {
    value.launch();
}

fn main() {}
