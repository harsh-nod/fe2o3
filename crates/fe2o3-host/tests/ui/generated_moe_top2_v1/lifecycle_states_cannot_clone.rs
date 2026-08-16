use fe2o3_host::{JoinedMoeTop2V1, ReviewedMoeTop2V1RuntimeAdapterV1};

fn clone_joined(value: JoinedMoeTop2V1<'_, '_, '_, '_, '_, '_, '_, '_>) {
    let _copy = value.clone();
}

fn require_adapter<A: ReviewedMoeTop2V1RuntimeAdapterV1>() {}

fn main() {}
