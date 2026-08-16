use fe2o3_host::{LoadedMoeTop2V1, ReviewedMoeTop2V1RuntimeAdapterV1};

fn unload_early<A: ReviewedMoeTop2V1RuntimeAdapterV1>(
    loaded: LoadedMoeTop2V1<'_, '_, '_, '_, '_, '_, '_, '_, A>,
) {
    let _ = loaded.unload();
}

fn main() {}
